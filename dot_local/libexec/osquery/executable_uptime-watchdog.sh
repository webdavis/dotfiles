#!/usr/bin/env bash
#
# uptime-watchdog.sh, polled every 15 min by launchd. Asserts the osquery
# notification pipeline is actually ALIVE, because a dead pipeline otherwise looks
# identical to "all quiet" (the alerter is edge-triggered and the queries are
# differential, so genuine silence is normal). Fires a single CRITICAL page via the
# shared dispatcher if any component is down or wedged; silent when everything is
# healthy. Deliberately does NOT use results.log mtime as a signal: hours of
# healthy silence are expected.
#
# It also carries the pipeline's only SCHEDULED integrity check. The rest of the
# pipeline-integrity manifest is enforced through osquery file_events, and osquery
# watches PATHS, so tampering that generates no event on a watched path (a hard link
# or a symlink pointing the executed bytes somewhere else) is invisible to it. The
# content audit below hashes every manifested path on each tick, which needs no
# event to have fired.
#
# Cardinal invariant: FAIL-SAFE toward paging. Any ambiguous or failed check (an
# unloaded or unknown-state agent, a wedged osqueryd, an unhealthy route, an
# unreadable queue count, a manifest the audit cannot use) resolves to a CRIT, never
# a silent all-healthy. A watchdog that fails quietly is worse than no watchdog.
#
# The watchdog only READS: it probes the delivery queue counts but never drains it
# (the scheduled alert-drainer owns draining), it hashes the manifested files but
# never repairs them, and it renders only KNOWN agent labels plus validated-numeric
# exit codes and counts plus static text, so a value influenced by a compromised
# launchd, or a path influenced by whoever tampered with the pipeline, cannot inject
# into the page.

set -euo pipefail

# The #priority route the pages actually use (the one send_alert POSTs). Probed
# with a bare GET: no signing header, so the HMAC key never reaches this wire.
HERMES_PRIORITY_URL="${OSQUERY_HERMES_PRIORITY_URL:-http://127.0.0.1:8644/webhooks/osquery-priority}"
ROUTE_TIMEOUT="${OSQUERY_WATCHDOG_ROUTE_TIMEOUT:-3}"
# Canary freshness bound (shared with the heartbeat via OSQUERY_CANARY_MAX_AGE): the
# scheduled canary runs every 600s, so 1800s (three intervals) tolerates a missed
# tick while still catching a stopped or wedged daemon within one watchdog cycle.
canary_max_age="${OSQUERY_CANARY_MAX_AGE:-1800}"
[[ $canary_max_age =~ ^[0-9]+$ ]] || canary_max_age=1800
# Cross-run state: per-agent {runs, streak} (so a crash-loop needs a genuine
# RE-RUN, not a frozen daily exit) and the pending backlog {count, growth_streak}
# (so a sustained backlog, not a one-tick burst, pages). Owner-only, atomic.
STATE="${OSQUERY_WATCHDOG_STATE:-$HOME/.local/state/osquery-watchdog-state.json}"
# Every deployed osquery LaunchAgent EXCEPT this watchdog (which, if running, is
# loaded by definition). No osquery plist sets KeepAlive, so launchd will not
# reload an unloaded agent: this list is the sole liveness backstop. A calendar or
# interval agent that is merely idle between runs still reports loaded, so listing
# it here cannot false-alarm.
AGENTS=(
  "com.webdavis.osquery-results-alerter"
  "com.webdavis.osquery-firewall-gatekeeper-monitor"
  "com.webdavis.osquery-alert-drainer"
  "com.webdavis.osquery-digest"
  "com.webdavis.osquery-heartbeat"
  "com.webdavis.osquery-tailscale-monitor"
)

# shellcheck source=/dev/null
source "$HOME/.local/libexec/osquery/alert-dispatch.sh"
# The shared canary-freshness seam (newest_canary_timestamp), the same one the daily
# heartbeat uses, so the watchdog judges the root daemon by the REAL artifact (a live
# daemon's scheduled canary), never a blind osqueryi one-shot.
# shellcheck source=/dev/null
source "$HOME/.local/libexec/osquery/canary-freshness.sh"
# The periodic content audit of the pipeline-integrity manifest (probe 5). It also
# pulls in the verdict helper, which owns the manifest path and the root-ownership
# check both consumers share.
# shellcheck source=/dev/null
source "$HOME/.local/libexec/osquery/pipeline-audit.sh"

# Never leave a partial temp state (or the writability probe) behind, on any exit path.
trap 'rm -f "$STATE.tmp" "$STATE.probe"' EXIT

# write_state <compact-json>, atomically persist the cross-run state owner-only
# (0600) via a private temp file plus an atomic rename. Returns nonzero on failure.
# A persistently unpersistable state is NOT harmless: it resets prev_state to {}
# every tick, so a crash-loop or backlog-growth streak can never accrue and its
# alarm is silently disabled. That is why writability is probed up front and an
# unpersistable state PAGES (see the state-persistability probe below), rather than
# being swallowed as a best-effort log.
write_state() {
  local dir
  dir="$(dirname "$STATE")"
  mkdir -p "$dir" 2>/dev/null || return 1
  (
    umask 077
    printf '%s\n' "$1" >"$STATE.tmp"
  ) && mv -f "$STATE.tmp" "$STATE" && chmod 600 "$STATE"
}

# field_raw <launchctl-print-output> <field-label>, print the RAW text after
# "<label> = " on its first matching line, or return nonzero when the field line is
# absent. Extraction is kept separate from policy so the caller can demand an integer
# for "runs" while CLASSIFYING "last exit code" (a number, the "(never exited)"
# sentinel, or an unknown value that must fail safe). Every caller validates before
# use, so nothing raw is ever used in arithmetic or rendered into a page.
field_raw() {
  local text="$1" label="$2" line
  line="$(printf '%s\n' "$text" | grep -m1 -F "$label = ")" || return 1
  printf '%s' "${line#*"$label = "}"
}

# Prior cross-run state. Absent or corrupt (unparseable) starts fresh, never a
# crash: a fresh start means empty streaks and no false growth signal.
prev_state="{}"
if [[ -r $STATE ]]; then
  prev_state="$(cat "$STATE" 2>/dev/null || printf '{}')"
fi
printf '%s' "$prev_state" | jq -e . >/dev/null 2>&1 || prev_state="{}"

uid="$(id -u)"
problems=()

# The cross-run state must be writable, or the streak alarms silently degrade: an
# unpersistable state resets prev_state to {} every tick (see write_state), so a
# crash-looping agent's streak resets to 1 each run and never reaches the loop
# threshold, and a backlog's growth streak likewise never accrues. Probe writability
# up front and treat an unpersistable state as an unhealthy condition that PAGES.
if ! mkdir -p "$(dirname "$STATE")" 2>/dev/null || ! (
  umask 077
  : >"$STATE.probe"
) 2>/dev/null; then
  problems+=("the watchdog cannot persist its state ($STATE); the crash-loop and backlog-growth alarms are degraded until this is fixed")
fi
rm -f "$STATE.probe" 2>/dev/null || true

# 1) osqueryd present AND actually PRODUCING SCHEDULED RESULTS. pgrep proves the
#    process exists; the daemon's OWN scheduled heartbeat canary proves it is alive
#    AND running its schedule. A standalone osqueryi one-shot is deliberately NOT
#    used: it spins up its own ephemeral engine and answers even when the running
#    daemon is wedged or stopped (R2-8, a blind checkmark). A missing, stale, or
#    future-dated canary means the daemon is not scheduling. The freshness read needs
#    a trustworthy clock first: a failed clock read cannot judge freshness, so it
#    fails safe to a page rather than treating every old canary as fresh. Only
#    validated numerics reach the message.
now="$(date -u +%s 2>/dev/null || true)"
if [[ ! $now =~ ^[0-9]+$ ]]; then
  problems+=("the watchdog cannot read the system clock, so it cannot verify osqueryd is producing scheduled results")
elif ! pgrep -fq '/opt/osquery/.*osqueryd'; then
  problems+=("osqueryd is not running")
else
  canary_timestamp="$(newest_canary_timestamp)"
  # Fail-safe structure, mirroring the heartbeat: HEALTHY only when a canary sits
  # inside the freshness window in EITHER direction; anything else PAGES. The default
  # (else) is to page, so an unexpected value cannot fall through silent. The seam
  # range-bounds the timestamp, so the arithmetic below can never overflow or be
  # misread as octal; the fail-safe default is defense in depth on top of that.
  if [[ -z $canary_timestamp ]]; then
    problems+=("osqueryd is not producing scheduled results (the heartbeat canary is MISSING); the daemon is stopped or wedged")
  elif ((now - canary_timestamp <= canary_max_age)) && ((canary_timestamp - now <= canary_max_age)); then
    : # a canary within the window in either direction: osqueryd is alive and scheduling
  elif ((canary_timestamp > now)); then
    problems+=("osqueryd heartbeat canary timestamp is IMPLAUSIBLE ($((canary_timestamp - now))s in the future); clock skew or a bad row, not a trustworthy liveness signal")
  else
    problems+=("osqueryd is not producing scheduled results (the heartbeat canary is STALE, $((now - canary_timestamp))s old); the daemon is stopped or wedged")
  fi
fi

# 2) Every watched agent is loaded AND not crash-looping. `launchctl print` failing
#    means the agent is not loaded (fail-safe: page). A loaded agent that exits
#    nonzero is a crash-loop candidate, but ONLY when it actually RE-RAN: launchd's
#    `runs` counter must have advanced since the last check. A daily agent's exit
#    is FROZEN between the 15-min checks, so gating on `runs` stops a single daily
#    failure from paging every tick forever. Two failing re-runs (streak >= 2) is
#    the loop; one is a tolerated transient.
# The EXACT (whitespace-tolerant) launchd not-exited sentinel. Matched anchored, not
# as a substring, so a malformed value that merely CONTAINS "(never exited)" with junk
# around it is an unknown state that pages, not a healthy free pass.
never_exited_re='^[[:space:]]*\(never exited\)[[:space:]]*$'
agent_state_json="{}"
for label in "${AGENTS[@]}"; do
  if ! print_out="$(launchctl print "gui/$uid/$label" 2>/dev/null)"; then
    problems+=("LaunchAgent not loaded: $label")
    continue
  fi
  runs_readable=1
  runs_raw="$(field_raw "$print_out" 'runs')" || runs_raw=""
  if [[ $runs_raw =~ ^[0-9]+$ ]]; then
    runs=$runs_raw
  else
    runs=-1
    runs_readable=0
  fi
  prev_runs="$(jq -r --arg l "$label" '.agents[$l].runs // -1' <<<"$prev_state" 2>/dev/null || printf -- '-1')"
  [[ $prev_runs =~ ^-?[0-9]+$ ]] || prev_runs=-1
  prev_streak="$(jq -r --arg l "$label" '.agents[$l].streak // 0' <<<"$prev_state" 2>/dev/null || printf '0')"
  [[ $prev_streak =~ ^[0-9]+$ ]] || prev_streak=0
  # Classify the last-exit-code field. An ABSENT field, or one present but neither a
  # number nor the "(never exited)" sentinel, is an UNKNOWN agent state: fail SAFE to
  # a page, never default the exit to 0 (which would read every agent as healthy and
  # silently disable crash-loop detection, the fail-open trap). "(never exited)" is a
  # running or never-run process, a legitimate not-a-failure. A number drives the
  # runs-gated streak. Only the validated leading integer is ever used or rendered.
  streak=$prev_streak
  if ! exit_raw="$(field_raw "$print_out" 'last exit code')"; then
    problems+=("LaunchAgent state is unreadable (launchctl print has no last-exit-code field): $label")
  elif [[ $exit_raw =~ ^[[:space:]]*(-?[0-9]+) ]]; then
    exit_code="${BASH_REMATCH[1]}"
    if [[ $exit_code -eq 0 ]]; then
      streak=0 # healthy or recovered
    elif [[ $runs_readable -eq 1 && $runs -eq $prev_runs ]]; then
      streak=$prev_streak # a FROZEN nonzero exit (no re-run): do not accumulate
    else
      streak=$((prev_streak + 1)) # a fresh failing re-run (or unreadable runs: fail-safe)
    fi
    if [[ $streak -ge 2 ]]; then
      problems+=("LaunchAgent crash-looping (last exit $exit_code, $streak failing re-runs): $label")
    fi
  elif [[ $exit_raw =~ $never_exited_re ]]; then
    streak=0 # exactly "(never exited)": currently running or never run, not a failure
  else
    problems+=("LaunchAgent exit state is unreadable (unexpected last-exit-code value): $label")
  fi
  agent_state_json="$(jq -c --arg l "$label" --argjson r "$runs" --argjson s "$streak" \
    '. + {($l): {runs: $r, streak: $s}}' <<<"$agent_state_json")"
done

# 3) The hermes #priority route is configured and reachable. A GET to the POST-only
#    route returns 405 (route present, rejects GET) or 2xx = healthy; 000 (gateway
#    down), 404 (route not configured), or 5xx (gateway erroring) are unhealthy.
route_code="$(curl -s -o /dev/null -w '%{http_code}' --max-time "$ROUTE_TIMEOUT" "$HERMES_PRIORITY_URL" 2>/dev/null)" || route_code=000
[[ $route_code =~ ^[0-9]+$ ]] || route_code=000
case "$route_code" in
  2[0-9][0-9] | 405) : ;; # route present and reachable
  *) problems+=("hermes #priority route unhealthy (HTTP $route_code) at $HERMES_PRIORITY_URL") ;;
esac

# 4) Delivery-backlog health. ANY dead-letter is a permanently-failed alert (the
#    drainer gave up), so it pages unconditionally; an unreadable count is a broken
#    store, which fails safe to a page. A pending backlog pages only on SUSTAINED
#    growth (it grew across two consecutive checks, a growth streak), so a transient
#    burst the drainer absorbs in its next cycle does not false-page. The counts
#    come from the dispatch library's read-only counters; the watchdog never drains.
dead_letter_count="$(osquery_dead_letter_count 2>/dev/null)" || dead_letter_count=""
if [[ $dead_letter_count =~ ^[0-9]+$ ]]; then
  if [[ $dead_letter_count -gt 0 ]]; then
    problems+=("$dead_letter_count alert(s) permanently failed delivery (dead-lettered); the pipeline is broken")
  fi
else
  problems+=("the dead-letter count is unreadable (the alert store may be broken)")
fi

pending_count="$(osquery_pending_alert_count 2>/dev/null)" || pending_count=""
prev_pending="$(jq -r '.pending.count // -1' <<<"$prev_state" 2>/dev/null || printf -- '-1')"
[[ $prev_pending =~ ^-?[0-9]+$ ]] || prev_pending=-1
prev_growth_streak="$(jq -r '.pending.growth_streak // 0' <<<"$prev_state" 2>/dev/null || printf '0')"
[[ $prev_growth_streak =~ ^[0-9]+$ ]] || prev_growth_streak=0
if [[ $pending_count =~ ^[0-9]+$ ]]; then
  pending_for_state=$pending_count
  if [[ $prev_pending -ge 0 && $pending_count -gt $prev_pending ]]; then
    growth_streak=$((prev_growth_streak + 1))
  else
    growth_streak=0
  fi
  if [[ $growth_streak -ge 2 ]]; then
    problems+=("the undelivered-alert backlog has grown for $growth_streak consecutive checks (now $pending_count pending); delivery is not keeping up")
  fi
else
  problems+=("the pending-alert count is unreadable (the alert store may be broken)")
  pending_for_state=-1
  growth_streak=0
fi

# 5) Periodic CONTENT AUDIT of the pipeline-integrity manifest. Every other check
#    of that manifest hangs off osquery file_events, and osquery watches PATHS: an
#    attacker who hard-links a manifested pipeline script to a writable path outside
#    the pipeline home and overwrites the outside alias mutates the SAME INODE, so
#    the event names their path, nothing fires for the watched one, no verdict runs,
#    and the tampered script executes with nothing paged. A symlink referent is the
#    same blind spot in a second shape. That gap is in event GENERATION, so it can
#    only be closed by comparing bytes on a SCHEDULE. The seam hashes every
#    manifested path and reports what disagrees; it refuses (nonzero, with a reason
#    token) rather than report a false all-clear when the manifest itself cannot be
#    used, and it is bounded on entries, per-file bytes, and wall clock.
audit_rc=0
audit_report="$(pipeline_audit_scan 2>/dev/null)" || audit_rc=$?
audit_reason=""
audit_divergence_count=0
if [[ $audit_rc -ne 0 ]]; then
  # A refusal token, validated against the shape of the fixed vocabulary before it
  # can select a message; anything unexpected still pages, via the default arm.
  audit_reason="$audit_report"
  [[ $audit_reason =~ ^[a-z]{1,32}$ ]] || audit_reason="unknown"
elif [[ -n $audit_report ]]; then
  audit_divergence_count="$(printf '%s\n' "$audit_report" | wc -l | tr -d '[:space:]')"
  # A count that cannot be read must not read as ZERO (that would silently discard a
  # real divergence): fall back to the refusal path, which pages.
  if [[ ! $audit_divergence_count =~ ^[1-9][0-9]{0,5}$ ]]; then
    audit_reason="unknown"
    audit_divergence_count=0
  fi
fi

# Page-once discipline, on the watchdog's existing streak conventions. The audit is
# LEVEL-triggered against a condition that can persist for days, so re-paging every
# 15 minutes would train the operator to ignore it. The findings are reduced to a
# FINGERPRINT (a sha256 over the sorted report), which is what the state remembers:
# the raw report holds attacker-influenceable paths, and a hash both keeps them out
# of the persisted state and answers the only question that matters across ticks -
# is this the same condition as last time?
#
#   - the SAME fingerprint on two consecutive ticks is the confirmation threshold;
#     one tick alone is the in-flight-apply shape (the deployed files have changed
#     and the manifest has not been regenerated yet), and it resolves itself;
#   - a fingerprint already paged for does not page again;
#   - a CHANGED fingerprint restarts the confirmation, so new tampering is reported;
#   - a clean audit forgets both, so a recurrence pages again.
#
# The fingerprint covers WHICH paths disagree and HOW, not the bytes they now hold,
# so re-tampering with an already-reported file does not page a second time. That is
# deliberate: the operator has already been told that file is compromised, and the
# alternative re-pages on every keystroke an attacker makes.
audit_fingerprint=""
audit_fingerprint_is_valid=1
if [[ $audit_rc -ne 0 || -n $audit_report ]]; then
  audit_fingerprint="$(printf '%s\n' "$audit_report" | LC_ALL=C sort | shasum -a 256 2>/dev/null)"
  audit_fingerprint="${audit_fingerprint%% *}"
  if [[ ! $audit_fingerprint =~ ^[0-9a-f]{64}$ ]]; then
    audit_fingerprint=""
    audit_fingerprint_is_valid=0
  fi
fi

prev_audit_fingerprint="$(jq -r '.pipeline_audit.fingerprint // ""' <<<"$prev_state" 2>/dev/null || printf '')"
[[ $prev_audit_fingerprint =~ ^[0-9a-f]{64}$ ]] || prev_audit_fingerprint=""
prev_audit_paged="$(jq -r '.pipeline_audit.paged_fingerprint // ""' <<<"$prev_state" 2>/dev/null || printf '')"
[[ $prev_audit_paged =~ ^[0-9a-f]{64}$ ]] || prev_audit_paged=""
prev_audit_streak="$(jq -r '.pipeline_audit.streak // 0' <<<"$prev_state" 2>/dev/null || printf '0')"
[[ $prev_audit_streak =~ ^[0-9]{1,3}$ ]] || prev_audit_streak=0
if [[ $prev_audit_streak -gt 99 ]]; then prev_audit_streak=99; fi

audit_should_page=0
audit_streak=0
audit_paged_fingerprint=""
if [[ $audit_rc -eq 0 && -z $audit_report ]]; then
  : # a completed, clean audit: forget the streak and the paged marker
elif [[ $audit_fingerprint_is_valid -eq 0 ]]; then
  # Without a fingerprint a repeat cannot be told from a new condition, so the
  # page-once machinery cannot run. Page every tick rather than risk going silent.
  audit_should_page=1
else
  if [[ $audit_fingerprint == "$prev_audit_fingerprint" ]]; then
    audit_streak=$((prev_audit_streak + 1))
    if [[ $audit_streak -gt 99 ]]; then audit_streak=99; fi
  else
    audit_streak=1
  fi
  audit_paged_fingerprint="$prev_audit_paged"
  if [[ $audit_streak -ge 2 && $audit_fingerprint != "$prev_audit_paged" ]]; then
    audit_should_page=1
    audit_paged_fingerprint="$audit_fingerprint"
  fi
fi

# The rendered problem carries a VALIDATED COUNT and static text only. The
# manifested paths are deliberately not rendered: in the very scenario this audit
# exists to catch they are attacker-influenceable, and the operator's next step is
# the same either way. That keeps this watchdog's rule intact - known labels,
# validated numerics, and static text reach a page body, nothing else.
if [[ $audit_should_page -eq 1 ]]; then
  if [[ -n $audit_reason ]]; then
    case "$audit_reason" in
      missing)
        problems+=("the pipeline-integrity manifest is missing or unreadable, so the periodic content audit cannot verify the deployed pipeline; tampering would go unseen until it is restored")
        ;;
      untrustworthy)
        problems+=("the pipeline-integrity manifest is no longer root-owned (or is group/world-writable), so it can no longer vouch for the deployed pipeline")
        ;;
      malformed)
        problems+=("the pipeline-integrity manifest holds a malformed entry, so the periodic content audit cannot verify the deployed pipeline")
        ;;
      overlong)
        problems+=("the pipeline-integrity manifest lists more files than one audit tick will examine, so the deployed pipeline cannot be fully verified")
        ;;
      budget)
        problems+=("the periodic content audit ran out of its time budget before checking every file in the pipeline-integrity manifest, so the deployed pipeline cannot be fully verified")
        ;;
      *)
        problems+=("the periodic content audit could not verify the deployed pipeline against the pipeline-integrity manifest")
        ;;
    esac
  else
    problems+=("$audit_divergence_count file(s) in the pipeline-integrity manifest no longer match it (content changed, missing, or not a regular file); no file event reported this, which is what a hard-linked or relocated pipeline script looks like")
  fi
fi

# The refreshed state to persist once the tick is resolved. Built now, written only
# AFTER a page is durably queued (notify-before-persist), so a page that cannot be
# stored never advances a streak or growth baseline and masks the signal.
new_state="$(jq -cn --argjson agents "$agent_state_json" \
  --argjson pc "$pending_for_state" --argjson gs "$growth_streak" \
  --arg afp "$audit_fingerprint" --argjson as "$audit_streak" \
  --arg apfp "$audit_paged_fingerprint" \
  '{agents: $agents, pending: {count: $pc, growth_streak: $gs},
    pipeline_audit: {fingerprint: $afp, streak: $as, paged_fingerprint: $apfp}}')"

# Healthy: persist the refreshed baselines (no page to order against) and exit
# silent. A persist failure here only forgets one cycle of streak memory.
if [[ ${#problems[@]} -eq 0 ]]; then
  write_state "$new_state" || printf 'uptime-watchdog: could not persist state (%s)\n' "$STATE" >&2
  exit 0
fi

# Unhealthy: page ONE CRIT (level-triggered, so a persisting outage keeps
# reminding every tick). A dead pipeline is always CRITICAL and always carries a
# sound, so it reaches the #priority channel and pings. bt holds a literal backtick
# so a command name renders as Discord inline-code without shell expansion. The
# body renders only known labels + validated numerics + static text, no raw output.
bt='`'
title="🔴 **CRITICAL**"
if [[ ${#problems[@]} -gt 1 ]]; then title="🔴 **CRITICAL** (${#problems[@]} issues)"; fi
body="**Monitoring is DOWN**"
for problem in "${problems[@]}"; do body+=$'\n'"- $problem"; done
body+=$'\n'"- **Diagnose:** ${bt}launchctl list | grep -i osquery${bt}"
body+=$'\n'"- Restart the down component, then re-check."

# Notify-before-persist: only advance the persisted baselines after send_alert
# durably queues the page. send_alert is write-ahead durable, so on a hard store
# failure it returns nonzero: leave the state as-is and exit nonzero, so launchd
# logs the failure and the next tick re-detects instead of masking the signal.
if send_alert CRIT "$title" "$body" "Sosumi"; then
  if write_state "$new_state"; then
    exit 0
  fi
  printf 'uptime-watchdog: state could not be persisted (%s); the streak alarms are degraded, retrying next tick\n' "$STATE" >&2
  exit 1
fi
printf 'uptime-watchdog: send_alert could not durably queue the CRIT page; state not advanced, retrying next tick\n' >&2
exit 1
