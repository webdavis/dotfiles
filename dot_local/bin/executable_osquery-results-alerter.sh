#!/usr/bin/env bash
#
# osquery-results-alerter.sh — fired by launchd (WatchPaths) whenever
# ~/.local/log/osquery/osqueryd.results.log changes. Reads new lines since the
# last run (byte-offset state file), and surfaces every differential finding
# from the scheduled packs (intrusion-detection, security-policy-regression,
# installed-software-drift) AND the file-events query. A confirmed-critical batch
# becomes one #priority page via osquery-alert-dispatch.sh; everything else digests
# or stays log-only. v2 has no #osquery channel.
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
  # A security-policy row is CRITICAL only when the protection turned OFF, not on
  # every change. For the boolean states that is an "added" row carrying the off
  # value. Re-enables, version bumps, sharing changes, and the paired "removed"
  # old-value rows fall through to NOTICE (log-only). Firewall/Gatekeeper pack rows
  # are log-only at the gate — the dedicated 60s poller owns those pages.
  #
  # FileVault is NOT detected via filevault_state. That query emits one row per
  # FileVault-on APFS volume, and on sealed-system-volume macOS a single row can
  # leave the differential set (a base system volume unmounting, a snapshot
  # replacing it, APFS identity churn) while the data-bearing volume stays
  # encrypted — so "one filevault_state row removed" does NOT mean FileVault is off
  # (issue #18). A removed filevault_state row falls through to NOTICE.
  #
  # Genuine FileVault-off is detected by filevault_off: one constant row only when
  # NO APFS volume is encrypted. It is a DIFFERENTIAL query (not a snapshot) on
  # purpose — snapshot output goes to osqueryd.snapshots.log, which this alerter
  # does not read, so the earlier snapshot form never reached here (the
  # false-negative half of #18). As a differential its off-row is logged "added"
  # to results.log, matched below; the constant row is immune to APFS churn.
  def protection_off:
    (.name == "pack_security-policy-regression_firewall_state" and .action == "added" and (.columns.global_state // "") == "0")
    or (.name == "pack_security-policy-regression_gatekeeper_state" and .action == "added" and (.columns.assessments_enabled // "") == "0")
    or (.name == "pack_security-policy-regression_sip_state" and .action == "added" and (.columns.enabled // "") == "0")
    or (.name == "pack_security-policy-regression_screenlock_state" and .action == "added" and (.columns.enabled // "") == "0")
    or (.name == "pack_security-policy-regression_filevault_off" and .action == "added");
  def sev:
    # file_events tiering is decided AUTHORITATIVELY by the gate file_events_recent
    # arm below (authorized_keys / sshd_config page; sudoers / allowlist_file digest;
    # pipeline_integrity page-or-silent; everything else log-only), so it is NOT
    # pre-classified here. The old clause tested a non-existent ssh category and
    # mis-marked sudoers CRIT — dead, misleading text that the gate already overrode.
    if protection_off
       or (.name == "new_admin_user")
       or (.name == "pack_intrusion-detection_suid_bin_unexpected")
    then "CRIT"
    elif (.name | startswith("pack_security-policy-regression_"))
       or (.name | test("^pack_intrusion-detection_persistence_"))
       or (.name == "pack_intrusion-detection_kernel_extensions_new")
       or (.name == "pack_intrusion-detection_system_extensions_new")
       or (.name == "file_events_recent")
       or (.name == "es_launchd_writes")
    then "NOTICE"
    else "INFO" end;
  select(.name != null and ((.name | startswith("pack_")) or (.name == "file_events_recent") or (.name == "es_launchd_writes") or (.name == "new_admin_user") or (.name == "persistence_launchd") or (.name == "agent_exposure_changed") or (.name == "agent_authfile_changed") or (.name == "agent_binary_changed")))
  | select((.columns.target_path // "") | test("/\\.renameio-TempDir") | not)
  # Per-query baseline policy (FX5). The old unconditional counter==0 discard silently
  # ACCEPTED a pre-existing compromise: an admin already added, sharing already enabled,
  # a listener already exposed, or FileVault already off would all seed silently on the
  # first osqueryd run. MEMBERSHIP queries (new_admin_user, persistence, extensions, suid
  # — a differential against a seeded baseline set) legitimately calibrate on the first
  # observation and stay silent at counter==0. ABSOLUTE-STATE queries emit a row ONLY
  # when the current state is already unsafe (filevault_off = no volume encrypted;
  # remote_access_sharing_state = a service enabled; agent_exposure_changed = a port
  # off-loopback), so a counter==0 row is an unsafe FIRST observation that must PAGE, not
  # seed. Keep counter==0 rows ONLY for those absolute-state queries; discard the rest.
  | select((.counter // 1) != 0
      or (.name | test("filevault_off$"))
      or (.name | test("remote_access_sharing_state$"))
      or (.name | test("agent_exposure_changed$")))
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
  # Emit one compact JSON object per finding; the bash side enriches, gates it to a
  # tier, then renders a CRIT into a #priority block via the header/field/step maps.
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

# FX2: legitimacy is the EXACT (target_path, sha256) tuple, not the hash alone. The
# manifest is shasum format ("<sha256>  <path>"); a line passes only when BOTH its hash
# and its path match. Binding the hash to ITS path defeats the swap-in-place probe
# (replacing dispatch.sh with a byte-copy of heartbeat.sh, whose hash is in the manifest
# but bound to a different path). Case-insensitive on the hash (shasum and osquery both
# emit lowercase, but normalize defensively). A missing/unreadable manifest returns 1.
_manifest_has_tuple() {
  [[ -r $PIPELINE_MANIFEST ]] || return 1
  local want_path="$1" want_hash h p
  want_hash=$(printf '%s' "$2" | tr '[:upper:]' '[:lower:]')
  while read -r h p; do
    h=$(printf '%s' "$h" | tr '[:upper:]' '[:lower:]')
    [[ $h == "$want_hash" && $p == "$want_path" ]] && return 0
  done <"$PIPELINE_MANIFEST"
  return 1
}

# FX3: with directory watches (~/.local/bin, ~/Library/LaunchAgents) the pipeline
# categories fire for every file in the dir, so the tracked set is filtered here by
# basename: only the osquery pipeline scripts and our own LaunchAgents are pipeline
# infrastructure. Verdict: return 0 = PAGE (tamper / cannot confirm legit), 1 = SILENT
# (an untracked neighbor, or a change whose exact tuple is known-good). An empty/absent
# sha256 (which live MOVED_TO/ROOT_CHANGED/ATTRIBUTES_MODIFIED/DELETED rows carry), any
# mismatch, or a missing manifest cannot confirm a legit apply → page (fail-safe loud).
_pipeline_verdict() {
  local target="$1" hash_value="$2" base="${1##*/}"
  case "$base" in
    osquery-*.sh | com.webdavis.osquery-*.plist) ;;
    *) return 1 ;; # a neighbor file in the watched dir → log-only
  esac
  [[ -n $hash_value ]] && _manifest_has_tuple "$target" "$hash_value" && return 1
  return 0
}

# Digest tier (v2): suspicious-but-ambiguous findings accumulate here as NDJSON for a
# daily grouped summary instead of paging. Best-effort by design — failing to record a
# digest line must never abort detection, so every step is guarded and the function
# always succeeds. Deliberate page/digest asymmetry: the page basenames a path for
# one-glance clarity, but the digest stores the FULL path (.cols.path) — the daily
# digest is a private single-user triage view where the full path disambiguates (which
# .env?). No secret/token/sha256 is ever stored here, so invariant #4 still holds.
DIGEST_STORE="${OSQUERY_DIGEST_STORE:-$HOME/.local/state/osquery-digest-spool/digest.ndjson}"
_digest_append() {
  local finding="$1"
  mkdir -p "$(dirname "$DIGEST_STORE")" 2>/dev/null || true
  # The store persists FULL filesystem paths (and the .last copy keeps them indefinitely),
  # so it must not be world-readable — dir 700 / file 600, the way the page spool is. No
  # secret/sha256 is stored, but path metadata still discloses project/.env locations.
  chmod 700 "$(dirname "$DIGEST_STORE")" 2>/dev/null || true
  jq -c --arg timestamp "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    '{
      timestamp: $timestamp,
      detector: .q,
      category: (.cols.category // ""),
      identity: (if .q == "listening_ports_non_loopback"
                 then ((.cols.name // .cols.path // "?") + " " + (.cols.address // "?") + ":" + (.cols.port // "?"))
                 else (.cols.label // .cols.identifier // .cols.target_path // .cols.path // .cols.username // "?") end),
      action: .act,
      summary: (.q + " " + ((.cols.label // .cols.identifier // .cols.target_path // .cols.path // .cols.username) // "?"))
    }' <<<"$finding" >>"$DIGEST_STORE" 2>/dev/null || true
  chmod 600 "$DIGEST_STORE" 2>/dev/null || true
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
    # off-loopback exposes the operator's primary remote-access path. Only the "added"
    # (newly-exposed) transition pages; a "removed" row is the port being un-exposed
    # (the exposure being FIXED) — good news, never a page.
    agent_exposure_changed)
      [[ $(jq -r '.act' <<<"$obj") == added ]] || continue
      sev="CRIT"
      ;;
    # A NEW unexpected setuid-root binary pages (a privilege-escalation backdoor) via
    # the pre-gate CRIT classification; this arm exists only to drop the good-news
    # "removed" row — the binary being deleted, the threat going away — never a page.
    suid_bin_unexpected) [[ $(jq -r '.act' <<<"$obj") == added ]] || continue ;;
    # The pipeline HMAC key and the paseo daemon keypair are the operator's own auth:
    # tampering forges/mutes alerts or hijacks remote access → page. The rotation-prone
    # rest (.env, config.toml, cli-client-id) is noisier → digest.
    agent_authfile_changed)
      # A content change emits removed{old hash} + added{new hash} on the same path; the
      # change is carried by the added row, so the removed row is a pure duplicate. Guard
      # on added so a rotation neither double-pages nor writes two digest lines.
      [[ $(jq -r '.act' <<<"$obj") == added ]] || continue
      case "$(jq -r '.cols.path // ""' <<<"$obj")" in
        */webhook-secret | */daemon-keypair.json) sev="CRIT" ;;
        *)
          _digest_append "$obj"
          continue
          ;;
      esac
      ;;
    # Firewall and Gatekeeper transitions are owned by the dedicated 60s poller
    # (osquery-firewall-gatekeeper-monitor.sh), which pages the moment a protection
    # flips off. The security-policy pack ALSO runs differential firewall_state /
    # gatekeeper_state queries; routing those to log-only here keeps one disable
    # event from firing a second #priority page. The poller is the single owner.
    firewall_state | gatekeeper_state) continue ;;
    # SIP is intentionally off on this developer box: an on->off transition cannot
    # occur, so the snapshot floor is pure noise. Log-only (no page, no digest).
    sip_state) continue ;;
    # The kernel_extensions table lists LOADED kexts (load/unload on demand) — a
    # firehose of hundreds of events. Wrong signal entirely; log-only.
    kernel_extensions_new) continue ;;
    # Startup-item / crontab churn is log-only per the tier matrix (too noisy to page).
    # Without an explicit arm it falls through to the enricher, which promotes an unsigned
    # path NOTICE -> CRIT and pages — give it the same continue its noisy siblings have.
    persistence_startup_items_crontab) continue ;;
    # A high-risk remote-access service (screen sharing, remote management, remote
    # apple events, internet sharing) newly enabled — a remote-control path opened
    # into this Mac. SSH/Remote Login is the operator's own access path and is
    # excluded by the query. The query emits a row per ENABLED service, so an
    # "added" row is an ON transition (page); a "removed" row is a service turning
    # OFF (good news) → log-only, never a "service enabled" page.
    remote_access_sharing_state)
      case "$(jq -r '.act' <<<"$obj")" in
        added) sev="CRIT" ;;
        *) continue ;;
      esac
      ;;
    # Endpoint-Security launchd writes are forensic enrichment only (the writer
    # process), not a deliverable signal on their own. Log-only.
    es_launchd_writes) continue ;;
    persistence_launchd)
      # Only a NEW persistence item is actionable; a removed row is a deletion (an
      # uninstall or cleanup — e.g. removing Docker/VPN drops its LaunchDaemon), never
      # a "new startup item". Guard the whole arm on the added transition.
      [[ $(jq -r '.act' <<<"$obj") == added ]] || continue
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
    # A NEW off-loopback listener (something started exposing a port) is generic
    # exposure awareness the agent-pattern page detector deliberately does not cover —
    # a calm daily heads-up, not a page. Only the "added" direction: a removed row is a
    # listener going away (the exposure closing) and stays log-only.
    listening_ports_non_loopback)
      [[ $(jq -r '.act' <<<"$obj") == added ]] || continue
      _digest_append "$obj"
      continue
      ;;
    # Agent binary hash changes cannot distinguish a frequent legit self-update from a
    # swap, so they are inherently noisy — log-only (recorded in results.log for
    # forensics, never paged or digested).
    agent_binary_changed) continue ;;
    # Screen lock off is posture drift, not an intrusion — low actionability.
    screenlock_state)
      _digest_append "$obj"
      continue
      ;;
    # file_events fans out by category, then by target path within the category (FX3:
    # the watches are containing directories now, so a category fires for every file in
    # the dir). FX1: every production FSEvents verb is actionable — CREATED, UPDATED,
    # MOVED_TO (atomic replacement, the dominant live verb), ROOT_CHANGED (a parent dir
    # renamed), ATTRIBUTES_MODIFIED (chmod/chown/xattr), and DELETED (destructive). The
    # old CREATED/UPDATED-only filter dropped the rest, so the tamper detector and the
    # sshd page could never fire. No verb is dropped here; the category + target decide
    # the tier, and an unknown verb in a paging category pages conservatively.
    file_events_recent)
      target=$(jq -r '.cols.target_path // ""' <<<"$obj")
      base=${target##*/}
      case "$cat" in
        # ~/.ssh directory watch (FX3): authorized_keys{,2} are remote-auth entry points
        # → page on any verb (DELETED included: removing the key file can be an attacker
        # locking the operator out). Every other ~/.ssh file (private keys, config,
        # known_hosts) is sensitive but operator-churned → digest (restored broad
        # coverage the exact-file narrowing had lost).
        ssh)
          case "$base" in
            authorized_keys | authorized_keys2) sev="CRIT" ;;
            *)
              _digest_append "$obj"
              continue
              ;;
          esac
          ;;
        # sshd_config: remote-auth policy → page on any verb.
        sshd_config) sev="CRIT" ;;
        # pipeline_integrity (~/.local/bin watch) and our own LaunchAgents (launch_agents
        # / launch_daemons watch) are the alerter's own tooling. The verdict filters to
        # the tracked set by basename and validates the exact (path, sha256) tuple: a
        # legit apply whose tuple is known-good is silent; anything unconfirmable (empty
        # hash, mismatch, missing manifest) pages. Never digests — page or silent.
        pipeline_integrity | launch_agents | launch_daemons)
          hash_value=$(jq -r '.cols.sha256 // ""' <<<"$obj")
          if _pipeline_verdict "$target" "$hash_value"; then sev="CRIT"; else continue; fi
          ;;
        # sudoers churns (visudo / chezmoi) → digest on any verb.
        sudoers)
          _digest_append "$obj"
          continue
          ;;
        # ~/.config/osquery watch (FX3): only the page-allowlist itself is the
        # security-relevant page-suppressor edit → digest. Neighbors (e.g. webhook-secret,
        # covered by agent_authfile_changed) are log-only here.
        allowlist_file)
          case "$base" in
            page-launchd-allowlist.txt)
              _digest_append "$obj"
              continue
              ;;
            *) continue ;;
          esac
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

# Render the #priority page body in one jq pass: focused labeled blocks (header +
# decision-relevant fields + one "→" next step). Layout follows the user's ADHD
# surfacing research: one thing, glanceable, minimal fields, ending in a single
# action, no raw query jargon. v2 renders only this body — there is no #osquery line.
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
    elif .q == "new_admin_user" then "New administrator account"
    elif .q == "agent_exposure_changed" then "Agent port exposed off-loopback"
    elif .q == "agent_authfile_changed" then "Agent credential changed"
    elif .q == "remote_access_sharing_state" then "Remote-access service enabled"
    elif .q == "kernel_extensions_new" then "New kernel extension"
    elif .q == "system_extensions_new" then "New system extension"
    elif .q == "listening_ports_non_loopback" then "New network listener"
    elif .q == "recent_logins" then "Login"
    elif .q == "installed_apps" then "New app"
    elif .q == "homebrew_packages" then "New Homebrew package"
    elif (.q | test("_extensions$|_addons$")) then "New browser extension"
    elif .q == "file_events_recent" then
      ((.cols.category // "") as $cat | ((.cols.target_path // "") | split("/") | last) as $bn |
        # A tracked pipeline file can arrive under pipeline_integrity OR (for our own
        # LaunchAgents) launch_agents/launch_daemons, so key the tooling header on the
        # basename, not only the category.
        if ($bn | test("^osquery-.*\\.sh$")) or ($bn | test("^com\\.webdavis\\.osquery-.*\\.plist$")) then "Security tooling changed"
        elif ($cat == "ssh" or $cat == "authorized_keys") then "SSH key file changed"
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
    # Strip markdown metacharacters from the (attacker-influenceable) signing authority
    # so a crafted certificate subject cannot inject backticks/emphasis into the body.
    # Every other rendered value already goes through `code`; this is the lone exception.
    .cols as $c | ((.signing // null) | if type == "string" then gsub("[`*]"; "") else . end) as $sig |
    (if $sig then
       (if ($sig | test("unsigned|untrusted|ad-hoc|unverified|no authority"; "i"))
        then ["- **Signing:** ⚠️ **\($sig)**"] else ["- **Signing:** \($sig)"] end)
     else [] end) as $sg |
    if .q == "persistence_launchd" then ["- **What:** \(($c.label // "?") | code)", "- **Program:** \(($c.program // "?") | code)"] + $sg
    elif .q == "persistence_startup_items_crontab" then ["- **What:** \(($c.name // "?") | code)", "- **Command:** \(($c.command // "?") | code)"] + $sg
    elif .q == "suid_bin_unexpected" then ["- **Path:** \(($c.path // "?") | code)"] + $sg + ["- **Owner:** \(($c.username // "?") | code)"]
    elif .q == "new_admin_user" then ["- **User:** \(($c.username // "?") | code)", "- **UID:** \(($c.uid // "?") | code)"]
    elif .q == "agent_exposure_changed" then ["- **Process:** \(($c.name // "?") | code)", "- **Address:** \(($c.address // "?") | code)", "- **Port:** \(($c.port // "?") | code)"]
    elif .q == "agent_authfile_changed" then ["- **File:** \((($c.path // "") | split("/") | last) | code)"]
    elif .q == "remote_access_sharing_state" then ["- **Service:** \(($c.service // "?") | code)"]
    elif .q == "system_extensions_new" then ["- **Name:** \(($c.identifier // "?") | code)", "- **Team:** \(($c.team // "?") | code)"] + $sg
    elif .q == "kernel_extensions_new" then ["- **Name:** \(($c.name // "?") | code)", "- **Path:** \(($c.path // "?") | code)"] + $sg
    elif .q == "file_events_recent" then ["- **File:** \(($c.target_path // "?") | code)", "- **Action:** \($c.action // .act)"]
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
    elif .q == "new_admin_user" then
      ["- Did you create this account? If not, someone gained **admin access** — investigate now.", "- Review accounts: System Settings → Users & Groups"]
    elif .q == "agent_exposure_changed" then
      ["- Did you expose this? If not, an agent API is reachable **off-box** — close it now.", "- Re-bind it to 127.0.0.1 or block the port at the firewall."]
    elif .q == "agent_authfile_changed" then
      ["- Did you rotate this? If not, an attacker may forge or mute alerts, or hijack remote access — **investigate now**."]
    elif .q == "remote_access_sharing_state" then
      ["- Did you enable this? If not, someone opened a remote-control path into this Mac — **disable it now**.", "- System Settings → General → Sharing"]
    elif .q == "file_events_recent" then
      (((.cols.target_path // "") | split("/") | last) as $bn |
       if ((.cols.category // "") == "pipeline_integrity")
          or ($bn | test("^osquery-.*\\.sh$")) or ($bn | test("^com\\.webdavis\\.osquery-.*\\.plist$"))
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
    # Cap the page at eight blocks + a marker so a large simultaneous-CRIT batch cannot
    # exceed the Discord 2000-char limit and get stuck undelivered in the spool (an
    # over-length POST is rejected and re-spooled forever — retry never shrinks it). The
    # dropped detail still lands in results.log. Mirrors the digest group cap.
    pbody: (
      ($crit[0:8] | map(block) | join("\n\n"))
      + (if ($crit | length) > 8
         then "\n\n… and \(($crit | length) - 8) more CRITICAL finding(s) — see results.log"
         else "" end)
    )
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
