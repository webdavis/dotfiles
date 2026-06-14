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
POLLER="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-firewall-gatekeeper-monitor.sh"
DISPATCH="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-alert-dispatch.sh"
DIGEST_BUILDER="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-digest.sh"

# Stand up a temp HOME whose osquery-alert-dispatch.sh is a 1-line send_alert stub
# that records each dispatch as "<severity>\t<title>\t<detail>" to $SEND_ALERT_LOG.
setup_harness() {
  HARNESS_HOME="$(mktemp -d)"
  mkdir -p "$HARNESS_HOME/.local/bin" "$HARNESS_HOME/.local/log/osquery" "$HARNESS_HOME/.local/state"
  cat >"$HARNESS_HOME/.local/bin/osquery-alert-dispatch.sh" <<'STUB'
# Flatten the (multi-line) detail to one physical line so a dispatch is one record.
# Fields: severity, title, detail, sound (sound matters for the silent digest tier).
send_alert() { printf '%s\t%s\t%s\t%s\n' "$1" "$2" "${3//$'\n'/ }" "${4-}" >>"$SEND_ALERT_LOG"; }
STUB
  export SEND_ALERT_LOG="$HARNESS_HOME/send_alert.log"
  : >"$SEND_ALERT_LOG"
  export OSQUERY_DIGEST_STORE="$HARNESS_HOME/.local/state/osquery-digest-spool/digest.ndjson"
}

teardown_harness() { [[ -n ${HARNESS_HOME:-} ]] && rm -rf "$HARNESS_HOME"; }

# Posture poller harness: the same temp HOME + send_alert stub, plus a fake osqueryi
# that prints the posture array held in $POLLER_POSTURE (so a test controls the
# "current" firewall/gatekeeper reading).
setup_poller_harness() {
  setup_harness
  cat >"$HARNESS_HOME/.local/bin/osqueryi" <<'SHIM'
#!/usr/bin/env bash
printf '%s\n' "$POLLER_POSTURE"
SHIM
  chmod +x "$HARNESS_HOME/.local/bin/osqueryi"
  POSTURE_STATE="$HARNESS_HOME/.local/state/osquery-posture-state.json"
}

# run_poller <prev-posture-object> <current-posture-object> — seed the prior state,
# set the current reading, run the real poller. osqueryi --json returns an array, so
# the current object is wrapped in [ ] for the fake.
run_poller() {
  printf '%s\n' "$1" >"$POSTURE_STATE"
  HOME="$HARNESS_HOME" \
    OSQUERYI="$HARNESS_HOME/.local/bin/osqueryi" \
    OSQUERY_POSTURE_STATE="$POSTURE_STATE" \
    POLLER_POSTURE="[$2]" \
    bash "$POLLER"
}

# Build one digest NDJSON record in the shape the alerter's _digest_append emits.
# digest_record <detector> <identity> <summary>
digest_record() {
  jq -cn --arg detector "$1" --arg identity "$2" --arg summary "$3" \
    '{timestamp:"2026-06-13T00:00:00Z",detector:$detector,category:"",identity:$identity,action:"added",summary:$summary}'
}

# Seed the digest store with NDJSON lines (each argument is one record).
seed_digest() {
  mkdir -p "$(dirname "$OSQUERY_DIGEST_STORE")"
  printf '%s\n' "$@" >"$OSQUERY_DIGEST_STORE"
}

# Run the real digest builder against the seeded store (uses the send_alert stub).
run_digest() {
  HOME="$HARNESS_HOME" OSQUERY_DIGEST_STORE="$OSQUERY_DIGEST_STORE" bash "$DIGEST_BUILDER"
}

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
    OSQUERY_DIGEST_STORE="$OSQUERY_DIGEST_STORE" \
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

# Delivery (H2) harness: source the REAL dispatcher with curl + alerter shimmed on
# PATH, so a test asserts exactly which webhook URL (if any) a send_alert POSTs to.
# The curl shim records each invocation and emits a 2xx like real `curl -w`.
setup_dispatch_harness() {
  HARNESS_HOME="$(mktemp -d)"
  mkdir -p "$HARNESS_HOME/bin" "$HARNESS_HOME/.local/log/osquery"
  printf '#!/usr/bin/env bash\nexit 0\n' >"$HARNESS_HOME/bin/alerter"
  cat >"$HARNESS_HOME/bin/curl" <<'SHIM'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$CURL_LOG"
printf '200'
SHIM
  chmod +x "$HARNESS_HOME/bin/alerter" "$HARNESS_HOME/bin/curl"
  export CURL_LOG="$HARNESS_HOME/curl.log"
  : >"$CURL_LOG"
  export PATH="$HARNESS_HOME/bin:$PATH"
  export OSQUERY_WEBHOOK_SECRET="testsecret"
  export OSQUERY_DELIVERY_LOG="$HARNESS_HOME/.local/log/osquery/webhook-delivery.log"
  HOME="$HARNESS_HOME"
  # shellcheck source=/dev/null
  source "$DISPATCH"
}

# A webhook POST went to a URL containing <substring>.
assert_posted_to() {
  if ! grep -qF -- "$1" "$CURL_LOG"; then
    echo "expected a POST to a URL containing '$1'; curl log: $(cat "$CURL_LOG")" >&2
    return 1
  fi
}

# No webhook POST happened at all.
assert_no_post() {
  if [[ -s $CURL_LOG ]]; then
    echo "expected NO webhook POST, but curl was called: $(cat "$CURL_LOG")" >&2
    return 1
  fi
}

# Log-only means zero delivery — no page, no #osquery line, nothing dispatched.
assert_no_dispatch() {
  if [[ -s $SEND_ALERT_LOG ]]; then
    echo "expected NO dispatch, but send_alert was called: $(cat "$SEND_ALERT_LOG")" >&2
    return 1
  fi
}

# The digest builder dispatched exactly one CRIT message titled "...daily digest...".
assert_digest_sent() {
  local n
  n=$(grep -c $'^CRIT\t' "$SEND_ALERT_LOG" 2>/dev/null || echo 0)
  if [[ $n -ne 1 ]]; then
    echo "expected exactly 1 CRIT digest send, got $n: $(cat "$SEND_ALERT_LOG")" >&2
    return 1
  fi
  if ! grep -qF 'daily digest' "$SEND_ALERT_LOG"; then
    echo "digest send title lacks 'daily digest': $(cat "$SEND_ALERT_LOG")" >&2
    return 1
  fi
}

# The one digest send used an empty sound (silent, non-interruptive — not a page).
assert_digest_silent() {
  local sound
  sound=$(awk -F'\t' '$1=="CRIT"{print $4}' "$SEND_ALERT_LOG")
  if [[ -n $sound ]]; then
    echo "expected silent digest (empty sound), got '$sound'" >&2
    return 1
  fi
}

# The digest send body contains <substring> (the multi-line body is flattened to one line).
assert_digest_body_has() {
  if ! grep -qF -- "$1" "$SEND_ALERT_LOG"; then
    echo "expected digest body to contain '$1': $(cat "$SEND_ALERT_LOG")" >&2
    return 1
  fi
}

# After a send the live store is cleared but a .last snapshot is kept.
assert_store_rotated() {
  if [[ -e $OSQUERY_DIGEST_STORE ]]; then
    echo "expected the live digest store cleared after send" >&2
    return 1
  fi
  if [[ ! -e $OSQUERY_DIGEST_STORE.last ]]; then
    echo "expected a .last snapshot kept" >&2
    return 1
  fi
}

# The digest store holds one NDJSON line per suspicious-but-ambiguous finding.
assert_digest_count() {
  local want="$1" got=0
  [[ -f $OSQUERY_DIGEST_STORE ]] && got=$(grep -c . "$OSQUERY_DIGEST_STORE" 2>/dev/null || echo 0)
  if [[ $got -ne $want ]]; then
    echo "expected $want digest line(s), got $got: $(cat "$OSQUERY_DIGEST_STORE" 2>/dev/null || echo '(no store)')" >&2
    return 1
  fi
}
