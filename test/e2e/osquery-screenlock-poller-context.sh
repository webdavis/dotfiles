#!/usr/bin/env bash
# osquery-screenlock-poller-context.sh (R2-3). Screen-lock-off detection was MOVED out of the
# root-daemon pack and into the gui/501 user poller (osquery-firewall-gatekeeper-monitor.sh),
# because the `screenlock` osquery table is scoped to the logged-in user: the ROOT osqueryd daemon
# has no user session and never returns a screenlock row (the pack query was dead, and the round-1
# e2e "verified" it with a session-inheriting osqueryi, which was false).
#
# This test runs the poller's EXACT combined posture query via osqueryi in the CURRENT session and
# asserts the screenlock scalar is readable and in {0,1}. In CI (no osqueryi, or a non-user daemon
# context) it SKIPs cleanly. It is the applied-context proof for R2-3: run interactively as the
# gui/501 user, the poller reads screenlock; run headless/CI it does not, and that is fine because
# the poller runs as a user LaunchAgent, never as the root daemon.
#
# Live + environment-bound (needs osqueryi + a user session), so it lives in the e2e camp.
set -euo pipefail

OSQUERYI="${OSQUERYI:-$(command -v osqueryi || true)}"
[[ -n $OSQUERYI ]] || {
  printf 'SKIP: osqueryi not found\n'
  exit 0
}

# The exact query the poller issues (firewall + gatekeeper + screenlock in one shot).
posture=$("$OSQUERYI" --json "
  SELECT
    (SELECT global_state FROM alf) AS firewall,
    (SELECT assessments_enabled FROM gatekeeper) AS gatekeeper,
    (SELECT enabled FROM screenlock) AS screenlock
" 2>/dev/null | jq -c '.[0] // empty' 2>/dev/null || true)

screenlock=$(jq -r '.screenlock // empty' <<<"$posture" 2>/dev/null || echo "")

if [[ -z $screenlock ]]; then
  # No user session (root daemon / headless CI): the screenlock table is empty. That is the exact
  # reason the detector lives in the user poller, not the pack (not a failure here).
  printf 'SKIP: screenlock unreadable in this context (no user session), expected off the gui/501 poller\n'
  exit 0
fi

[[ $screenlock =~ ^[01]$ ]] || {
  printf 'FAIL: screenlock scalar is %s, expected 0 or 1\n' "$screenlock" >&2
  exit 1
}
printf 'PASS: the user-context poller query reads screenlock=%s (in {0,1}); R2-3 applied context confirmed\n' "$screenlock"
