#!/usr/bin/env bash
# The apply-time PROOF that Neovim's plugin and tool state matches what the
# config declares: given a lock file, a lazy root, a resolved Mason tool list
# and a Mason packages root, which declared names are not where they should be.
#
# Why the CHECKED-OUT COMMIT and not just a directory. `nvim --headless
# -c 'Lazy! restore' -c 'qa!'` exits 0 whether the restore moved every plugin
# to its pin or none of them, and the acceptance rehearsal caught exactly that:
# 48 plugins sat one commit behind their pins and the apply finished green. A
# directory proves a clone happened, never that the clone is at the commit the
# lock names, so every case below asks the sharper question.
#
# Every input is a PATH the caller supplies, so nothing here reads $HOME, the
# operator's editor, or the machine's real plugin tree. The fixture is a
# throwaway tree of real one-commit-apart git repositories, built once for the
# whole file.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit: both belong to a script that runs on its own, and either one
# here would reach into the runner's own shell. The subject is sourced for the
# same reason and keeps its own `set` line inside its execution guard.
#
# assert_same, never assert_equals: bashunit's assert_equals normalizes away
# ANSI and control characters before comparing (measured on 0.50.1), and a
# plugin name or a sha carrying one is a real defect that must not pass.

subject_under_test() {
  printf '%s' "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/dot_local/libexec/executable_verify-nvim-bootstrap.sh"
}

# shellcheck source=dot_local/libexec/executable_verify-nvim-bootstrap.sh
source "$(subject_under_test)"

# --- fixture ----------------------------------------------------------------
#
# Four plugins in one lock, each a different way a pin can go unmet:
#
#   on-pin      a real repository checked out at the commit the lock names
#   off-pin     the same repository, one commit behind
#   absent      named in the lock, no directory at all
#   not-a-repo  a directory that is not a git checkout
#
# `git` runs through `env -u` here for the same reason the subject does it:
# this suite runs from the pre-commit hook, and git exports GIT_DIR to every
# hook, which would silently point `git init` and `rev-parse` at this
# repository instead of at the fixture.

fixture="$(mktemp -d)"

tear_down_after_script() {
  rm -rf "$fixture"
}

fixture_git() {
  env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE \
    git -c user.email=fixture@example.invalid -c user.name=fixture \
    -c init.defaultBranch=main -c commit.gpgsign=false "$@"
}

make_two_commit_repo() { # <dir>
  mkdir -p "$1"
  fixture_git -C "$1" init -q
  fixture_git -C "$1" commit -q --allow-empty -m first
  fixture_git -C "$1" commit -q --allow-empty -m second
}

mkdir -p "$fixture/lazy"
make_two_commit_repo "$fixture/lazy/on-pin"
make_two_commit_repo "$fixture/lazy/off-pin"
mkdir -p "$fixture/lazy/not-a-repo"

on_pin_head="$(fixture_git -C "$fixture/lazy/on-pin" rev-parse HEAD)"
off_pin_head="$(fixture_git -C "$fixture/lazy/off-pin" rev-parse HEAD)"
off_pin_previous="$(fixture_git -C "$fixture/lazy/off-pin" rev-parse HEAD~1)"
fixture_git -C "$fixture/lazy/off-pin" checkout -q --detach "$off_pin_previous"

absent_commit="0000000000000000000000000000000000000000"
not_a_repo_commit="1111111111111111111111111111111111111111"

jq -n \
  --arg on_pin "$on_pin_head" \
  --arg off_pin "$off_pin_head" \
  --arg absent "$absent_commit" \
  --arg not_a_repo "$not_a_repo_commit" \
  '{"on-pin":{"commit":$on_pin},"off-pin":{"commit":$off_pin},
    "absent":{"commit":$absent},"not-a-repo":{"commit":$not_a_repo}}' \
  >"$fixture/lock.json"

jq -n --arg on_pin "$on_pin_head" '{"on-pin":{"commit":$on_pin}}' >"$fixture/only-on-pin.json"

printf 'not json at all\n' >"$fixture/broken-lock.json"
printf 'gopls\nstylua\n' >"$fixture/tools"
: >"$fixture/empty-tools"

mkdir -p "$fixture/all-packages/gopls" "$fixture/all-packages/stylua"
mkdir -p "$fixture/no-package/gopls"

# --- lazy.nvim --------------------------------------------------------------

function test_a_plugin_checked_out_at_its_pinned_commit_is_not_reported() {
  assert_same "" "$(mispinned_lazy_plugins "$fixture/only-on-pin.json" "$fixture/lazy")"
}

function test_a_plugin_one_commit_behind_its_pin_is_reported_with_both_shas() {
  # The rehearsal defect. The directory is there and the clone is healthy, so
  # every existence check passes it, and only the commit comparison catches it.
  assert_same "off-pin $off_pin_head $off_pin_previous" \
    "$(mispinned_lazy_plugins "$fixture/lock.json" "$fixture/lazy" | grep '^off-pin ')"
}

function test_a_plugin_that_was_never_cloned_is_reported_as_absent() {
  assert_same "absent $absent_commit absent" \
    "$(mispinned_lazy_plugins "$fixture/lock.json" "$fixture/lazy" | grep '^absent ')"
}

function test_a_directory_that_is_not_a_git_checkout_reads_as_unknown_never_as_satisfied() {
  # Fail-safe: a half-cloned plugin cannot answer for its own commit, and
  # silence there would pass exactly the state this gate exists to catch.
  assert_same "not-a-repo $not_a_repo_commit unknown" \
    "$(mispinned_lazy_plugins "$fixture/lock.json" "$fixture/lazy" | grep '^not-a-repo ')"
}

function test_only_the_unmet_pins_are_reported() {
  assert_same 3 "$(mispinned_lazy_plugins "$fixture/lock.json" "$fixture/lazy" | wc -l | tr -d ' ')"
}

function test_a_lock_that_cannot_be_parsed_is_refused_rather_than_read_as_empty() {
  # Fail-closed: through a process substitution jq's status is invisible, and a
  # truncated lock would then yield zero pins and report nothing unmet, which
  # is a gate that passes on a corrupt input.
  mispinned_lazy_plugins "$fixture/broken-lock.json" "$fixture/lazy" >/dev/null 2>&1
  assert_same 2 "$?"
}

# --- mason ------------------------------------------------------------------

function test_a_tool_list_whose_every_name_has_a_package_directory_reports_nothing() {
  assert_same "" "$(missing_mason_packages "$fixture/tools" "$fixture/all-packages")"
}

function test_a_tool_with_no_package_directory_is_the_only_name_reported() {
  assert_same "stylua" "$(missing_mason_packages "$fixture/tools" "$fixture/no-package")"
}

# --- the script's own boundary ----------------------------------------------

function test_a_complete_installation_prints_nothing_and_exits_zero() {
  local output
  output="$(main "$fixture/only-on-pin.json" "$fixture/lazy" "$fixture/tools" "$fixture/all-packages")"
  assert_same 0 "$?"
  assert_same "" "$output"
}

function test_an_unmet_pin_is_printed_with_the_commit_wanted_and_the_commit_found() {
  assert_same "lazy plugin off its pin: off-pin wants $off_pin_head, has $off_pin_previous" \
    "$(main "$fixture/lock.json" "$fixture/lazy" "$fixture/tools" "$fixture/all-packages" | grep ' off-pin ')"
}

function test_every_unmet_name_is_printed_with_the_tool_that_owns_it() {
  local output
  output="$(main "$fixture/only-on-pin.json" "$fixture/lazy" "$fixture/tools" "$fixture/no-package")"
  assert_same "missing mason package: stylua" "$output"
}

function test_anything_unmet_exits_one() {
  main "$fixture/lock.json" "$fixture/lazy" "$fixture/tools" "$fixture/all-packages" >/dev/null
  assert_same 1 "$?"
}

function test_an_empty_tool_list_is_refused_rather_than_passing_vacuously() {
  # An empty list is what a failed extraction leaves behind, not evidence that
  # no tool is required. Accepting it would delete the Mason half of this gate
  # without printing a word.
  main "$fixture/only-on-pin.json" "$fixture/lazy" "$fixture/empty-tools" "$fixture/all-packages" >/dev/null 2>&1
  assert_same 2 "$?"
}

function test_the_wrong_number_of_arguments_is_an_error_not_a_help_screen() {
  main "$fixture/lock.json" >/dev/null 2>&1
  assert_same 2 "$?"
}

function test_newline_only_tool_inventory_is_refused() {
  printf '\n' >"$fixture/newline-tools"
  main "$fixture/only-on-pin.json" "$fixture/lazy" "$fixture/newline-tools" "$fixture/all-packages" >/dev/null 2>&1
  assert_same 2 "$?"
}

function test_whitespace_only_tool_inventory_is_refused() {
  printf ' \t\n' >"$fixture/blank-tools"
  main "$fixture/only-on-pin.json" "$fixture/lazy" "$fixture/blank-tools" "$fixture/all-packages" >/dev/null 2>&1
  assert_same 2 "$?"
}

function test_empty_plugin_inventory_is_refused() {
  printf '{}\n' >"$fixture/empty-lock.json"
  mispinned_lazy_plugins "$fixture/empty-lock.json" "$fixture/lazy" >/dev/null 2>&1
  assert_same 2 "$?"
}

function test_every_deferral_changes_the_retry_marker_size_with_a_fixed_timestamp() {
  local marker="$fixture/retry" first second
  bump_nvim_retry_marker "$marker"
  touch -t 202601010000.00 "$marker"
  first="$(wc -c <"$marker")"
  bump_nvim_retry_marker "$marker"
  touch -t 202601010000.00 "$marker"
  second="$(wc -c <"$marker")"
  assert_same 1 "$((second - first))"
}

function test_later_failure_replays_and_retains_earlier_success_status_diagnostics() {
  local log="$fixture/steps.log" output status
  nvim_bootstrap_step "$log" install printf 'fixture download failed but tool exited zero\n'
  output="$(nvim_bootstrap_step "$log" verify bash -c 'printf "fixture unmet package\n"; exit 1' 2>&1)"
  status="$?"
  assert_same 1 "$status"
  assert_contains 'fixture download failed but tool exited zero' "$output"
  assert_contains 'fixture unmet package' "$output"
  assert_contains 'fixture download failed but tool exited zero' "$(cat "$log")"
}

function test_successful_steps_are_quiet() {
  local output
  output="$(nvim_bootstrap_step "$fixture/quiet.log" install printf 'progress\n' 2>&1)"
  assert_same 0 "$?"
  assert_same '' "$output"
}
