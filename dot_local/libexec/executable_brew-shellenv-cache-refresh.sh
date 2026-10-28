#!/usr/bin/env bash
# Caches `brew shellenv` so each shell sources a file instead of spawning brew.
# It runs the real generator, which also restores the etc/paths file path_helper reads.
set -euo pipefail

readonly DEFAULT_HOMEBREW_PREFIX='/opt/homebrew'
readonly CACHE_FILE_NAME='brew-shellenv.sh'
readonly EXIT_USAGE=2

# An absolute path, because Homebrew may not be on PATH yet. Tests can override it.
brew_executable="${BREW_SHELLENV_CACHE_BREW:-${HOMEBREW_PREFIX:-$DEFAULT_HOMEBREW_PREFIX}/bin/brew}"

# One fixed path, the same one the shell's guard reads.
cache_file="${XDG_CACHE_HOME:-$HOME/.cache}/$CACHE_FILE_NAME"

# Set only while a temp file exists, so the EXIT trap knows whether to clean up.
temporary_cache_file=''

script_name="${0##*/}"

usage() {
  printf 'Usage: %s\n' "$script_name"
  printf '\n'
  printf 'Regenerate %s from the output of: %s shellenv\n' "$cache_file" "$brew_executable"
  printf 'Takes no arguments.\n'
}

fail() {
  printf '%s: %s\n' "$script_name" "$*" >&2
  exit 1
}

remove_temporary_cache_file() {
  if [[ -n $temporary_cache_file ]]; then
    rm -f "$temporary_cache_file"
  fi
  return 0
}

homebrew_is_installed() {
  [[ -x $brew_executable ]]
}

# `mv` would move the file into a directory, so anything but a regular file is left for a human.
cache_path_is_replaceable() {
  [[ ! -e $cache_file || -f $cache_file ]]
}

# brew shellenv prints nothing when PATH already starts with Homebrew, and an empty cache never heals.
brew_emitted_an_environment() {
  local candidate_file="$1"
  [[ -s $candidate_file ]]
}

# A temp file renamed into place keeps the previous cache when a run fails or prints nothing.
write_cache_atomically() {
  mkdir -p "${cache_file%/*}"
  temporary_cache_file="$(mktemp "${cache_file}.XXXXXX")"
  "$brew_executable" shellenv >"$temporary_cache_file"
  brew_emitted_an_environment "$temporary_cache_file" ||
    fail "$brew_executable shellenv exited 0 but printed nothing, so the cache was left alone;" \
      "re-run from a shell whose PATH does not already start with the Homebrew bin and sbin pair"
  mv "$temporary_cache_file" "$cache_file"
  temporary_cache_file=''
}

reject_unknown_arguments() {
  local argument
  for argument in "$@"; do
    case "$argument" in
      -h | --help)
        usage
        exit 0
        ;;
      *)
        printf '%s: unknown argument: %s\n' "$script_name" "$argument" >&2
        usage >&2
        exit "$EXIT_USAGE"
        ;;
    esac
  done
}

main() {
  reject_unknown_arguments "$@"
  homebrew_is_installed ||
    fail "no Homebrew executable at $brew_executable; nothing to regenerate"
  cache_path_is_replaceable ||
    fail "$cache_file exists and is not a regular file; move or remove it, then re-run"
  write_cache_atomically
  printf 'Regenerated %s\n' "$cache_file"
}

trap remove_temporary_cache_file EXIT
main "$@"
