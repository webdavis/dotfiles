#!/usr/bin/env bats
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

setup_file() {
  local repo_root
  repo_root="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)"
  # shellcheck source=../helpers/extract-shell-notifier.sh
  source "$repo_root/test/helpers/extract-shell-notifier.sh"
  extract_shell_notifier "$repo_root" "$BATS_FILE_TMPDIR/notifier.sh"
  export NOTIFIER="$BATS_FILE_TMPDIR/notifier.sh"
}

setup() {
  # A state dir with a space, because the path is interpolated everywhere, and
  # one that does NOT exist yet, because a machine whose first pns write is the
  # shell's own is the case that has to work.
  export PNS_STATE_DIR="$BATS_TEST_TMPDIR/state dir"
  # BATS' OWN EXIT TRAP IS PUT BACK AFTER EVERY SOURCE. The notifier installs
  # `trap __cmd_notify_clear_marker EXIT`, which is the behaviour test 6 exists
  # for, and sourcing it here would replace the trap bats reports failures
  # through: a failing test then ABORTS the file ("Executed 6 instead of
  # expected 11 tests") instead of naming itself, which is a suite that cannot
  # be trusted to go red. The real shell's trap is exercised where it belongs,
  # in the one test that spawns a real shell.
  local bats_exit_trap
  bats_exit_trap="$(trap -p EXIT)"
  # shellcheck source=/dev/null
  source "$NOTIFIER"
  eval "${bats_exit_trap:-trap - EXIT}"
  # THIS SUITE'S OWN SANDBOX CHECK, asked of the code under test rather than of
  # the variable this function just set. The notifier resolves its directory at
  # source time, so an inherited PNS_STATE_DIR, or a resolution that stopped
  # reading it, would point these tests at the operator's real markers and
  # delete one. Asserting the RESOLVED path is what catches that.
  [[ $__cmd_notify_marker_dir == "$PNS_STATE_DIR"/* ]] || {
    echo "the notifier resolved outside this test's sandbox: $__cmd_notify_marker_dir" >&2
    return 1
  }
  MARKER="$PNS_STATE_DIR/lights-shell/$$"
}

teardown() {
  # The unwritable-parent test makes a directory nothing can delete under it.
  [[ -d $BATS_TEST_TMPDIR/locked ]] && chmod 700 "$BATS_TEST_TMPDIR/locked"
  return 0
}

# ends <exit-code> <elapsed>: the prompt coming back after a command that ran
# for that long. Kept under the notifier's 30s tier so nothing is spawned.
ends() {
  __cmd_notify_start=0
  SECONDS="$2"
  # `(exit N)` is what feeds precmd's own `local exit_code=$?`, and bats runs a
  # test body under errexit, so the deliberate non-zero is fenced off here.
  set +e
  (exit "$1")
  __cmd_notify_precmd
  set -e
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
  cat >"$BATS_TEST_TMPDIR/pane-a.sh" <<'PANE'
source "$NOTIFIER"
trap - EXIT
__cmd_notify_preexec 'cargo build --release'
printf '%s\n' "$__cmd_notify_marker"
PANE
  OTHER_PANE="$(bash --noprofile --norc "$BATS_TEST_TMPDIR/pane-a.sh")"
  [[ -f $OTHER_PANE ]] || {
    echo "the second pane published nothing to $OTHER_PANE" >&2
    return 1
  }
}

perms_of() { stat -c '%a' "$1" 2>/dev/null || stat -f '%Lp' "$1"; }

@test "the marker is present while a tracked command runs and gone once it ends" {
  __cmd_notify_preexec 'cargo build --release'
  [[ -f $MARKER ]]
  ends 0 3
  [[ ! -e $MARKER ]]
}

@test "the marker this shell writes is named for this shell's own pid" {
  __cmd_notify_preexec 'cargo build --release'
  [[ -f "$PNS_STATE_DIR/lights-shell/$$" ]]
}

@test "a command that failed still clears the marker" {
  __cmd_notify_preexec 'cargo build --release'
  ends 1 3
  [[ ! -e $MARKER ]]
}

@test "a pane reaching its FIRST prompt leaves another pane's marker alone" {
  # bash-preexec runs precmd at a session's first prompt, before anything has
  # been typed, so opening a herdr pane or a nested bash reaches this path with
  # an empty timer. With one shared marker file that prompt deleted a running
  # build's evidence next door; the removal now sits below the timer guard and
  # names only this shell's own pid.
  another_pane_publishes
  __cmd_notify_start=""
  __cmd_notify_precmd
  [[ -f $OTHER_PANE ]]
}

@test "a short command in one pane leaves another pane's marker alone" {
  another_pane_publishes
  __cmd_notify_preexec 'ls'
  ends 0 1
  [[ ! -e $MARKER ]]
  [[ -f $OTHER_PANE ]]
}

@test "a shell that exits without another prompt leaves no marker of its own" {
  # `exit`, a closed tab and a killed terminal all end the shell with no further
  # prompt, so precmd never runs and the last command's marker would outlive the
  # shell that wrote it. This one needs a REAL shell exiting, so it is the one
  # test here that spawns.
  cat >"$BATS_TEST_TMPDIR/pane.sh" <<'PANE'
source "$NOTIFIER"
__cmd_notify_preexec 'cargo build --release'
[[ -f $__cmd_notify_marker ]] || exit 3
printf '%s\n' "$$"
PANE
  run bash --noprofile --norc "$BATS_TEST_TMPDIR/pane.sh"
  [[ $status -eq 0 ]]
  [[ -n $output ]]
  [[ ! -e "$PNS_STATE_DIR/lights-shell/$output" ]]
}

@test "every interactive TUI on the skip list publishes no marker" {
  # THE WHOLE LIST, because one list feeds both halves now: shrinking it to the
  # one entry a test names would leave the other eleven publishing false build
  # markers, and a lamp breathing all afternoon over an editor left open.
  local tui
  for tui in vim nvim less man top btop ssh herdr claude hermes codex fzf; do
    __cmd_notify_preexec "$tui src/main.rs"
    [[ ! -e $MARKER ]] || {
      echo "$tui with an argument published a marker" >&2
      return 1
    }
    __cmd_notify_preexec "$tui"
    [[ ! -e $MARKER ]] || {
      echo "bare $tui published a marker" >&2
      return 1
    }
  done
}

@test "a build whose name merely starts with a TUI's is still a build" {
  # The skip list is anchored at the start of the line, so without a trailing
  # word boundary `topaz`, `manage`, `lessc`, `sshuttle` and `codexify` all read
  # as editors. That used to cost one banner; it now also costs the lamp a long
  # build's whole run.
  local build
  for build in 'topaz build' 'manage.py migrate' 'lessc styles.less' 'sshuttle -r host 10.0.0.0/8' 'codexify run'; do
    __cmd_notify_clear_marker
    __cmd_notify_preexec "$build"
    [[ -f $MARKER ]] || {
      echo "$build published no marker; the skip list swallowed it" >&2
      return 1
    }
  done
}

@test "preexec succeeds whether or not it published, because extdebug cancels a command on a failed one" {
  run __cmd_notify_preexec 'cargo build --release'
  [[ $status -eq 0 ]]
  run __cmd_notify_preexec 'nvim src/main.rs'
  [[ $status -eq 0 ]]
}

@test "a state directory that cannot be created costs a marker, never the command" {
  # THE ONE SCENARIO `return 0` EXISTS FOR. Under `shopt -s extdebug` a preexec
  # function returning non-zero cancels the command the operator just typed, so
  # a state parent this user cannot write must cost the lamp one reading and
  # nothing else. Teardown restores the mode.
  mkdir -p "$BATS_TEST_TMPDIR/locked"
  chmod 500 "$BATS_TEST_TMPDIR/locked"
  export PNS_STATE_DIR="$BATS_TEST_TMPDIR/locked/state"
  local bats_exit_trap
  bats_exit_trap="$(trap -p EXIT)"
  # shellcheck source=/dev/null
  source "$NOTIFIER"
  eval "${bats_exit_trap:-trap - EXIT}"
  run __cmd_notify_preexec 'cargo build --release'
  [[ $status -eq 0 ]]
  [[ ! -e "$PNS_STATE_DIR/lights-shell/$$" ]]
}

@test "the marker is one epoch line, in a directory, readable by nobody else" {
  local started="$EPOCHSECONDS"
  __cmd_notify_preexec 'cargo build --release'
  [[ $(wc -l <"$MARKER") -eq 1 ]]
  local published
  published="$(cat "$MARKER")"
  [[ $published =~ ^[1-9][0-9]{9}$ ]]
  # The second the command STARTED, not some other clock: the tick subtracts
  # this from its own now to get the run's length.
  ((published >= started && published <= started + 5))
  [[ $(perms_of "$MARKER") == 600 ]]
  # The directory too: its NAMES are the pids of this operator's shells, which
  # is nobody else's business either.
  [[ $(perms_of "$PNS_STATE_DIR/lights-shell") == 700 ]]
}
