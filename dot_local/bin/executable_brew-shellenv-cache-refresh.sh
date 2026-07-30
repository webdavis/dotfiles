#!/usr/bin/env bash
#
# brew-shellenv-cache-refresh.sh -- regenerate the cached `brew shellenv` output
# that ~/.bashrc sources at startup. This is the ONLY implementation of that
# write; both callers run this deployed copy:
#
#   - ~/.bashrc's self-heal guard, detached, whenever the cache is missing or
#     stale. That guard is the only AUTOMATIC writer, because
#     `chezmoi apply --exclude=templates` (what `just a` and every agent runs)
#     skips templated scripts, so an apply-time regen script could not be relied
#     on and was removed.
#   - `just brew-cache-refresh`, on demand.
#
# Why a cache at all: every shell that reaches ~/.bashrc would otherwise run
# `eval "$(brew shellenv)"`, spawning the brew Bash dispatcher (~50ms) just to
# print six export lines. Two NON-interactive login doors reach ~/.bashrc
# (`bash -lc` via ~/.bash_profile, and `ssh host cmd` via Apple /bin/bash's
# SSH_SOURCE_BASHRC) and both need Homebrew on PATH, because /etc/paths does not
# carry ${HOMEBREW_PREFIX}/bin. Sourcing the pre-generated file costs ~1ms.
#
# Why run the real generator instead of hardcoding its output: `brew shellenv` is
# an upstream abstraction whose emitted exports change across Homebrew versions,
# and running it also RECREATES ${HOMEBREW_PREFIX}/etc/paths when that file is
# missing (Homebrew's Library/Homebrew/cmd/shellenv.sh does this). The
# path_helper line in the cached output reads that file at runtime, so without it
# ${HOMEBREW_PREFIX}/bin and /sbin silently drop out of PATH.
set -euo pipefail

readonly DEFAULT_HOMEBREW_PREFIX='/opt/homebrew'
readonly CACHE_FILE_NAME='brew-shellenv.sh'
readonly EXIT_USAGE=2

# The generator is overridable (BREW_SHELLENV_CACHE_BREW) so the test harness can
# inject a stub, the same seam shape ~/.local/bin/homebrew-weekly-upgrade.sh uses.
# It defaults to an ABSOLUTE path because ~/.bashrc launches this before Homebrew
# is necessarily on PATH, which is the very problem the cache exists to solve.
brew_executable="${BREW_SHELLENV_CACHE_BREW:-${HOMEBREW_PREFIX:-$DEFAULT_HOMEBREW_PREFIX}/bin/brew}"

# Destination, derived with the SAME expression ~/.bashrc's inline guard uses.
# Deliberately NOT overridable: a second way to move the file would let the guard
# and the writer disagree about which file they are talking about, and the guard
# cannot consult an env var this script invents.
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

# Is Homebrew installed where this host expects it?
homebrew_is_installed() {
  [[ -x $brew_executable ]]
}

# Regenerate the cache atomically: generate into a private temp (mktemp, mode
# 0600) in the SAME directory, and rename over the live cache only after brew has
# exited 0. A failing or half-finished run therefore leaves the previous cache
# intact rather than a truncated file that ~/.bashrc would source.
write_cache_atomically() {
  mkdir -p "${cache_file%/*}"
  temporary_cache_file="$(mktemp "${cache_file}.XXXXXX")"
  "$brew_executable" shellenv >"$temporary_cache_file"
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
  write_cache_atomically
  printf 'Regenerated %s\n' "$cache_file"
}

trap remove_temporary_cache_file EXIT
main "$@"
