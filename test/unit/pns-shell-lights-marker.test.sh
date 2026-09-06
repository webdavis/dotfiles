#!/usr/bin/env bash
# The shell's half of the loop lamp: the markers `pns lights tick` reads to know
# a plain command is running, beside the agent statuses herdr answers.
#
# ONE FILE PER INTERACTIVE SHELL, named for that shell's pid. Every pane runs
# these same two functions, so most of what follows is about what one pane must
# NOT do to another pane's marker.
#
# The subject is the interactive notifier's bash-preexec functions, taken from
# the RENDERED bashrc rather than a copy, so a repoint that edits one and not
# the other fails here. Nothing spawns for the marker: it is a redirect, which
# is the point of it, and every test but the EXIT-trap one drives the functions
# in this process.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit; test/validate-tests.sh pins that shape. A test body runs
# WITHOUT errexit, which is why the deliberate non-zero below needs no `set +e`
# fence and why every check is a real assertion.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

set_up_before_script() {
  FILE_FIXTURE="$(mktemp -d)"
  # shellcheck source=../helpers/extract-shell-notifier.sh
  source "$REPO_ROOT/test/helpers/extract-shell-notifier.sh"
  extract_shell_notifier "$REPO_ROOT" "$FILE_FIXTURE/notifier.sh"
  # Exported because two tests spawn a REAL second shell that sources it.
  export NOTIFIER="$FILE_FIXTURE/notifier.sh"
}

tear_down_after_script() { discard_fixture "$FILE_FIXTURE"; }

set_up() {
  TEST_FIXTURE="$(mktemp -d)"
  # A state dir with a space, because the path is interpolated everywhere, and
  # one that does NOT exist yet, because a machine whose first pns write is the
  # shell's own is the case that has to work.
  export PNS_STATE_DIR="$TEST_FIXTURE/state dir"
  source_notifier
  MARKER="$PNS_STATE_DIR/lights-shell/$$"
}

tear_down() {
  # The unwritable-parent test makes a directory nothing can delete under it.
  [[ -d $TEST_FIXTURE/locked ]] && chmod 700 "$TEST_FIXTURE/locked"
  discard_fixture "$TEST_FIXTURE"
  return 0
}

# discard_fixture <path>: remove one mktemp -d this file created, and nothing
# else. Plain rm -rf, the convention every other test in this repo uses; the
# suite also runs on a CI host with no Trash.
discard_fixture() {
  [[ -n ${1:-} && -d $1 ]] || return 0
  rm -rf "$1"
}

# source_notifier: source the extracted region and PUT BASHUNIT'S OWN EXIT TRAP
# BACK. The notifier installs `trap __cmd_notify_clear_marker EXIT`, which is
# the behaviour one test below exists for, and sourcing it replaces the trap
# bashunit reports a test's assertion counts through: a test would then be
# recorded as a runtime error naming nothing instead of naming itself. The real
# shell's trap is exercised where it belongs, in the one test that spawns.
#
# It also asks the CODE UNDER TEST where it resolved to, rather than trusting
# the variable set_up just exported: the notifier resolves its directory at
# source time, so an inherited PNS_STATE_DIR, or a resolution that stopped
# reading it, would point these tests at the operator's real markers and delete
# one. Asserting the RESOLVED path is what catches that.
source_notifier() {
  local bashunit_exit_trap
  bashunit_exit_trap="$(trap -p EXIT)"
  # shellcheck source=/dev/null
  source "$NOTIFIER"
  eval "${bashunit_exit_trap:-trap - EXIT}"
  # shellcheck disable=SC2154 # the notifier just sourced above assigns it, and
  # reading it from there rather than recomputing it is the whole check.
  if [[ $__cmd_notify_marker_dir != "$PNS_STATE_DIR"/* ]]; then
    printf 'the notifier resolved outside this test sandbox: %s\n' "$__cmd_notify_marker_dir" >&2
    return 1
  fi
}

# ends <exit-code> <elapsed>: the prompt coming back after a command that ran
# for that long. Kept under the notifier's 30s tier so nothing is spawned.
ends() {
  __cmd_notify_start=0
  SECONDS="$2"
  # `(exit N)` is what feeds precmd's own `local exit_code=$?`. No errexit fence
  # is needed here: a bashunit test body runs with `set +euo pipefail`.
  (exit "$1")
  __cmd_notify_precmd
}

# another_pane_publishes: a REAL second shell mid-build, its marker path echoed.
#
# A REAL SHELL AND NOT A PLANTED PATH, which is what makes the two tests below
# catch a marker every pane shares. A file this suite named itself would sit at
# a path no implementation writes to, so one shared file for the whole machine
# would leave both of them green while the bug they exist for was back.
#
# Dropping the EXIT trap is how a short script stands in for a shell that has
# not reached its next prompt: the build is still running, so the marker stays.
another_pane_publishes() {
  cat >"$TEST_FIXTURE/pane-a.sh" <<'PANE'
source "$NOTIFIER"
trap - EXIT
__cmd_notify_preexec 'cargo build --release'
printf '%s\n' "$__cmd_notify_marker"
PANE
  OTHER_PANE="$(bash --noprofile --norc "$TEST_FIXTURE/pane-a.sh")"
  assert_file_exists "$OTHER_PANE"
}

# path_exists <path>: a predicate for assert_false, so a negative check fails the
# test on its own rather than relying on an errexit a bashunit body does not set.
path_exists() { [[ -e $1 ]]; }

function test_the_marker_is_present_while_a_tracked_command_runs_and_gone_once_it_ends() {
  __cmd_notify_preexec 'cargo build --release'
  assert_file_exists "$MARKER"
  ends 0 3
  assert_false path_exists "$MARKER"
}

function test_the_marker_this_shell_writes_is_named_for_this_shells_own_pid() {
  __cmd_notify_preexec 'cargo build --release'
  assert_file_exists "$PNS_STATE_DIR/lights-shell/$$"
}

function test_a_command_that_failed_still_clears_the_marker() {
  __cmd_notify_preexec 'cargo build --release'
  ends 1 3
  assert_false path_exists "$MARKER"
}

function test_a_pane_reaching_its_first_prompt_leaves_another_panes_marker_alone() {
  # bash-preexec runs precmd at a session's first prompt, before anything has
  # been typed, so opening a herdr pane or a nested bash reaches this path with
  # an empty timer. With one shared marker file that prompt deleted a running
  # build's evidence next door; the removal now sits below the timer guard and
  # names only this shell's own pid.
  another_pane_publishes
  __cmd_notify_start=""
  __cmd_notify_precmd
  assert_file_exists "$OTHER_PANE"
}

function test_a_short_command_in_one_pane_leaves_another_panes_marker_alone() {
  another_pane_publishes
  __cmd_notify_preexec 'ls'
  ends 0 1
  assert_false path_exists "$MARKER"
  assert_file_exists "$OTHER_PANE"
}

function test_a_shell_that_exits_without_another_prompt_leaves_no_marker_of_its_own() {
  # `exit`, a closed tab and a killed terminal all end the shell with no further
  # prompt, so precmd never runs and the last command's marker would outlive the
  # shell that wrote it. This one needs a REAL shell exiting, so it is the one
  # test here that spawns.
  local pane_pid
  cat >"$TEST_FIXTURE/pane.sh" <<'PANE'
source "$NOTIFIER"
__cmd_notify_preexec 'cargo build --release'
[[ -f $__cmd_notify_marker ]] || exit 3
printf '%s\n' "$$"
PANE
  pane_pid="$(bash --noprofile --norc "$TEST_FIXTURE/pane.sh" 2>&1)"
  assert_exit_code 0
  assert_not_empty "$pane_pid"
  assert_false path_exists "$PNS_STATE_DIR/lights-shell/$pane_pid"
}

function test_every_interactive_tui_on_the_skip_list_publishes_no_marker() {
  # THE WHOLE LIST, because one list feeds both halves now: shrinking it to the
  # one entry a test names would leave the other eleven publishing false build
  # markers, and a lamp breathing all afternoon over an editor left open. The
  # offenders are collected and named in one assertion rather than stopping at
  # the first, so a red run says which entries of the list broke.
  local tui offenders=""
  for tui in vim nvim less man top btop ssh herdr claude hermes codex fzf; do
    __cmd_notify_preexec "$tui src/main.rs"
    [[ ! -e $MARKER ]] || offenders+="$tui with an argument; "
    __cmd_notify_preexec "$tui"
    [[ ! -e $MARKER ]] || offenders+="bare $tui; "
  done
  assert_empty "$offenders"
}

function test_a_build_whose_name_merely_starts_with_a_tuis_is_still_a_build() {
  # The skip list is anchored at the start of the line, so without a trailing
  # word boundary `topaz`, `manage`, `lessc`, `sshuttle` and `codexify` all read
  # as editors. That used to cost one banner; it now also costs the lamp a long
  # build's whole run.
  local build swallowed=""
  for build in 'topaz build' 'manage.py migrate' 'lessc styles.less' 'sshuttle -r host 10.0.0.0/8' 'codexify run'; do
    __cmd_notify_clear_marker
    __cmd_notify_preexec "$build"
    [[ -f $MARKER ]] || swallowed+="$build; "
  done
  assert_empty "$swallowed"
}

function test_preexec_succeeds_whether_or_not_it_published_because_extdebug_cancels_a_command_on_a_failed_one() {
  __cmd_notify_preexec 'cargo build --release'
  assert_exit_code 0
  __cmd_notify_preexec 'nvim src/main.rs'
  assert_exit_code 0
}

function test_a_state_directory_that_cannot_be_created_costs_a_marker_never_the_command() {
  # THE ONE SCENARIO `return 0` EXISTS FOR. Under `shopt -s extdebug` a preexec
  # function returning non-zero cancels the command the operator just typed, so
  # a state parent this user cannot write must cost the lamp one reading and
  # nothing else. Teardown restores the mode.
  mkdir -p "$TEST_FIXTURE/locked"
  chmod 500 "$TEST_FIXTURE/locked"
  export PNS_STATE_DIR="$TEST_FIXTURE/locked/state"
  source_notifier
  __cmd_notify_preexec 'cargo build --release'
  assert_exit_code 0
  assert_false path_exists "$PNS_STATE_DIR/lights-shell/$$"
}

function test_the_marker_is_one_epoch_line_in_a_directory_readable_by_nobody_else() {
  local started="$EPOCHSECONDS" published
  __cmd_notify_preexec 'cargo build --release'
  assert_same 1 "$(($(wc -l <"$MARKER")))"
  published="$(cat "$MARKER")"
  assert_matches '^[1-9][0-9]{9}$' "$published"
  # The second the command STARTED, not some other clock: the tick subtracts
  # this from its own now to get the run's length.
  assert_between "$started" "$((started + 5))" "$published"
  assert_file_permissions 600 "$MARKER"
  # The directory too: its NAMES are the pids of this operator's shells, which
  # is nobody else's business either.
  assert_file_permissions 700 "$PNS_STATE_DIR/lights-shell"
}
