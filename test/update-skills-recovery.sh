#!/usr/bin/env bash
# update-skills-recovery.sh proves the recovery state table (Wave 3a fix4
# brief step 1). Each crash-window state is fabricated, then __gen_recover is
# invoked (via UPDATE_SKILLS_LIB_ONLY sourcing) and the self-heal is asserted:
#   1. incomplete staging leftover        -> deleted
#   2. complete unpublished candidate that MATCHES desired state  -> reusable
#   2b. complete candidate that does NOT match desired state      -> deleted
#   3. published generation, stale store link / lock link         -> repaired
#   4. store entry is a REAL DIR where a link is expected          -> recorded
#      for re-absorption (GEN_REABSORB), links otherwise intact
#   5. partial-prune *.garbage.* leftover                          -> swept
#   6. multiple retained generations                              -> newest kept,
#                                                                     older pruned
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"
GMV_BIN="${UPDATE_SKILLS_GMV:-/opt/homebrew/bin/gmv}"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.agents/skills"
cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{"npxTracked":{"alpha":{"repo":"x/a"},"beta":{"repo":"x/b"}},"clawhubTracked":{}}
EOF

export UPDATE_SKILLS_GMV="$GMV_BIN"
export UPDATE_SKILLS_LIB_ONLY=1
# shellcheck disable=SC1090
source "$SCRIPT"

build_generation() {
  local dir="$1" id="$2" skill
  mkdir -p "$dir/skills"
  for skill in alpha beta; do
    mkdir -p "$dir/skills/$skill"
    printf -- '---\nname: %s\n---\n' "$skill" >"$dir/skills/$skill/SKILL.md"
  done
  printf '{}\n' >"$dir/.skill-lock.json"
  __gen_write_meta "$dir" "$id"
}

# A published live generation is the baseline for every state.
build_generation "$SKILLS_CURRENT" "live-1-1"
__gen_plant_store_link alpha
__gen_plant_store_link beta
__gen_plant_lock_link

# --- State 1: incomplete staging leftover (no ready marker) ------------------
mkdir -p "$GENERATIONS/incomplete-9-9/home/.agents/skills/alpha"
# (no .skill-lock.json, no generation.json -> incomplete)

# --- State 2: complete candidate matching desired state ----------------------
build_generation "$GENERATIONS/goodcand-8-8/home/.agents" "goodcand-8-8"

# --- State 2b: complete candidate NOT matching desired (bogus hashes) --------
mkdir -p "$GENERATIONS/stalecand-7-7/home/.agents/skills/alpha"
printf '{}\n' >"$GENERATIONS/stalecand-7-7/home/.agents/.skill-lock.json"
cat >"$GENERATIONS/stalecand-7-7/home/.agents/generation.json" <<'EOF'
{"id":"stalecand-7-7","createdAt":"2020-01-01T00:00:00Z","customLockHash":"deadbeef","updaterHash":"deadbeef"}
EOF

# --- State 3: stale store link + stale lock link -----------------------------
ln -sfn "../.skills-current/skills/WRONG" "$STORE/alpha" # wrong target
ln -sfn ".skills-current/WRONG-lock" "$SKILL_LOCK_LINK"  # wrong lock target

# --- State 4: store entry is a REAL DIR where a link is expected (beta) -------
rm -f "$STORE/beta"
mkdir -p "$STORE/beta"
printf 'competing-writer content\n' >"$STORE/beta/SKILL.md"

# --- State 5: partial-prune garbage leftover ---------------------------------
mkdir -p "$GENERATIONS/old-3-3.garbage.123.456/skills"

# --- State 6: two retained generations (bare <id> dirs) ----------------------
build_generation "$GENERATIONS/1000000000-1-1" "1000000000-1-1" # older
build_generation "$GENERATIONS/2000000000-2-2" "2000000000-2-2" # newer

# Run recovery.
__gen_recover

# 1) incomplete staging deleted.
[[ ! -d "$GENERATIONS/incomplete-9-9" ]] ||
  fail "state 1: incomplete staging leftover was not deleted"

# 2) matching candidate recorded as reusable.
[[ $GEN_REUSE_CANDIDATE == "$GENERATIONS/goodcand-8-8/home/.agents" ]] ||
  fail "state 2: complete matching candidate not marked reusable (got '$GEN_REUSE_CANDIDATE')"
[[ -d "$GENERATIONS/goodcand-8-8/home/.agents" ]] ||
  fail "state 2: reusable candidate was destroyed"

# 2b) stale candidate deleted.
[[ ! -d "$GENERATIONS/stalecand-7-7" ]] ||
  fail "state 2b: stale (non-matching) candidate was not deleted"

# 3) store link + lock link repaired.
[[ "$(readlink "$STORE/alpha")" == "../.skills-current/skills/alpha" ]] ||
  fail "state 3: stale store link for alpha was not repaired (got '$(readlink "$STORE/alpha")')"
[[ "$(readlink "$SKILL_LOCK_LINK")" == ".skills-current/.skill-lock.json" ]] ||
  fail "state 3: stale lock link was not repaired (got '$(readlink "$SKILL_LOCK_LINK")')"

# 4) competing-writer real dir recorded for re-absorption; content preserved.
recorded=""
for n in "${GEN_REABSORB[@]:-}"; do
  if [[ $n == beta ]]; then recorded=1; fi
done
[[ -n $recorded ]] || fail "state 4: competing-writer real dir 'beta' not recorded in GEN_REABSORB"
[[ -d "$STORE/beta" && ! -L "$STORE/beta" ]] ||
  fail "state 4: recovery must not destroy the competing-writer real dir before re-absorption"
grep -q 'competing-writer content' "$STORE/beta/SKILL.md" ||
  fail "state 4: competing-writer content was lost"

# 5) partial-prune garbage swept.
[[ ! -d "$GENERATIONS/old-3-3.garbage.123.456" ]] ||
  fail "state 5: partial-prune garbage was not swept"

# 6) newest retained generation kept, older pruned.
[[ -d "$GENERATIONS/2000000000-2-2" ]] ||
  fail "state 6: newest retained generation was pruned"
[[ ! -d "$GENERATIONS/1000000000-1-1" ]] ||
  fail "state 6: older retained generation was not pruned"

# Idempotence: a second recovery run changes nothing and finds no new drift.
prev_reuse="$GEN_REUSE_CANDIDATE"
__gen_recover
[[ $GEN_REUSE_CANDIDATE == "$prev_reuse" ]] ||
  fail "recovery is not idempotent: reusable candidate changed on the second run"
[[ "$(readlink "$STORE/alpha")" == "../.skills-current/skills/alpha" ]] ||
  fail "recovery is not idempotent: store link drifted on the second run"

echo "update-skills-recovery: OK"
