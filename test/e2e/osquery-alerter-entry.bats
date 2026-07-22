#!/usr/bin/env bats
# The osquery alerter ENTRY (executable_results-alerter.sh) end-to-end: it reads
# the new results-log rows since its cursor, runs them through the decomposed
# pipeline (normalize -> route -> render), delivers a CRIT page via send_alert,
# and advances its cursor ONLY after the batch is durably delivered. This is a
# whole-script flow, so it runs the real entry as a subprocess under a temp HOME
# with the real pipeline helpers and a recording send_alert spy (its exit code
# scriptable, so the checkpoint-after-durable ordering can be exercised).

setup() {
  REPO="$BATS_TEST_DIRNAME/../.."
  ENTRY="$REPO/dot_local/libexec/osquery/executable_results-alerter.sh"
  HELPER_SRC="$REPO/dot_local/libexec/osquery/results-alerter"

  HOME_DIR="$(mktemp -d)"
  export HOME="$HOME_DIR"
  mkdir -p "$HOME/.local/libexec/osquery/results-alerter" \
    "$HOME/.local/state" "$HOME/.local/log/osquery"

  # Mirror a chezmoi apply: the real pipeline helpers at their deployed path.
  cp "$HELPER_SRC"/*.sh "$HOME/.local/libexec/osquery/results-alerter/"

  # Stub dispatch library: a recording spy send_alert whose exit code is
  # scriptable via SEND_ALERT_RC (default 0). Records severity, title, detail.
  export SEND_ALERT_SPY="$HOME/send_alert.log"
  : >"$SEND_ALERT_SPY"
  cat >"$HOME/.local/libexec/osquery/alert-dispatch.sh" <<'STUB'
# shellcheck shell=bash
send_alert() {
  {
    printf 'CALL\tseverity=%s\ttitle=%s\n' "$1" "$2"
    printf 'DETAIL\t%s\n' "$3"
  } >>"$SEND_ALERT_SPY"
  return "${SEND_ALERT_RC:-0}"
}
STUB

  export OSQUERY_RESULTS_LOG="$HOME/.local/log/osquery/osqueryd.results.log"
  export OSQUERY_RESULTS_OFFSET="$HOME/.local/state/osquery-results-offset"
  : >"$OSQUERY_RESULTS_LOG"

  ADMIN_ROW='{"name":"new_admin_user","action":"added","columns":{"username":"eve","uid":"501"}}'
}

teardown() { rm -rf "$HOME_DIR"; }

log_inode() { ls -i "$OSQUERY_RESULTS_LOG" | awk '{print $1}'; }
log_size() { wc -c <"$OSQUERY_RESULTS_LOG" | tr -d '[:space:]'; }
seed_cursor() { printf '%s %s\n' "$(log_inode)" "$1" >"$OSQUERY_RESULTS_OFFSET"; }
cursor_offset() { awk '{print $2}' "$OSQUERY_RESULTS_OFFSET"; }
append_row() { printf '%s\n' "$1" >>"$OSQUERY_RESULTS_LOG"; }
crit_page_count() { grep -c 'New administrator account' "$SEND_ALERT_SPY" 2>/dev/null || true; }
send_alert_calls() { grep -c '^CALL' "$SEND_ALERT_SPY" 2>/dev/null || true; }

run_entry() { run bash "$ENTRY"; }

# (a) A CRIT row drives send_alert with a CRIT page.
@test "T-ENTRY-crit: a new_admin_user row delivers a CRIT page" {
  seed_cursor 0            # valid cursor at EOF of the empty log (no cursor-reset)
  append_row "$ADMIN_ROW"
  run_entry
  [ "$status" -eq 0 ]
  [ "$(crit_page_count)" -eq 1 ]
  grep -q 'severity=CRIT' "$SEND_ALERT_SPY"
}

# (b) The cursor advances after a delivered batch; a second run sends nothing new.
@test "T-ENTRY-advance: the cursor advances and a second run does not double-send" {
  seed_cursor 0
  append_row "$ADMIN_ROW"
  run_entry
  [ "$status" -eq 0 ]
  [ "$(cursor_offset)" -eq "$(log_size)" ]   # advanced to EOF
  local first_calls
  first_calls="$(send_alert_calls)"
  run_entry                                   # no new rows
  [ "$status" -eq 0 ]
  [ "$(send_alert_calls)" -eq "$first_calls" ] # no additional send
}

# (c) Checkpoint-after-durable: if send_alert fails, the cursor does NOT advance,
#     and the next run re-reads the same row (at-least-once, crash-safe).
@test "T-ENTRY-durable: a failed delivery keeps the cursor put and the row is re-read" {
  seed_cursor 0
  append_row "$ADMIN_ROW"
  SEND_ALERT_RC=1 run_entry                   # delivery hard-fails
  [ "$status" -eq 0 ]                          # exit 0 even on delivery failure
  [ "$(cursor_offset)" -eq 0 ]                 # cursor did NOT advance
  # The next run, with delivery succeeding, re-reads the SAME row and pages it.
  run_entry
  [ "$status" -eq 0 ]
  [ "$(crit_page_count)" -ge 1 ]
  [ "$(cursor_offset)" -eq "$(log_size)" ]     # now advanced
}

# (d) Log truncation resets the cursor and re-reads from the start. The old
#     content is several rows so the post-truncate size is clearly smaller than the
#     seeded offset, which is exactly the shrink the entry detects.
@test "T-ENTRY-truncate: a truncated log resets the cursor and re-reads" {
  append_row "$ADMIN_ROW"
  append_row "$ADMIN_ROW"
  append_row "$ADMIN_ROW"
  seed_cursor "$(log_size)"                     # cursor at EOF of the larger old content
  : >"$OSQUERY_RESULTS_LOG"                     # truncate in place (same inode, size 0)
  append_row "$ADMIN_ROW"                       # new, smaller content (size < seeded offset)
  run_entry
  [ "$status" -eq 0 ]
  [ "$(crit_page_count)" -eq 1 ]               # re-read from byte 0
}

# (e) A missing/corrupt cursor fires a loud cursor-reset page (visible, not silent).
@test "T-ENTRY-cursor-reset: a missing cursor pages a loud cursor-reset warning" {
  append_row "$ADMIN_ROW"
  rm -f "$OSQUERY_RESULTS_OFFSET"              # no cursor at all
  run_entry
  [ "$status" -eq 0 ]
  grep -q 'osquery cursor reset' "$SEND_ALERT_SPY"
  grep -q 'missing or corrupt' "$SEND_ALERT_SPY"
}

# (f) A malformed batch does not wedge the cursor: garbage rows drop out, no page,
#     and the cursor still advances past them.
@test "T-ENTRY-malformed: a garbage batch advances the cursor without paging" {
  seed_cursor 0
  append_row 'this is not json at all'
  append_row '{"name":"heartbeat_canary","action":"snapshot","columns":{}}'  # dropped by the select
  run_entry
  [ "$status" -eq 0 ]
  [ "$(crit_page_count)" -eq 0 ]               # nothing paged
  [ "$(cursor_offset)" -eq "$(log_size)" ]     # cursor advanced (no wedge)
}
