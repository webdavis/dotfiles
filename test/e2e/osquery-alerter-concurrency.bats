#!/usr/bin/env bats
# Concurrency: WatchPaths can fire a second alerter invocation while one is still
# running. Both would read the same cursor+snapshot, both send_alert (two local
# banners, since the local notification fires before any occurrence-dedup), and
# both race STATE.tmp. A nonblocking single-instance kernel lock held across the
# whole run (read -> route -> send_alert -> checkpoint) makes exactly one run
# deliver the batch; a contended run is a clean no-op (exit 0). The lock fd must
# not leak to a forked child, or a backgrounded grandchild would wedge it.

setup() {
  REPO="$BATS_TEST_DIRNAME/../.."
  ENTRY="$REPO/dot_local/libexec/osquery/executable_results-alerter.sh"
  HELPER_SRC="$REPO/dot_local/libexec/osquery/results-alerter"

  HOME_DIR="$(mktemp -d)"
  export HOME="$HOME_DIR"
  mkdir -p "$HOME/.local/libexec/osquery/results-alerter" "$HOME/.local/state" "$HOME/.local/log/osquery"
  cp "$HELPER_SRC"/*.sh "$HOME/.local/libexec/osquery/results-alerter/"

  export SEND_ALERT_SPY="$HOME/send_alert.log"
  : >"$SEND_ALERT_SPY"
  export OSQUERY_RESULTS_LOG="$HOME/.local/log/osquery/osqueryd.results.log"
  export OSQUERY_RESULTS_OFFSET="$HOME/.local/state/osquery-results-offset"
  : >"$OSQUERY_RESULTS_LOG"
  ADMIN_ROW='{"name":"new_admin_user","action":"added","columns":{"username":"eve","uid":"501"}}'
}
teardown() { rm -rf "$HOME_DIR"; }

log_inode() { ls -i "$OSQUERY_RESULTS_LOG" | awk '{print $1}'; }
log_size() { wc -c <"$OSQUERY_RESULTS_LOG" | tr -d '[:space:]'; }
seed_cursor() { printf '%s 0\n' "$(log_inode)" >"$OSQUERY_RESULTS_OFFSET"; }
cursor_offset() { awk '{print $2}' "$OSQUERY_RESULTS_OFFSET"; }
call_count() { grep -c '^CALL' "$SEND_ALERT_SPY" 2>/dev/null || true; }

# A send_alert stub that records the call and holds the delivery window open long
# enough that a second concurrent run overlaps a first one still delivering.
write_slow_dispatch() {
  cat >"$HOME/.local/libexec/osquery/alert-dispatch.sh" <<'STUB'
# shellcheck shell=bash
send_alert() {
  printf 'CALL\t%s\n' "$2" >>"$SEND_ALERT_SPY"
  sleep "${SEND_ALERT_DELAY:-1}"
  return 0
}
STUB
}

@test "T-CONCURRENT-one-notification: two parallel runs deliver a batch exactly once" {
  [[ -x /usr/bin/lockf ]] || skip "no /usr/bin/lockf; the single-instance lock is a darwin-only guarantee"
  write_slow_dispatch
  seed_cursor
  printf '%s\n' "$ADMIN_ROW" >>"$OSQUERY_RESULTS_LOG"

  SEND_ALERT_DELAY=1 bash "$ENTRY" &
  local p1=$!
  SEND_ALERT_DELAY=1 bash "$ENTRY" &
  local p2=$!
  local s1=0 s2=0
  wait "$p1" || s1=$?
  wait "$p2" || s2=$?

  [ "$s1" -eq 0 ]                    # both runs exit 0 (the loser is a clean no-op)
  [ "$s2" -eq 0 ]
  [ "$(call_count)" -eq 1 ]          # exactly ONE send_alert / ONE banner, no double-send
  [ "$(cursor_offset)" -eq "$(log_size)" ]  # cursor advanced once
}

# The lock fd must not leak to a child: a send_alert that spawns a long-lived,
# DETACHED grandchild must NOT keep the lock held after the run exits, or the next
# run would be locked out and drop its batch. Exactly the latent bug from the
# allowlist writer. The grandchild is nohup+disown'd (writing its pid) so it
# genuinely survives the run's exit; the test then asserts the lock is FREE by
# acquiring it directly, which fails only if the grandchild inherited the lock fd.
@test "T-CONCURRENT-fd-hygiene: a detached child never wedges the lock (fd not leaked)" {
  [[ -x /usr/bin/lockf ]] || skip "no /usr/bin/lockf; the single-instance lock is a darwin-only guarantee"
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
  run bash "$ENTRY"                  # delivers, spawns the detached grandchild, exits
  [ "$status" -eq 0 ]
  [ "$(call_count)" -eq 1 ]
  local child_pid
  child_pid="$(cat "$child_pid_file")"
  kill -0 "$child_pid"               # the grandchild is genuinely still alive
  # The lock MUST be free now: run 1 released it on exit, and the grandchild did not
  # inherit fd 9. Acquire it directly; this fails only if the fd leaked.
  local lock="$OSQUERY_RESULTS_OFFSET.lock" acquired=no
  if exec 8>>"$lock" && /usr/bin/lockf -s -t 0 8; then acquired=yes; fi
  exec 8>&-
  kill "$child_pid" 2>/dev/null || true
  [ "$acquired" = "yes" ]            # lock free -> the lock fd never leaked to the child
}
