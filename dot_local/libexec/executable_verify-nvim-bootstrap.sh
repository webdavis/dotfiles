#!/usr/bin/env bash
#
# verify-nvim-bootstrap.sh: does the Neovim state on this machine match what the
# config declares. Two questions, one per tool:
#
#   lazy.nvim   every plugin pinned in lazy-lock.json is CHECKED OUT AT the
#               commit the lock names
#   mason       every package name in the resolved tool list has a DIRECTORY
#               under the Mason packages root
#
# This is the bootstrap's proof, and it exists because Neovim's exit status is
# not one. `nvim --headless -c 'Lazy! restore' -c 'qa!'` exits 0 whether the
# restore moved every plugin to its pin or none of them, and
# `:MasonToolsInstallSync` behaves the same way, so a half-installed editor
# would otherwise read as a finished apply.
#
# The lazy question asks for the COMMIT, not merely for a directory. The
# acceptance rehearsal on 2026-09-05 finished green with 48 of 92 plugins one
# commit behind their pins: every directory was present and healthy, so an
# existence check had nothing to say about it. A directory proves a clone
# happened and never that the clone is where the lock says it should be.
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

# mispinned_lazy_plugins <lazy-lock.json> <lazy-dir>
#
# Prints every plugin pinned in the lock that is not checked out at the commit
# the lock names, one per line as `<name> <wanted> <found>`. `<found>` is the
# checked-out HEAD, or `absent` when there is no directory under <lazy-dir>, or
# `unknown` when git could not answer, which a half-finished clone cannot.
# Prints nothing when every pin is met. Returns 2 and prints nothing when the
# lock cannot be read or parsed.
mispinned_lazy_plugins() {
  local lock="$1" lazy_dir="$2" pinned name wanted found
  # jq's status is read BEFORE the loop rather than inside a process
  # substitution, where it is invisible: an unreadable or truncated lock would
  # then yield zero pins and this function would report nothing unmet, which is
  # a gate that passes on a corrupt input.
  pinned="$(jq -ers 'select(length == 1) | .[0] | objects |
    select(length > 0 and all(to_entries[];
      (.key | test("^[^[:space:]/]+$")) and (.value.commit | test("^[0-9a-f]{40}$")))) |
    to_entries[] | "\(.key) \(.value.commit)"' "$lock" 2>/dev/null)" || return 2
  while read -r name wanted || [[ -n $name ]]; do
    [[ -n $name ]] || continue
    if [[ ! -d "$lazy_dir/$name" ]]; then
      printf '%s %s absent\n' "$name" "$wanted"
      continue
    fi
    # `env -u`, not a bare `git -C`: GIT_DIR wins over -C, and git exports it
    # to every hook it runs. This repository's own pre-commit hook runs the
    # suite that drives this function, so without the scrub every plugin would
    # report this repository's HEAD.
    found="$(env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE \
      git -C "$lazy_dir/$name" rev-parse HEAD 2>/dev/null)" || found=''
    [[ -n $found ]] || found='unknown'
    [[ $found == "$wanted" ]] || printf '%s %s %s\n' "$name" "$wanted" "$found"
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
  local lazy_unmet mason_missing name wanted found status=0

  if ! lazy_unmet="$(mispinned_lazy_plugins "$lock" "$lazy_dir")"; then
    printf 'verify-nvim-bootstrap: cannot read the plugin lock: %s\n' "$lock" >&2
    return 2
  fi

  # An empty tool list is not evidence that no tool is required: it is what a
  # failed extraction leaves behind, and reading it as "nothing to check" would
  # delete the Mason half of this gate without printing a word.
  if [[ ! -r $tool_list ]] || ! grep -q '[^[:space:]]' "$tool_list"; then
    printf 'verify-nvim-bootstrap: the Mason tool list is empty or absent: %s\n' "$tool_list" >&2
    return 2
  fi

  if ! mason_missing="$(missing_mason_packages "$tool_list" "$packages_dir")"; then
    printf 'verify-nvim-bootstrap: cannot read the Mason tool list: %s\n' "$tool_list" >&2
    return 2
  fi

  while read -r name wanted found; do
    [[ -n $name ]] || continue
    printf 'lazy plugin off its pin: %s wants %s, has %s\n' "$name" "$wanted" "$found"
    status=1
  done <<<"$lazy_unmet"

  while IFS= read -r name; do
    [[ -n $name ]] || continue
    printf 'missing mason package: %s\n' "$name"
    status=1
  done <<<"$mason_missing"

  return "$status"
}

# Byte count is the rendered retry token, so two deferrals in one second
# still differ. No contents are read at render time.
bump_nvim_retry_marker() {
  local marker="$1"
  mkdir -p "$(dirname "$marker")"
  [[ ! -e $marker || -f $marker ]] || return 1
  printf 'x' >>"$marker"
}

# Keep each step's transcript until all verification is over. A tool may exit
# zero after reporting its own failure; the later failing gate needs that text.
nvim_bootstrap_step() {
  local log="$1" name="$2"
  shift 2
  printf '\n%s\n' "$name" >>"$log"
  if ! "$@" >>"$log" 2>&1; then
    cat "$log" >&2
    printf 'nvim bootstrap: %s failed; log retained at %s\n' "$name" "$log" >&2
    return 1
  fi
}

# Only this block is a process. Sourcing the file defines the functions above
# and runs nothing, which is what the unit suite relies on.
if [[ ${BASH_SOURCE[0]} == "$0" ]]; then
  set -euo pipefail
  main "$@"
fi
