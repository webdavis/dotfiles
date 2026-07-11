#!/usr/bin/env bash
# update-skills-lock.sh (R2-1): the serialize lock is a KERNEL lock via macOS
# /usr/bin/lockf (flock(2)-backed), NOT the old hand-rolled mkdir-owner-token
# machinery. The kernel grants the lock to exactly one process and releases it
# automatically when the holding file descriptor closes (process exit or crash),
# so the whole two-owner class the mkdir lock kept re-introducing is gone: there
# is no owner token, no dead-owner reclaim, and no EXIT-trap cleanup.
#
# Assertions:
#   1. the new lock model is in force: a LOCKFILE (a file, not a *.lock.d dir) is
#      defined, and the old owner-token/publish/reclaim helpers no longer exist;
#   2. a second REAL process is refused while a first holds the lock (two real
#      subshells drive the actual lockf acquisition, not a stub);
#   3. the lock is released purely by the holder's process exit (fd close), with
#      no manual cleanup;
#   4. end to end: a run parked mid-lane holds the lock and a concurrent run
#      defers with "another run in progress", then the parked run finishes.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# The kernel lock is darwin-only (the weekly LaunchAgent is darwin-only). On a
# host without /usr/bin/lockf there is nothing to exercise; skip cleanly.
if [[ "$(uname -s)" != "Darwin" || ! -x /usr/bin/lockf ]]; then
  echo "update-skills-lock: SKIP (no /usr/bin/lockf on this host)"
  exit 0
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.agents/skills"

cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{
  "version": 2,
  "tiers": {},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": {},
  "clawhubTracked": {},
  "forks": {}
}
EOF

# Offline stub so a proceeding full run is fast and never hits the network.
stub="$tmp/stub"
mkdir -p "$stub"
cat >"$stub/npx" <<'EOF'
#!/usr/bin/env bash
echo stub
EOF
chmod +x "$stub"/*
export PATH="$stub:$PATH"

# ── 1) the new lock model is in force ───────────────────────────────────────
# `set --` clears positional params so the sourced script's arg parser sees none.
model="$(SCRIPT="$SCRIPT" UPDATE_SKILLS_LIB_ONLY=1 bash -c '
  set --
  # shellcheck disable=SC1090
  source "$SCRIPT"
  printf "LOCKFILE=%s\n" "${LOCKFILE:-<unset>}"
  declare -F __update_skills_publish_lock >/dev/null 2>&1 && printf "HAS_PUBLISH_LOCK\n"
  declare -F __update_skills_reclaim_dead_lock >/dev/null 2>&1 && printf "HAS_RECLAIM\n"
  declare -F __update_skills_owner_token >/dev/null 2>&1 && printf "HAS_OWNER_TOKEN\n"
  declare -F __update_skills_acquire_lock >/dev/null 2>&1 && printf "HAS_ACQUIRE\n"
  true
')"
grep -q 'LOCKFILE=.*/\.update-skills\.lock$' <<<"$model" ||
  fail "LOCKFILE is not the new kernel-lock file path: $model"
grep -q 'HAS_ACQUIRE' <<<"$model" || fail "__update_skills_acquire_lock is not defined: $model"
grep -q 'HAS_PUBLISH_LOCK' <<<"$model" &&
  fail "the removed owner-token publish helper still exists (mkdir lock not replaced)"
grep -q 'HAS_RECLAIM' <<<"$model" &&
  fail "the removed dead-owner reclaim helper still exists (mkdir lock not replaced)"
grep -q 'HAS_OWNER_TOKEN' <<<"$model" &&
  fail "the removed owner-token helper still exists (mkdir lock not replaced)"

# A helper each subshell runs: source the script (lib-only), acquire the real
# lock, and report the outcome. On success it optionally holds until a release
# sentinel appears, proving the kernel releases on process exit.
acquire_helper="$tmp/acquire.sh"
cat >"$acquire_helper" <<'EOF'
set -u
script="$1"                     # updater script
held_sentinel="$2"              # touched once the lock is held
release_sentinel="$3"           # when it appears, exit (releasing the lock)
export UPDATE_SKILLS_LIB_ONLY=1
set --                          # clear positional params before sourcing
# shellcheck disable=SC1090
source "$script"
if __update_skills_acquire_lock; then
  printf 'ACQUIRED\n'
  [[ -n $held_sentinel ]] && : >"$held_sentinel"
  if [[ -n $release_sentinel ]]; then
    while [[ ! -e $release_sentinel ]]; do sleep 0.02; done
  fi
else
  printf 'REFUSED\n'
fi
EOF

# ── 2) a second real process is refused while the first holds the lock ──────
held="$tmp/held"
release="$tmp/release"
rm -f "$held" "$release"
out_a="$tmp/a.out"
bash "$acquire_helper" "$SCRIPT" "$held" "$release" >"$out_a" 2>&1 &
holder_pid=$!
for _ in $(seq 1 200); do
  [[ -e $held ]] && break
  sleep 0.02
done
[[ -e $held ]] || fail "the first process never acquired the lock: $(cat "$out_a" 2>/dev/null)"
grep -q ACQUIRED "$out_a" || fail "the first process did not report ACQUIRED: $(cat "$out_a")"

out_b="$(bash "$acquire_helper" "$SCRIPT" "" "" 2>&1)"
grep -q REFUSED <<<"$out_b" ||
  fail "a second process acquired the lock while the first held it (two owners): $out_b"

# ── 3) the lock is released purely by the holder's process exit ─────────────
: >"$release"
wait "$holder_pid" 2>/dev/null || true
out_c="$(bash "$acquire_helper" "$SCRIPT" "" "" 2>&1)"
grep -q ACQUIRED <<<"$out_c" ||
  fail "the lock was not released when the holder exited (fd-close release failed): $out_c"

# ── 4) end to end: a parked run holds the lock; a concurrent run defers ──────
cat >"$stub/npx" <<EOF
#!/usr/bin/env bash
while [[ ! -e "$tmp/go" ]]; do sleep 0.02; done
echo stub
EOF
chmod +x "$stub/npx"
rm -f "$tmp/go"
o1="$tmp/o1.log"
(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" >"$o1" 2>&1) &
run_pid=$!
# Wait until the parked run holds the lock (its lock file exists and a second
# acquisition is refused).
for _ in $(seq 1 300); do
  if [[ -e "$HOME/.agents/.update-skills.lock" ]] &&
    grep -q REFUSED <<<"$(bash "$acquire_helper" "$SCRIPT" "" "" 2>&1)"; then
    break
  fi
  sleep 0.02
done
out_concurrent="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1)" ||
  fail "the concurrent run exited non-zero instead of deferring: $out_concurrent"
grep -qi 'another run in progress' <<<"$out_concurrent" ||
  fail "a concurrent run did not defer to the live lock holder: $out_concurrent"
touch "$tmp/go"
wait "$run_pid" 2>/dev/null || true
grep -qF '[update-skills] done' "$o1" || fail "the parked run did not finish: $(cat "$o1")"

echo "update-skills-lock: OK"
