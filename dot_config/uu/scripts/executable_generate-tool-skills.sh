#!/usr/bin/env bash
# Regenerates the ElevenLabs, GitButler and Pylon skills, which their own CLIs write.

set -euo pipefail

readonly skills_folder="$HOME/.agents/skills"
readonly broccoli_skills_folder="$HOME/.config/agents/broccoli-tech-support/.agents/skills"
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

skill_folders_in() {
  local folder=$1 prefix=$2
  find "$folder" -mindepth 1 -maxdepth 1 -type d -name "$prefix*" -exec basename {} \; | sort
}

generated_set_differs_from_store() {
  local generated=$1 store=$2 prefix=$3
  local name
  if [[ $(skill_folders_in "$generated" "$prefix") != $(skill_folders_in "$store" "$prefix") ]]; then
    return 0
  fi
  while read -r name; do
    if ! diff -rq "$generated/$name" "$store/$name" >/dev/null 2>&1; then
      return 0
    fi
  done < <(skill_folders_in "$generated" "$prefix")
  return 1
}

swap_generated_set_into_store() {
  local generated=$1 store=$2 prefix=$3
  local name
  while read -r name; do
    rm -rf "${store:?}/${name:?}"
  done < <(skill_folders_in "$store" "$prefix")
  while read -r name; do
    mv "$generated/$name" "$store/$name"
  done < <(skill_folders_in "$generated" "$prefix")
}

regenerate_elevenlabs_skills() {
  if ! tool_is_installed elevenlabs; then
    print_error missing-tool "the elevenlabs CLI is not installed, so the ElevenLabs skills were not generated"
    return 1
  fi
  local generated status=0
  generated="$(mktemp -d)"
  if ! elevenlabs generate-skills --output-dir "$generated" --quiet </dev/null >/dev/null 2>&1; then
    print_error generate-failed "elevenlabs generate-skills failed; run it by hand to see why"
    status=1
  elif [[ -z $(skill_folders_in "$generated" elevenlabs-) ]]; then
    print_error generate-failed "elevenlabs generate-skills wrote no elevenlabs-* folder"
    status=1
  elif generated_set_differs_from_store "$generated" "$skills_folder" elevenlabs-; then
    swap_generated_set_into_store "$generated" "$skills_folder" elevenlabs-
    printf 'elevenlabs: regenerated %s skills\n' "$(skill_folders_in "$skills_folder" elevenlabs- | wc -l | tr -d ' ')"
  fi
  rm -rf "$generated"
  return "$status"
}

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

this_mac_does_broccoli_work() {
  tool_is_installed pylon && [[ -d $broccoli_skills_folder ]]
}

regenerate_pylon_skills() {
  if ! this_mac_does_broccoli_work; then
    return 0
  fi
  local generated status=0
  generated="$(mktemp -d)"
  if ! pylon generate-skills --output-dir "$generated" </dev/null >/dev/null 2>&1; then
    print_error generate-failed "pylon generate-skills failed; run it by hand to see why"
    status=1
  elif [[ -z $(skill_folders_in "$generated" pylon-) ]]; then
    print_error generate-failed "pylon generate-skills wrote no pylon-* folder"
    status=1
  elif generated_set_differs_from_store "$generated" "$broccoli_skills_folder" pylon-; then
    swap_generated_set_into_store "$generated" "$broccoli_skills_folder" pylon-
    printf 'pylon: regenerated %s skills\n' "$(skill_folders_in "$broccoli_skills_folder" pylon- | wc -l | tr -d ' ')"
  fi
  rm -rf "$generated"
  return "$status"
}

main() {
  PATH="$fnm_default_node_bin_directory:$PATH"
  mkdir -p "$skills_folder"
  local status=0
  regenerate_elevenlabs_skills || status=1
  regenerate_gitbutler_skill || status=1
  regenerate_pylon_skills || status=1
  exit "$status"
}

main "$@"
