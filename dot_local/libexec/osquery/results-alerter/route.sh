#!/usr/bin/env bash
#
# route.sh - a sourced helper for results-alerter.sh. Functions only, no main;
# nothing here delivers, checkpoints, or exits. It owns the routing stage: the
# base severity matrix (route_severity, below) and, in later behaviors, the
# three-outcome page/digest/log-only gate that composes the leaf verdict helpers.
#
# route_severity reads normalized-finding NDJSON on stdin (one {q, act, cols, ep}
# object per line, as normalize emits) and prints one base severity per finding,
# in order. It is the pure classifier only; the gate may override this tier for a
# specific detector (e.g. promote agent_exposure_changed to a page). Adapted from
# c69baab's sev/protection_off defs, rebased to test the prefix-stripped q rather
# than the full pack-qualified name (normalize already stripped the pack prefix,
# so there is no common pack prefix left to test - the security-policy-regression
# membership is an explicit set of the stripped query names).

route_severity() {
  jq -r '
    # protection_off: a security-policy-regression protection observed in its
    # UNSAFE state. For the boolean states that is an "added" differential row
    # carrying the off value in its state column; for FileVault it is any
    # filevault_off "added" row (that query emits a row only when NO APFS volume
    # is encrypted, so its presence IS the off state - no column check needed).
    # This is the criterion-2 fix: filevault_off is now a DIFFERENTIAL query, so
    # the off state arrives as action:added, not the old snapshot "currently-off".
    def protection_off:
      (.q == "firewall_state" and .act == "added" and ((.cols.global_state // "") == "0"))
      or (.q == "gatekeeper_state" and .act == "added" and ((.cols.assessments_enabled // "") == "0"))
      or (.q == "sip_state" and .act == "added" and ((.cols.enabled // "") == "0"))
      or (.q == "filevault_off" and .act == "added");
    # The security-policy-regression pack, by stripped query name. Any such row
    # that is not protection_off (a re-enable, a version bump, the paired removed
    # row, FileVault APFS churn) falls through to NOTICE, never silently to INFO.
    def security_policy_query:
      .q | IN("filevault_off", "filevault_state", "firewall_state", "gatekeeper_state", "remote_access_sharing_state", "sip_state");
    def sev:
      if protection_off or .q == "new_admin_user" or .q == "suid_bin_unexpected"
      then "CRIT"
      elif security_policy_query
        or (.q | startswith("persistence_"))
        or .q == "kernel_extensions_new"
        or .q == "system_extensions_new"
        or .q == "file_events_recent"
        or .q == "es_launchd_writes"
      then "NOTICE"
      else "INFO" end;
    sev
  '
}
