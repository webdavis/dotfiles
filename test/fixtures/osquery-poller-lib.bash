#!/usr/bin/env bash
# Test harness (makeSUT) for the security-posture poller
# (dot_local/libexec/osquery/executable_firewall-gatekeeper-monitor.sh).
#
# The poller runs as a gui/501 user LaunchAgent every 60s: it reads the live
# firewall (alf), Gatekeeper, and screen-lock posture through osqueryi and
# persists it as an owner-only baseline. This harness stands the poller up in
# isolation and records what it does through two recording spies:
#
#   - a programmable, recording osqueryi stub: it appends the SQL it was handed
#     to $POLLER_OSQUERYI_QUERY and a marker per call to $POLLER_OSQUERYI_CALLS,
#     then prints $POLLER_OSQUERYI_JSON, so a test sets a known posture and can
#     prove BOTH what the poller asked for and what it read, with no real
#     osquery/launchd dependency; and
#   - a recording send_alert spy, installed as a stand-in dispatch library at the
#     new libexec path the poller sources ($HOME/.local/libexec/osquery/
#     alert-dispatch.sh). It never delivers; it records each call's argv and
#     whether the baseline already existed at the moment of the call, so a test
#     can prove the poller stays silent AND that any page fires only AFTER the
#     baseline is written.
#
# A fresh temp HOME keeps every run off the operator's real ~/.local/state and
# ~/.local/libexec. Sourced by the poller suite; no main.

# BATS_TEST_DIRNAME under bats; the lib's own location when a plain suite *.sh
# sources this file directly (both are exactly two levels below the repo root).
POLLER_TOOL="${BATS_TEST_DIRNAME:-$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)}/../../dot_local/libexec/osquery/executable_firewall-gatekeeper-monitor.sh"

# set_posture <json-array> -- the JSON array of row objects the osqueryi stub
# returns. osquery --json emits an array and the poller reads .[0]; scalars are
# strings, matching osquery's JSON output for these integer columns.
set_posture() {
  export POLLER_OSQUERYI_JSON="$1"
}

# set_posture_controls <json-array> -- the declared-controls file the poller
# reads (normally the chezmoi render of macos_posture_controls.yaml). The
# harness default is an EMPTY declaration; a posture-controls test installs its
# own records.
set_posture_controls() {
  printf '%s\n' "$1" >"$OSQUERY_POSTURE_CONTROLS"
}

setup_poller_harness() {
  export POLLER_HOME
  POLLER_HOME="$(mktemp -d)"
  # Ownership marker set only after our own mktemp, so teardown removes this
  # path and never a pre-set or inherited POLLER_HOME.
  _POLLER_HARNESS_OWNED_DIR="$POLLER_HOME"

  mkdir -p "$POLLER_HOME/bin" \
    "$POLLER_HOME/.local/libexec/osquery" \
    "$POLLER_HOME/.local/state"

  # The env-overridable baseline path, under the sandbox HOME.
  export OSQUERY_POSTURE_STATE="$POLLER_HOME/.local/state/osquery-posture-state.json"

  # Recording osqueryi stub: log the query and a per-call marker, then print the
  # programmed posture. It ignores the SQL for its OUTPUT (the test drives the
  # posture directly) but RECORDS it so a test can assert the read shape.
  export POLLER_OSQUERYI_QUERY="$POLLER_HOME/osqueryi-query.log"
  export POLLER_OSQUERYI_CALLS="$POLLER_HOME/osqueryi-calls.log"
  : >"$POLLER_OSQUERYI_QUERY"
  : >"$POLLER_OSQUERYI_CALLS"
  cat >"$POLLER_HOME/bin/osqueryi" <<'SHIM'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$POLLER_OSQUERYI_QUERY"
printf 'call\n' >>"$POLLER_OSQUERYI_CALLS"
# POLLER_OSQUERYI_SLEEP models a wedged or slow query. exec into sleep so this is
# a SINGLE process (no child holding stdout open): a timeout kill then closes the
# pipe at the bound. It never emits, so a bounded poller reads empty and gaps;
# an unbounded poller blocks until the sleep ends.
if [[ -n ${POLLER_OSQUERYI_SLEEP:-} ]]; then
  exec sleep "$POLLER_OSQUERYI_SLEEP"
fi
# POLLER_OSQUERYI_EXIT models a hard osqueryi failure: a non-zero value means no
# stdout and that exit status (a missing binary, or the daemon not up on a fresh
# boot). Zero, the default, prints the programmed posture.
exit_code="${POLLER_OSQUERYI_EXIT:-0}"
if [[ $exit_code -ne 0 ]]; then
  exit "$exit_code"
fi
# POLLER_OSQUERYI_EXIT_AFTER_OUTPUT models a FAILED query that still printed
# healthy-looking JSON (rows emitted, then a nonzero death). A failed probe's
# output is untrustworthy; the poller must refuse it, not baseline it.
if [[ -n ${POLLER_OSQUERYI_EXIT_AFTER_OUTPUT:-} ]]; then
  printf '%s\n' "${POLLER_OSQUERYI_JSON:-[]}"
  exit "$POLLER_OSQUERYI_EXIT_AFTER_OUTPUT"
fi
printf '%s\n' "${POLLER_OSQUERYI_JSON:-[]}"
SHIM
  chmod +x "$POLLER_HOME/bin/osqueryi"
  export POLLER_OSQUERYI="$POLLER_HOME/bin/osqueryi"

  # The declared-controls file. Default: an EMPTY declaration (no extra
  # controls), so the legacy firewall/Gatekeeper/screen-lock tests exercise
  # exactly the pre-controls surface; posture-controls tests install records
  # via set_posture_controls.
  export OSQUERY_POSTURE_CONTROLS="$POLLER_HOME/posture-controls.json"
  printf '[]\n' >"$OSQUERY_POSTURE_CONTROLS"

  # Recording, programmable probe stubs for the declared-control readers, plus
  # always-refuse spies for tools the poller must never touch. EVERY invocation
  # lands in $POLLER_PROBE_CALLS ("tool argv"); any argv that is not the exact
  # read-only status query lands in $POLLER_MUTATION_LOG and fails with exit
  # 97. The poller must never invoke a mutating command, and
  # assert_no_mutation_attempt pins that the violation log stayed empty.
  export POLLER_PROBE_CALLS="$POLLER_HOME/probe-calls.log"
  export POLLER_MUTATION_LOG="$POLLER_HOME/mutation-violations.log"
  : >"$POLLER_PROBE_CALLS"
  : >"$POLLER_MUTATION_LOG"

  # fdesetup: only `status` is legitimate. Output FIRST, then the exit status:
  # the indeterminate-on-nonzero tests need a probe that prints healthy-looking
  # text AND fails. ${VAR-default} (not :-) so a test can program deliberately
  # empty output. POLLER_FDESETUP_SLEEP models a WEDGED tool exactly like the
  # osqueryi stub's hook: exec into sleep (a single process, no child holding
  # stdout), so a bounded poller kills it at the bound and gaps while an
  # unbounded poller blocks. Every probe stub below carries the same hook.
  cat >"$POLLER_HOME/bin/fdesetup" <<'SHIM'
#!/usr/bin/env bash
printf 'fdesetup %s\n' "$*" >>"$POLLER_PROBE_CALLS"
if [[ "$*" != "status" ]]; then
  printf 'fdesetup %s\n' "$*" >>"$POLLER_MUTATION_LOG"
  exit 97
fi
if [[ -n ${POLLER_FDESETUP_SLEEP:-} ]]; then
  exec sleep "$POLLER_FDESETUP_SLEEP"
fi
printf '%s\n' "${POLLER_FDESETUP_OUTPUT-FileVault is On.}"
exit "${POLLER_FDESETUP_EXIT:-0}"
SHIM
  chmod +x "$POLLER_HOME/bin/fdesetup"

  # csrutil: only `status`. Default matches the repo declaration (SIP is
  # deliberately disabled on this machine, expect: disabled).
  cat >"$POLLER_HOME/bin/csrutil" <<'SHIM'
#!/usr/bin/env bash
printf 'csrutil %s\n' "$*" >>"$POLLER_PROBE_CALLS"
if [[ "$*" != "status" ]]; then
  printf 'csrutil %s\n' "$*" >>"$POLLER_MUTATION_LOG"
  exit 97
fi
if [[ -n ${POLLER_CSRUTIL_SLEEP:-} ]]; then
  exec sleep "$POLLER_CSRUTIL_SLEEP"
fi
printf '%s\n' "${POLLER_CSRUTIL_OUTPUT-System Integrity Protection status: disabled.}"
exit "${POLLER_CSRUTIL_EXIT:-0}"
SHIM
  chmod +x "$POLLER_HOME/bin/csrutil"

  # sysadminctl: only `-guestAccount status` (automatic login reads the
  # loginwindow DECLARATION via the defaults stub below, never sysadminctl's
  # effective state). The real tool reports on STDERR with an NSLog prefix
  # (verified on the target machine), so the stub mirrors that.
  cat >"$POLLER_HOME/bin/sysadminctl" <<'SHIM'
#!/usr/bin/env bash
printf 'sysadminctl %s\n' "$*" >>"$POLLER_PROBE_CALLS"
case "$*" in
  "-guestAccount status")
    if [[ -n ${POLLER_SYSADMINCTL_SLEEP:-} ]]; then
      exec sleep "$POLLER_SYSADMINCTL_SLEEP"
    fi
    printf '%s\n' "${POLLER_SYSADMINCTL_GUEST_OUTPUT-2026-07-27 00:00:00.000 sysadminctl[100:100] Guest account disabled.}" >&2
    exit "${POLLER_SYSADMINCTL_GUEST_EXIT:-0}"
    ;;
  *)
    printf 'sysadminctl %s\n' "$*" >>"$POLLER_MUTATION_LOG"
    exit 97
    ;;
esac
SHIM
  chmod +x "$POLLER_HOME/bin/sysadminctl"

  # defaults: only the exact read of loginwindow's autoLoginUser key (the
  # declared-intent auto-login reader) is legitimate; every other argv is a
  # recorded violation. POLLER_DEFAULTS_AUTOLOGIN_MODE models the three real
  # outcomes (each verified on the target machine): absent (the canonical
  # does-not-exist diagnostic on stderr, exit 1 -- the healthy no-declaration
  # state, and the default), present (a username on stdout, exit 0),
  # unreadable (a non-canonical failure, exit 1).
  cat >"$POLLER_HOME/bin/defaults" <<'SHIM'
#!/usr/bin/env bash
printf 'defaults %s\n' "$*" >>"$POLLER_PROBE_CALLS"
if [[ "$*" != "read /Library/Preferences/com.apple.loginwindow autoLoginUser" ]]; then
  printf 'defaults %s\n' "$*" >>"$POLLER_MUTATION_LOG"
  exit 97
fi
if [[ -n ${POLLER_DEFAULTS_SLEEP:-} ]]; then
  exec sleep "$POLLER_DEFAULTS_SLEEP"
fi
case "${POLLER_DEFAULTS_AUTOLOGIN_MODE:-absent}" in
  present)
    printf '%s\n' "${POLLER_DEFAULTS_AUTOLOGIN_USER-stephen}"
    exit 0
    ;;
  unreadable)
    printf '2026-07-27 00:00:00.000 defaults[100:100]\nCould not read domain /Library/Preferences/com.apple.loginwindow\n' >&2
    exit 1
    ;;
  *)
    printf '2026-07-27 00:00:00.000 defaults[100:100]\nThe domain/default pair of (/Library/Preferences/com.apple.loginwindow, autoLoginUser) does not exist\n' >&2
    exit 1
    ;;
esac
SHIM
  chmod +x "$POLLER_HOME/bin/defaults"

  # pgrep: only the exact user-scoped process-name read of OverSight (the
  # oversight running-state reader) is legitimate; every other argv is a
  # recorded violation. POLLER_PGREP_MODE models three outcomes (pgrep exit
  # statuses verified against the installed binary and its man page: 0
  # matched, 1 no match with no output, 2 invalid options): running (a pid on
  # stdout, exit 0, the default), stopped (no output at all, exit 1), and
  # error (a pid on stdout AND exit 2, the untrustworthy-failure form: output
  # that LOOKS running with a failed status must never be believed).
  # POLLER_PGREP_OUTPUT/POLLER_PGREP_EXIT override output and status directly
  # for the remaining forms (exit 1 that still printed something, exit 0 that
  # printed nothing); ${VAR+x} so a deliberately empty output is programmable.
  cat >"$POLLER_HOME/bin/pgrep" <<'SHIM'
#!/usr/bin/env bash
printf 'pgrep %s\n' "$*" >>"$POLLER_PROBE_CALLS"
if [[ "$*" != "-x -U $(id -u) OverSight" ]]; then
  printf 'pgrep %s\n' "$*" >>"$POLLER_MUTATION_LOG"
  exit 97
fi
if [[ -n ${POLLER_PGREP_SLEEP:-} ]]; then
  exec sleep "$POLLER_PGREP_SLEEP"
fi
if [[ -n ${POLLER_PGREP_OUTPUT+x} || -n ${POLLER_PGREP_EXIT:-} ]]; then
  if [[ -n ${POLLER_PGREP_OUTPUT:-} ]]; then
    printf '%s\n' "$POLLER_PGREP_OUTPUT"
  fi
  exit "${POLLER_PGREP_EXIT:-0}"
fi
case "${POLLER_PGREP_MODE:-running}" in
  stopped)
    exit 1
    ;;
  error)
    printf '3424\n'
    exit 2
    ;;
  *)
    printf '3424\n'
    exit 0
    ;;
esac
SHIM
  chmod +x "$POLLER_HOME/bin/pgrep"

  # Tools the poller has NO business invoking at all, status or otherwise: any
  # call is a recorded violation. run_poller prepends this bin dir to PATH, so
  # a stray PATH-resolved invocation is caught even without an env override.
  local forbidden_tool
  for forbidden_tool in sudo spctl socketfilterfw launchctl; do
    cat >"$POLLER_HOME/bin/$forbidden_tool" <<'SHIM'
#!/usr/bin/env bash
printf '%s %s\n' "$(basename "$0")" "$*" >>"$POLLER_MUTATION_LOG"
exit 97
SHIM
    chmod +x "$POLLER_HOME/bin/$forbidden_tool"
  done

  # Recording send_alert spy at the NEW dispatch path the poller sources. It
  # never delivers; it records each call's argv and the baseline as it stood at
  # call time, so a test can prove the ordering (notify-before-persist).
  export POLLER_SEND_ALERT_LOG="$POLLER_HOME/send-alert.log"
  export POLLER_SEND_ALERT_SEVERITY="$POLLER_HOME/send-alert-severity.log"
  export POLLER_SEND_ALERT_STATE_AT_CALL="$POLLER_HOME/send-alert-state-at-call.log"
  : >"$POLLER_SEND_ALERT_LOG"
  : >"$POLLER_SEND_ALERT_SEVERITY"
  : >"$POLLER_SEND_ALERT_STATE_AT_CALL"
  cat >"$POLLER_HOME/.local/libexec/osquery/alert-dispatch.sh" <<'SHIM'
# shellcheck shell=bash
send_alert() {
  # Severity (arg 1) on its own line: one line per call, so a test counts pages.
  printf '%s\n' "${1:-}" >>"$POLLER_SEND_ALERT_SEVERITY"
  # Full argv (severity, title, body, sound) for body/naming assertions.
  printf '%s\n' "$*" >>"$POLLER_SEND_ALERT_LOG"
  # The baseline as it stood when the page fired, so a test can prove the ordering
  # (notify-before-persist: the baseline still holds the PRIOR value).
  if [[ -f ${OSQUERY_POSTURE_STATE:-/nonexistent} ]]; then
    cat "$OSQUERY_POSTURE_STATE" >>"$POLLER_SEND_ALERT_STATE_AT_CALL"
  fi
  # POLLER_SEND_ALERT_EXIT models a dispatch that could NOT durably queue the
  # page (nonzero). Default 0 (queued): the poller then advances the baseline.
  return "${POLLER_SEND_ALERT_EXIT:-0}"
}
SHIM

  # Default posture: all protections ON (healthy). A test overrides via
  # set_posture before running the poller.
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
}

teardown_poller_harness() {
  [[ -n ${_POLLER_HARNESS_OWNED_DIR:-} ]] || return 0
  rm -rf "$_POLLER_HARNESS_OWNED_DIR"
  unset _POLLER_HARNESS_OWNED_DIR
}

# run_poller [args...] -- run the poller under the harness env. HOME is the temp
# home so the sourced dispatch spy and the default state path resolve inside the
# sandbox; OSQUERYI and the posture-probe overrides point at the recording
# stubs, and the stub bin dir leads PATH so any stray PATH-resolved tool call
# hits a spy instead of the real system.
run_poller() {
  HOME="$POLLER_HOME" \
    PATH="$POLLER_HOME/bin:$PATH" \
    OSQUERYI="$POLLER_OSQUERYI" \
    OSQUERY_POSTURE_STATE="$OSQUERY_POSTURE_STATE" \
    OSQUERY_POSTURE_CONTROLS="$OSQUERY_POSTURE_CONTROLS" \
    OSQUERY_POSTURE_FDESETUP="$POLLER_HOME/bin/fdesetup" \
    OSQUERY_POSTURE_CSRUTIL="$POLLER_HOME/bin/csrutil" \
    OSQUERY_POSTURE_SYSADMINCTL="$POLLER_HOME/bin/sysadminctl" \
    OSQUERY_POSTURE_DEFAULTS="$POLLER_HOME/bin/defaults" \
    OSQUERY_POSTURE_PGREP="$POLLER_HOME/bin/pgrep" \
    bash "$POLLER_TOOL" "$@"
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

# assert_osqueryi_call_count <n> -- the poller invoked osqueryi exactly <n>
# times (one combined query per tick, not one per protection).
assert_osqueryi_call_count() {
  local count
  count=$(wc -l <"$POLLER_OSQUERYI_CALLS") # one marker line per invocation
  count=${count//[[:space:]]/}
  if [[ $count -ne $1 ]]; then
    printf 'expected %s osqueryi call(s), got %s\n' "$1" "$count" >&2
    return 1
  fi
}

# assert_query_reads <substring> -- the recorded osqueryi query contains
# <substring> (a table/column the combined read must ask for).
assert_query_reads() {
  if ! grep -qF -- "$1" "$POLLER_OSQUERYI_QUERY"; then
    printf 'expected the osqueryi query to read %s; query was:\n%s\n' \
      "$1" "$(cat "$POLLER_OSQUERYI_QUERY")" >&2
    return 1
  fi
}

# assert_baseline_scalar <key> <value> -- the persisted baseline's <key> equals
# <value> (a JSON scalar, string-typed as osquery emits).
assert_baseline_scalar() {
  local got
  got=$(jq -r --arg k "$1" '.[$k] // empty' "$OSQUERY_POSTURE_STATE" 2>/dev/null || echo "")
  if [[ $got != "$2" ]]; then
    printf 'expected baseline .%s == %s, got %q; baseline:\n%s\n' \
      "$1" "$2" "$got" "$(cat "$OSQUERY_POSTURE_STATE" 2>/dev/null || echo '(no file)')" >&2
    return 1
  fi
}

# assert_no_page -- the poller did not call send_alert at all (silent).
assert_no_page() {
  if [[ -s $POLLER_SEND_ALERT_LOG ]]; then
    printf 'expected NO page, but send_alert was called:\n%s\n' \
      "$(cat "$POLLER_SEND_ALERT_LOG")" >&2
    return 1
  fi
}

# seed_baseline <compact-json-object> -- write a known-good posture baseline at
# 0600, so a sad-path test can prove a failed or empty read leaves it untouched.
seed_baseline() {
  mkdir -p "$(dirname "$OSQUERY_POSTURE_STATE")"
  printf '%s\n' "$1" >"$OSQUERY_POSTURE_STATE"
  chmod 600 "$OSQUERY_POSTURE_STATE"
}

# snapshot_baseline -- copy the current baseline aside so assert_baseline_unchanged
# can compare byte-for-byte after a run.
snapshot_baseline() {
  cp "$OSQUERY_POSTURE_STATE" "$POLLER_HOME/baseline.snapshot"
}

# assert_baseline_unchanged -- the baseline is byte-for-byte identical to the last
# snapshot (a failed or empty read must neither clobber nor blank it).
assert_baseline_unchanged() {
  if ! cmp -s "$POLLER_HOME/baseline.snapshot" "$OSQUERY_POSTURE_STATE"; then
    printf 'expected the baseline byte-for-byte preserved.\nsnapshot:\n%s\nnow:\n%s\n' \
      "$(cat "$POLLER_HOME/baseline.snapshot" 2>/dev/null || echo '(no snapshot)')" \
      "$(cat "$OSQUERY_POSTURE_STATE" 2>/dev/null || echo '(missing)')" >&2
    return 1
  fi
}

# assert_page_count <n> -- send_alert was called exactly <n> times (one severity
# line per call; the body may span many lines, so the severity log is the count).
assert_page_count() {
  local count
  count=$(wc -l <"$POLLER_SEND_ALERT_SEVERITY")
  count=${count//[[:space:]]/}
  if [[ $count -ne $1 ]]; then
    printf 'expected %s page(s), got %s; send_alert log:\n%s\n' \
      "$1" "$count" "$(cat "$POLLER_SEND_ALERT_LOG")" >&2
    return 1
  fi
}

# assert_page_severity_is <severity> -- a page fired and every page carried
# <severity> (only a CRIT reaches the #priority webhook, so the severity arg is
# the security-relevant one, not just the title text).
assert_page_severity_is() {
  if [[ ! -s $POLLER_SEND_ALERT_SEVERITY ]]; then
    printf 'expected a %s page, but send_alert was never called\n' "$1" >&2
    return 1
  fi
  if grep -qvxF "$1" "$POLLER_SEND_ALERT_SEVERITY"; then
    printf 'expected every page at severity %s, got:\n%s\n' \
      "$1" "$(cat "$POLLER_SEND_ALERT_SEVERITY")" >&2
    return 1
  fi
}

# assert_page_body_has <substring> -- some page's argv contained <substring> (the
# body names which protection turned off, or its prior state text).
assert_page_body_has() {
  if ! grep -qF -- "$1" "$POLLER_SEND_ALERT_LOG"; then
    printf 'expected a page naming %s; send_alert log:\n%s\n' \
      "$1" "$(cat "$POLLER_SEND_ALERT_LOG")" >&2
    return 1
  fi
}

# assert_page_body_lacks <substring> -- no page mentioned <substring> (a steady
# protection is never named in an unrelated transition's page).
assert_page_body_lacks() {
  if grep -qF -- "$1" "$POLLER_SEND_ALERT_LOG"; then
    printf 'expected NO page mentioning %s; send_alert log:\n%s\n' \
      "$1" "$(cat "$POLLER_SEND_ALERT_LOG")" >&2
    return 1
  fi
}

# assert_page_saw_baseline <compact-json> -- at the moment send_alert fired, the
# persisted baseline already held <compact-json>, proving write_state ran before
# the page (the ordering that lets a slow alerter never double-page off a stale
# baseline).
assert_page_saw_baseline() {
  if ! grep -qF -- "$1" "$POLLER_SEND_ALERT_STATE_AT_CALL"; then
    printf 'expected the baseline at page time to be %s; saw:\n%s\n' \
      "$1" "$(cat "$POLLER_SEND_ALERT_STATE_AT_CALL" 2>/dev/null || echo '(none)')" >&2
    return 1
  fi
}

# assert_gap_marker -- the page-once monitoring-gap marker (STATE.gap) exists.
assert_gap_marker() {
  if [[ ! -f $OSQUERY_POSTURE_STATE.gap ]]; then
    printf 'expected the gap marker %s.gap to exist, but it does not\n' "$OSQUERY_POSTURE_STATE" >&2
    return 1
  fi
}

# assert_no_gap_marker -- the monitoring-gap marker does not exist (never paged,
# or cleared on recovery).
assert_no_gap_marker() {
  if [[ -f $OSQUERY_POSTURE_STATE.gap ]]; then
    printf 'expected NO gap marker, but %s.gap exists\n' "$OSQUERY_POSTURE_STATE" >&2
    return 1
  fi
}

# assert_no_baseline -- no baseline file exists (a first-observation page whose
# send_alert failed must not have seeded one).
assert_no_baseline() {
  if [[ -f $OSQUERY_POSTURE_STATE ]]; then
    printf 'expected NO baseline file, but %s exists with:\n%s\n' \
      "$OSQUERY_POSTURE_STATE" "$(cat "$OSQUERY_POSTURE_STATE")" >&2
    return 1
  fi
}

# assert_probe_calls <tool> <n> -- the poller invoked the stubbed <tool>
# exactly <n> times (each stub logs "tool argv", one line per call).
assert_probe_calls() {
  local count
  count=$(grep -c "^$1 " "$POLLER_PROBE_CALLS" || true)
  if [[ $count -ne $2 ]]; then
    printf 'expected %s call(s) to %s, got %s; probe log:\n%s\n' \
      "$2" "$1" "$count" "$(cat "$POLLER_PROBE_CALLS")" >&2
    return 1
  fi
}

# assert_probe_argv <exact-line> <n> -- the probe log holds exactly <n> lines
# matching "<tool> <argv>" WHOLE. A per-member property, not a per-tool count:
# a bare count would pass under a doubled probe of one subcommand that starved
# another (e.g. autologin probed twice, guest never).
assert_probe_argv() {
  local count
  count=$(grep -cxF -- "$1" "$POLLER_PROBE_CALLS" || true)
  if [[ $count -ne $2 ]]; then
    printf 'expected %s exact call(s) of [%s], got %s; probe log:\n%s\n' \
      "$2" "$1" "$count" "$(cat "$POLLER_PROBE_CALLS")" >&2
    return 1
  fi
}

# assert_no_probe_calls -- the poller invoked no probe stub at all (a refused
# controls file must be rejected BEFORE any read runs).
assert_no_probe_calls() {
  if [[ -s $POLLER_PROBE_CALLS ]]; then
    printf 'expected NO probe invocation, but the stubs recorded:\n%s\n' \
      "$(cat "$POLLER_PROBE_CALLS")" >&2
    return 1
  fi
}

# assert_no_mutation_attempt -- no stubbed tool ever saw a non-status argv, and
# no always-refuse spy (sudo, spctl, defaults, socketfilterfw, launchctl) was
# invoked at all: the poller never runs a mutating command.
assert_no_mutation_attempt() {
  if [[ -s $POLLER_MUTATION_LOG ]]; then
    printf 'expected NO mutating invocation, but the spies recorded:\n%s\n' \
      "$(cat "$POLLER_MUTATION_LOG")" >&2
    return 1
  fi
}
