#!/usr/bin/env bash
# The apply-time PROOF that Neovim's plugin and tool state matches what the
# config declares: given a lock file, a lazy root, a resolved Mason tool list
# and a Mason packages root, which declared names have no directory on disk.
#
# Why a directory and not an exit status. `nvim --headless -c 'Lazy! restore'
# -c 'qa!'` exits 0 whether the restore cloned every repository or none of
# them, and `:MasonToolsInstallSync` behaves the same way, so the bootstrap
# cannot read either one as evidence. A clone that finished left a directory
# behind; that is the observable these functions answer against, and every case
# below is one way that observable can lie.
#
# Every input is a PATH the caller supplies, so nothing here reads $HOME, the
# operator's editor, or the machine's real plugin tree. The fixture is a
# throwaway tree built once for the whole file.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit: both belong to a script that runs on its own, and either one
# here would reach into the runner's own shell. The subject is sourced for the
# same reason and keeps its own `set` line inside its execution guard.
#
# assert_same, never assert_equals: bashunit's assert_equals normalizes away
# ANSI and control characters before comparing (measured on 0.50.1), and a
# plugin name carrying one is a real defect that must not pass.

subject_under_test() {
  printf '%s' "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/dot_local/libexec/executable_verify-nvim-bootstrap.sh"
}

# shellcheck source=dot_local/libexec/executable_verify-nvim-bootstrap.sh
source "$(subject_under_test)"

# --- fixture ----------------------------------------------------------------
#
# One lock and one tool list, read against four roots that differ only in what
# is actually on disk:
#
#   complete/    every pin is a directory, every package is a directory
#   one-absent/  `blink.cmp` was never cloned
#   as-a-file/   `blink.cmp` is a FILE of the right name
#   no-package/  the `stylua` package directory is absent

fixture="$(mktemp -d)"

tear_down_after_script() {
  rm -rf "$fixture"
}

printf '{"aerial.nvim":{"commit":"aaa"},"blink.cmp":{"commit":"bbb"}}\n' >"$fixture/lock.json"
printf 'gopls\nstylua\n' >"$fixture/tools"
printf 'not json at all\n' >"$fixture/broken-lock.json"
: >"$fixture/empty-tools"

mkdir -p "$fixture/complete/lazy/aerial.nvim" "$fixture/complete/lazy/blink.cmp"
mkdir -p "$fixture/complete/packages/gopls" "$fixture/complete/packages/stylua"

mkdir -p "$fixture/one-absent/lazy/aerial.nvim"

mkdir -p "$fixture/as-a-file/lazy/aerial.nvim"
: >"$fixture/as-a-file/lazy/blink.cmp"

mkdir -p "$fixture/no-package/packages/gopls"

# --- lazy.nvim --------------------------------------------------------------

function test_a_lock_whose_every_pin_has_a_directory_reports_nothing() {
  assert_same "" "$(missing_lazy_dirs "$fixture/lock.json" "$fixture/complete/lazy")"
}

function test_a_pin_that_was_never_cloned_is_the_only_name_reported() {
  assert_same "blink.cmp" "$(missing_lazy_dirs "$fixture/lock.json" "$fixture/one-absent/lazy")"
}

function test_a_pin_present_as_a_file_rather_than_a_directory_is_missing() {
  # A test that only asked "does the path exist" passes the case above and
  # fails this one: lazy.nvim clones into a directory, so a file of that name
  # is an interrupted or hand-made stub, never an installed plugin.
  assert_same "blink.cmp" "$(missing_lazy_dirs "$fixture/lock.json" "$fixture/as-a-file/lazy")"
}

function test_a_lock_that_cannot_be_parsed_is_refused_rather_than_read_as_empty() {
  # Fail-closed: through a process substitution jq's status is invisible, and a
  # truncated lock would then yield zero pins and report nothing missing, which
  # is a gate that passes on a corrupt input.
  missing_lazy_dirs "$fixture/broken-lock.json" "$fixture/complete/lazy" >/dev/null 2>&1
  assert_same 2 "$?"
}

# --- mason ------------------------------------------------------------------

function test_a_tool_list_whose_every_name_has_a_package_directory_reports_nothing() {
  assert_same "" "$(missing_mason_packages "$fixture/tools" "$fixture/complete/packages")"
}

function test_a_tool_with_no_package_directory_is_the_only_name_reported() {
  assert_same "stylua" "$(missing_mason_packages "$fixture/tools" "$fixture/no-package/packages")"
}

# --- the script's own boundary ----------------------------------------------

function test_a_complete_installation_prints_nothing_and_exits_zero() {
  local output
  output="$(main "$fixture/lock.json" "$fixture/complete/lazy" "$fixture/tools" "$fixture/complete/packages")"
  assert_same 0 "$?"
  assert_same "" "$output"
}

function test_every_missing_name_is_printed_with_the_tool_that_owns_it() {
  local output
  output="$(main "$fixture/lock.json" "$fixture/one-absent/lazy" "$fixture/tools" "$fixture/no-package/packages")"
  assert_same "missing lazy plugin: blink.cmp
missing mason package: stylua" "$output"
}

function test_anything_missing_exits_one() {
  main "$fixture/lock.json" "$fixture/one-absent/lazy" "$fixture/tools" "$fixture/complete/packages" >/dev/null
  assert_same 1 "$?"
}

function test_an_empty_tool_list_is_refused_rather_than_passing_vacuously() {
  # An empty list is what a failed extraction leaves behind, not evidence that
  # no tool is required. Accepting it would delete the Mason half of this gate
  # without printing a word.
  main "$fixture/lock.json" "$fixture/complete/lazy" "$fixture/empty-tools" "$fixture/complete/packages" >/dev/null 2>&1
  assert_same 2 "$?"
}

function test_the_wrong_number_of_arguments_is_an_error_not_a_help_screen() {
  main "$fixture/lock.json" >/dev/null 2>&1
  assert_same 2 "$?"
}
