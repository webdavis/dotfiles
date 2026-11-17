#!/usr/bin/env bash
# Puts the Neovim plugin of each given GitHub repo on the commit its installed program was built from.
# It takes the folder that holds the plugins, then each repo as owner/name.
# A repo whose program is not installed, or whose commit cannot be fetched, keeps the plugin it has.

set -uo pipefail

readonly installed_crates_record="${CARGO_INSTALLED_CRATES_RECORD:-$HOME/.cargo/.crates2.json}"
readonly cargo_git_database="${CARGO_GIT_DATABASE:-$HOME/.cargo/git/db}"
export GIT_TERMINAL_PROMPT=0

installed_commit() {
  local repo=$1
  [[ -f $installed_crates_record ]] || return 0
  jq -r --arg source "(git+https://github.com/${repo}#" \
    '[.installs | keys[] | select(contains($source)) | split("#")[1][0:40]][0] // empty' \
    "$installed_crates_record"
}

plugin_is_on() {
  local plugin=$1 commit=$2
  [[ $(git -C "$plugin" rev-parse -q --verify HEAD 2>/dev/null) == "$commit" ]]
}

plugin_has() {
  local plugin=$1 commit=$2
  git -C "$plugin" cat-file -e "$commit^{commit}" 2>/dev/null
}

# Cargo already downloaded the commit to build the program, so its copy is tried before the network.
fetch_commit() {
  local plugin=$1 repo=$2 commit=$3 cargo_copy
  cargo_copy="$(compgen -G "$cargo_git_database/${repo##*/}-*" | head -n 1)"
  if [[ -n $cargo_copy ]]; then
    git -C "$plugin" fetch -q "$cargo_copy" '+refs/*:refs/cargo/*' 2>/dev/null
    if plugin_has "$plugin" "$commit"; then
      return 0
    fi
  fi
  git -C "$plugin" fetch -q "https://github.com/$repo" "$commit" 2>/dev/null
}

pair_plugin_with_program() {
  local plugins_folder=$1 repo=$2
  local name=${repo##*/} plugin commit
  plugin="$plugins_folder/$name"
  commit="$(installed_commit "$repo")"
  if [[ ! $commit =~ ^[0-9a-f]{40}$ ]]; then
    return 0
  fi
  if plugin_is_on "$plugin" "$commit"; then
    return 0
  fi
  if [[ ! -d $plugin/.git ]] && ! git init -q "$plugin"; then
    printf '%s: error[paired-plugin]: could not create %s\n' "$name" "$plugin" >&2
    return 1
  fi
  if ! plugin_has "$plugin" "$commit" && ! fetch_commit "$plugin" "$repo" "$commit"; then
    printf '%s: error[paired-plugin]: could not fetch %s, so the plugin stays as it is; the next apply retries.\n' "$name" "$commit" >&2
    return 1
  fi
  # A branch, because lazy.nvim's update check fails on a checkout that is on no branch.
  if ! git -C "$plugin" checkout -q -B paired "$commit"; then
    printf '%s: error[paired-plugin]: could not check out %s in %s\n' "$name" "$commit" "$plugin" >&2
    return 1
  fi
  printf '%s: plugin now matches the program at %s\n' "$name" "${commit:0:7}"
}

main() {
  local plugins_folder=$1 repo pairing_status=0
  shift
  for repo in "$@"; do
    pair_plugin_with_program "$plugins_folder" "$repo" || pairing_status=1
  done
  return "$pairing_status"
}

main "$@"
