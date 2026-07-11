#!/usr/bin/env bash
# update-skills-validation-failure.sh — proves the whole-candidate discard rule
# (Wave 3a fix4 brief step 4): ANY lane failure or validation failure discards
# the WHOLE candidate (no partial promotion, ever). The live generation stays
# byte-identical, a required failure is recorded, nothing is published, and the
# staging is cleaned.
#   Case A: a build lane exits non-zero (npx add fails).
#   Case B: the lanes succeed but a roster skill is missing its SKILL.md.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"
GMV_BIN="${UPDATE_SKILLS_GMV:-/opt/homebrew/bin/gmv}"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

live_hash() { find "$SKILLS_CURRENT" -type f -exec shasum {} \; | sed "s#$SKILLS_CURRENT##" | sort; }

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.agents/skills"
cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{"tiers":{"alpha":"core"},"npxTracked":{"alpha":{"repo":"x/a"}},"clawhubTracked":{}}
EOF

# npx stub whose behavior is switched by a marker file: normally installs, but
# when $tmp/npx-fail exists it exits non-zero (a failing lane).
stub="$tmp/stub"
mkdir -p "$stub"
cat >"$stub/npx" <<STUB
#!/usr/bin/env bash
set -euo pipefail
if [[ -f "$tmp/npx-fail" ]]; then
  echo "stub npx: forced failure" >&2
  exit 1
fi
mode=""; prev=""; skills=()
for a in "\$@"; do
  case "\$a" in add) mode=add ;; esac
  [[ \$prev == --skill ]] && skills+=("\$a")
  prev="\$a"
done
if [[ \$mode == add ]]; then
  for s in "\${skills[@]}"; do
    mkdir -p "\$HOME/.agents/skills/\$s"
    printf -- '---\nname: %s\n---\n' "\$s" >"\$HOME/.agents/skills/\$s/SKILL.md"
  done
fi
STUB
chmod +x "$stub/npx"
export PATH="$stub:$PATH"
export UPDATE_SKILLS_GMV="$GMV_BIN"
export UPDATE_SKILLS_LIB_ONLY=1
# shellcheck disable=SC1090
source "$SCRIPT"

# Seed a published live generation.
mkdir -p "$SKILLS_CURRENT/skills/alpha"
printf -- '---\nname: alpha\n---\n# LIVE\n' >"$SKILLS_CURRENT/skills/alpha/SKILL.md"
printf '{}' >"$SKILLS_CURRENT/.skill-lock.json"
__gen_write_meta "$SKILLS_CURRENT" "live-1-1"
__gen_plant_store_link alpha
__gen_plant_lock_link
before="$(live_hash)"

# The orchestration a full run performs on a candidate, with the discard rule.
attempt() {
  local id
  __gen_recover
  id="$(__gen_new_id)"
  __gen_build_candidate "$id" || return 1
  if ! __gen_run_lanes "$GEN_CANDIDATE_HOME" "$id" >/dev/null 2>&1; then
    record_required_failure "lanes failed"
    __gen_garbage_destroy "$GENERATIONS/$id"
    return 1
  fi
  if ! __gen_validate_candidate "$GEN_CANDIDATE_AGENTS"; then
    record_required_failure "validation failed"
    __gen_garbage_destroy "$GENERATIONS/$id"
    return 1
  fi
  __gen_publish "$GEN_CANDIDATE_AGENTS"
}

# --- Case A: failing lane.
touch "$tmp/npx-fail"
REQUIRED_FAILURES=0
attempt && fail "case A: attempt should have failed on the lane error"
[[ $REQUIRED_FAILURES -ge 1 ]] || fail "case A: no required failure was recorded"
[[ "$(live_hash)" == "$before" ]] || fail "case A: the live generation was modified by a failed attempt"
[[ "$(__gen_meta_field "$SKILLS_CURRENT" id)" == "live-1-1" ]] ||
  fail "case A: the live generation was replaced despite the lane failure"
shopt -s nullglob
leftovers=("$GENERATIONS"/*/home)
shopt -u nullglob
[[ ${#leftovers[@]} -eq 0 ]] || fail "case A: staging was not cleaned: ${leftovers[*]}"

# --- Case B: lanes succeed but a roster skill loses its SKILL.md. Model this
#     with an npx stub variant that omits the SKILL.md.
rm -f "$tmp/npx-fail"
cat >"$stub/npx" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
mode=""; prev=""; skills=()
for a in "$@"; do
  case "$a" in add) mode=add ;; esac
  [[ $prev == --skill ]] && skills+=("$a")
  prev="$a"
done
if [[ $mode == add ]]; then
  for s in "${skills[@]}"; do
    mkdir -p "$HOME/.agents/skills/$s" # dir but NO SKILL.md (a broken install)
    rm -f "$HOME/.agents/skills/$s/SKILL.md"
  done
fi
STUB
chmod +x "$stub/npx"
REQUIRED_FAILURES=0
attempt && fail "case B: attempt should have failed validation (missing SKILL.md)"
[[ $REQUIRED_FAILURES -ge 1 ]] || fail "case B: no required failure recorded on validation failure"
[[ "$(live_hash)" == "$before" ]] || fail "case B: the live generation was modified by a failed validation"
[[ "$(__gen_meta_field "$SKILLS_CURRENT" id)" == "live-1-1" ]] ||
  fail "case B: the live generation was replaced despite validation failure"
shopt -s nullglob
leftovers=("$GENERATIONS"/*/home)
shopt -u nullglob
[[ ${#leftovers[@]} -eq 0 ]] || fail "case B: staging was not cleaned: ${leftovers[*]}"

echo "update-skills-validation-failure: OK"
