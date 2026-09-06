#!/usr/bin/env bash
#
# verify-nvim-bootstrap.sh: does the Neovim state on this machine match what the
# config declares. Two questions, one per tool:
#
#   lazy.nvim   every plugin pinned in lazy-lock.json has a DIRECTORY under the
#               lazy root
#   mason       every package name in the resolved tool list has a DIRECTORY
#               under the Mason packages root
#
# This is the bootstrap's proof, and it exists because Neovim's exit status is
# not one. `nvim --headless -c 'Lazy! restore' -c 'qa!'` exits 0 whether the
# restore cloned every repository or none of them, and `:MasonToolsInstallSync`
# behaves the same way, so a half-installed editor would otherwise read as a
# finished apply. A clone that finished left a directory behind, and that is the
# observable answered here.
#
# Everything is a function of the PATHS the caller passes: nothing reads $HOME,
# guesses a root, or consults the operator's editor, which is what lets the unit
# suite drive these one behavior at a time against a throwaway tree.
#
# SOURCEABLE. `test/unit/nvim-bootstrap-verify.test.sh` sources this file, so
# `set -euo pipefail` lives inside the execution guard at the bottom rather than
# at file scope: at file scope it would reach into bashunit's own shell and kill
# the run on its first failing assertion.

usage='usage: verify-nvim-bootstrap.sh <lazy-lock.json> <lazy-dir> <mason-tool-list> <mason-packages-dir>'

# missing_lazy_dirs <lazy-lock.json> <lazy-dir>
#
# Prints each plugin pinned in the lock that has no directory under <lazy-dir>,
# one name per line, in the lock's sorted key order. Returns 2 and prints
# nothing when the lock cannot be read or parsed.
missing_lazy_dirs() {
  local lock="$1" lazy_dir="$2" pinned name
  # jq's status is read BEFORE the loop rather than inside a process
  # substitution, where it is invisible: an unreadable or truncated lock would
  # then yield zero pins and this function would report nothing missing, which
  # is a gate that passes on a corrupt input.
  pinned="$(jq -r 'keys[]' "$lock" 2>/dev/null)" || return 2
  while IFS= read -r name || [[ -n $name ]]; do
    [[ -n $name ]] || continue
    [[ -d "$lazy_dir/$name" ]] || printf '%s\n' "$name"
  done <<<"$pinned"
}

# missing_mason_packages <mason-tool-list> <mason-packages-dir>
#
# Prints each name in the tool list that has no directory under
# <mason-packages-dir>, one per line, in the list's own order. Blank lines are
# skipped and a repeated name is answered where it appears. Returns 2 and prints
# nothing when the list cannot be read.
missing_mason_packages() {
  local tool_list="$1" packages_dir="$2" name
  [[ -r $tool_list ]] || return 2
  while IFS= read -r name || [[ -n $name ]]; do
    [[ -n $name ]] || continue
    [[ -d "$packages_dir/$name" ]] || printf '%s\n' "$name"
  done <"$tool_list"
}

# main <lazy-lock.json> <lazy-dir> <mason-tool-list> <mason-packages-dir>
#
# Prints every missing name with the tool that owns it and returns 1; prints
# nothing and returns 0 when everything declared is on disk. Returns 2 when an
# input is unusable, which is a different answer from "nothing is missing".
main() {
  if (($# != 4)); then
    printf '%s\n' "$usage" >&2
    return 2
  fi

  local lock="$1" lazy_dir="$2" tool_list="$3" packages_dir="$4"
  local lazy_missing mason_missing name status=0

  if ! lazy_missing="$(missing_lazy_dirs "$lock" "$lazy_dir")"; then
    printf 'verify-nvim-bootstrap: cannot read the plugin lock: %s\n' "$lock" >&2
    return 2
  fi

  # An empty tool list is not evidence that no tool is required: it is what a
  # failed extraction leaves behind, and reading it as "nothing to check" would
  # delete the Mason half of this gate without printing a word.
  if [[ ! -s $tool_list ]]; then
    printf 'verify-nvim-bootstrap: the Mason tool list is empty or absent: %s\n' "$tool_list" >&2
    return 2
  fi

  if ! mason_missing="$(missing_mason_packages "$tool_list" "$packages_dir")"; then
    printf 'verify-nvim-bootstrap: cannot read the Mason tool list: %s\n' "$tool_list" >&2
    return 2
  fi

  while IFS= read -r name; do
    [[ -n $name ]] || continue
    printf 'missing lazy plugin: %s\n' "$name"
    status=1
  done <<<"$lazy_missing"

  while IFS= read -r name; do
    [[ -n $name ]] || continue
    printf 'missing mason package: %s\n' "$name"
    status=1
  done <<<"$mason_missing"

  return "$status"
}

# Only this block is a process. Sourcing the file defines the functions above
# and runs nothing, which is what the unit suite relies on.
if [[ ${BASH_SOURCE[0]} == "$0" ]]; then
  set -euo pipefail
  main "$@"
fi
