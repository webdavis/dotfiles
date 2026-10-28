#!/usr/bin/env bash

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SKILLS_DATA="${REPO_ROOT}/.chezmoidata/agent-skills.yaml"
DEFAULT_OUTPUT_FOLDER="${HOME}/Desktop/claude-desktop-skills"
ZIP_EXCLUDES=("*.DS_Store" ".claude-plugin/*" ".github/*" ".git/*")

usage() {
  cat >&2 <<'USAGE'
usage: claude-desktop-skills.sh [output-folder]
USAGE
  exit 1
}

fail() {
  local message="$1"
  echo "error: ${message}" >&2
  exit 1
}

require_commands() {
  local command_name
  for command_name in "$@"; do
    command -v "$command_name" >/dev/null || fail "${command_name} is required but not installed"
  done
}

claude_desktop_repos() {
  yq -r '.skills.harnesses.claude_desktop[].repo' "$SKILLS_DATA"
}

skills_of_repo() {
  local repo="$1"
  yq -r ".skills.harnesses.claude_desktop[] | select(.repo == \"${repo}\") | .skills[]" "$SKILLS_DATA"
}

clone_folder_for_repo() {
  local stage="$1"
  local repo="$2"
  echo "${stage}/${repo//\//-}"
}

clone_repo() {
  local repo="$1"
  local clone="$2"
  gh repo clone "$repo" "$clone" -- --depth 1 --quiet
}

skill_folder_in_clone() {
  local clone="$1"
  local skill="$2"
  find "$clone" -name SKILL.md -not -path '*/node_modules/*' -exec grep -lx "name: ${skill}" {} + |
    head -n 1 | xargs -I{} dirname {}
}

zip_exclude_arguments() {
  local skill="$1"
  local exclude
  for exclude in "${ZIP_EXCLUDES[@]}"; do
    if [[ $exclude == */* ]]; then
      echo "${skill}/${exclude}"
    else
      echo "$exclude"
    fi
  done
}

zip_skill_folder() {
  local stage="$1"
  local skill="$2"
  local zip_file="$3"
  local excludes=()
  mapfile -t excludes < <(zip_exclude_arguments "$skill")
  (cd "$stage" && zip -FSrq "$zip_file" "$skill" -x "${excludes[@]}")
}

stage_skill() {
  local stage="$1"
  local repo="$2"
  local clone="$3"
  local skill="$4"

  local folder
  folder="$(skill_folder_in_clone "$clone" "$skill")"
  [[ -n $folder ]] || fail "${repo} has no SKILL.md named ${skill}"

  rm -rf "${stage:?}/${skill}"
  cp -R "$folder" "${stage}/${skill}"
}

zip_repo_skills() {
  local stage="$1"
  local repo="$2"
  local output_folder="$3"

  local clone
  clone="$(clone_folder_for_repo "$stage" "$repo")"
  clone_repo "$repo" "$clone"

  local skill
  while read -r skill; do
    stage_skill "$stage" "$repo" "$clone" "$skill"
    zip_skill_folder "$stage" "$skill" "${output_folder}/${skill}.zip"
  done < <(skills_of_repo "$repo")
}

zip_every_skill() {
  local output_folder="$1"

  local stage
  stage="$(mktemp -d)"
  trap 'rm -rf "$stage"' EXIT

  local repo
  while read -r repo; do
    zip_repo_skills "$stage" "$repo" "$output_folder"
  done < <(claude_desktop_repos)
}

main() {
  [[ $# -le 1 ]] || usage
  local output_folder="${1:-$DEFAULT_OUTPUT_FOLDER}"

  require_commands gh yq zip
  mkdir -p "$output_folder"
  zip_every_skill "$output_folder"
  echo "Drag the zips in ${output_folder} into Claude Desktop."
}

[[ ${BASH_SOURCE[0]} == "$0" ]] && main "$@"
