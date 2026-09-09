#!/usr/bin/env bash
# The shell captures time and status; pns owns the marker, skip list and tiers.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
scratch="$(mktemp -d)"
# Retain this private fixture on failure as well as success.
extract_shell_notifier() {
  local repo_root="$1" destination="$2" rendered="$2.bashrc"
  if ! CI=1 chezmoi --source "$repo_root" execute-template --no-tty \
    <"$repo_root/dot_bashrc.tmpl" >"$rendered" 2>/dev/null; then
    printf 'extract_shell_notifier: rendering %s/dot_bashrc.tmpl failed\n' "$repo_root" >&2
    return 1
  fi
  sed -n '/^  __cmd_notify_start=""$/,/^  precmd_functions+=(__cmd_notify_precmd)$/p' \
    "$rendered" | sed 's/^  //' >"$destination"
  # THE END ANCHOR IS ASSERTED, NOT TRUSTED. sed prints to the end of the file
  # when a range's closing address never matches, and that result is still
  # non-empty, so an emptiness guard passes on an extraction that swallowed the
  # rest of the bashrc. What fails then is whatever sourcing the runaway
  # produces, which on today's file is a bare `syntax error near unexpected
  # token fi` naming nothing. The anchor is the range's own last line, so its
  # absence is exactly the runaway (and an unmatched opening address leaves an
  # empty file, which this catches too).
  if ! grep -qxF 'precmd_functions+=(__cmd_notify_precmd)' "$destination"; then
    printf 'extract_shell_notifier: the notifier region was not found in %s; the sed range anchors have moved\n' "$rendered" >&2
    return 1
  fi
}

extract_shell_notifier "$REPO_ROOT" "$scratch/notifier.sh"
mkdir -p "$scratch/home/.cargo/bin"
cat >"$scratch/home/.cargo/bin/pns" <<'ENGINE'
#!/usr/bin/env bash
printf '%s\n' "$@" >"$CALLS_FILE"
printf 'suppressed stdout\n'
printf 'suppressed stderr\n' >&2
exit "${ENGINE_STATUS:-0}"
ENGINE
chmod 700 "$scratch/home/.cargo/bin/pns"
export NOTIFIER="$scratch/notifier.sh" CALLS_FILE="$scratch/calls"
export HOME="$scratch/home" PNS_STATE_DIR="$scratch/state" HERDR_PANE_ID=t1:p2

fail() {
  printf '%s\n' "$1" >&2
  exit 1
}

# Named successors of the old marker tests that actually belong to Bash.
test_a_pane_reaching_its_first_prompt_leaves_another_panes_marker_alone() {
  bash --noprofile --norc -c '
    source "$NOTIFIER"
    trap - EXIT
    __cmd_notify_precmd
  '
  [[ ! -e $CALLS_FILE ]] || fail 'a first prompt must not call end'
}

test_preexec_succeeds_whether_or_not_it_published_because_extdebug_cancels_a_command_on_a_failed_one() {
  ENGINE_STATUS=19 bash --noprofile --norc -c '
    source "$NOTIFIER"
    trap - EXIT
    shopt -s extdebug
    __cmd_notify_preexec "cargo build --secret value"
    result=$?
    [[ $result == 0 ]] || exit 3
    printf "%s\n" "$$" >"$CALLS_FILE.pid"
  ' >"$scratch/preexec.stdout" 2>"$scratch/preexec.stderr"
  [[ ! -s $scratch/preexec.stdout && ! -s $scratch/preexec.stderr ]] || fail 'preexec must stay silent'
  diff -u <(printf '%s\n' shell begin --pid "$(cat "$CALLS_FILE.pid")" --command 'cargo build --secret value') "$CALLS_FILE"
}

test_a_shell_that_exits_without_another_prompt_leaves_no_marker_of_its_own() {
  local status=0
  bash --noprofile --norc -c '
    source "$NOTIFIER"
    printf "%s\n" "$$" >"$CALLS_FILE.pid"
    exit 7
  ' || status=$?
  [[ $status == 7 ]] || fail 'EXIT cleanup changed the shell status'
  diff -u <(printf '%s\n' shell end --pid "$(cat "$CALLS_FILE.pid")" --command '' --exit 0 --elapsed 0) "$CALLS_FILE"
}

# PS0 captures before preexec, including the first command. End receives the
# captured status/time verbatim, even when engine startup itself refuses.
test_prompt_captures_status_time_and_history_before_the_engine() {
  ENGINE_STATUS=19 bash --noprofile --norc -c '
    source "$NOTIFIER"
    trap - EXIT
    fc() { printf "  cargo build --private arg\n"; }
    SECONDS=10
    eval "printf %s \"$PS0\"" >"$CALLS_FILE.ps0"
    [[ $__cmd_notify_start == 10 ]] || exit 3
    SECONDS=39
    (exit 17)
    __cmd_notify_precmd
    [[ -z $__cmd_notify_start ]] || exit 4
    printf "%s\n" "$$" >"$CALLS_FILE.pid"
  ' >"$scratch/precmd.stdout" 2>"$scratch/precmd.stderr"
  [[ ! -s $scratch/precmd.stdout && ! -s $scratch/precmd.stderr && ! -s $CALLS_FILE.ps0 ]] || fail 'prompt callback leaked output'
  diff -u <(printf '%s\n' shell end --pid "$(cat "$CALLS_FILE.pid")" --command 'cargo build --private arg' --exit 17 --elapsed 29) "$CALLS_FILE"
}

test_a_pane_reaching_its_first_prompt_leaves_another_panes_marker_alone
test_preexec_succeeds_whether_or_not_it_published_because_extdebug_cancels_a_command_on_a_failed_one
test_a_shell_that_exits_without_another_prompt_leaves_no_marker_of_its_own
test_prompt_captures_status_time_and_history_before_the_engine
