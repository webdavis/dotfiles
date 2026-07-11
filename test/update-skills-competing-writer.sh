#!/usr/bin/env bash
# update-skills-competing-writer.sh proves competing-writer drift is re-absorbed
# (Wave 3a fix4 hostile test). A store link is replaced with a REAL DIR mid-state
# (as the HyperFrames self-updater or an interrupted migration would leave it).
# The next run detects it, re-absorbs its content into the candidate, and the
# store returns to link topology with the content preserved (or refreshed).
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"
fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# Resolve the generation-exchange tool the way the updater does at run time
# (gmv on a Homebrew host, plain mv in the Nix devshell), never a hardcoded
# host path.
# shellcheck source=test/fixtures/exchange-tool.lib.sh
source "$REPO_ROOT/test/fixtures/exchange-tool.lib.sh"
GMV_BIN="$(resolve_exchange_tool)" ||
  fail "no GNU coreutils mv with a working --exchange on PATH (need gmv or mv)"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.agents/skills"
cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{"tiers":{"mover":"core"},"npxTracked":{"mover":{"repo":"x/mover"}},"clawhubTracked":{}}
EOF

stub="$tmp/stub"
mkdir -p "$stub"
# The npx stub refreshes SKILL.md but leaves any other files in place (a real
# add does not purge sibling files), so an absorbed competing file survives.
# Like the real CLI (verified against skills 1.5.16), it maintains its global
# lock at $XDG_STATE_HOME/skills/.skill-lock.json.
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
  cli_lock="${XDG_STATE_HOME:-$HOME/.local/state}/skills/.skill-lock.json"
  mkdir -p "$(dirname "$cli_lock")"
  [[ -f $cli_lock ]] || printf '{"version":3,"skills":{}}\n' >"$cli_lock"
  for s in "${skills[@]}"; do
    mkdir -p "$HOME/.agents/skills/$s"
    printf -- '---\nname: %s\n---\n# refreshed\n' "$s" >"$HOME/.agents/skills/$s/SKILL.md"
    jq --arg s "$s" '.skills[$s] = {source: "github:fixture"}' \
      "$cli_lock" >"$cli_lock.tmp" && mv "$cli_lock.tmp" "$cli_lock"
  done
fi
STUB
chmod +x "$stub/npx"
export PATH="$stub:$PATH"
export UPDATE_SKILLS_GMV="$GMV_BIN"
export UPDATE_SKILLS_LIB_ONLY=1
# shellcheck disable=SC1090
source "$SCRIPT"

# Seed a published generation with mover as a store symlink.
mkdir -p "$SKILLS_CURRENT/skills/mover"
printf -- '---\nname: mover\n---\n# original\n' >"$SKILLS_CURRENT/skills/mover/SKILL.md"
printf '{}' >"$SKILLS_CURRENT/.skill-lock.json"
__gen_write_meta "$SKILLS_CURRENT" "live-1-1"
__gen_plant_store_link mover
__gen_plant_lock_link
[[ -L "$STORE/mover" ]] || fail "precondition: store/mover should be a symlink"

# A competing writer replaces the store link with a REAL DIR carrying updated
# content plus a unique marker file.
rm -f "$STORE/mover"
mkdir -p "$STORE/mover"
printf -- '---\nname: mover\n---\n# COMPETING WRITER version\n' >"$STORE/mover/SKILL.md"
printf 'COMPETING-MARKER' >"$STORE/mover/competing.txt"

# The full-run orchestration for the re-absorption path.
__gen_recover
recorded=""
for n in "${GEN_REABSORB[@]:-}"; do
  if [[ $n == mover ]]; then recorded=1; fi
done
[[ -n $recorded ]] || fail "recovery did not record the competing-writer real dir for re-absorption"

id="$(__gen_new_id)"
__gen_build_candidate "$id" || fail "candidate build failed"
# The candidate must have absorbed the competing writer's content BEFORE the lanes.
[[ -f "$GEN_CANDIDATE_AGENTS/skills/mover/competing.txt" ]] ||
  fail "the candidate did not absorb the competing-writer content"
__gen_run_lanes "$GEN_CANDIDATE_HOME" "$id" >/dev/null 2>&1 || fail "lanes failed"
__gen_validate_candidate "$GEN_CANDIDATE_AGENTS" || fail "candidate failed validation"
__gen_publish "$GEN_CANDIDATE_AGENTS" || fail "publish failed"
# Post-publish: reconcile the re-absorbed store name back to link topology.
for n in "${GEN_REABSORB[@]:-}"; do __gen_absorb_store_link "$n"; done

# The store is back to a link that resolves, and the absorbed content survived.
[[ -L "$STORE/mover" ]] || fail "store/mover did not return to link topology after re-absorption"
[[ "$(readlink "$STORE/mover")" == "../.skills-current/skills/mover" ]] ||
  fail "store/mover points at the wrong target after re-absorption"
[[ -f "$STORE/mover/SKILL.md" ]] || fail "store/mover does not resolve to a skill"
[[ "$(cat "$STORE/mover/competing.txt" 2>/dev/null)" == "COMPETING-MARKER" ]] ||
  fail "the competing-writer content was lost (not preserved through re-absorption)"

echo "update-skills-competing-writer: OK"
