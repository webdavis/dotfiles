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
    --arg sha256 "${4:-e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855}" \
    '{name:"file_events_recent",action:"added",counter:1,columns:{action:$file_action,category:$category,target_path:$target_path,sha256:$sha256,time:"1780000000"},hostIdentifier:"dresden",unixTime:1780000000}'
}

ALERTER="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-results-alerter.sh"
POLLER="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-firewall-gatekeeper-monitor.sh"
TAILSCALE_MONITOR="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-tailscale-monitor.sh"
DISPATCH="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-alert-dispatch.sh"
DIGEST_BUILDER="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-digest.sh"
ALLOWLIST_TOOL="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-allowlist.sh"

# Stand up a temp HOME whose osquery-alert-dispatch.sh is a 1-line send_alert stub
# that records each dispatch as "<severity>\t<title>\t<detail>" to $SEND_ALERT_LOG.
setup_harness() {
  HARNESS_HOME="$(mktemp -d)"
  mkdir -p "$HARNESS_HOME/.local/bin" "$HARNESS_HOME/.local/log/osquery" "$HARNESS_HOME/.local/state"
  cat >"$HARNESS_HOME/.local/bin/osquery-alert-dispatch.sh" <<'STUB'
# Flatten the (multi-line) detail to one physical line so a dispatch is one record.
# Fields: severity, title, detail, sound (sound matters for the silent digest tier).
send_alert() { printf '%s\t%s\t%s\t%s\n' "$1" "$2" "${3//$'\n'/ }" "${4-}" >>"$SEND_ALERT_LOG"; }
# The alerter drains the spool at startup; H1 isn't testing delivery, so it's a no-op.
_drain_spool() { :; }
STUB
  export SEND_ALERT_LOG="$HARNESS_HOME/send_alert.log"
  : >"$SEND_ALERT_LOG"
  export OSQUERY_DIGEST_STORE="$HARNESS_HOME/.local/state/osquery-digest-spool/digest.ndjson"
  mkdir -p "$HARNESS_HOME/.config/osquery"
  export OSQUERY_LAUNCHD_ALLOWLIST="$HARNESS_HOME/.config/osquery/page-launchd-allowlist.txt"
  export OSQUERY_PIPELINE_MANIFEST="$HARNESS_HOME/.config/osquery/pipeline-known-good.sha256"
}

# Seed the pipeline-integrity manifest with known-good lines (e.g. "<sha256>  <path>").
seed_manifest() {
  printf '%s\n' "$@" >"$OSQUERY_PIPELINE_MANIFEST"
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

# Allowlist tool harness: a fresh temp allowlist file the writer reads via its env.
setup_allowlist_harness() {
  ALLOWLIST_HOME="$(mktemp -d)"
  export OSQUERY_LAUNCHD_ALLOWLIST="$ALLOWLIST_HOME/page-launchd-allowlist.txt"
}
teardown_allowlist_harness() { [[ -n ${ALLOWLIST_HOME:-} ]] && rm -rf "$ALLOWLIST_HOME"; }

# Run the allowlist writer with the harness env (pass tool args verbatim).
run_allowlist() {
  OSQUERY_LAUNCHD_ALLOWLIST="$OSQUERY_LAUNCHD_ALLOWLIST" bash "$ALLOWLIST_TOOL" "$@"
}

# Exact full-line membership in the allowlist file (matches the reader's grep -qxF).
assert_allowlisted() {
  if ! grep -qxF -- "$1" "$OSQUERY_LAUNCHD_ALLOWLIST" 2>/dev/null; then
    echo "expected '$1' in the allowlist: $(cat "$OSQUERY_LAUNCHD_ALLOWLIST" 2>/dev/null || echo '(no file)')" >&2
    return 1
  fi
}
assert_not_allowlisted() {
  if grep -qxF -- "$1" "$OSQUERY_LAUNCHD_ALLOWLIST" 2>/dev/null; then
    echo "expected '$1' NOT in the allowlist: $(cat "$OSQUERY_LAUNCHD_ALLOWLIST")" >&2
    return 1
  fi
}
# Count of label lines (non-comment, non-blank).
assert_allowlist_label_count() {
  local n
  n=$(grep -cvE '^[[:space:]]*(#|$)' "$OSQUERY_LAUNCHD_ALLOWLIST" 2>/dev/null || echo 0)
  if [[ $n -ne $1 ]]; then
    echo "expected $1 label(s), got $n: $(cat "$OSQUERY_LAUNCHD_ALLOWLIST" 2>/dev/null)" >&2
    return 1
  fi
}

# Build one digest NDJSON record in the shape the alerter's _digest_append emits.
# digest_record <detector> <identity> <summary>
digest_record() {
  jq -cn --arg detector "$1" --arg identity "$2" --arg summary "$3" \
    '{timestamp:"2026-06-13T00:00:00Z",detector:$detector,category:"",identity:$identity,action:"added",summary:$summary}'
}

# Tailscale funnel poller harness: a fake `tailscale` that prints $TAILSCALE_FUNNEL_OUTPUT.
setup_tailscale_harness() {
  setup_harness
  cat >"$HARNESS_HOME/.local/bin/tailscale" <<'SHIM'
#!/usr/bin/env bash
printf '%s\n' "$TAILSCALE_FUNNEL_OUTPUT"
SHIM
  chmod +x "$HARNESS_HOME/.local/bin/tailscale"
  TAILSCALE_STATE="$HARNESS_HOME/.local/state/osquery-tailscale-funnel"
}

# run_tailscale_monitor <prev-state|""> <funnel-output> — seed prior state (empty =
# first run), set the fake funnel output, run the real poller.
run_tailscale_monitor() {
  if [ -n "$1" ]; then printf '%s\n' "$1" >"$TAILSCALE_STATE"; else rm -f "$TAILSCALE_STATE"; fi
  HOME="$HARNESS_HOME" \
    OSQUERY_TAILSCALE_BIN="$HARNESS_HOME/.local/bin/tailscale" \
    OSQUERY_TAILSCALE_STATE="$TAILSCALE_STATE" \
    TAILSCALE_FUNNEL_OUTPUT="$2" \
    bash "$TAILSCALE_MONITOR"
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
    OSQUERY_LAUNCHD_ALLOWLIST="$OSQUERY_LAUNCHD_ALLOWLIST" \
    OSQUERY_PIPELINE_MANIFEST="$OSQUERY_PIPELINE_MANIFEST" \
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

# The inverse: NO CRIT page may contain <substring> — used to prove redaction
# (a full path or sha256 must never reach the payload, invariant #4).
assert_page_lacks() {
  if grep $'^CRIT\t' "$SEND_ALERT_LOG" | grep -qF -- "$1"; then
    echo "expected NO CRIT page to contain '$1', but one did: $(grep $'^CRIT\t' "$SEND_ALERT_LOG")" >&2
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
  # curl shim: record the invocation, then emit the next queued http code (one per
  # line in $CURL_CODES_FILE, popped per call), defaulting to 200 when the queue is
  # empty — so a test can script "503 503 503 then success".
  cat >"$HARNESS_HOME/bin/curl" <<'SHIM'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$CURL_LOG"
code=200
if [ -s "$CURL_CODES_FILE" ]; then
  code=$(head -1 "$CURL_CODES_FILE")
  tail -n +2 "$CURL_CODES_FILE" >"$CURL_CODES_FILE.tmp" 2>/dev/null && mv "$CURL_CODES_FILE.tmp" "$CURL_CODES_FILE"
fi
printf '%s' "$code"
SHIM
  chmod +x "$HARNESS_HOME/bin/alerter" "$HARNESS_HOME/bin/curl"
  export CURL_LOG="$HARNESS_HOME/curl.log"
  : >"$CURL_LOG"
  export CURL_CODES_FILE="$HARNESS_HOME/curl_codes"
  : >"$CURL_CODES_FILE"
  export PATH="$HARNESS_HOME/bin:$PATH"
  export OSQUERY_WEBHOOK_SECRET="testsecret"
  export OSQUERY_DELIVERY_LOG="$HARNESS_HOME/.local/log/osquery/webhook-delivery.log"
  export OSQUERY_SPOOL_DIR="$HARNESS_HOME/.local/state/osquery-spool"
  export OSQUERY_RETRY_BACKOFF_BASE=0 # don't really sleep between retries in tests
  HOME="$HARNESS_HOME"
  # shellcheck source=/dev/null
  source "$DISPATCH"
}

# Queue the http codes the curl shim will return, one per send/drain POST.
set_curl_codes() { printf '%s\n' "$@" >"$CURL_CODES_FILE"; }

# Drain the spool via the real dispatcher's _drain_spool (sourced in setup).
run_drain() { _drain_spool; }

# Count spooled page files (one file per undelivered page).
assert_spool_count() {
  local n=0
  [[ -d $OSQUERY_SPOOL_DIR ]] && n=$(find "$OSQUERY_SPOOL_DIR" -type f 2>/dev/null | wc -l | tr -d ' ')
  if [[ $n -ne $1 ]]; then
    echo "expected $1 spooled file(s), got $n: $(ls -la "$OSQUERY_SPOOL_DIR" 2>/dev/null)" >&2
    return 1
  fi
}

# A path has the expected octal permission bits (GNU stat in the nix shell; BSD fallback).
assert_mode() {
  local mode
  mode=$(stat -c '%a' "$2" 2>/dev/null || stat -f '%Lp' "$2" 2>/dev/null)
  if [[ $mode != "$1" ]]; then
    echo "expected mode $1 on $2, got $mode" >&2
    return 1
  fi
}

# Count webhook POSTs the shim recorded.
assert_post_count() {
  local n
  n=$(grep -c 'POST' "$CURL_LOG" 2>/dev/null || echo 0)
  if [[ $n -ne $1 ]]; then
    echo "expected $1 POST(s), got $n: $(cat "$CURL_LOG")" >&2
    return 1
  fi
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
