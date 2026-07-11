#!/usr/bin/env bash
# update-skills-generation-lanes.sh proves the candidate build, the env -i
# isolated lanes, validation, and sibling-path resolution (Wave 3a fix4 brief
# steps 2-4 and the isolation + sibling-path hostile tests).
#
#   1. Happy path: clone the current generation, run the npx + clawhub + overlay
#      lanes inside a candidate fake HOME, validate, and publish. The published
#      generation carries every roster skill (with SKILL.md), the on-demand
#      overlay, and clawhub origin metadata.
#   2. Isolation (hostile): the lanes run under env -i with HOME / XDG_* / TMPDIR
#      / npm cache pinned inside the candidate. A DECOY real HOME laid out with
#      sentinel XDG and npm dirs is byte-identical after the run; a lane can
#      only write into the candidate.
#   3. Sibling-path resolution: from inside a RESOLVED workflow skill, the
#      relative sibling reference ../core reaches the SAME generation's core.
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
{
  "tiers": {"workflow": "on-demand", "core": "on-demand", "claw": "core"},
  "npxTracked": {"workflow": {"repo": "x/hf"}, "core": {"repo": "x/hf"}},
  "clawhubTracked": {"claw": {"slug": "@o/claw", "registry": "https://c.example"}}
}
EOF

# --- Stubs. The npx stub deliberately references HOME, XDG_CACHE_HOME,
#     npm_config_cache and TMPDIR to write breadcrumbs, so the isolation
#     assertion is meaningful: if __gen_run_lanes failed to pin one of them the
#     breadcrumb would land in the decoy real HOME.
stub="$tmp/stub"
mkdir -p "$stub"
cat >"$stub/npx" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
mode=""; prev=""; skills=()
for a in "$@"; do
  case "$a" in add) mode=add ;; update) mode=update ;; esac
  [[ $prev == --skill ]] && skills+=("$a")
  prev="$a"
done
if [[ $mode == add ]]; then
  for s in "${skills[@]}"; do
    mkdir -p "$HOME/.agents/skills/$s"
    printf -- '---\nname: %s\n---\n# %s\n' "$s" "$s" >"$HOME/.agents/skills/$s/SKILL.md"
  done
  # environment breadcrumbs (must all resolve inside the candidate):
  : >"$HOME/.agents/.skill-lock.json"
  printf '{"updated":true}\n' >"$HOME/.agents/.skill-lock.json"
  mkdir -p "$XDG_CACHE_HOME" "$npm_config_cache" "$TMPDIR"
  : >"$XDG_CACHE_HOME/npx-ran"
  : >"$npm_config_cache/npm-ran"
fi
STUB
cat >"$stub/clawhub" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
wd=""; dir="skills"; mode=""; prev=""
for a in "$@"; do
  case "$prev" in --workdir) wd="$a" ;; --dir) dir="$a" ;; esac
  case "$a" in install) mode=install ;; update) mode=update ;; esac
  prev="$a"
done
args=("$@"); slug="${args[${#args[@]} - 1]}"
if [[ $mode == install ]]; then
  dest="$wd/$dir/$slug"; mkdir -p "$dest/.clawhub"
  printf -- '---\nname: %s\n---\n' "$(basename "$slug")" >"$dest/SKILL.md"
  printf '{"slug":"%s"}\n' "$(basename "$slug")" >"$dest/.clawhub/origin.json"
fi
STUB
chmod +x "$stub/npx" "$stub/clawhub"
export PATH="$stub:$PATH"
export UPDATE_SKILLS_GMV="$GMV_BIN"

# --- Decoy real HOME with sentinel XDG + npm dirs (must stay byte-identical).
decoy="$tmp/decoy"
mkdir -p "$decoy/.cache" "$decoy/.config" "$decoy/.npm" "$decoy/.local/share" "$decoy/.agents/skills"
printf 'SENTINEL' >"$decoy/.cache/keep"
printf 'SENTINEL' >"$decoy/.npm/keep"
printf 'SENTINEL' >"$decoy/.agents/keep"
decoy_before="$(find "$decoy" -type f -exec shasum {} \; | sort)"

export UPDATE_SKILLS_LIB_ONLY=1
# shellcheck disable=SC1090
source "$SCRIPT"

# Seed the current generation (as if migrated): workflow, core, claw.
seed() {
  local name="$1"
  mkdir -p "$SKILLS_CURRENT/skills/$name"
  printf -- '---\nname: %s\n---\n' "$name" >"$SKILLS_CURRENT/skills/$name/SKILL.md"
}
seed workflow
seed core
mkdir -p "$SKILLS_CURRENT/skills/claw/.clawhub"
printf -- '---\nname: claw\n---\n' >"$SKILLS_CURRENT/skills/claw/SKILL.md"
printf '{"slug":"claw"}\n' >"$SKILLS_CURRENT/skills/claw/.clawhub/origin.json"
printf '{}' >"$SKILLS_CURRENT/.skill-lock.json"
__gen_write_meta "$SKILLS_CURRENT" "cur-1-1"
__gen_plant_store_link workflow
__gen_plant_store_link core
__gen_plant_store_link claw
__gen_plant_lock_link

# --- Build + lanes + validate + publish.
__gen_recover
id="$(__gen_new_id)"
__gen_build_candidate "$id" || fail "candidate build failed"
__gen_run_lanes "$GEN_CANDIDATE_HOME" "$id" >/dev/null 2>&1 || fail "lanes returned non-zero"
__gen_validate_candidate "$GEN_CANDIDATE_AGENTS" || fail "candidate failed validation"

# 2) Isolation: the breadcrumbs landed in the candidate, and the decoy is intact.
[[ -f "$GEN_CANDIDATE_HOME/.cache/npx-ran" ]] ||
  fail "isolation: XDG_CACHE_HOME breadcrumb did not land in the candidate"
[[ -f "$GEN_CANDIDATE_HOME/.npm/npm-ran" ]] ||
  fail "isolation: npm cache breadcrumb did not land in the candidate"
decoy_after="$(find "$decoy" -type f -exec shasum {} \; | sort)"
[[ $decoy_before == "$decoy_after" ]] ||
  fail "isolation: the decoy real HOME was modified by the staged lanes"

__gen_publish "$GEN_CANDIDATE_AGENTS" || fail "publish failed"

# 1) Published generation carries every roster skill + overlay + origin metadata.
for name in workflow core claw; do
  [[ -f "$STORE/$name/SKILL.md" ]] || fail "published store/$name/SKILL.md does not resolve"
done
grep -q 'allow_implicit_invocation: false' "$STORE/workflow/agents/openai.yaml" ||
  fail "published on-demand skill workflow has no Codex overlay"
[[ -f "$STORE/claw/.clawhub/origin.json" ]] || fail "published claw lost its origin metadata"

# 3) Sibling-path resolution: from inside the resolved workflow skill, ../core
#    reaches the SAME generation's core (both physically under one generation).
sibling="$(cd -P "$STORE/workflow" && cd -P ../core && pwd -P)"
workflow_gen="$(cd -P "$STORE/workflow" && cd -P .. && pwd -P)"
[[ $sibling == "$workflow_gen/core" ]] ||
  fail "sibling ../core did not resolve within the workflow skill's own generation ($sibling vs $workflow_gen/core)"
[[ -f "$sibling/SKILL.md" ]] || fail "sibling ../core does not resolve to a real skill"

echo "update-skills-generation-lanes: OK"
