#!/usr/bin/env bash
# update-skills-retention-preserves-recovery.sh (fix-A F7): when the exchange
# LANDED but the retention `mv` failed, the previous generation sits in the
# candidate workspace with the marker kept, but the CALLER then unconditionally
# garbage-destroyed that workspace, and recovery had the same flaw (it dropped
# the marker on a move failure, so the staging walk deleted the workspace). The
# only copy of the previous generation was irrecoverably lost. The fix: on a
# landed-but-retention-incomplete publish, PRESERVE both the workspace and the
# marker; the caller must not garbage-destroy it, and recovery must keep it on a
# move failure and exclude it from normal staging deletion.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"
fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# shellcheck source=test/fixtures/exchange-tool.lib.sh
source "$REPO_ROOT/test/fixtures/exchange-tool.lib.sh"
GMV_BIN="$(resolve_exchange_tool)" ||
  fail "no GNU coreutils mv with a working --exchange on PATH (need gmv or mv)"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# ─────────────────────────────────────────────────────────────────────────────
# Part A: the CALLER (full weekly run) must not destroy the workspace when the
# retention move fails, the displaced previous generation must survive.
# ─────────────────────────────────────────────────────────────────────────────
HOME="$tmp/homeA"
export HOME
export UPDATE_SKILLS_GMV="$GMV_BIN"
mkdir -p "$HOME/.agents/skills"
AGENTS="$HOME/.agents"
CURRENT="$AGENTS/.skills-current"
GENERATIONS="$AGENTS/.skills-generations"
LOCK="$AGENTS/custom-skill-lock.json"
STAMP="$HOME/.local/state/update-skills/last-success"
MARKER="$GENERATIONS/.exchange-in-flight"

cat >"$LOCK" <<'EOF'
{
  "version": 2,
  "tiers": {"alpha": "core"},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": {"alpha": {"repo": "fixture/pack"}},
  "clawhubTracked": {},
  "forks": {}
}
EOF

stub="$tmp/stub"
mkdir -p "$stub"
cat >"$stub/npx" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
prev=""; skills=()
for a in "$@"; do [[ $prev == --skill ]] && skills+=("$a"); prev="$a"; done
cli_lock="${XDG_STATE_HOME:-$HOME/.local/state}/skills/.skill-lock.json"
mkdir -p "$(dirname "$cli_lock")"
[[ -f $cli_lock ]] || printf '{"version":3,"skills":{}}\n' >"$cli_lock"
for s in "${skills[@]}"; do
  mkdir -p "$HOME/.agents/skills/$s"
  printf -- '---\nname: %s\n---\n# lane\n' "$s" >"$HOME/.agents/skills/$s/SKILL.md"
  jq --arg s "$s" '.skills[$s] = {source: "github:fixture/pack", agents: ["claude-code","codex"]}' \
    "$cli_lock" >"$cli_lock.tmp" && mv "$cli_lock.tmp" "$cli_lock"
done
EOF
printf '#!/usr/bin/env bash\nexit 0\n' >"$stub/alerter"
chmod +x "$stub/npx" "$stub/alerter"

# mv stub (its OWN dir, prepended only for the failing run): fails EXACTLY the
# retention move (a candidate .../home/.agents into a bare generation-id slot
# under .skills-generations); every other mv passes through.
mvstub="$tmp/mvstub"
mkdir -p "$mvstub"
cat >"$mvstub/mv" <<'EOF'
#!/usr/bin/env bash
if [[ ${1:-} == */home/.agents && ${2:-} == */.skills-generations/* && ${2:-} != *.garbage.* ]]; then
  echo "mv-stub: refusing retention move $1 -> $2" >&2
  exit 1
fi
exec /bin/mv "$@"
EOF
chmod +x "$mvstub/mv"

# Establish the FIRST generation (the one that must survive). The npx lane owns
# the published content, so the OLD generation is identified by its ID, not by a
# content marker.
mkdir -p "$AGENTS/skills/alpha"
printf -- '---\nname: alpha\n---\n# seed\n' >"$AGENTS/skills/alpha/SKILL.md"
printf '{"skills":{"alpha":{}}}\n' >"$AGENTS/.skill-lock.json"
PATH="$stub:$PATH" UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" >/dev/null 2>&1 ||
  fail "part A setup full run failed"
OLD_ID="$(jq -r '.id' "$CURRENT/generation.json")"
[[ -n $OLD_ID && $OLD_ID != null ]] || fail "part A setup: no OLD generation id"
# Each run is a distinct process, so __gen_new_id (epoch-PID-random) yields a
# distinct id without waiting on the clock.

# Second run WITH the retention-failing mv stub: the exchange lands, retention
# fails. The caller must preserve the workspace + marker (not garbage-destroy).
# A weekly run withholds the success stamp on an internal failure but still
# exits 0, so the no-stamp is the failure signal here.
rm -f "$STAMP" # the setup run stamped; the failing run must not re-stamp
out="$(PATH="$mvstub:$stub:$PATH" UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1)" || true
[[ ! -f $STAMP ]] || fail "a success stamp was written despite the incomplete retention: $out"

NEW_ID="$(jq -r '.id' "$CURRENT/generation.json")"
[[ $NEW_ID != "$OLD_ID" ]] ||
  fail "the exchange did not land (live id unchanged), fixture drift: $out"
# The displaced OLD generation must survive in the workspace (caller preserved it).
WS_AGENTS="$GENERATIONS/$NEW_ID/home/.agents"
[[ -d $WS_AGENTS ]] ||
  fail "the caller garbage-destroyed the workspace holding the displaced previous generation (OLD gen lost)"
[[ "$(jq -r '.id' "$WS_AGENTS/generation.json" 2>/dev/null)" == "$OLD_ID" ]] ||
  fail "the workspace no longer holds the OLD generation"
[[ -f "$WS_AGENTS/skills/alpha/SKILL.md" ]] ||
  fail "the displaced OLD generation content was lost"
[[ -f $MARKER ]] || fail "the exchange-in-flight marker was removed (retention cannot be resumed)"

# Retry WITHOUT the mv failure: recovery alone must complete the retention,
# moving the OLD generation to its retained slot, it survived the whole ordeal.
# Driven via the lib-only marker handler so neither a full rebuild nor the
# retained-generation prune obscures that the previous generation was resumed.
outR="$(
  HOME="$HOME" UPDATE_SKILLS_GMV="$GMV_BIN" UPDATE_SKILLS_LIB_ONLY=1 \
    bash -s "$SCRIPT" <<'INNER'
set -euo pipefail
script="$1"
set --
# shellcheck disable=SC1090
source "$script"
__gen_recover_exchange_marker
INNER
)"
[[ ! -f $MARKER ]] || fail "the marker survived a successful retry: $outR"
# The OLD generation was retained (recovery completed the retention) rather than
# irrecoverably lost.
[[ -d "$GENERATIONS/$OLD_ID" ]] ||
  fail "part A retry: recovery did not complete the retention of the previous generation: $outR"

# ─────────────────────────────────────────────────────────────────────────────
# Part B: RECOVERY itself, on a retention-move failure, keeps the marker and the
# workspace (the only copy of the previous generation), so a later retry resumes.
# ─────────────────────────────────────────────────────────────────────────────
NEW2="2000000100-42-1111" # the workspace/new-live id; OLD2 lives in the INNER shell
build_crash_state() {
  local home="$1" agents="$1/.agents"
  rm -rf "$home"
  mkdir -p "$agents/skills"
  cat >"$agents/custom-skill-lock.json" <<'LOCK'
{"version":2,"tiers":{"alpha":"core"},"hermesProfiles":{},"hermesRegistry":{},"npxTracked":{"alpha":{"repo":"fixture/pack"}},"clawhubTracked":{},"forks":{}}
LOCK
  mkdir -p "$agents/.skills-current/skills/alpha"
  printf -- '---\nname: alpha\n---\n# refreshed\n' >"$agents/.skills-current/skills/alpha/SKILL.md"
  printf '{"skills":{"alpha":{}}}\n' >"$agents/.skills-current/.skill-lock.json"
  mkdir -p "$agents/.skills-generations/$NEW2/home/.agents/skills/alpha"
  printf -- '---\nname: alpha\n---\n# PREV-GEN\n' \
    >"$agents/.skills-generations/$NEW2/home/.agents/skills/alpha/SKILL.md"
  printf '{"skills":{"alpha":{}}}\n' \
    >"$agents/.skills-generations/$NEW2/home/.agents/.skill-lock.json"
  ln -s "../.skills-current/skills/alpha" "$agents/skills/alpha"
  ln -s ".skills-current/.skill-lock.json" "$agents/.skill-lock.json"
}

homeB="$tmp/homeB"
build_crash_state "$homeB"
outB="$(
  HOME="$homeB" UPDATE_SKILLS_LIB_ONLY=1 bash -s "$SCRIPT" "$homeB" <<'INNER'
set -euo pipefail
script="$1"; home="$2"
set --
# shellcheck disable=SC1090
source "$script"
NEW2="2000000100-42-1111"; OLD2="1000000100-41-9999"
__gen_write_meta "$home/.agents/.skills-current" "$NEW2" full
__gen_write_meta "$home/.agents/.skills-generations/$NEW2/home/.agents" "$OLD2" full
jq -n --arg oldId "$OLD2" --arg workspaceId "$NEW2" \
  '{oldId: $oldId, workspaceId: $workspaceId}' \
  >"$home/.agents/.skills-generations/.exchange-in-flight"
# Fail exactly the retention-completion move (workspace .agents -> retained slot).
# shellcheck disable=SC2317
mv() {
  if [[ ${1:-} == */home/.agents && ${2:-} == */.skills-generations/"$OLD2" ]]; then
    return 1
  fi
  command mv "$@"
}
# First recovery: retention cannot complete; the marker + workspace must survive.
__gen_recover
printf 'MARKER_AFTER_FAIL=%s\n' "$([[ -f "$home/.agents/.skills-generations/.exchange-in-flight" ]] && echo yes || echo no)"
printf 'WS_AFTER_FAIL=%s\n' "$([[ -f "$home/.agents/.skills-generations/$NEW2/home/.agents/skills/alpha/SKILL.md" ]] && echo yes || echo no)"
# Second recovery WITHOUT the mv failure: retention completes.
unset -f mv
__gen_recover
printf 'RETAINED=%s\n' "$([[ -f "$home/.agents/.skills-generations/$OLD2/skills/alpha/SKILL.md" ]] && echo yes || echo no)"
printf 'MARKER_AFTER_OK=%s\n' "$([[ -f "$home/.agents/.skills-generations/.exchange-in-flight" ]] && echo yes || echo no)"
INNER
)"

grep -q 'MARKER_AFTER_FAIL=yes' <<<"$outB" ||
  fail "part B: recovery dropped the marker on a retention-move failure: $outB"
grep -q 'WS_AFTER_FAIL=yes' <<<"$outB" ||
  fail "part B: recovery let the workspace (only copy of the previous generation) be deleted on a move failure: $outB"
grep -q 'RETAINED=yes' <<<"$outB" ||
  fail "part B: a later retry did not complete the retention: $outB"
grep -q 'MARKER_AFTER_OK=yes' <<<"$outB" &&
  fail "part B: the marker survived a successful retention: $outB"

echo "update-skills-retention-preserves-recovery: OK"
