#!/usr/bin/env bash
#
# osquery-results-alerter.sh — fired by launchd (WatchPaths) whenever
# ~/.local/log/osquery/osqueryd.results.log changes. Reads new lines since the
# last run (byte-offset state file), and surfaces every differential finding
# from the scheduled packs (intrusion-detection, security-policy-regression,
# installed-software-drift) AND the file-events query. Each batch becomes a
# single notification, delivered to both the local notifier and the #osquery
# Discord channel via osquery-alert-dispatch.sh.
#
# Supersedes an earlier file-events-only notifier that watched the same log.

set -euo pipefail

LOG="${OSQUERY_RESULTS_LOG:-$HOME/.local/log/osquery/osqueryd.results.log}"
STATE="${OSQUERY_RESULTS_OFFSET:-$HOME/.local/state/osquery-results-offset}"

# shellcheck source=/dev/null
source "$HOME/.local/bin/osquery-alert-dispatch.sh"

# Flush any pages spooled while the gateway was down. `|| true` keeps the drain off
# the detection path: a delivery feature must never abort the alerter (set -euo
# pipefail) before it reads results.log.
_drain_spool || true

mkdir -p "$(dirname "$STATE")"
[[ -f $LOG ]] || exit 0

# Portable size + inode (wc -c / ls -i work on macOS and Linux; BSD `stat -f`
# does not). Inode lets us notice a rotated/recreated log at the same path.
size=$(wc -c <"$LOG")
size=${size//[[:space:]]/}
# shellcheck disable=SC2012  # $LOG is a fixed, controlled path — ls -i is safe and portable
inode=$(ls -i "$LOG" | awk '{print $1}')

# State holds "<inode> <offset>". Re-seed silently when it is missing or not in
# that exact two-integer form (first run, or migrating the old single-int file).
prev_inode=""
prev_offset=""
if [[ -f $STATE ]]; then read -r prev_inode prev_offset <"$STATE" || true; fi
# Capture-then-validate, not branch-on-read: a state file missing its trailing
# newline makes `read` return non-zero even though it populated the vars, so
# keying the re-seed on read's exit status would skip a whole differential batch.
if ! [[ $prev_inode =~ ^[0-9]+$ && $prev_offset =~ ^[0-9]+$ ]]; then
  printf '%s %s\n' "$inode" "$size" >"$STATE.tmp" && mv -f "$STATE.tmp" "$STATE"
  exit 0
fi

# New inode (rotation/recreation) or a shrink (truncation) → read the current
# file from byte 0 so nothing is skipped or replayed from the old file.
if [[ $inode != "$prev_inode" || $size -lt $prev_offset ]]; then
  prev_offset=0
fi

[[ $size -eq $prev_offset ]] && exit 0

# Notify on ALL observed activity, tiered by severity so high-threat events
# stand out from routine ones. Each row is classified:
#   CRIT (🔴)   — a protection turned off (security-policy-regression), a new
#                 unexpected setuid binary, or ssh-key / sudoers tampering.
#   NOTICE (🟡) — new persistence/auto-start, launchd-dir file changes, or a
#                 new kernel/system extension.
#   INFO (🔵)   — installed-software drift, listening ports, logins.
# chezmoi's renameio temp-file churn is excluded. jq emits "<SEV>\t<text>".
# Bound the read to the snapshot window (head -c) so rows appended after we
# captured $size aren't consumed early and re-fired next time; `|| true`
# absorbs head's SIGPIPE. jq -rR + per-line try/fromjson means one malformed
# line yields nothing for that line instead of aborting the whole batch.
# $size > $prev_offset is guaranteed by the shrink-reset and equality-exit guards
# above, but clamp defensively: an inode-reusing rotation in the window since we
# captured $size could otherwise hand head -c a non-positive count.
span=$((size - prev_offset))
new_lines=""
if [[ $span -gt 0 ]]; then
  new_lines=$(tail -c "+$((prev_offset + 1))" "$LOG" | head -c "$span" || true)
fi
raw_findings=$(printf '%s\n' "$new_lines" | jq -rR '
  . as $line | (try ($line | fromjson) catch empty) |
  # A security-policy row is CRITICAL only when the protection turned OFF, not
  # on every change. For the boolean states that is an "added" row carrying the
  # off value; for filevault (the query returns only encrypted volumes) it is a
  # "removed" row — a volume left the encrypted set. Re-enables, version bumps,
  # sharing changes, and the paired "removed" old-value rows fall through to
  # NOTICE. (sharing cannot be direction-classified from a single row, so it is
  # always NOTICE; the poller covers firewall/Gatekeeper transitions too.)
  def protection_off:
    (.name == "pack_security-policy-regression_firewall_state" and .action == "added" and (.columns.global_state // "") == "0")
    or (.name == "pack_security-policy-regression_gatekeeper_state" and .action == "added" and (.columns.assessments_enabled // "") == "0")
    or (.name == "pack_security-policy-regression_sip_state" and .action == "added" and (.columns.enabled // "") == "0")
    or (.name == "pack_security-policy-regression_screenlock_state" and .action == "added" and (.columns.enabled // "") == "0")
    or (.name == "pack_security-policy-regression_filevault_state" and .action == "removed")
    or (.action == "currently-off" and (.name | startswith("pack_security-policy-regression_")));
  def sev:
    if protection_off
       or (.name == "new_admin_user")
       or (.name == "pack_intrusion-detection_suid_bin_unexpected")
       or (.name == "file_events_recent" and ((.columns.category // "") | test("^(ssh|sudoers|sshd_config)$")))
    then "CRIT"
    elif (.name | startswith("pack_security-policy-regression_"))
       or (.name | test("^pack_intrusion-detection_persistence_"))
       or (.name == "pack_intrusion-detection_kernel_extensions_new")
       or (.name == "pack_intrusion-detection_system_extensions_new")
       or (.name == "file_events_recent")
       or (.name == "es_launchd_writes")
    then "NOTICE"
    else "INFO" end;
  # Snapshot rows (absolute-state floor *_off queries) log under a "snapshot"
  # array with action "snapshot"; explode each into a synthetic "currently-off"
  # row so the rest of the pipeline treats it uniformly. Empty snapshot = silent.
  (if .action == "snapshot"
   then (.name as $n | .snapshot[]? | {name: $n, action: "currently-off", columns: .})
   else . end)
  | select(.name != null and ((.name | startswith("pack_")) or (.name == "file_events_recent") or (.name == "es_launchd_writes") or (.name == "new_admin_user") or (.name == "persistence_launchd") or (.name == "agent_exposure_changed") or (.name == "agent_authfile_changed") or (.name == "agent_binary_changed")))
  | select((.columns.target_path // "") | test("/\\.renameio-TempDir") | not)
  # Discard the counter==0 baseline (first-observation) row — calibration, not a real event.
  | select((.counter // 1) != 0)
  | (.name | sub("^pack_[^_]+_"; "")) as $q
  | (.action // "changed") as $act
  # The path the enricher should inspect (a plist, bundle, or binary) per query
  # type — empty when signing/trust does not apply.
  | ((if .name == "es_launchd_writes" then (.columns.path // "")
      elif .name == "file_events_recent" then (.columns.target_path // "")
      elif (.name | test("_persistence_launchd$")) then (.columns.path // "")
      elif (.name | test("_persistence_startup_items_crontab$")) then (.columns.path // "")
      elif (.name | test("_kernel_extensions_new$")) then (.columns.path // "")
      elif (.name | test("_system_extensions_new$")) then (.columns.bundle_path // .columns.path // "")
      elif (.name | test("_suid_bin_unexpected$")) then (.columns.path // "")
      else "" end) | gsub("[\t\n]"; " ")) as $ep
  # Emit one compact JSON object per finding; the bash side enriches, then renders
  # it into a #priority block or an #osquery line via the header/field/step maps.
  | {sev: sev, q: $q, act: $act, cols: (.columns // {}), ep: $ep} | @json
' 2>/dev/null || true)

# Advance state before notifying so a slow/failed dispatch never re-fires the
# same batch on the next WatchPaths trigger. Format: "<inode> <offset>".
printf '%s %s\n' "$inode" "$size" >"$STATE.tmp" && mv -f "$STATE.tmp" "$STATE"

[[ -z $raw_findings ]] && exit 0

# Enrich CRIT/NOTICE findings with deterministic signing facts (osquery-enrich-finding.sh).
# An UNTRUSTED result promotes NOTICE -> CRIT (louder, never quieter). Fail-open: if the
# helper is absent or errors, the finding still surfaces, just without a Signing: field.
# Nothing is ever suppressed here. Each raw line is one compact JSON finding object; we
# inject .signing and the (possibly promoted) .sev back into it.
ENRICH="$HOME/.local/bin/osquery-enrich-finding.sh"

# The launchd page-allowlist: labels whose new *user* LaunchAgents are known-good
# and fully suppressed (neither page nor digest). Curated by the one writer,
# osquery-allowlist.sh, which shares this exact path + env (reader == writer — a
# mismatch makes every allow a silent no-op). Load once; fail-open if the file is
# missing/unreadable (suppress nothing). Strip comments/whitespace/blank lines.
ALLOWLIST_FILE="${OSQUERY_LAUNCHD_ALLOWLIST:-$HOME/.config/osquery/page-launchd-allowlist.txt}"
allow_set=""
if [[ -r $ALLOWLIST_FILE ]]; then
  allow_set=$(sed -e 's/#.*//' -e 's/[[:space:]]//g' "$ALLOWLIST_FILE" | grep -v '^$' || true)
fi
_allowlisted() { [[ -n $allow_set ]] && grep -qxF -- "$1" <<<"$allow_set"; }

# The pipeline-integrity manifest: root-owned, source-derived sha256 lines for the
# alerter's own scripts/plists. A file_events:pipeline_integrity change pages ONLY
# when its sha256 is absent from this manifest (tamper); a legit chezmoi apply
# produces a matching hash → silent. Fail-safe: a missing manifest or an empty hash
# means "cannot confirm legitimate" → page (loud), never a silent miss.
PIPELINE_MANIFEST="${OSQUERY_PIPELINE_MANIFEST:-/var/osquery/pipeline-known-good.sha256}"

# Digest tier (v2): suspicious-but-ambiguous findings accumulate here as NDJSON for a
# daily grouped summary instead of paging. Best-effort by design — failing to record a
# digest line must never abort detection, so every step is guarded and the function
# always succeeds.
DIGEST_STORE="${OSQUERY_DIGEST_STORE:-$HOME/.local/state/osquery-digest-spool/digest.ndjson}"
_digest_append() {
  local finding="$1"
  mkdir -p "$(dirname "$DIGEST_STORE")" 2>/dev/null || true
  jq -c --arg timestamp "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    '{
      timestamp: $timestamp,
      detector: .q,
      category: (.cols.category // ""),
      identity: (.cols.label // .cols.identifier // .cols.target_path // .cols.path // .cols.username // "?"),
      action: .act,
      summary: (.q + " " + ((.cols.label // .cols.identifier // .cols.target_path // .cols.path // .cols.username) // "?"))
    }' <<<"$finding" >>"$DIGEST_STORE" 2>/dev/null || true
}

enriched=""
while IFS= read -r obj; do
  [[ -z $obj ]] && continue
  # Read the fields we need one-per-line. A tab/space IFS would collapse runs and
  # shift columns when a middle field is empty (e.g. absent category); line-per-
  # field preserves empties. Safe because none of these values contains a newline
  # (ep had newlines stripped upstream; the rest are tokens/labels).
  {
    read -r sev
    read -r ep
    read -r q
    read -r cat
    read -r lbl
  } < <(
    jq -r '.sev, (.ep // ""), .q, (.cols.category // ""), (.cols.label // .cols.name // "")' <<<"$obj"
  )
  # Three-outcome gate (v2, incremental): reroute digest-tier detectors out of the
  # page/dispatch path entirely. Detectors not named here fall through to the legacy
  # classification + dispatch below, so untriaged behavior is preserved until its own
  # test migrates it onto a gate arm.
  case "$q" in
    # Page/core: rare, high-confidence, actionable. An agent API port newly bound
    # off-loopback exposes the operator's primary remote-access path.
    agent_exposure_changed) sev="CRIT" ;;
    # The pipeline HMAC key and the paseo daemon keypair are the operator's own auth:
    # tampering forges/mutes alerts or hijacks remote access → page. The rotation-prone
    # rest (.env, config.toml, cli-client-id) is noisier → digest.
    agent_authfile_changed)
      case "$(jq -r '.cols.path // ""' <<<"$obj")" in
        */webhook-secret | */daemon-keypair.json) sev="CRIT" ;;
        *)
          _digest_append "$obj"
          continue
          ;;
      esac
      ;;
    # SIP is intentionally off on this developer box: an on->off transition cannot
    # occur, so the snapshot floor is pure noise. Log-only (no page, no digest).
    sip_state) continue ;;
    # The kernel_extensions table lists LOADED kexts (load/unload on demand) — a
    # firehose of hundreds of events. Wrong signal entirely; log-only.
    kernel_extensions_new) continue ;;
    # Dead code in v1: never emits a real on->off transition, so any row is false
    # assurance. Log-only until rebuilt as a Remote-Login/Screen-Sharing detector.
    remote_access_sharing_state) continue ;;
    # Endpoint-Security launchd writes are forensic enrichment only (the writer
    # process), not a deliverable signal on their own. Log-only.
    es_launchd_writes) continue ;;
    persistence_launchd)
      # A root-level LaunchDaemon runs as root at boot — a higher-privilege threat
      # that pages. A per-user LaunchAgent is lower-stakes and digests.
      case "$(jq -r '.cols.path // ""' <<<"$obj")" in
        /System/Library/*) continue ;;
        */LaunchDaemons/*) sev="CRIT" ;;
        *)
          # A known-good user LaunchAgent (label in the page-allowlist) is fully
          # suppressed; an unknown one digests for the daily review.
          _allowlisted "$lbl" && continue
          _digest_append "$obj"
          continue
          ;;
      esac
      ;;
    # Suspicious-but-ambiguous: digest for the daily summary, never page. A new
    # non-Apple system extension is usually an app upgrade re-activating a sysext.
    system_extensions_new)
      _digest_append "$obj"
      continue
      ;;
    # Agent binary hash changes cannot distinguish a frequent legit self-update from a
    # swap, so they are inherently noisy — log-only (recorded in results.log for
    # forensics, never paged or digested).
    agent_binary_changed) continue ;;
    # Screen lock off is posture drift, not an intrusion — low actionability.
    screenlock_state | screenlock_off)
      _digest_append "$obj"
      continue
      ;;
    # file_events fans out by category. authorized_keys / sshd_config are remote-auth
    # tampering → page. sudoers churns (visudo / chezmoi) and allowlist_file is
    # runtime-mutable → digest. Everything else (launch dirs, etc.) → log-only.
    file_events_recent)
      # Only a create/modify is actionable; a delete is your own revert/cleanup noise.
      case "$(jq -r '.cols.action // ""' <<<"$obj")" in
        CREATED | UPDATED) ;;
        *) continue ;;
      esac
      case "$cat" in
        authorized_keys | sshd_config) sev="CRIT" ;;
        pipeline_integrity)
          # Page only on content-mismatch: a sha256 that matches the source-derived,
          # root-owned manifest is a legit apply (silent); an absent hash or any other
          # content is tamper (page). Never digests — page or silent.
          hash_value=$(jq -r '.cols.sha256 // ""' <<<"$obj")
          if [[ -n $hash_value ]] && grep -qiF -- "$hash_value" "$PIPELINE_MANIFEST" 2>/dev/null; then
            continue
          fi
          sev="CRIT"
          ;;
        sudoers | allowlist_file)
          _digest_append "$obj"
          continue
          ;;
        *) continue ;;
      esac
      ;;
  esac
  sig=""
  if [[ -n $ep && ($sev == CRIT || $sev == NOTICE) && -x $ENRICH ]]; then
    rc=0
    sig=$("$ENRICH" "$ep" 2>/dev/null) || rc=$?
    [[ $rc -eq 10 && $sev == NOTICE ]] && sev="CRIT"
  fi
  obj=$(jq -c --arg sev "$sev" --arg sig "$sig" \
    '.sev = $sev | (if $sig == "" then . else .signing = $sig end)' <<<"$obj")
  enriched+="$obj"$'\n'
done <<<"$raw_findings"
enriched=${enriched%$'\n'}

[[ -z $enriched ]] && exit 0

# Render both channel bodies in one jq pass. #priority gets focused labeled blocks
# (header + decision-relevant fields + one "→" next step); #osquery gets one compact
# humanized line per finding. Layout follows the user's ADHD surfacing research: one
# thing, glanceable, minimal fields, ending in a single action, no raw query jargon.
render=$(printf '%s\n' "$enriched" | jq -s '
  # Wrap a value in Discord inline-code backticks. The value is attacker-controlled
  # (launchd label, path); strip backticks so it cannot break out of the inline-code
  # span and inject markdown. Display-only — does not affect detection/severity.
  def code: "`" + (gsub("`"; "") ) + "`";
  # Plain-English name of a macOS protection query, or null if the finding is not one.
  def protname:
    if (.q | test("^firewall")) then "Firewall"
    elif (.q | test("^gatekeeper")) then "Gatekeeper"
    elif (.q | test("^sip")) then "System Integrity Protection"
    elif (.q | test("^filevault")) then "FileVault"
    elif (.q | test("^screenlock")) then "Screen lock"
    elif (.q | test("^remote_access_sharing")) then "Sharing"
    else null end;
  # Human header for a finding (kernel/system extensions matched before the generic
  # browser-extension regex so they keep their specific labels).
  def header:
    (protname) as $p |
    if $p != null then (if .sev == "CRIT" then "\($p) turned OFF" else "\($p) changed" end)
    elif .q == "persistence_launchd" then "New startup item"
    elif .q == "persistence_launchd_overrides" then "Startup override changed"
    elif .q == "persistence_startup_items_crontab" then "New startup/cron entry"
    elif .q == "suid_bin_unexpected" then "New setuid root binary"
    elif .q == "agent_exposure_changed" then "Agent port exposed off-loopback"
    elif .q == "agent_authfile_changed" then "Agent credential changed"
    elif .q == "kernel_extensions_new" then "New kernel extension"
    elif .q == "system_extensions_new" then "New system extension"
    elif .q == "listening_ports_non_loopback" then "New network listener"
    elif .q == "recent_logins" then "Login"
    elif .q == "installed_apps" then "New app"
    elif .q == "homebrew_packages" then "New Homebrew package"
    elif (.q | test("_extensions$|_addons$")) then "New browser extension"
    elif .q == "file_events_recent" then
      ((.cols.category // "") as $cat |
        if $cat == "authorized_keys" then "SSH key file changed"
        elif $cat == "sudoers" then "sudoers changed"
        elif $cat == "sshd_config" then "sshd_config changed"
        elif $cat == "pipeline_integrity" then "Security tooling changed"
        elif $cat == "allowlist_file" then "Allowlist changed"
        elif ($cat == "launch_agents" or $cat == "launch_daemons") then "Startup folder changed"
        else "Watched file changed" end)
    elif .q == "es_launchd_writes" then "Startup item written by a process"
    else (.q | gsub("_"; " ")) end;
  # Best single identifier for a finding.
  def keyid:
    .cols as $c |
    ($c.label // $c.identifier // $c.name // $c.target_path // $c.path // $c.username // "?");
  # Decision-relevant "Label: value" lines for a #priority block. Values are wrapped
  # in Discord inline-code; an untrusted signing verdict is flagged and bolded.
  def fields:
    .cols as $c | (.signing // null) as $sig |
    (if $sig then
       (if ($sig | test("unsigned|untrusted|ad-hoc|unverified|no authority"; "i"))
        then ["- **Signing:** ⚠️ **\($sig)**"] else ["- **Signing:** \($sig)"] end)
     else [] end) as $sg |
    if .q == "persistence_launchd" then ["- **What:** \(($c.label // "?") | code)", "- **Program:** \(($c.program // "?") | code)"] + $sg
    elif .q == "persistence_startup_items_crontab" then ["- **What:** \(($c.name // "?") | code)", "- **Command:** \(($c.command // "?") | code)"] + $sg
    elif .q == "suid_bin_unexpected" then ["- **Path:** \(($c.path // "?") | code)"] + $sg + ["- **Owner:** \(($c.username // "?") | code)"]
    elif .q == "agent_exposure_changed" then ["- **Process:** \(($c.name // "?") | code)", "- **Address:** \(($c.address // "?") | code)", "- **Port:** \(($c.port // "?") | code)"]
    elif .q == "agent_authfile_changed" then ["- **File:** \((($c.path // "") | split("/") | last) | code)"]
    elif .q == "system_extensions_new" then ["- **Name:** \(($c.identifier // "?") | code)", "- **Team:** \(($c.team // "?") | code)"] + $sg
    elif .q == "kernel_extensions_new" then ["- **Name:** \(($c.name // "?") | code)", "- **Path:** \(($c.path // "?") | code)"] + $sg
    elif .q == "file_events_recent" then ["- **File:** \(($c.target_path // "?") | code)", "- **Action:** \(.act)"]
    elif .q == "es_launchd_writes" then ["- **Process:** \(($c.path // "?") | code)", "- **Wrote:** \(($c.filename // $c.dest_filename // "?") | code)"] + $sg
    elif (protname) != null then ["- **State:** **OFF**"]
    else $sg + ["- **What:** \((keyid) | code)"] end;
  # One or two instructive next-step lines for a #priority (always CRIT) block.
  def nextstep:
    (.ep // "") as $ep |
    if (protname) != null then
      ["- Did you turn this off? If not, something else did — **investigate now**.", "- Re-enable it in System Settings."]
    elif (.q == "system_extensions_new" or .q == "kernel_extensions_new") then
      ["- Did you install this? If not, **remove it** — an extension can intercept traffic or load at boot.", "- Manage at: System Settings → General → Login Items & Extensions"]
    elif .q == "suid_bin_unexpected" then
      ["- Did you create this? If not, it lets a program run as **root** — a backdoor.", "- **Inspect:** " + (("codesign -dv \"" + $ep + "\"") | code)]
    elif .q == "agent_exposure_changed" then
      ["- Did you expose this? If not, an agent API is reachable **off-box** — close it now.", "- Re-bind it to 127.0.0.1 or block the port at the firewall."]
    elif .q == "agent_authfile_changed" then
      ["- Did you rotate this? If not, an attacker may forge or mute alerts, or hijack remote access — **investigate now**."]
    elif .q == "file_events_recent" then
      (if (.cols.category // "") == "pipeline_integrity"
       then ["- Did you just apply your dotfiles? If not, your **security tooling was modified** — investigate now.", "- **Compare:** " + (("shasum -a 256 \"" + $ep + "\"") | code)]
       else ["- Did you change this? If not, someone altered who can log in or run as **root**.", "- **Review:** " + (("sudo cat \"" + $ep + "\"") | code)] end)
    elif (.q == "persistence_launchd" or .q == "persistence_startup_items_crontab") then
      ["- Did you set this up? If not, it **auto-runs at every login** — likely malware.", "- **Inspect:** " + (("cat \"" + $ep + "\"") | code)]
    elif .q == "es_launchd_writes" then
      ["- Did you run this? If not, a process is **installing persistence** — investigate it and remove the file.", "- **Inspect the writer:** " + (("codesign -dv \"" + $ep + "\"") | code)]
    elif ($ep != "") then ["- **Review:** " + ($ep | code)]
    else [] end;
  def block:
    (["**" + header + "**"] + fields + nextstep) | join("\n");
  ([.[] | select(.sev == "CRIT")]) as $crit |
  {
    pcount: ($crit | length),
    pbody: ($crit | map(block) | join("\n\n"))
  }
')

# v2 dispatches ONLY the #priority page (confirmed CRIT). Everything non-CRIT is
# either digested upstream by the gate or stays log-only on disk — there is no
# #osquery notice/info channel.
pcount=$(jq -r '.pcount' <<<"$render")

if [[ $pcount -gt 0 ]]; then
  title="🔴 **CRITICAL**"
  if [[ $pcount -gt 1 ]]; then title="🔴 **CRITICAL** · $pcount"; fi
  send_alert CRIT "$title" "$(jq -r '.pbody' <<<"$render")" "Sosumi"
fi
