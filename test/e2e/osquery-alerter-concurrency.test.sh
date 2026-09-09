#!/usr/bin/env bash
# Concurrency: WatchPaths can fire a second alerter invocation while one is still
# running. Both would read the same cursor+snapshot, both send_alert (two local
# banners, since the local notification fires before any occurrence-dedup), and
# both race STATE.tmp. A nonblocking single-instance kernel lock held across the
# whole run (read -> route -> send_alert -> checkpoint) makes exactly one run
# deliver the batch; a contended run is a clean no-op (exit 0). The lock fd must
# not leak to a forked child, or a backgrounded grandchild would wedge it.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit: both belong to a script that runs on its own, and either one
# here would reach into the runner's own shell. The shebang stays for shellcheck
# and for editors, and is never executed. test/validate-tests.sh pins that shape;
# `just test-e2e` runs it.
#
# Every check below is a real bashunit assertion. bashunit runs each test function
# under `set +euo pipefail`, so a bare `kill -0 ...` or a `[ ... ]` left dangling
# at the end of a test reports nothing and passes silently, which is exactly what
# the bats file's bracket checks would have become. Statuses are captured into a
# variable and asserted, never left to the shell.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ENTRY="$REPO_ROOT/dot_local/libexec/osquery/executable_results-alerter.sh"
HELPER_SRC="$REPO_ROOT/dot_local/libexec/osquery/results-alerter"

# One results row that routes to a CRIT page, so a delivery either happens or
# demonstrably does not.
ADMIN_ROW='{"name":"new_admin_user","action":"added","columns":{"username":"eve","uid":"501"}}'

function set_up() {
  HOME_DIR="$(mktemp -d)"
  # Record ownership only after our own mktemp, so tear_down removes this path
  # and never a pre-set or inherited HOME_DIR.
  _CONCURRENCY_OWNED_DIR="$HOME_DIR"
  export HOME="$HOME_DIR"
  mkdir -p "$HOME/.local/libexec/osquery/results-alerter" "$HOME/.local/state" "$HOME/.local/log/osquery"
  cp "$HELPER_SRC"/*.sh "$HOME/.local/libexec/osquery/results-alerter/"

  export SEND_ALERT_SPY="$HOME/send_alert.log"
  : >"$SEND_ALERT_SPY"
  export OSQUERY_RESULTS_LOG="$HOME/.local/log/osquery/osqueryd.results.log"
  export OSQUERY_RESULTS_OFFSET="$HOME/.local/state/osquery-results-offset"
  : >"$OSQUERY_RESULTS_LOG"
}

function tear_down() {
  [[ -n ${_CONCURRENCY_OWNED_DIR:-} ]] || return 0
  rm -rf "$_CONCURRENCY_OWNED_DIR"
  unset _CONCURRENCY_OWNED_DIR
}

# The cursor's inode field, read the way the entry itself reads it. GNU and BSD
# stat spell the inode under different flags, so `ls -i` is the one call that
# runs on both; the entry carries the same disable, for the same reason, on the
# same call.
# shellcheck disable=SC2012  # a fixed mktemp path this file created; ls -i is safe and portable
log_inode() { ls -i "$OSQUERY_RESULTS_LOG" | awk '{print $1}'; }
log_size() { wc -c <"$OSQUERY_RESULTS_LOG" | tr -d '[:space:]'; }
seed_cursor() { printf '%s 0\n' "$(log_inode)" >"$OSQUERY_RESULTS_OFFSET"; }
cursor_offset() { awk '{print $2}' "$OSQUERY_RESULTS_OFFSET"; }
call_count() { grep -c '^CALL' "$SEND_ALERT_SPY" 2>/dev/null || true; }

# Hold the first delivery until the other entry has returned. A timeout or an
# early first exit fails the case; elapsed time alone never establishes overlap.
write_held_dispatch() {
  cat >"$HOME/.local/libexec/osquery/alert-dispatch.sh" <<'STUB'
# shellcheck shell=bash
send_alert() {
  local release
  printf 'CALL\t%s\n' "$2" >>"$SEND_ALERT_SPY"
  printf 'ready\n' >&"$SEND_ALERT_READY_FD"
  IFS= read -r release <&"$SEND_ALERT_RELEASE_FD"
  [[ $release == release ]]
}
STUB
}

function test_two_parallel_runs_deliver_a_batch_exactly_once() {
  bashunit::skip_unless '[[ -x /usr/bin/lockf ]]' \
    "no /usr/bin/lockf; the single-instance lock is a darwin-only guarantee"
  write_held_dispatch
  seed_cursor
  printf '%s\n' "$ADMIN_ROW" >>"$OSQUERY_RESULTS_LOG"

  local ready_fd release_fd done_fd
  mkfifo "$HOME/ready" "$HOME/release" "$HOME/done"
  exec {ready_fd}<>"$HOME/ready"
  exec {release_fd}<>"$HOME/release"
  exec {done_fd}<>"$HOME/done"
  export SEND_ALERT_READY_FD="$ready_fd" SEND_ALERT_RELEASE_FD="$release_fd"

  # Every bound below is a HANG CEILING, not a measurement. The stub blocks in
  # `read` until this test releases it, so a healthy run reaches each waypoint as
  # fast as the subject can get there and none of these numbers is ever spent. A
  # green run is no slower for them being generous. They were eight times tighter
  # and CI failed on the first one: a shared runner under load did not start bash
  # and reach send_alert inside 250 ms, `read` returned 142 (SIGALRM), and the run
  # was scored as a broken lock. Only a subject that really never signals waits
  # the full ceiling, and it still fails.
  timeout --kill-after=0.5 5 bash "$ENTRY" &
  local p1=$! p2="" ready="" second_status="" cursor_before=""
  local ready_status=0 done_status=1 first_alive=1 s1=0 s2=0
  IFS= read -r -t 2 -u "$ready_fd" ready || ready_status=$?
  if [[ $ready_status == 0 && $ready == ready ]]; then
    # shellcheck disable=SC2016 # Positional arguments expand in the child shell.
    timeout --kill-after=0.5 3 bash -c '
      bash "$1"
      printf "%s\n" "$?" >&"$2"
    ' _ "$ENTRY" "$done_fd" &
    p2=$!
    IFS= read -r -t 2.5 -u "$done_fd" second_status && done_status=0
    first_alive=0
    kill -0 "$p1" 2>/dev/null || first_alive=$?
    cursor_before="$(cursor_offset)"
  fi

  # Release both possible readers, even with locking broken, before assertions.
  printf 'release\nrelease\n' >&"$release_fd"
  wait "$p1" || s1=$?
  if [[ -n $p2 ]]; then wait "$p2" || s2=$?; fi
  exec {ready_fd}>&-
  exec {release_fd}>&-
  exec {done_fd}>&-
  unset SEND_ALERT_READY_FD SEND_ALERT_RELEASE_FD

  assert_exit_code 0 "" "$ready_status"
  assert_same ready "$ready"
  assert_exit_code 0 "" "$done_status"
  assert_same 0 "$second_status"
  assert_exit_code 0 "" "$first_alive"
  assert_same 0 "$cursor_before"
  assert_exit_code 0 "" "$s1"
  assert_exit_code 0 "" "$s2"
  assert_same 1 "$(call_count)"
  assert_same "$(log_size)" "$(cursor_offset)"
}

# The lock fd must not leak to a child: a send_alert that spawns a long-lived,
# DETACHED grandchild must NOT keep the lock held after the run exits, or the next
# run would be locked out and drop its batch. Exactly the latent bug from the
# allowlist writer. The grandchild is nohup+disown'd (writing its pid) so it
# genuinely survives the run's exit; the test then asserts the lock is FREE by
# acquiring it directly, which fails only if the grandchild inherited the lock fd.
function test_a_detached_child_never_wedges_the_lock_because_the_lock_fd_is_not_leaked() {
  bashunit::skip_unless '[[ -x /usr/bin/lockf ]]' \
    "no /usr/bin/lockf; the single-instance lock is a darwin-only guarantee"
  local child_pid_file="$HOME/child.pid"
  cat >"$HOME/.local/libexec/osquery/alert-dispatch.sh" <<STUB
# shellcheck shell=bash
send_alert() {
  printf 'CALL\t%s\n' "\$2" >>"$SEND_ALERT_SPY"
  # A detached, long-lived grandchild that survives this run's exit. If it inherited
  # the lock fd (fd 9), it keeps the kernel lock held after the run is gone.
  nohup sleep 30 </dev/null >/dev/null 2>&1 &
  printf '%s\n' "\$!" >"$child_pid_file"
  disown
  return 0
}
STUB
  seed_cursor
  printf '%s\n' "$ADMIN_ROW" >>"$OSQUERY_RESULTS_LOG"

  # Delivers, spawns the detached grandchild, exits.
  local entry_status=0 entry_output
  entry_output="$(bash "$ENTRY" 2>&1)" || entry_status=$?
  [[ $entry_status -eq 0 ]] || printf 'entry exited %s:\n%s\n' "$entry_status" "$entry_output"
  assert_exit_code 0 "" "$entry_status"
  assert_same 1 "$(call_count)"

  local child_pid
  child_pid="$(cat "$child_pid_file")"
  # The grandchild is genuinely still alive, so the lock question is a real one.
  local child_alive=0
  kill -0 "$child_pid" 2>/dev/null || child_alive=$?
  assert_exit_code 0 "" "$child_alive"

  # The lock MUST be free now: run 1 released it on exit, and the grandchild did not
  # inherit fd 9. Acquire it directly; this fails only if the fd leaked.
  local lock="$OSQUERY_RESULTS_OFFSET.lock" acquired=no
  if exec 8>>"$lock" && /usr/bin/lockf -s -t 0 8; then acquired=yes; fi
  exec 8>&-
  kill "$child_pid" 2>/dev/null || true
  # Lock free -> the lock fd never leaked to the child.
  assert_same yes "$acquired"
}
