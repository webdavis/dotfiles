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

# route_findings: the three-outcome gate. Read normalized findings ({q, act, cols,
# ep}) as NDJSON on stdin and route each to EXACTLY ONE outcome:
#   PAGE      -> emit the finding (with .sev = "CRIT") as NDJSON on stdout, for
#                render_page to render into the #priority body.
#   DIGEST    -> record the finding via digest_append (a spool side effect); emit
#                nothing.
#   LOG-ONLY  -> emit nothing and record nothing (the row stays only in results.log).
#
# It composes route_severity for the base tier, then a per-detector case mirrors
# c69baab's gate, overriding the base tier where the detector demands it. The gate
# is the authority on the final outcome.
#
# Enrichment runs BEFORE the per-detector case so a finding promoted to CRIT by an
# untrusted signing verdict can never be suppressed by the allowlist (the security
# invariant: an untrusted program behind an allowlisted label still pages).
#
# digest_append (digest-store.sh), allowlist_verdict (allowlist-verdict.sh),
# pipeline_verdict (pipeline-verdict.sh), and the enrich-finding.sh script are
# expected to be available alongside this helper; the entry script sources all
# helpers into one process.
route_findings() {
  local -a objs=() controls=() sevs=()
  local obj
  # Read the whole batch (a bounded log-tail, already fully in hand), so the two
  # jq-heavy steps below run ONCE over the batch instead of per finding.
  while IFS= read -r obj; do
    [[ -n $obj ]] || continue
    objs+=("$obj")
  done
  [[ ${#objs[@]} -gt 0 ]] || return 0
  # One jq pass extracts the control fields (q, act, category, file basename, path,
  # label, program) per finding; one route_severity pass gives the base severity.
  # Both emit exactly one line per input, in order, so they align with objs by
  # index. The fields are joined with the ASCII Unit Separator (0x1F), NOT a tab:
  # a tab is whitespace in IFS, so `read` would collapse the empty category/base
  # fields and shift the later fields off. 0x1F is non-whitespace, so empty fields
  # are preserved, and it cannot occur in an osquery path/label/program.
  mapfile -t controls < <(printf '%s\n' "${objs[@]}" |
    jq -rc '[.q, .act, (.cols.category // ""), ((.cols.target_path // "") | split("/") | last), (.cols.path // ""), (.cols.label // ""), (.cols.program // ""), (.cols.target_path // ""), (.cols.sha256 // ""), (.cols.action // ""), (.ep // "")] | join("\u001f")')
  mapfile -t sevs < <(printf '%s\n' "${objs[@]}" | route_severity)

  # The signing enricher (enrich-finding.sh): given an inspectable path it emits a
  # trust fact string and exits 10 when the code is UNTRUSTED. Overridable for tests;
  # absent/non-executable -> enrichment is skipped (fail-open, the finding still surfaces).
  local enrich_script="${OSQUERY_ENRICH_SCRIPT:-$HOME/.local/libexec/osquery/enrich-finding.sh}"

  local -a pages=()
  local i q act category base path label program target hash verb ep sev av signing enrich_status
  for i in "${!objs[@]}"; do
    obj=${objs[i]}
    IFS=$'\x1f' read -r q act category base path label program target hash verb ep <<<"${controls[i]}"
    sev=${sevs[i]}
    # Enrichment runs BEFORE the per-detector case (the security invariant): a
    # finding an untrusted signature promotes to CRIT here can NOT then be
    # suppressed by the allowlist below - a promoted CRIT (an untrusted binary
    # behind an allowlisted label) always pages. For a finding with an inspectable
    # path and a CRIT/NOTICE base tier, get the signing verdict, attach it as
    # .signing for render-page, and promote NOTICE -> CRIT (louder, never quieter)
    # when the verdict is UNTRUSTED (enricher exit 10). Fail-open: an absent or
    # erroring enricher leaves the finding surfaced, just without a Signing field.
    signing=""
    if [[ -n $ep && ($sev == "CRIT" || $sev == "NOTICE") && -x $enrich_script ]]; then
      enrich_status=0
      signing=$("$enrich_script" "$ep" 2>/dev/null) || enrich_status=$?
      [[ $enrich_status -eq 10 && $sev == "NOTICE" ]] && sev="CRIT"
    fi
    [[ -n $signing ]] && obj=$(jq -c --arg sig "$signing" '.signing = $sig' <<<"$obj")
    case "$q" in
      # Poller-owned protections: the dedicated 60s poller pages a firewall /
      # Gatekeeper / SIP flip, so routing them here too would double-page. Log-only,
      # overriding route_severity's CRIT for the unsafe transition.
      firewall_state | gatekeeper_state | sip_state) continue ;;
      # Wrong-signal or too-noisy-to-surface detectors: log-only.
      kernel_extensions_new | persistence_startup_items_crontab | es_launchd_writes | agent_binary_changed) continue ;;
      # Digest tier: suspicious-but-ambiguous, summarized daily, never paged.
      # agent_authfile_changed is the 3 NON-secret configs (.env, config.toml,
      # cli-client-id) - routine churn digests, it never pages (the 2 true secrets
      # are agent_secretfile_changed, a separate paging detector).
      agent_authfile_changed)
        digest_append "$obj"
        continue
        ;;
      system_extensions_new)
        digest_append "$obj"
        continue
        ;;
      listening_ports_non_loopback)
        [[ $act == added ]] && digest_append "$obj"
        continue
        ;;
      # Page tier, direction-gated to the unsafe transition only.
      agent_secretfile_changed) sev="CRIT" ;; # a change to one of the 2 secrets pages
      agent_exposure_changed)
        # Only the newly-exposed "added" transition pages; a "removed" row is the
        # port being un-exposed (good news), log-only.
        [[ $act == added ]] || continue
        sev="CRIT"
        ;;
      remote_access_sharing_state)
        # An "added" row is a service turning ON (page); "removed" is OFF, log-only.
        [[ $act == added ]] || continue
        sev="CRIT"
        ;;
      suid_bin_unexpected)
        # A new setuid-root binary; only the "added" direction. sev is already CRIT.
        [[ $act == added ]] || continue
        ;;
      persistence_launchd)
        [[ $act == added ]] || continue
        case "$path" in
          /System/Library/*) continue ;;   # Apple's own launchd items, log-only
          */LaunchDaemons/*) sev="CRIT" ;; # a root LaunchDaemon runs at boot -> page by path
          *)
            # A user LaunchAgent. DEFAULT-DENY (operator ruling 2026-07-22): an
            # unallowlisted user LaunchAgent PAGES; the operator seeds known-good
            # agents via the allowlist writer (osquery-allowlist.sh) to suppress
            # them. This is a deliberate hardening over the digest-unknowns of the
            # reverted #52 (c69baab), which silently digested an unknown agent.
            # allowlist_verdict: 0 = full-tuple match (known-good) -> suppress;
            # 2 = reused label (identity diverges) -> page; 1 = not-found -> page.
            av=0
            allowlist_verdict "$label" "$path" "$program" || av=$?
            case "$av" in
              # Known-good tuple -> suppress, UNLESS enrichment already promoted this
              # to CRIT on an untrusted program. A promoted CRIT is never suppressed;
              # the allowlist only quiets a non-CRIT finding. The tuple's plist-hash
              # dimension catches a tampered PLIST and the signing verdict catches an
              # untrusted PROGRAM - separate defenses, both able to page.
              0) [[ $sev == "CRIT" ]] || continue ;;
              *) sev="CRIT" ;; # reused label OR unknown -> page (default-deny)
            esac
            ;;
        esac
        ;;
      file_events_recent)
        case "$category" in
          ssh)
            case "$base" in
              authorized_keys | authorized_keys2) sev="CRIT" ;; # remote-auth entry points page
              *)
                digest_append "$obj" # other ~/.ssh files (private keys, config) digest
                continue
                ;;
            esac
            ;;
          sshd_config) sev="CRIT" ;; # remote-auth policy pages
          # The alerter's own scripts/plists (pipeline_integrity) and our own
          # LaunchAgents (launch_agents/launch_daemons) consult pipeline_verdict:
          # 0 = page (tamper / cannot confirm / no manifest -> fail-open), 1 = silent
          # (an untracked neighbor, or an exact (path, sha256) manifest match). Never
          # digests - page or silent. Until the manifest slice lands, a tracked
          # change fails open to a page (criterion 6).
          pipeline_integrity | launch_agents | launch_daemons)
            if pipeline_verdict "$target" "$hash" "$verb"; then sev="CRIT"; else continue; fi
            ;;
          sudoers)
            digest_append "$obj"
            continue
            ;;
          allowlist_file)
            case "$base" in
              page-launchd-allowlist.txt)
                digest_append "$obj" # the page-suppressor edit itself digests
                continue
                ;;
              *) continue ;; # other ~/.config/osquery neighbors, log-only
            esac
            ;;
          *) continue ;;
        esac
        ;;
        # Detectors with no arm (new_admin_user, filevault_off, filevault_state, the
        # installed-software drift queries, recent_logins, persistence_launchd_overrides)
        # fall through with the base severity route_severity assigned.
    esac
    [[ $sev == "CRIT" ]] || continue
    pages+=("$obj")
  done
  # Emit the page-candidates, in input order, with .sev stamped CRIT - one jq pass.
  [[ ${#pages[@]} -gt 0 ]] && printf '%s\n' "${pages[@]}" | jq -c '.sev = "CRIT"'
  return 0
}
