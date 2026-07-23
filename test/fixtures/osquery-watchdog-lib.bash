# Test harness for the uptime watchdog
# (dot_local/libexec/osquery/executable_uptime-watchdog.sh).
#
# The watchdog runs as a user LaunchAgent every 15 min. It asserts the osquery
# notification pipeline is ALIVE (a dead pipeline looks identical to "all quiet"),
# and pages ONE CRIT if any component is down or wedged, silent when healthy. It
# has four probes: osqueryd present-and-answering, every OTHER osquery LaunchAgent
# loaded-and-not-crash-looping, the hermes #priority route reachable, and the
# delivery backlog (dead-letter count and a sustained pending-growth streak).
#
# This harness stands the watchdog up in isolation against recording spies, with
# no real launchd / osquery / hermes dependency:
#
#   - stub `pgrep`, `launchctl`, `curl` on a sandbox PATH, plus a stub `osqueryi`
#     via OSQUERYI. The launchctl stub answers `print gui/<uid>/<label>` from a
#     per-agent spec dir (a `runs` counter and a `last exit code`), or exits
#     nonzero for an unloaded agent, so a test programs each agent's launchd state;
#   - a recording send_alert spy plus the two read-only queue counters, installed
#     as a stand-in dispatch library at the libexec path the watchdog sources.
#     send_alert never delivers; it records each call's severity, sound, and argv,
#     and the state file as it stood at call time (to prove notify-before-persist).
#
# A fresh temp HOME keeps every run off the operator's real ~/.local/state and
# ~/.local/libexec. Sourced by the watchdog suite; no main.

WD_TOOL="${BATS_TEST_DIRNAME}/../../dot_local/libexec/osquery/executable_uptime-watchdog.sh"

# The six watched agents (every deployed osquery LaunchAgent except the watchdog
# itself). Kept here so a test can iterate them; the watchdog owns its own copy.
WD_WATCHED_AGENTS=(
  "com.webdavis.osquery-results-alerter"
  "com.webdavis.osquery-firewall-gatekeeper-monitor"
  "com.webdavis.osquery-alert-drainer"
  "com.webdavis.osquery-digest"
  "com.webdavis.osquery-heartbeat"
  "com.webdavis.osquery-tailscale-monitor"
)

setup_watchdog_harness() {
  export WD_HOME
  WD_HOME="$(mktemp -d)"
  # Ownership marker set only after our own mktemp, so teardown removes this path
  # and never a pre-set or inherited WD_HOME.
  _WD_HARNESS_OWNED_DIR="$WD_HOME"

  mkdir -p "$WD_HOME/bin" \
    "$WD_HOME/.local/libexec/osquery" \
    "$WD_HOME/.local/state" \
    "$WD_HOME/agents"

  export WD_AGENTS_DIR="$WD_HOME/agents"
  export OSQUERY_WATCHDOG_STATE="$WD_HOME/.local/state/osquery-watchdog-state.json"

  # Default programmable knobs (a healthy pipeline): osqueryd running and
  # answering, the #priority route present (405 to a GET), an empty queue.
  export WATCHDOG_OSQUERYD_RUNNING=1
  export WATCHDOG_OSQUERYI_OK=1
  export WATCHDOG_HTTP_CODE=405
  export WATCHDOG_PENDING_COUNT=0
  export WATCHDOG_DEAD_LETTER_COUNT=0
  export WD_SEND_ALERT_EXIT=0

  # pgrep stub: the watchdog calls `pgrep -fq '<osqueryd pattern>'`. Exit 0 when
  # osqueryd is "running", nonzero otherwise.
  cat >"$WD_HOME/bin/pgrep" <<'SHIM'
#!/usr/bin/env bash
[[ ${WATCHDOG_OSQUERYD_RUNNING:-1} == 1 ]] && exit 0
exit 1
SHIM

  # osqueryi stub (via OSQUERYI): a one-shot query succeeds (answering) or fails
  # (a wedged daemon that passes pgrep but cannot answer).
  cat >"$WD_HOME/bin/osqueryi" <<'SHIM'
#!/usr/bin/env bash
[[ ${WATCHDOG_OSQUERYI_OK:-1} == 1 ]] && exit 0
exit 1
SHIM

  # launchctl stub: only `print gui/<uid>/<label>` is used. Each loaded agent has a
  # `<label>.runs` file (the launchd run counter) and an optional `<label>.exit`
  # file (the raw last-exit-code line value, so an injection test can plant a
  # hostile string). An absent `.runs` file models an UNLOADED agent (exit 113,
  # the real not-found status). The stub prints the two fields the watchdog reads.
  cat >"$WD_HOME/bin/launchctl" <<'SHIM'
#!/usr/bin/env bash
if [[ ${1:-} == print ]]; then
  target="${2:-}"
  label="${target##*/}"
  runs_file="$WD_AGENTS_DIR/$label.runs"
  exit_file="$WD_AGENTS_DIR/$label.exit"
  [[ -f $runs_file ]] || exit 113
  printf '\tstate = not running\n'
  printf '\truns = %s\n' "$(cat "$runs_file")"
  if [[ -f $exit_file ]]; then
    printf '\tlast exit code = %s\n' "$(cat "$exit_file")"
  else
    printf '\tlast exit code = 0\n'
  fi
  exit 0
fi
exit 0
SHIM

  # curl stub: the route probe is `curl -s -o /dev/null -w '%{http_code}' ... URL`.
  # Record the argv (so a test can prove NO signing header / secret is on the wire)
  # and print the programmed HTTP code, exactly as the real -w would.
  export WD_CURL_ARGS="$WD_HOME/curl-args.log"
  : >"$WD_CURL_ARGS"
  cat >"$WD_HOME/bin/curl" <<'SHIM'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$WD_CURL_ARGS"
printf '%s' "${WATCHDOG_HTTP_CODE:-405}"
SHIM

  chmod +x "$WD_HOME/bin/pgrep" "$WD_HOME/bin/osqueryi" \
    "$WD_HOME/bin/launchctl" "$WD_HOME/bin/curl"

  # Recording send_alert spy plus the two public read-only queue counters, at the
  # NEW dispatch path the watchdog sources. send_alert records each call's severity
  # (one line per call, so a test counts pages), sound (the page/muted tier), full
  # argv (for body assertions), and the state file as it stood at call time (to
  # prove notify-before-persist). The counters echo the programmed values; a
  # non-numeric value models an unreadable count (the fail-safe path).
  export WD_SEND_ALERT_SEVERITY="$WD_HOME/send-alert-severity.log"
  export WD_SEND_ALERT_SOUND="$WD_HOME/send-alert-sound.log"
  export WD_SEND_ALERT_LOG="$WD_HOME/send-alert.log"
  export WD_SEND_ALERT_STATE_AT_CALL="$WD_HOME/send-alert-state-at-call.log"
  : >"$WD_SEND_ALERT_SEVERITY"
  : >"$WD_SEND_ALERT_SOUND"
  : >"$WD_SEND_ALERT_LOG"
  : >"$WD_SEND_ALERT_STATE_AT_CALL"
  cat >"$WD_HOME/.local/libexec/osquery/alert-dispatch.sh" <<'SHIM'
# shellcheck shell=bash
send_alert() {
  printf '%s\n' "${1:-}" >>"$WD_SEND_ALERT_SEVERITY"
  printf '%s\n' "${4:-}" >>"$WD_SEND_ALERT_SOUND"
  printf '%s\n' "$*" >>"$WD_SEND_ALERT_LOG"
  if [[ -f ${OSQUERY_WATCHDOG_STATE:-/nonexistent} ]]; then
    cat "$OSQUERY_WATCHDOG_STATE" >>"$WD_SEND_ALERT_STATE_AT_CALL"
  else
    printf '(no-state-file)\n' >>"$WD_SEND_ALERT_STATE_AT_CALL"
  fi
  return "${WD_SEND_ALERT_EXIT:-0}"
}
osquery_pending_alert_count() { printf '%s' "${WATCHDOG_PENDING_COUNT:-0}"; }
osquery_dead_letter_count() { printf '%s' "${WATCHDOG_DEAD_LETTER_COUNT:-0}"; }
SHIM

  # Default: every watched agent loaded, one clean run each.
  local label
  for label in "${WD_WATCHED_AGENTS[@]}"; do
    set_agent "$label" 1 0
  done
}

teardown_watchdog_harness() {
  [[ -n ${_WD_HARNESS_OWNED_DIR:-} ]] || return 0
  rm -rf "$_WD_HARNESS_OWNED_DIR"
  unset _WD_HARNESS_OWNED_DIR
}

# ---- programming the launchd state ----------------------------------------

# set_agent <label> <runs> [exit-code] -- the agent is loaded; launchctl print
# reports <runs> and the given last exit code (default 0).
set_agent() {
  printf '%s' "$2" >"$WD_AGENTS_DIR/$1.runs"
  printf '%s' "${3:-0}" >"$WD_AGENTS_DIR/$1.exit"
}

# set_agent_raw_exit <label> <runs> <raw-exit-line> -- like set_agent but the raw
# text after "last exit code = " is planted verbatim (an injection payload), so a
# test proves the watchdog extracts only the validated number, never the raw line.
set_agent_raw_exit() {
  printf '%s' "$2" >"$WD_AGENTS_DIR/$1.runs"
  printf '%s' "$3" >"$WD_AGENTS_DIR/$1.exit"
}

# unload_agent <label> -- the agent is not loaded (launchctl print exits nonzero).
unload_agent() {
  rm -f "$WD_AGENTS_DIR/$1.runs" "$WD_AGENTS_DIR/$1.exit"
}

# ---- programming prior cross-run state ------------------------------------

# seed_watchdog_state <compact-json> -- write the prior state file at 0600, so a
# streak/growth test starts from a known baseline.
seed_watchdog_state() {
  mkdir -p "$(dirname "$OSQUERY_WATCHDOG_STATE")"
  printf '%s\n' "$1" >"$OSQUERY_WATCHDOG_STATE"
  chmod 600 "$OSQUERY_WATCHDOG_STATE"
}

# ---- running ---------------------------------------------------------------

# run_watchdog -- run the watchdog under the sandbox env. HOME is the temp home so
# the sourced dispatch spy and the default state path resolve inside the sandbox;
# the stub bin dir shadows pgrep/launchctl/curl; OSQUERYI points at the stub.
run_watchdog() {
  HOME="$WD_HOME" \
    PATH="$WD_HOME/bin:$PATH" \
    OSQUERYI="$WD_HOME/bin/osqueryi" \
    OSQUERY_HERMES_PRIORITY_URL="http://127.0.0.1:8644/webhooks/osquery-priority" \
    OSQUERY_WATCHDOG_STATE="$OSQUERY_WATCHDOG_STATE" \
    bash "$WD_TOOL"
}

# ---- assertions ------------------------------------------------------------

# refute_file_contains <fixed-substring> <file> -- fail (return 1) when the
# substring (fixed, case-insensitive) appears in the file. The robust NEGATIVE
# assertion: a bare `! grep` is exempted from set -e in bats and silently no-ops,
# so a plain function whose nonzero return set -e DOES catch closes that gap.
refute_file_contains() {
  if grep -qiF -- "$1" "$2"; then
    printf 'expected %q NOT to appear in %s, but it does:\n%s\n' "$1" "$2" "$(cat "$2")" >&2
    return 1
  fi
  return 0
}

# assert_mode <octal> <path> -- the path carries the expected permission bits.
# GNU stat first (the nix shell), BSD stat as the fallback (the portable order).
assert_mode() {
  local mode
  mode=$(stat -c '%a' "$2" 2>/dev/null || stat -f '%Lp' "$2" 2>/dev/null)
  if [[ $mode != "$1" ]]; then
    printf 'expected mode %s on %s, got %s\n' "$1" "$2" "$mode" >&2
    return 1
  fi
}

# assert_no_page -- the watchdog did not call send_alert at all (silent).
assert_no_page() {
  if [[ -s $WD_SEND_ALERT_LOG ]]; then
    printf 'expected NO page, but send_alert was called:\n%s\n' \
      "$(cat "$WD_SEND_ALERT_LOG")" >&2
    return 1
  fi
}

# assert_page_count <n> -- send_alert was called exactly <n> times (one severity
# line per call; the body may span many lines, so the severity log is the count).
assert_page_count() {
  local count
  count=$(wc -l <"$WD_SEND_ALERT_SEVERITY")
  count=${count//[[:space:]]/}
  if [[ $count -ne $1 ]]; then
    printf 'expected %s page(s), got %s; send_alert log:\n%s\n' \
      "$1" "$count" "$(cat "$WD_SEND_ALERT_LOG")" >&2
    return 1
  fi
}

# assert_page_severity_is <severity> -- a page fired and every page carried
# <severity> (only a CRIT reaches the #priority webhook, so the severity arg is the
# security-relevant one, not just the title text).
assert_page_severity_is() {
  if [[ ! -s $WD_SEND_ALERT_SEVERITY ]]; then
    printf 'expected a %s page, but send_alert was never called\n' "$1" >&2
    return 1
  fi
  if grep -qvxF "$1" "$WD_SEND_ALERT_SEVERITY"; then
    printf 'expected every page at severity %s, got:\n%s\n' \
      "$1" "$(cat "$WD_SEND_ALERT_SEVERITY")" >&2
    return 1
  fi
}

# assert_page_sound_nonempty -- a page fired and every page carried a NON-EMPTY
# sound (the page tier). An empty sound is the muted tier: a down-pipeline page
# must ping, never mute.
assert_page_sound_nonempty() {
  if [[ ! -s $WD_SEND_ALERT_SOUND ]]; then
    printf 'expected a page with a non-empty sound, but send_alert was never called\n' >&2
    return 1
  fi
  if grep -qxF '' "$WD_SEND_ALERT_SOUND"; then
    printf 'expected every page to carry a non-empty sound (page tier), got a muted call:\n%s\n' \
      "$(sed 's/^$/<empty>/' "$WD_SEND_ALERT_SOUND")" >&2
    return 1
  fi
  return 0
}

# assert_page_body_has <substring> -- some page's argv contained <substring>.
assert_page_body_has() {
  if ! grep -qF -- "$1" "$WD_SEND_ALERT_LOG"; then
    printf 'expected a page naming %s; send_alert log:\n%s\n' \
      "$1" "$(cat "$WD_SEND_ALERT_LOG")" >&2
    return 1
  fi
}

# assert_curl_probe_unsigned -- the recorded route-probe argv carries NO webhook
# signing header and no secret: it is a bare reachability GET, so the HMAC key can
# never leak onto the wire through it.
assert_curl_probe_unsigned() {
  if [[ ! -s $WD_CURL_ARGS ]]; then
    printf 'expected the route probe to call curl, but it did not\n' >&2
    return 1
  fi
  if grep -qiE 'X-Webhook-Signature|X-Request-ID|Authorization' "$WD_CURL_ARGS"; then
    printf 'expected the route probe to carry NO signing header, but the argv has one:\n%s\n' \
      "$(cat "$WD_CURL_ARGS")" >&2
    return 1
  fi
  return 0
}

# assert_file_absent <path> -- the path does not exist (an injection payload must
# not have executed and created its marker file).
assert_file_absent() {
  if [[ -e $1 ]]; then
    printf 'expected %s NOT to exist, but it does\n' "$1" >&2
    return 1
  fi
}

# snapshot_watchdog_state / assert_watchdog_state_unchanged -- copy the state file
# aside, then prove a run left it byte-for-byte identical (notify-before-persist: a
# page that could not be queued must not advance the persisted baseline).
snapshot_watchdog_state() {
  cp "$OSQUERY_WATCHDOG_STATE" "$WD_HOME/state.snapshot"
}

assert_watchdog_state_unchanged() {
  if ! cmp -s "$WD_HOME/state.snapshot" "$OSQUERY_WATCHDOG_STATE"; then
    printf 'expected the state byte-for-byte preserved.\nsnapshot:\n%s\nnow:\n%s\n' \
      "$(cat "$WD_HOME/state.snapshot" 2>/dev/null || echo '(no snapshot)')" \
      "$(cat "$OSQUERY_WATCHDOG_STATE" 2>/dev/null || echo '(missing)')" >&2
    return 1
  fi
}

# assert_state_has <substring> -- the persisted state file contains <substring>.
assert_state_has() {
  if ! grep -qF -- "$1" "$OSQUERY_WATCHDOG_STATE" 2>/dev/null; then
    printf 'expected the state to contain %s; state:\n%s\n' \
      "$1" "$(cat "$OSQUERY_WATCHDOG_STATE" 2>/dev/null || echo '(no state file)')" >&2
    return 1
  fi
}
