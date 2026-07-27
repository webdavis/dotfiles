#!/usr/bin/env bash
# macos-posture-indeterminate.sh -- the indeterminate-on-nonzero discipline,
# asserted PER declared posture control (slice 6, absorbed from the retired
# apply-time posture reminder): a probe that exits NONZERO is INDETERMINATE
# regardless of what it printed, because a failed probe's output is
# untrustworthy. Untrustworthy means it may print healthy-looking text yet have
# failed, so each stub here prints EXACTLY the healthy text for its control and
# exits 1. The required outcome, per control:
#
#   - a monitoring-gap page fires, naming the control (never a silent pass:
#     fail-open is the cardinal sin);
#   - the read is never classified as the healthy value: the baseline is
#     byte-for-byte untouched, so no healthy value was persisted and no
#     transition was fabricated.
#
# Runs the real poller against the recording harness (stubbed osqueryi, probe
# stubs, send_alert spy); no live machine, no chezmoi apply.
set -euo pipefail

# Scrubbed at SCRIPT scope. Git exports GIT_DIR to every hook it runs and this
# suite runs from the pre-push hook.
unset MACOS_DEFAULTS_SOURCE_DIR GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

# shellcheck source=../fixtures/osquery-poller-lib.bash
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../fixtures" && pwd)/osquery-poller-lib.bash"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# One case per control: <control-id> <healthy-looking-output-env> <exit-env>.
# The healthy text is the stub default, so only the exit override is set; the
# point is exactly that healthy-looking output plus a nonzero exit must never
# read as healthy.
run_case() { # <control-id> <exit-override-env-var>
  local control_id="$1" exit_env="$2" poller_status=0

  setup_poller_harness
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
  set_posture_controls '[
    {"id":"filevault","description":"FileVault disk encryption","tier":"verify","reader":"fdesetup_status","expect":"on"},
    {"id":"sip","description":"System Integrity Protection","tier":"verify","reader":"csrutil_status","expect":"disabled"},
    {"id":"autologin","description":"Automatic login at the login window","tier":"verify","reader":"sysadminctl_autologin","expect":"off"},
    {"id":"guest","description":"The macOS Guest account","tier":"verify","reader":"sysadminctl_guest","expect":"disabled"}
  ]'
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1","filevault":"on","filevault:expect":"on","sip":"disabled","sip:expect":"disabled","autologin":"off","autologin:expect":"off","guest":"disabled","guest:expect":"disabled"}'
  snapshot_baseline

  export "$exit_env=1" # healthy default output + nonzero exit
  run_poller >/dev/null 2>&1 || poller_status=$?
  unset "$exit_env"

  # The gap page fired (exit 0 after a queued page) and named the control.
  [[ $poller_status -eq 0 ]] ||
    fail "$control_id: expected exit 0 after paging the gap, got $poller_status"
  assert_page_count 1 ||
    fail "$control_id: a nonzero probe must page a monitoring gap, never pass silently"
  assert_page_severity_is CRIT ||
    fail "$control_id: the gap page must be CRIT"
  assert_page_body_has 'monitoring gap' ||
    fail "$control_id: the page must name the monitoring gap"
  assert_page_body_has "$control_id" ||
    fail "$control_id: the gap page must name the control that gapped"
  # Never classified as healthy: the baseline is byte-for-byte untouched, so
  # the healthy-looking output was not believed and nothing advanced.
  assert_baseline_unchanged ||
    fail "$control_id: a nonzero probe must never advance the baseline"
  # And never a mutating call, even on the failure path.
  assert_no_mutation_attempt ||
    fail "$control_id: the failure path invoked a non-status command"

  teardown_poller_harness
}

run_case filevault POLLER_FDESETUP_EXIT
run_case sip POLLER_CSRUTIL_EXIT
run_case autologin POLLER_SYSADMINCTL_AUTOLOGIN_EXIT
run_case guest POLLER_SYSADMINCTL_GUEST_EXIT

printf 'ok: nonzero probes are indeterminate for every declared control\n'
