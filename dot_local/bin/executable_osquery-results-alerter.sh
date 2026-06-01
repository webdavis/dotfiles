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
if [[ ! -f $STATE ]] || ! read -r prev_inode prev_offset <"$STATE" ||
  ! [[ $prev_inode =~ ^[0-9]+$ && $prev_offset =~ ^[0-9]+$ ]]; then
  printf '%s %s\n' "$inode" "$size" >"$STATE"
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
new_lines=$(tail -c "+$((prev_offset + 1))" "$LOG" | head -c "$((size - prev_offset))" || true)
findings=$(printf '%s\n' "$new_lines" | jq -rR '
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
       or (.name == "pack_intrusion-detection_suid_bin_unexpected")
       or (.name == "file_events_recent" and ((.columns.category // "") | test("^(ssh|sudoers|sshd_config)$")))
    then "CRIT"
    elif (.name | startswith("pack_security-policy-regression_"))
       or (.name | test("^pack_intrusion-detection_persistence_"))
       or (.name == "pack_intrusion-detection_kernel_extensions_new")
       or (.name == "pack_intrusion-detection_system_extensions_new")
       or (.name == "file_events_recent")
    then "NOTICE"
    else "INFO" end;
  # Snapshot rows (absolute-state floor *_off queries) log under a "snapshot"
  # array with action "snapshot"; explode each into a synthetic "currently-off"
  # row so the rest of the pipeline treats it uniformly. Empty snapshot = silent.
  (if .action == "snapshot"
   then (.name as $n | .snapshot[]? | {name: $n, action: "currently-off", columns: .})
   else . end)
  | select(.name != null and ((.name | startswith("pack_")) or (.name == "file_events_recent")))
  | select((.columns.target_path // "") | test("/\\.renameio-TempDir") | not)
  | (.name | sub("^pack_[^_]+_"; "")) as $q
  | (.action // "changed") as $act
  | (.columns.target_path // .columns.label // .columns.identifier
     // .columns.name // .columns.path // .columns.username
     // ((.columns // {}) | to_entries | map("\(.key)=\(.value)") | join(", "))) as $raw
  | ($raw | gsub("[\t\n]"; " ")) as $id
  | "\(sev)\t\($q): \($act) \($id)"
' 2>/dev/null || true)

# Advance state before notifying so a slow/failed dispatch never re-fires the
# same batch on the next WatchPaths trigger. Format: "<inode> <offset>".
printf '%s %s\n' "$inode" "$size" >"$STATE"

[[ -z $findings ]] && exit 0

total=$(printf '%s\n' "$findings" | grep -c '' || true)

# Highest severity present sets the headline + sound (INFO is silent).
if printf '%s\n' "$findings" | grep -q '^CRIT'; then
  sev_word="CRITICAL"
  sev_emoji="🔴"
  sound="Sosumi"
elif printf '%s\n' "$findings" | grep -q '^NOTICE'; then
  sev_word="Notice"
  sev_emoji="🟡"
  sound="Glass"
else
  sev_word="Info"
  sev_emoji="🔵"
  sound=""
fi
# Pluralize without a command-substitution: under set -e, title="$([ ] && echo s)"
# aborts the script when total==1 (the test that surfaced this), silently
# dropping a single-finding alert after state was already advanced.
plural=""
if [[ $total -ne 1 ]]; then plural="s"; fi
title="$sev_emoji $sev_word — $total security event$plural"

# Order lines CRIT -> NOTICE -> INFO, prefix each with its emoji, cap at 12.
detail=$(printf '%s\n' "$findings" | awk -F'\t' '
  BEGIN {
    ord["CRIT"] = 1; ord["NOTICE"] = 2; ord["INFO"] = 3
    em["CRIT"] = "🔴"; em["NOTICE"] = "🟡"; em["INFO"] = "🔵"
  }
  { sv[NR] = $1; tx[NR] = $2 }
  END { for (p = 1; p <= 3; p++) for (i = 1; i <= NR; i++) if (ord[sv[i]] == p) print em[sv[i]] " " tx[i] }
')
if [[ $total -gt 12 ]]; then
  detail="$(printf '%s\n' "$detail" | head -12)"$'\n'"… and $((total - 12)) more"
fi

send_alert "$title" "$detail" "$sound"
