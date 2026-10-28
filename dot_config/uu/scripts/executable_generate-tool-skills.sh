#!/usr/bin/env bash
# Regenerates the skills that two CLIs write themselves: the ElevenLabs set
# (`elevenlabs generate-skills`, one elevenlabs-* folder per API group) and the
# GitButler skill (`but skill install`). Both land in the shared store,
# ~/.agents/skills, where the apply links them and Codex reads them.
#
# Prints one line per changed skill set and nothing when nothing changed.
# Problems go to stderr as `error[kind]: message`. The apply runs this from
# .chezmoiscripts/run_after_63-generate-tool-skills.sh.tmpl and the uu
# `tool-skills` lane runs it after the lanes that upgrade the two CLIs, so a
# new CLI version regenerates its skills without waiting for an apply.

set -euo pipefail

readonly skills_folder="$HOME/.agents/skills"
readonly fnm_default_node_bin_directory="$HOME/.local/share/fnm/aliases/default/bin"
readonly gitbutler_skill="$skills_folder/gitbutler"

print_error() {
  local kind=$1 message=$2
  printf 'error[%s]: %s\n' "$kind" "$message" >&2
}

tool_is_installed() {
  local tool=$1
  command -v "$tool" >/dev/null 2>&1
}

# --- elevenlabs ---

elevenlabs_folders_in() {
  local folder=$1
  find "$folder" -mindepth 1 -maxdepth 1 -type d -name 'elevenlabs-*' -exec basename {} \; | sort
}

generated_set_differs_from_store() {
  local generated=$1
  local name
  if [[ $(elevenlabs_folders_in "$generated") != $(elevenlabs_folders_in "$skills_folder") ]]; then
    return 0
  fi
  while read -r name; do
    if ! diff -rq "$generated/$name" "$skills_folder/$name" >/dev/null 2>&1; then
      return 0
    fi
  done < <(elevenlabs_folders_in "$generated")
  return 1
}

swap_generated_set_into_store() {
  local generated=$1
  local name
  while read -r name; do
    rm -rf "${skills_folder:?}/${name:?}"
  done < <(elevenlabs_folders_in "$skills_folder")
  while read -r name; do
    mv "$generated/$name" "$skills_folder/$name"
  done < <(elevenlabs_folders_in "$generated")
}

regenerate_elevenlabs_skills() {
  if ! tool_is_installed elevenlabs; then
    print_error missing-tool "the elevenlabs CLI is not installed, so the ElevenLabs skills were not generated"
    return 1
  fi
  local generated status=0
  generated="$(mktemp -d)"
  if ! elevenlabs generate-skills --output-dir "$generated" --quiet </dev/null >/dev/null 2>&1; then
    print_error generate-failed "elevenlabs generate-skills failed; run it by hand to see why (it needs elevenlabs auth login once)"
    status=1
  elif [[ -z $(elevenlabs_folders_in "$generated") ]]; then
    print_error generate-failed "elevenlabs generate-skills wrote no elevenlabs-* folder"
    status=1
  elif generated_set_differs_from_store "$generated"; then
    swap_generated_set_into_store "$generated"
    printf 'elevenlabs: regenerated %s skills\n' "$(elevenlabs_folders_in "$skills_folder" | wc -l | tr -d ' ')"
  fi
  rm -rf "$generated"
  return "$status"
}

# --- gitbutler ---

skill_version_in() {
  local folder=$1
  sed -n 's/^version: *//p' "$folder/SKILL.md" 2>/dev/null | head -n 1
}

regenerate_gitbutler_skill() {
  if ! tool_is_installed but; then
    print_error missing-tool "the but CLI is not installed, so the gitbutler skill was not generated"
    return 1
  fi
  local before after
  before="$(skill_version_in "$gitbutler_skill")"
  if ! but skill install --path "$gitbutler_skill" </dev/null >/dev/null 2>&1 || [[ ! -f $gitbutler_skill/SKILL.md ]]; then
    print_error generate-failed "but skill install --path $gitbutler_skill did not write a SKILL.md"
    return 1
  fi
  after="$(skill_version_in "$gitbutler_skill")"
  if [[ $before != "$after" ]]; then
    printf 'gitbutler: %s\n' "${before:+$before to }$after"
  fi
}

main() {
  PATH="$fnm_default_node_bin_directory:$PATH"
  mkdir -p "$skills_folder"
  local status=0
  regenerate_elevenlabs_skills || status=1
  regenerate_gitbutler_skill || status=1
  exit "$status"
}

main "$@"
