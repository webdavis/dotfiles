#!/usr/bin/env bash
# Test harness + realistic fixtures for the osquery results alerter.
#
# H1 harness: drive the REAL alerter against a fixture results.log with a 1-line
# send_alert stub, then assert on what it dispatched. Real osqueryd rows carry a
# 9-key envelope (action, calendarTime, columns, counter, epoch, hostIdentifier,
# name, numerics, unixTime) — the builders below emit that shape so fixtures match
# production, not a toy {name,action,columns}.

# Differential row: row <name> <action> <counter> <columns-json>
row() {
  jq -cn --arg name "$1" --arg action "$2" --argjson counter "$3" --argjson columns "$4" \
    '{name:$name,action:$action,counter:$counter,columns:$columns,hostIdentifier:"dresden",calendarTime:"Tue Jun 10 17:00:00 2026 UTC",epoch:0,numerics:false,unixTime:1780000000}'
}

# Evented file_events row: file_event_row <category> <target_path> <CREATED|UPDATED|DELETED>
# Outer action is always "added"; the real FSEvents verb is columns.action.
file_event_row() {
  jq -cn --arg category "$1" --arg target_path "$2" --arg file_action "$3" \
    '{name:"file_events_recent",action:"added",counter:1,columns:{action:$file_action,category:$category,target_path:$target_path,sha256:"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",time:"1780000000"},hostIdentifier:"dresden",unixTime:1780000000}'
}

ALERTER="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-results-alerter.sh"

# Stand up a temp HOME whose osquery-alert-dispatch.sh is a 1-line send_alert stub
# that records each dispatch as "<severity>\t<title>\t<detail>" to $SEND_ALERT_LOG.
setup_harness() {
  HARNESS_HOME="$(mktemp -d)"
  mkdir -p "$HARNESS_HOME/.local/bin" "$HARNESS_HOME/.local/log/osquery" "$HARNESS_HOME/.local/state"
  printf '%s\n' 'send_alert() { printf "%s\t%s\t%s\n" "$1" "$2" "$3" >>"$SEND_ALERT_LOG"; }' \
    >"$HARNESS_HOME/.local/bin/osquery-alert-dispatch.sh"
  export SEND_ALERT_LOG="$HARNESS_HOME/send_alert.log"
  : >"$SEND_ALERT_LOG"
}

teardown_harness() { [[ -n ${HARNESS_HOME:-} ]] && rm -rf "$HARNESS_HOME"; }

# Run the real alerter against fixture rows (NDJSON as the single argument).
run_alerter() {
  local results_log="$HARNESS_HOME/.local/log/osquery/osqueryd.results.log"
  printf '%s\n' "$1" >"$results_log"
  # "0 0" = a valid prior state with a non-matching inode, so the alerter reads
  # the whole file from byte 0 instead of seeding-and-exiting like a first run.
  printf '0 0\n' >"$HARNESS_HOME/.local/state/osquery-results-offset"
  HOME="$HARNESS_HOME" \
    OSQUERY_RESULTS_LOG="$results_log" \
    OSQUERY_RESULTS_OFFSET="$HARNESS_HOME/.local/state/osquery-results-offset" \
    bash "$ALERTER"
}

# A "page" is a CRIT dispatch (the #priority channel).
assert_no_page() {
  if grep -q $'^CRIT\t' "$SEND_ALERT_LOG"; then
    echo "expected NO page, but a CRIT was dispatched: $(grep $'^CRIT\t' "$SEND_ALERT_LOG")" >&2
    return 1
  fi
}

assert_page_has() {
  if ! grep $'^CRIT\t' "$SEND_ALERT_LOG" | grep -qF -- "$1"; then
    echo "expected a CRIT page containing '$1'; CRIT pages: $(grep $'^CRIT\t' "$SEND_ALERT_LOG" || echo '(none)')" >&2
    return 1
  fi
}
