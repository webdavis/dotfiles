#!/usr/bin/env bash
# Test harness (makeSUT) for the public-exposure monitor
# (dot_local/libexec/osquery/executable_tailscale-monitor.sh).
#
# The monitor runs as a user LaunchAgent every 60s: it reads
# `tailscale funnel status --json`, classifies whether a Funnel is exposing a
# local service to the PUBLIC internet, compares against the previous run's
# baseline, and pages CRIT only on an off->on transition (or a first-observation
# active, or a monitoring gap). This harness stands the monitor up in isolation
# and records what it does through two recording spies:
#
#   - a programmable, recording `tailscale` stub: it appends the argv it was
#     handed to $TS_TAILSCALE_ARGS, then prints $TAILSCALE_FUNNEL_JSON and exits
#     $TAILSCALE_FUNNEL_RC, so a test sets a known funnel status (or forces a
#     command failure) with no real tailscale/launchd dependency; and
#   - a recording send_alert spy, installed as a stand-in dispatch library at the
#     new libexec path the monitor sources ($HOME/.local/libexec/osquery/
#     alert-dispatch.sh). It never delivers; it records each call's severity,
#     sound, and argv, and the baseline as it stood at call time, so a test can
#     prove the monitor stays silent AND that any page fires only BEFORE the
#     baseline advances (notify-before-persist).
#
# A fresh temp HOME keeps every run off the operator's real ~/.local/state and
# ~/.local/libexec. Sourced by the tailscale suite; no main.

TS_TOOL="${BATS_TEST_DIRNAME}/../../dot_local/libexec/osquery/executable_tailscale-monitor.sh"

# set_funnel <json> -- the JSON body the `tailscale funnel status --json` stub
# prints. Empty models an empty (blank) status output.
set_funnel() {
  export TAILSCALE_FUNNEL_JSON="$1"
}

# set_funnel_rc <rc> -- the exit code the `tailscale` stub returns (default 0),
# so a test can force a status-command failure.
set_funnel_rc() {
  export TAILSCALE_FUNNEL_RC="$1"
}

setup_tailscale_harness() {
  export TS_HOME
  TS_HOME="$(mktemp -d)"
  # Ownership marker set only after our own mktemp, so teardown removes this
  # path and never a pre-set or inherited TS_HOME.
  _TS_HARNESS_OWNED_DIR="$TS_HOME"

  mkdir -p "$TS_HOME/bin" \
    "$TS_HOME/.local/libexec/osquery" \
    "$TS_HOME/.local/state"

  # The env-overridable state path, under the sandbox HOME.
  export OSQUERY_TAILSCALE_STATE="$TS_HOME/.local/state/osquery-tailscale-funnel.json"

  # Recording `tailscale` stub: log the argv (so a test proves the monitor asked
  # for `funnel status --json`), then print the programmed JSON and exit the
  # programmed code. printf '%s' (no newline) keeps an empty program truly empty
  # so the empty-output gap path is exercised.
  export TS_TAILSCALE_ARGS="$TS_HOME/tailscale-args.log"
  : >"$TS_TAILSCALE_ARGS"
  cat >"$TS_HOME/bin/tailscale" <<'SHIM'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$TS_TAILSCALE_ARGS"
printf '%s' "${TAILSCALE_FUNNEL_JSON:-}"
exit "${TAILSCALE_FUNNEL_RC:-0}"
SHIM
  chmod +x "$TS_HOME/bin/tailscale"
  export TS_TAILSCALE_BIN="$TS_HOME/bin/tailscale"

  # Recording send_alert spy at the NEW dispatch path the monitor sources. It
  # never delivers; it records each call's severity (one line per call, so a test
  # counts pages), sound (the page/muted tier), full argv (for body assertions),
  # and the baseline as it stood at call time (to prove notify-before-persist).
  export TS_SEND_ALERT_SEVERITY="$TS_HOME/send-alert-severity.log"
  export TS_SEND_ALERT_SOUND="$TS_HOME/send-alert-sound.log"
  export TS_SEND_ALERT_LOG="$TS_HOME/send-alert.log"
  export TS_SEND_ALERT_STATE_AT_CALL="$TS_HOME/send-alert-state-at-call.log"
  : >"$TS_SEND_ALERT_SEVERITY"
  : >"$TS_SEND_ALERT_SOUND"
  : >"$TS_SEND_ALERT_LOG"
  : >"$TS_SEND_ALERT_STATE_AT_CALL"
  cat >"$TS_HOME/.local/libexec/osquery/alert-dispatch.sh" <<'SHIM'
# shellcheck shell=bash
send_alert() {
  # Severity (arg 1) on its own line: one line per call, so a test counts pages.
  printf '%s\n' "${1:-}" >>"$TS_SEND_ALERT_SEVERITY"
  # Sound (arg 4) on its own line: a non-empty sound is the page tier, an empty
  # sound is muted. A funnel exposure and every gap must page, never mute.
  printf '%s\n' "${4:-}" >>"$TS_SEND_ALERT_SOUND"
  # Full argv (severity, title, body, sound) for body/naming assertions.
  printf '%s\n' "$*" >>"$TS_SEND_ALERT_LOG"
  # The baseline as it stood when the page fired, so a test can prove the ordering
  # (notify-before-persist: the baseline has NOT yet advanced to active).
  if [[ -f ${OSQUERY_TAILSCALE_STATE:-/nonexistent} ]]; then
    cat "$OSQUERY_TAILSCALE_STATE" >>"$TS_SEND_ALERT_STATE_AT_CALL"
  fi
  # TS_SEND_ALERT_EXIT models a dispatch that could NOT durably queue the page
  # (nonzero). Default 0 (queued): the monitor then advances the baseline.
  return "${TS_SEND_ALERT_EXIT:-0}"
}
SHIM

  # Default: no prior baseline, no funnel configured (idle), status succeeds.
  set_funnel '{}'
  set_funnel_rc 0
}

teardown_tailscale_harness() {
  [[ -n ${_TS_HARNESS_OWNED_DIR:-} ]] || return 0
  rm -rf "$_TS_HARNESS_OWNED_DIR"
  unset _TS_HARNESS_OWNED_DIR
}

# seed_funnel_state <token> -- write the prior baseline (and gap marker) the run
# starts from. "" removes it (first run); active/inactive write the 0600 JSON
# baseline; gap writes an inactive baseline PLUS the page-once gap marker (a prior
# blind window); corrupt writes non-JSON garbage.
seed_funnel_state() {
  local marker="$OSQUERY_TAILSCALE_STATE.gap"
  rm -f "$OSQUERY_TAILSCALE_STATE" "$marker"
  case "$1" in
    "") : ;;
    active) _write_state_0600 '{"funnel":"active"}' ;;
    inactive) _write_state_0600 '{"funnel":"inactive"}' ;;
    gap)
      _write_state_0600 '{"funnel":"inactive"}'
      : >"$marker"
      ;;
    corrupt) printf 'not-json-garbage\n' >"$OSQUERY_TAILSCALE_STATE" ;;
    *)
      printf 'seed_funnel_state: unknown token %q\n' "$1" >&2
      return 1
      ;;
  esac
}

# _write_state_0600 <compact-json> -- write a known-good baseline at 0600, so a
# sad-path test can prove a gap read leaves it untouched.
_write_state_0600() {
  mkdir -p "$(dirname "$OSQUERY_TAILSCALE_STATE")"
  printf '%s\n' "$1" >"$OSQUERY_TAILSCALE_STATE"
  chmod 600 "$OSQUERY_TAILSCALE_STATE"
}

# run_tailscale_monitor -- run the monitor under the harness env. HOME is the temp
# home so the sourced dispatch spy and the default state path resolve inside the
# sandbox; OSQUERY_TAILSCALE_BIN points at the recording stub.
run_tailscale_monitor() {
  HOME="$TS_HOME" \
    OSQUERY_TAILSCALE_BIN="$TS_TAILSCALE_BIN" \
    OSQUERY_TAILSCALE_STATE="$OSQUERY_TAILSCALE_STATE" \
    bash "$TS_TOOL"
}

# run_tailscale_monitor_missing_bin -- the configured binary does not exist (the
# dead-monitor regression that left funnel paging silently disabled).
run_tailscale_monitor_missing_bin() {
  HOME="$TS_HOME" \
    OSQUERY_TAILSCALE_BIN="$TS_HOME/bin/no-such-tailscale" \
    OSQUERY_TAILSCALE_STATE="$OSQUERY_TAILSCALE_STATE" \
    bash "$TS_TOOL" 2>/dev/null
}

# run_tailscale_monitor_path_resolved -- NO env override: the monitor must find
# the stub via `command -v` on PATH (the homebrew-formula resolution path).
run_tailscale_monitor_path_resolved() {
  HOME="$TS_HOME" \
    PATH="$TS_HOME/bin:$PATH" \
    OSQUERY_TAILSCALE_STATE="$OSQUERY_TAILSCALE_STATE" \
    env -u OSQUERY_TAILSCALE_BIN bash "$TS_TOOL"
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

# assert_tailscale_called_with <substring> -- the recorded tailscale argv
# contains <substring> (proves the monitor read via the verified --json path).
assert_tailscale_called_with() {
  if ! grep -qF -- "$1" "$TS_TAILSCALE_ARGS"; then
    printf 'expected the tailscale argv to contain %s; argv was:\n%s\n' \
      "$1" "$(cat "$TS_TAILSCALE_ARGS")" >&2
    return 1
  fi
}

# assert_no_page -- the monitor did not call send_alert at all (silent).
assert_no_page() {
  if [[ -s $TS_SEND_ALERT_LOG ]]; then
    printf 'expected NO page, but send_alert was called:\n%s\n' \
      "$(cat "$TS_SEND_ALERT_LOG")" >&2
    return 1
  fi
}

# assert_page_count <n> -- send_alert was called exactly <n> times (one severity
# line per call; the body may span many lines, so the severity log is the count).
assert_page_count() {
  local count
  count=$(wc -l <"$TS_SEND_ALERT_SEVERITY")
  count=${count//[[:space:]]/}
  if [[ $count -ne $1 ]]; then
    printf 'expected %s page(s), got %s; send_alert log:\n%s\n' \
      "$1" "$count" "$(cat "$TS_SEND_ALERT_LOG")" >&2
    return 1
  fi
}

# assert_page_severity_is <severity> -- a page fired and every page carried
# <severity> (only a CRIT reaches the #priority webhook, so the severity arg is
# the security-relevant one, not just the title text).
assert_page_severity_is() {
  if [[ ! -s $TS_SEND_ALERT_SEVERITY ]]; then
    printf 'expected a %s page, but send_alert was never called\n' "$1" >&2
    return 1
  fi
  if grep -qvxF "$1" "$TS_SEND_ALERT_SEVERITY"; then
    printf 'expected every page at severity %s, got:\n%s\n' \
      "$1" "$(cat "$TS_SEND_ALERT_SEVERITY")" >&2
    return 1
  fi
}

# assert_page_sound_nonempty -- a page fired and every page carried a NON-EMPTY
# sound (the page tier). An empty sound is the muted tier: a public-exposure or
# blind-monitor page must ping, never mute.
assert_page_sound_nonempty() {
  if [[ ! -s $TS_SEND_ALERT_SOUND ]]; then
    printf 'expected a page with a non-empty sound, but send_alert was never called\n' >&2
    return 1
  fi
  # One sound line per call; an EMPTY line is a muted (empty-sound) call. grep -xF ''
  # matches only whole empty lines, so this fails if any page was muted.
  if grep -qxF '' "$TS_SEND_ALERT_SOUND"; then
    printf 'expected every page to carry a non-empty sound (page tier), got a muted call:\n%s\n' \
      "$(sed 's/^$/<empty>/' "$TS_SEND_ALERT_SOUND")" >&2
    return 1
  fi
  return 0
}

# assert_page_body_has <substring> -- some page's argv contained <substring>.
assert_page_body_has() {
  if ! grep -qF -- "$1" "$TS_SEND_ALERT_LOG"; then
    printf 'expected a page naming %s; send_alert log:\n%s\n' \
      "$1" "$(cat "$TS_SEND_ALERT_LOG")" >&2
    return 1
  fi
}

# assert_baseline_funnel <value> -- the persisted baseline's .funnel equals <value>.
assert_baseline_funnel() {
  local got
  got=$(jq -r '.funnel // empty' "$OSQUERY_TAILSCALE_STATE" 2>/dev/null || echo "")
  if [[ $got != "$1" ]]; then
    printf 'expected baseline .funnel == %s, got %q; baseline:\n%s\n' \
      "$1" "$got" "$(cat "$OSQUERY_TAILSCALE_STATE" 2>/dev/null || echo '(no file)')" >&2
    return 1
  fi
}

# snapshot_baseline -- copy the baseline aside so assert_baseline_unchanged can
# compare byte-for-byte after a run.
snapshot_baseline() {
  cp "$OSQUERY_TAILSCALE_STATE" "$TS_HOME/baseline.snapshot"
}

# assert_baseline_unchanged -- the baseline is byte-for-byte identical to the last
# snapshot (a gap read must neither clobber nor blank it).
assert_baseline_unchanged() {
  if ! cmp -s "$TS_HOME/baseline.snapshot" "$OSQUERY_TAILSCALE_STATE"; then
    printf 'expected the baseline byte-for-byte preserved.\nsnapshot:\n%s\nnow:\n%s\n' \
      "$(cat "$TS_HOME/baseline.snapshot" 2>/dev/null || echo '(no snapshot)')" \
      "$(cat "$OSQUERY_TAILSCALE_STATE" 2>/dev/null || echo '(missing)')" >&2
    return 1
  fi
}

# assert_gap_marker -- the page-once monitoring-gap marker (STATE.gap) exists.
assert_gap_marker() {
  if [[ ! -f $OSQUERY_TAILSCALE_STATE.gap ]]; then
    printf 'expected the gap marker %s.gap to exist, but it does not\n' "$OSQUERY_TAILSCALE_STATE" >&2
    return 1
  fi
}

# assert_no_gap_marker -- the monitoring-gap marker does not exist (never paged,
# or cleared on recovery).
assert_no_gap_marker() {
  if [[ -f $OSQUERY_TAILSCALE_STATE.gap ]]; then
    printf 'expected NO gap marker, but %s.gap exists\n' "$OSQUERY_TAILSCALE_STATE" >&2
    return 1
  fi
}

# assert_page_saw_prior_not_active -- at the moment send_alert fired, the persisted
# baseline was NOT yet active (either absent, or holding the prior inactive value),
# proving the page fired BEFORE the baseline advanced (notify-before-persist).
assert_page_saw_prior_not_active() {
  if grep -qF '"funnel":"active"' "$TS_SEND_ALERT_STATE_AT_CALL"; then
    printf 'expected the baseline NOT to be active at page time (notify-before-persist), saw:\n%s\n' \
      "$(cat "$TS_SEND_ALERT_STATE_AT_CALL")" >&2
    return 1
  fi
  return 0
}

# assert_no_state -- no baseline file exists (a first-observation page whose
# send_alert failed must not have seeded one).
assert_no_state() {
  if [[ -f $OSQUERY_TAILSCALE_STATE ]]; then
    printf 'expected NO baseline file, but %s exists with:\n%s\n' \
      "$OSQUERY_TAILSCALE_STATE" "$(cat "$OSQUERY_TAILSCALE_STATE")" >&2
    return 1
  fi
}

# assert_file_absent <path> -- the path does not exist (an injection payload must
# not have executed and created its marker file).
assert_file_absent() {
  if [[ -e $1 ]]; then
    printf 'expected %s NOT to exist, but it does\n' "$1" >&2
    return 1
  fi
}
