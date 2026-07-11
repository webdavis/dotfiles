#!/usr/bin/env bash
# update-skills-dryrun-readonly.sh, --dry-run is a READ-ONLY contention check
# (Wave 3a item 5). The audit found dry-run still created the lock dir and could
# delete a dead lock, and on a fresh home with .agents absent it failed as false
# contention. A dry run must create nothing, delete nothing, tolerate an absent
# .agents parent, and leave the filesystem byte-identical while still previewing
# whether it would run or defer.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

tmp="$(mktemp -d)"
# Some cases chmod a dir read-only; restore write before cleanup so the trap can
# remove everything.
trap 'chmod -R u+w "$tmp" 2>/dev/null || true; rm -rf "$tmp"' EXIT

# Recording npx + clawhub stubs: a dry run must NEVER invoke either package CLI
# (the npx CLI treats `update --help` as a real update, observed live), so any
# entry in these logs is a failure asserted at the end.
stub="$tmp/stub"
mkdir -p "$stub"
CLI_LOG="$tmp/cli-invocations.log"
printf '#!/usr/bin/env bash\nprintf "npx %%s\\n" "$*" >>"%s"\necho stub\n' "$CLI_LOG" >"$stub/npx"
printf '#!/usr/bin/env bash\nprintf "clawhub %%s\\n" "$*" >>"%s"\necho stub\n' "$CLI_LOG" >"$stub/clawhub"
chmod +x "$stub/npx" "$stub/clawhub"
export PATH="$stub:$PATH"

snapshot() { find "$1" 2>/dev/null | sort; }

# ── 1) absent .agents parent → preview, no false contention, nothing created ─
HOME1="$tmp/home1"
export HOME="$HOME1"
mkdir -p "$HOME1" # .agents deliberately absent
before="$(snapshot "$HOME1")"
out="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --dry-run 2>&1)" ||
  fail "--dry-run on an absent-.agents home exited non-zero: $out"
after="$(snapshot "$HOME1")"
[[ $before == "$after" ]] || fail "--dry-run mutated a fresh home:
--- before ---
$before
--- after ---
$after"
[[ ! -e "$HOME1/.agents/.update-skills.lock" ]] ||
  fail "--dry-run created a lock file on a fresh home (false contention path)"
grep -qiE 'another run in progress' <<<"$out" &&
  fail "--dry-run on a fresh home reported false contention: $out"
grep -qi 'would run' <<<"$out" || fail "--dry-run did not preview 'would run': $out"

# ── 2) leftover lock FILE, nobody holding it → previews would-run, no delete ──
# (Kernel-lock model: the lock file persists on disk by design; only a live
# open fd means "held". A leftover file from a finished/crashed run previews as
# would-run and is never deleted or truncated by the dry run.)
HOME2="$tmp/home2"
export HOME="$HOME2"
mkdir -p "$HOME2/.agents/skills"
LOCKFILE2="$HOME2/.agents/.update-skills.lock"
printf 'leftover-bytes' >"$LOCKFILE2" # a crashed run's leftover; nobody holds it
before="$(snapshot "$HOME2")"
out="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --dry-run 2>&1)" ||
  fail "--dry-run with an unheld lock file exited non-zero: $out"
after="$(snapshot "$HOME2")"
[[ $before == "$after" ]] || fail "--dry-run mutated lock state around an unheld lock file:
--- before ---
$before
--- after ---
$after"
[[ -f $LOCKFILE2 && "$(cat "$LOCKFILE2")" == "leftover-bytes" ]] ||
  fail "--dry-run deleted or truncated the lock file (must never mutate lock state)"
grep -qi 'would run' <<<"$out" || fail "--dry-run did not preview 'would run' past an unheld lock file: $out"

# ── 3) lock HELD by a live process → previews would-defer, changes nothing ────
# A real holder process acquires the kernel lock the way the updater does (fd 9
# + /usr/bin/lockf) and parks until released.
HOME3="$tmp/home3"
export HOME="$HOME3"
mkdir -p "$HOME3/.agents/skills"
LOCKFILE3="$HOME3/.agents/.update-skills.lock"
if [[ -x /usr/bin/lockf ]]; then
  held3="$tmp/held3"
  release3="$tmp/release3"
  rm -f "$held3" "$release3"
  (
    exec 9>"$LOCKFILE3"
    /usr/bin/lockf -s -t 0 9 || exit 1
    : >"$held3"
    while [[ ! -e $release3 ]]; do sleep 0.02; done
  ) &
  holder_pid=$!
  for _ in $(seq 1 200); do
    [[ -e $held3 ]] && break
    sleep 0.02
  done
  [[ -e $held3 ]] || fail "the test's lock holder never acquired the lock"
  before="$(snapshot "$HOME3")"
  out="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --dry-run 2>&1)" ||
    fail "--dry-run with a held lock exited non-zero: $out"
  after="$(snapshot "$HOME3")"
  [[ $before == "$after" ]] || fail "--dry-run mutated state around a held lock"
  grep -qi 'would defer' <<<"$out" || fail "--dry-run did not preview 'would defer' under a held lock: $out"
  : >"$release3"
  wait "$holder_pid" 2>/dev/null || true
fi

# ── 4) read-only home → dry-run writes nothing, exits 0. The roster is
#      NON-EMPTY here (an absent npx skill and an absent clawhub skill a full
#      run would install), so the never-invokes-a-package-CLI assertion at the
#      end is load-bearing, not vacuous. ──────────────────────────────────────
HOME4="$tmp/home4"
export HOME="$HOME4"
mkdir -p "$HOME4/.agents/skills"
cat >"$HOME4/.agents/custom-skill-lock.json" <<'EOF'
{
  "version": 2,
  "tiers": {"wanted": "core", "clawwanted": "core"},
  "hermesProfiles": {"wanted": [], "clawwanted": []},
  "hermesRegistry": {},
  "npxTracked": {"wanted": {"repo": "fixture/wanted"}},
  "clawhubTracked": {"clawwanted": {"slug": "@fixture/clawwanted", "registry": "https://clawhub.example"}},
  "superpowersRouting": {},
  "forks": {}
}
EOF
chmod -R a-w "$HOME4/.agents" # read-only tree
before="$(snapshot "$HOME4")"
out="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --dry-run 2>&1)" ||
  fail "--dry-run under a read-only home exited non-zero: $out"
after="$(snapshot "$HOME4")"
[[ $before == "$after" ]] || fail "--dry-run wrote to a read-only home:
--- before ---
$before
--- after ---
$after"
chmod -R u+w "$HOME4/.agents"

# ── 5) across ALL the dry runs above: neither package CLI was ever invoked ────
if [[ -s $CLI_LOG ]]; then
  fail "--dry-run invoked a package CLI (npx/clawhub must never run in a dry run): $(cat "$CLI_LOG")"
fi

echo "update-skills-dryrun-readonly: OK"
