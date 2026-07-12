#!/usr/bin/env bash
# Test harness + realistic fixtures for the osquery results alerter.
#
# H1 harness: drive the REAL alerter against a fixture results.log with a 1-line
# send_alert stub, then assert on what it dispatched. Real osqueryd rows carry a
# 9-key envelope (action, calendarTime, columns, counter, epoch, hostIdentifier,
# name, numerics, unixTime) - the builders below emit that shape so fixtures match
# production, not a toy {name,action,columns}.

# Differential row: row <name> <action> <counter> <columns-json>
row() {
  jq -cn --arg name "$1" --arg action "$2" --argjson counter "$3" --argjson columns "$4" \
    '{name:$name,action:$action,counter:$counter,columns:$columns,hostIdentifier:"dresden",calendarTime:"Tue Jun 10 17:00:00 2026 UTC",epoch:0,numerics:false,unixTime:1780000000}'
}

# Evented file_events row: file_event_row <category> <target_path> <verb> [sha256]
# Outer action is always "added"; the real FSEvents verb is columns.action (CREATED,
# UPDATED, MOVED_TO, ROOT_CHANGED, ATTRIBUTES_MODIFIED, DELETED - the production set).
# The sha256 arg uses ${4-default} (default only when UNSET), so an explicit "" models
# the empty sha256 that live MOVED_TO/ROOT_CHANGED/ATTRIBUTES_MODIFIED/DELETED rows
# carry (osquery does not content-hash a rename/attribute/delete event).
file_event_row() {
  jq -cn --arg category "$1" --arg target_path "$2" --arg file_action "$3" \
    --arg sha256 "${4-e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855}" \
    '{name:"file_events_recent",action:"added",counter:1,columns:{action:$file_action,category:$category,target_path:$target_path,sha256:$sha256,time:"1780000000"},hostIdentifier:"dresden",unixTime:1780000000}'
}

ALERTER="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-results-alerter.sh"
POLLER="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-firewall-gatekeeper-monitor.sh"
TAILSCALE_MONITOR="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-tailscale-monitor.sh"
DISPATCH="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-alert-dispatch.sh"
DIGEST_BUILDER="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-digest.sh"
ALLOWLIST_TOOL="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-allowlist.sh"
WATCHDOG="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-uptime-watchdog.sh"
HEARTBEAT="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-heartbeat.sh"

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
  # R2-10: the pipeline re-hash debounce (a real sleep in production) is 0 in tests - the
  # on-disk target and manifest are already settled, so the wait only slows the suite.
  export OSQUERY_PIPELINE_REHASH_DELAY=0
  export OSQUERY_DIGEST_STORE="$HARNESS_HOME/.local/state/osquery-digest-spool/digest.ndjson"
  mkdir -p "$HARNESS_HOME/.config/osquery"
  export OSQUERY_LAUNCHD_ALLOWLIST="$HARNESS_HOME/.config/osquery/page-launchd-allowlist.txt"
  export OSQUERY_PIPELINE_MANIFEST="$HARNESS_HOME/.config/osquery/pipeline-known-good.sha256"
}

# Seed the pipeline-integrity manifest with known-good lines (e.g. "<sha256>  <path>").
seed_manifest() {
  printf '%s\n' "$@" >"$OSQUERY_PIPELINE_MANIFEST"
}

# Seed one persistence-allowlist tuple (R2-1). The allowlist binds a launchd LABEL to its
# known-good identity (canonical plist path + program [+ pinned plist sha256]); the alerter
# suppresses ONLY a full-tuple match and PAGES a label reused with a different path/program.
# seed_allowlist_tuple <label> <path> <program> [sha256]  (append one NDJSON object per call)
seed_allowlist_tuple() {
  jq -cn --arg label "$1" --arg path "$2" --arg program "$3" --arg sha256 "${4-}" \
    '{label:$label, path:$path, program:$program, sha256:$sha256}' >>"$OSQUERY_LAUNCHD_ALLOWLIST"
}

# Install an enricher stub that reports every path UNTRUSTED (rc=10), so a NOTICE
# finding with a signable path gets promoted NOTICE->CRIT - exercises the promotion
# path a log-only detector must NOT be able to reach.
install_untrusted_enricher() {
  printf '#!/usr/bin/env bash\nexit 10\n' >"$HARNESS_HOME/.local/bin/osquery-enrich-finding.sh"
  chmod +x "$HARNESS_HOME/.local/bin/osquery-enrich-finding.sh"
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

# run_poller <prev-posture-object> <current-posture-object> - seed the prior state,
# set the current reading, run the real poller. osqueryi --json returns an array, so
# the current object is wrapped in [ ] for the fake. The seeded baseline is chmod 600 to
# match production (the poller writes its state owner-only), so it is a TRUSTED baseline.
run_poller() {
  printf '%s\n' "$1" >"$POSTURE_STATE"
  chmod 600 "$POSTURE_STATE"
  HOME="$HARNESS_HOME" \
    OSQUERYI="$HARNESS_HOME/.local/bin/osqueryi" \
    OSQUERY_POSTURE_STATE="$POSTURE_STATE" \
    POLLER_POSTURE="[$2]" \
    bash "$POLLER"
}

# run_poller_firstrun <current-posture> - no prior baseline (first observation).
run_poller_firstrun() {
  rm -f "$POSTURE_STATE"
  HOME="$HARNESS_HOME" \
    OSQUERYI="$HARNESS_HOME/.local/bin/osqueryi" \
    OSQUERY_POSTURE_STATE="$POSTURE_STATE" \
    POLLER_POSTURE="[$1]" \
    bash "$POLLER"
}

# run_poller_badperms <prev-posture> <current-posture> - a baseline that exists but is
# group/world-readable (mode 644), i.e. NOT owner-only, so it must be treated as untrusted.
run_poller_badperms() {
  printf '%s\n' "$1" >"$POSTURE_STATE"
  chmod 644 "$POSTURE_STATE"
  HOME="$HARNESS_HOME" \
    OSQUERYI="$HARNESS_HOME/.local/bin/osqueryi" \
    OSQUERY_POSTURE_STATE="$POSTURE_STATE" \
    POLLER_POSTURE="[$2]" \
    bash "$POLLER"
}

# Uptime-watchdog harness: setup_harness (send_alert stub) + PATH stubs for pgrep / launchctl /
# curl and an osqueryi stub. Test knobs (env):
#   WATCHDOG_DOWN_AGENTS   space-separated labels launchctl reports NOT loaded (exit 1)
#   WATCHDOG_CRASH_AGENTS  space-separated labels whose `list` reports a nonzero LastExitStatus
#   WATCHDOG_CRASH_STATUS  the nonzero LastExitStatus value (default 78)
#   WATCHDOG_HTTP_CODE     the route probe's HTTP code (default 405 = the POST-only route exists)
setup_watchdog_harness() {
  setup_harness
  printf '#!/usr/bin/env bash\nexit 0\n' >"$HARNESS_HOME/.local/bin/pgrep"
  cat >"$HARNESS_HOME/.local/bin/curl" <<'SHIM'
#!/usr/bin/env bash
printf '%s' "${WATCHDOG_HTTP_CODE:-405}"
SHIM
  printf '#!/usr/bin/env bash\necho %s\n' "'[{\"ok\":\"1\"}]'" >"$HARNESS_HOME/.local/bin/osqueryi"
  cat >"$HARNESS_HOME/.local/bin/launchctl" <<'SHIM'
#!/usr/bin/env bash
# `list <label>`: exit 1 for a not-loaded agent; else print a plist dict whose LastExitStatus
# is nonzero for a crash-looping agent (WATCHDOG_CRASH_AGENTS), 0 otherwise.
if [ "${1:-}" = "list" ]; then
  label="${2:-}"
  for down in $WATCHDOG_DOWN_AGENTS; do [ "$label" = "$down" ] && exit 1; done
  les=0
  for crash in $WATCHDOG_CRASH_AGENTS; do [ "$label" = "$crash" ] && les="${WATCHDOG_CRASH_STATUS:-78}"; done
  printf '{\n\t"Label" = "%s";\n\t"LastExitStatus" = %s;\n\t"PID" = 4242;\n};\n' "$label" "$les"
  exit 0
fi
exit 0
SHIM
  chmod +x "$HARNESS_HOME/.local/bin/pgrep" "$HARNESS_HOME/.local/bin/curl" \
    "$HARNESS_HOME/.local/bin/osqueryi" "$HARNESS_HOME/.local/bin/launchctl"
  export OSQUERY_WATCHDOG_STATE="$HARNESS_HOME/.local/state/osquery-watchdog-state.json"
  export OSQUERY_SPOOL_DIR="$HARNESS_HOME/.local/state/osquery-spool"
}

# run_watchdog [down-agent-labels] - run the real watchdog with the stubs on PATH. Other knobs
# (WATCHDOG_CRASH_AGENTS/_STATUS, WATCHDOG_HTTP_CODE) are read from the (exported) environment.
run_watchdog() {
  HOME="$HARNESS_HOME" \
    PATH="$HARNESS_HOME/.local/bin:$PATH" \
    OSQUERYI="$HARNESS_HOME/.local/bin/osqueryi" \
    OSQUERY_WATCHDOG_STATE="$OSQUERY_WATCHDOG_STATE" \
    OSQUERY_SPOOL_DIR="$OSQUERY_SPOOL_DIR" \
    WATCHDOG_DOWN_AGENTS="${1:-}" \
    bash "$WATCHDOG"
}

# Seed a spooled page file aged <minutes-old> (a stuck, undelivered page).
seed_spool_file() {
  mkdir -p "$OSQUERY_SPOOL_DIR"
  local f="$OSQUERY_SPOOL_DIR/osquery-stuck-$RANDOM"
  printf '%s\tosquery-stuck\thttp://127.0.0.1:8644/x\tYm9keQ==\n' "$(date -u +%s)" >"$f"
  # Age its mtime by <minutes-old> so the staleness check trips (GNU touch -d first, BSD fallback).
  local mins="${1:-0}"
  if [[ $mins -gt 0 ]]; then
    touch -d "-${mins} minutes" "$f" 2>/dev/null ||
      touch -t "$(date -v"-${mins}M" +%Y%m%d%H%M 2>/dev/null)" "$f" 2>/dev/null || true
  fi
}

# End-to-end redaction harness (H2): the REAL dispatcher (not the send_alert stub),
# a no-op alerter, and a curl shim that always fails - so the REAL alerter spools a
# page to disk and a test can base64-decode the on-disk body to prove no full path,
# sha256, or secret survives the alerter's basename redaction end to end.
setup_redaction_h2_harness() {
  setup_harness
  cp "$DISPATCH" "$HARNESS_HOME/.local/bin/osquery-alert-dispatch.sh"
  printf '#!/usr/bin/env bash\nexit 0\n' >"$HARNESS_HOME/.local/bin/alerter"
  printf '#!/usr/bin/env bash\nprintf 503\n' >"$HARNESS_HOME/.local/bin/curl"
  chmod +x "$HARNESS_HOME/.local/bin/alerter" "$HARNESS_HOME/.local/bin/curl"
  export OSQUERY_WEBHOOK_SECRET="testsecret"
  export OSQUERY_SPOOL_DIR="$HARNESS_HOME/.local/state/osquery-spool"
  export OSQUERY_DELIVERY_LOG="$HARNESS_HOME/.local/log/osquery/webhook-delivery.log"
}

# run_redaction_h2 <fixture-row> - drive the REAL alerter into the REAL dispatcher
# (curl forced to fail), so a CRIT finding spools to disk.
run_redaction_h2() {
  local results_log="$HARNESS_HOME/.local/log/osquery/osqueryd.results.log"
  printf '%s\n' "$1" >"$results_log"
  printf '0 0\n' >"$HARNESS_HOME/.local/state/osquery-results-offset"
  HOME="$HARNESS_HOME" \
    PATH="$HARNESS_HOME/.local/bin:$PATH" \
    OSQUERY_RESULTS_LOG="$results_log" \
    OSQUERY_RESULTS_OFFSET="$HARNESS_HOME/.local/state/osquery-results-offset" \
    OSQUERY_DIGEST_STORE="$OSQUERY_DIGEST_STORE" \
    OSQUERY_LAUNCHD_ALLOWLIST="$OSQUERY_LAUNCHD_ALLOWLIST" \
    OSQUERY_PIPELINE_MANIFEST="$OSQUERY_PIPELINE_MANIFEST" \
    OSQUERY_WEBHOOK_SECRET="$OSQUERY_WEBHOOK_SECRET" \
    OSQUERY_SPOOL_DIR="$OSQUERY_SPOOL_DIR" \
    OSQUERY_DELIVERY_LOG="$OSQUERY_DELIVERY_LOG" \
    OSQUERY_RETRY_BACKOFF_BASE=0 \
    bash "$ALERTER"
}

# Drive the REAL alerter into the REAL dispatcher with delivery AND durable spool both
# broken (curl fails, secret absent, spool dir unwritable), so the batch page HARD-fails
# (R2-6). The alerter must NOT advance the cursor past a page it could neither deliver nor
# store, so the next run retries it. <fixture-row> plus the pre-run cursor "0 0".
run_alerter_hardfail_spool() {
  local results_log="$HARNESS_HOME/.local/log/osquery/osqueryd.results.log"
  local offset="$HARNESS_HOME/.local/state/osquery-results-offset"
  printf '%s\n' "$1" >"$results_log"
  printf '0 0\n' >"$offset"
  touch "$HARNESS_HOME/spool-blocker" # parent is a FILE → mkdir of the spool dir cannot succeed
  HOME="$HARNESS_HOME" \
    PATH="$HARNESS_HOME/.local/bin:$PATH" \
    OSQUERY_RESULTS_LOG="$results_log" \
    OSQUERY_RESULTS_OFFSET="$offset" \
    OSQUERY_DIGEST_STORE="$OSQUERY_DIGEST_STORE" \
    OSQUERY_LAUNCHD_ALLOWLIST="$OSQUERY_LAUNCHD_ALLOWLIST" \
    OSQUERY_PIPELINE_MANIFEST="$OSQUERY_PIPELINE_MANIFEST" \
    OSQUERY_SPOOL_DIR="$HARNESS_HOME/spool-blocker/spool" \
    OSQUERY_DELIVERY_LOG="$HARNESS_HOME/.local/log/osquery/webhook-delivery.log" \
    OSQUERY_RETRY_BACKOFF_BASE=0 \
    bash "$ALERTER"
}

# Flip the H2 curl shim to success and drain the spool via the real dispatcher, so a
# test can prove a spooled (capped) page actually delivers and clears on recovery.
run_h2_drain() {
  printf '#!/usr/bin/env bash\nprintf 200\n' >"$HARNESS_HOME/.local/bin/curl"
  chmod +x "$HARNESS_HOME/.local/bin/curl"
  HOME="$HARNESS_HOME" \
    PATH="$HARNESS_HOME/.local/bin:$PATH" \
    OSQUERY_WEBHOOK_SECRET="$OSQUERY_WEBHOOK_SECRET" \
    OSQUERY_SPOOL_DIR="$OSQUERY_SPOOL_DIR" \
    OSQUERY_DELIVERY_LOG="$OSQUERY_DELIVERY_LOG" \
    bash -c 'source "$HOME/.local/bin/osquery-alert-dispatch.sh"; _drain_spool'
}

# Heartbeat harness (R2-8): setup_harness (send_alert stub) + a controllable snapshots.log. The
# heartbeat verifies the ROOT DAEMON by checking that its scheduled heartbeat_canary snapshot is
# FRESH - not by launching a standalone osqueryi (which succeeds even while osqueryd is stopped).
setup_heartbeat_harness() {
  setup_harness
  export OSQUERY_SNAPSHOTS_LOG="$HARNESS_HOME/.local/log/osquery/osqueryd.snapshots.log"
  : >"$OSQUERY_SNAPSHOTS_LOG"
}

# seed_canary <seconds-ago> - append a heartbeat_canary snapshot row timestamped that long ago,
# in the shape the root daemon writes to osqueryd.snapshots.log.
seed_canary() {
  local ts
  ts=$(($(date -u +%s) - $1))
  jq -cn --argjson t "$ts" \
    '{name:"heartbeat_canary",action:"snapshot",snapshot:[{unix_time:($t|tostring)}],unixTime:$t,hostIdentifier:"dresden"}' \
    >>"$OSQUERY_SNAPSHOTS_LOG"
}

# run_heartbeat - run the real heartbeat against the seeded snapshots.log.
run_heartbeat() {
  HOME="$HARNESS_HOME" \
    OSQUERY_SNAPSHOTS_LOG="$OSQUERY_SNAPSHOTS_LOG" \
    bash "$HEARTBEAT"
}

# Allowlist tool harness: a fresh temp allowlist file the writer curates, plus an osqueryi
# stub the writer uses to CAPTURE a label's identity (R2-1). The stub prints
# $ALLOWLIST_OSQUERYI_ROW (default an empty result, so a fake label captures a label-only
# entry); a capture test sets it to a real {path,program} row.
setup_allowlist_harness() {
  ALLOWLIST_HOME="$(mktemp -d)"
  export OSQUERY_LAUNCHD_ALLOWLIST="$ALLOWLIST_HOME/page-launchd-allowlist.txt"
  mkdir -p "$ALLOWLIST_HOME/bin"
  cat >"$ALLOWLIST_HOME/bin/osqueryi" <<'SHIM'
#!/usr/bin/env bash
printf '%s\n' "${ALLOWLIST_OSQUERYI_ROW:-[]}"
SHIM
  chmod +x "$ALLOWLIST_HOME/bin/osqueryi"
  export ALLOWLIST_OSQUERYI="$ALLOWLIST_HOME/bin/osqueryi"
}
teardown_allowlist_harness() { [[ -n ${ALLOWLIST_HOME:-} ]] && rm -rf "$ALLOWLIST_HOME"; }

# Run the allowlist writer with the harness env (pass tool args verbatim).
run_allowlist() {
  OSQUERY_LAUNCHD_ALLOWLIST="$OSQUERY_LAUNCHD_ALLOWLIST" OSQUERYI="$ALLOWLIST_OSQUERYI" \
    bash "$ALLOWLIST_TOOL" "$@"
}

# Membership by the JSON .label field (the file is NDJSON tuples now, R2-1).
assert_allowlisted() {
  if ! grep -qF "\"label\":\"$1\"" "$OSQUERY_LAUNCHD_ALLOWLIST" 2>/dev/null; then
    echo "expected label '$1' in the allowlist: $(cat "$OSQUERY_LAUNCHD_ALLOWLIST" 2>/dev/null || echo '(no file)')" >&2
    return 1
  fi
}
assert_not_allowlisted() {
  if grep -qF "\"label\":\"$1\"" "$OSQUERY_LAUNCHD_ALLOWLIST" 2>/dev/null; then
    echo "expected label '$1' NOT in the allowlist: $(cat "$OSQUERY_LAUNCHD_ALLOWLIST")" >&2
    return 1
  fi
}
# Count of entry lines (non-comment, non-blank) - one NDJSON tuple per line.
assert_allowlist_label_count() {
  local n
  n=$(grep -cvE '^[[:space:]]*(#|$)' "$OSQUERY_LAUNCHD_ALLOWLIST" 2>/dev/null || echo 0)
  if [[ $n -ne $1 ]]; then
    echo "expected $1 entr(y/ies), got $n: $(cat "$OSQUERY_LAUNCHD_ALLOWLIST" 2>/dev/null)" >&2
    return 1
  fi
}

# Build one digest NDJSON record in the shape the alerter's _digest_append emits.
# digest_record <detector> <identity> <summary>
digest_record() {
  jq -cn --arg detector "$1" --arg identity "$2" --arg summary "$3" \
    '{timestamp:"2026-06-13T00:00:00Z",detector:$detector,category:"",identity:$identity,action:"added",summary:$summary}'
}

# Tailscale funnel poller harness: a fake `tailscale` that prints $TAILSCALE_FUNNEL_OUTPUT and
# exits $TAILSCALE_FUNNEL_RC (default 0), so a test can force a status-command failure.
setup_tailscale_harness() {
  setup_harness
  cat >"$HARNESS_HOME/.local/bin/tailscale" <<'SHIM'
#!/usr/bin/env bash
printf '%s\n' "$TAILSCALE_FUNNEL_OUTPUT"
exit "${TAILSCALE_FUNNEL_RC:-0}"
SHIM
  chmod +x "$HARNESS_HOME/.local/bin/tailscale"
  TAILSCALE_STATE="$HARNESS_HOME/.local/state/osquery-tailscale-funnel"
}

# Seed the prior state (R2-5 state is JSON now). Token → JSON: "" removes it (first run),
# active/inactive/missing map to the JSON shape, corrupt writes garbage, anything else is verbatim.
_seed_ts_state() {
  case "$1" in
    "") rm -f "$TAILSCALE_STATE" ;;
    inactive) printf '{"funnel":"inactive","monitor":"ok"}\n' >"$TAILSCALE_STATE" ;;
    active) printf '{"funnel":"active","monitor":"ok"}\n' >"$TAILSCALE_STATE" ;;
    missing) printf '{"funnel":"inactive","monitor":"missing"}\n' >"$TAILSCALE_STATE" ;;
    corrupt) printf 'not-json-garbage\n' >"$TAILSCALE_STATE" ;;
    *) printf '%s\n' "$1" >"$TAILSCALE_STATE" ;;
  esac
}

# run_tailscale_monitor <prev-token|""> <funnel-output> [rc] - seed prior state, set the fake
# funnel output (and optional nonzero exit code), run the real poller.
run_tailscale_monitor() {
  _seed_ts_state "$1"
  HOME="$HARNESS_HOME" \
    OSQUERY_TAILSCALE_BIN="$HARNESS_HOME/.local/bin/tailscale" \
    OSQUERY_TAILSCALE_STATE="$TAILSCALE_STATE" \
    TAILSCALE_FUNNEL_OUTPUT="$2" \
    TAILSCALE_FUNNEL_RC="${3:-0}" \
    bash "$TAILSCALE_MONITOR"
}

# Same, but the configured binary does not exist (the dead-monitor regression).
run_tailscale_monitor_missing_bin() {
  _seed_ts_state "$1"
  HOME="$HARNESS_HOME" \
    OSQUERY_TAILSCALE_BIN="$HARNESS_HOME/.local/bin/no-such-tailscale" \
    OSQUERY_TAILSCALE_STATE="$TAILSCALE_STATE" \
    bash "$TAILSCALE_MONITOR" 2>/dev/null
}

# Same, but with NO env override: the poller must find the shim via `command -v`
# on PATH (the homebrew-formula resolution path).
run_tailscale_monitor_path_resolved() {
  _seed_ts_state "$1"
  HOME="$HARNESS_HOME" \
    PATH="$HARNESS_HOME/.local/bin:$PATH" \
    OSQUERY_TAILSCALE_STATE="$TAILSCALE_STATE" \
    TAILSCALE_FUNNEL_OUTPUT="$2" \
    env -u OSQUERY_TAILSCALE_BIN bash "$TAILSCALE_MONITOR"
}

# The stored funnel field (for asserting a gap PRESERVED the prior valid state).
tailscale_state_funnel() { jq -r '.funnel // empty' <"$TAILSCALE_STATE" 2>/dev/null || echo ""; }

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

# Run the real alerter with the cursor state in a chosen shape (R2-2): "missing" removes the
# offset file, "corrupt" writes an unparseable one. A missing/corrupt cursor must PROCESS the
# recent tail (never silently seek-to-EOF), so the fixture rows are seen, not skipped.
run_alerter_cursor() {
  local shape="$1" rows="$2" results_log="$HARNESS_HOME/.local/log/osquery/osqueryd.results.log"
  local offset="$HARNESS_HOME/.local/state/osquery-results-offset"
  printf '%s\n' "$rows" >"$results_log"
  case "$shape" in
    missing) rm -f "$offset" ;;
    corrupt) printf 'not-a-cursor\n' >"$offset" ;;
    *)
      echo "run_alerter_cursor: unknown shape '$shape'" >&2
      return 2
      ;;
  esac
  HOME="$HARNESS_HOME" \
    OSQUERY_RESULTS_LOG="$results_log" \
    OSQUERY_RESULTS_OFFSET="$offset" \
    OSQUERY_DIGEST_STORE="$OSQUERY_DIGEST_STORE" \
    OSQUERY_LAUNCHD_ALLOWLIST="$OSQUERY_LAUNCHD_ALLOWLIST" \
    OSQUERY_PIPELINE_MANIFEST="$OSQUERY_PIPELINE_MANIFEST" \
    bash "$ALERTER"
}

# Read back the cursor's stored byte offset (second field), or "" when the file is absent.
cursor_offset() {
  local offset="$HARNESS_HOME/.local/state/osquery-results-offset" _inode off
  [[ -f $offset ]] || {
    printf ''
    return 0
  }
  read -r _inode off <"$offset" 2>/dev/null || true
  printf '%s' "$off"
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

assert_warn_has() {
  if ! grep $'^WARN\t' "$SEND_ALERT_LOG" | grep -qF -- "$1"; then
    echo "expected a WARN containing '$1'; WARNs: $(grep $'^WARN\t' "$SEND_ALERT_LOG" || echo '(none)')" >&2
    return 1
  fi
}

# The inverse: NO CRIT page may contain <substring> - used to prove redaction
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
  # The alerter shim records its argv so a test can assert the local notification's
  # content (e.g. the loud "Discord channel broken - no secret" message).
  export ALERTER_LOG="$HARNESS_HOME/alerter.log"
  : >"$ALERTER_LOG"
  printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$*" >>"%s"\nexit 0\n' "$ALERTER_LOG" >"$HARNESS_HOME/bin/alerter"
  # curl shim: record the invocation, then emit the next queued http code (one per
  # line in $CURL_CODES_FILE, popped per call), defaulting to 200 when the queue is
  # empty - so a test can script "503 503 503 then success".
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

# The alerter (local notification) is fired fire-and-forget in the background, so poll the
# alerter shim's log up to ~2s for <extended-regex> instead of racing a single grep.
wait_for_alerter() {
  local _
  for _ in $(seq 1 20); do
    grep -qiE "$1" "$ALERTER_LOG" 2>/dev/null && return 0
    sleep 0.1
  done
  echo "expected the alerter log to match /$1/ within 2s: $(cat "$ALERTER_LOG" 2>/dev/null)" >&2
  return 1
}

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

# Log-only means zero delivery - no page, no #osquery line, nothing dispatched.
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

# The one digest send used an empty sound (silent, non-interruptive - not a page).
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
