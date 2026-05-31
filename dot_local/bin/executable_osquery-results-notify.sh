#!/usr/bin/env bash
#
# osquery-results-notify.sh — fired by launchd (WatchPaths) whenever
# ~/.local/log/osquery/osqueryd.results.log changes. Reads new lines since the
# last run (byte-offset state file), and surfaces every differential finding
# from the scheduled packs (intrusion-detection, security-policy-regression,
# installed-software-drift) AND the file-events query. Each batch becomes a
# single notification, delivered to both the local notifier and the #osquery
# Discord channel via osquery-send-alert.sh.
#
# Supersedes the former osquery-fim-notify.sh (which watched the same log but
# only handled file-events).

set -euo pipefail

LOG="${OSQUERY_RESULTS_LOG:-$HOME/.local/log/osquery/osqueryd.results.log}"
STATE="${OSQUERY_RESULTS_OFFSET:-$HOME/.local/state/osquery-results-offset}"

# shellcheck source=/dev/null
source "$HOME/.local/bin/osquery-send-alert.sh"

mkdir -p "$(dirname "$STATE")"
[[ -f $LOG ]] || exit 0

size=$(stat -f %z "$LOG")
prev_offset=$(cat "$STATE" 2>/dev/null || echo 0)

# Log rotated/truncated — reset to current EOF, don't replay old content.
if [[ $size -lt $prev_offset ]]; then
  prev_offset=$size
fi

# First-ever run: seed offset at EOF and stay silent (no backlog storm).
if [[ ! -f $STATE ]]; then
  echo "$size" >"$STATE"
  exit 0
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
new_lines=$(tail -c "+$((prev_offset + 1))" "$LOG")
findings=$(printf '%s\n' "$new_lines" | jq -r '
  def sev:
    if (.name | startswith("pack_security-policy-regression_"))
       or (.name == "pack_intrusion-detection_suid_bin_unexpected")
       or (.name == "file_events_recent" and ((.columns.category // "") | test("^(ssh|sudoers)$")))
    then "CRIT"
    elif (.name | test("^pack_intrusion-detection_persistence_"))
       or (.name == "pack_intrusion-detection_kernel_extensions_new")
       or (.name == "pack_intrusion-detection_system_extensions_new")
       or (.name == "file_events_recent")
    then "NOTICE"
    else "INFO" end;
  select(.name != null and ((.name | startswith("pack_")) or (.name == "file_events_recent")))
  | select((.columns.target_path // "") | test("/\\.renameio-TempDir") | not)
  | (.name | sub("^pack_[^_]+_"; "")) as $q
  | (.action // "changed") as $act
  | (.columns.target_path // .columns.label // .columns.identifier
     // .columns.name // .columns.path // .columns.username
     // (.columns | to_entries | map("\(.key)=\(.value)") | join(", "))) as $id
  | "\(sev)\t\($q): \($act) \($id)"
' 2>/dev/null || true)

# Advance the offset before notifying so a slow/failed dispatch never re-fires
# the same batch on the next WatchPaths trigger.
echo "$size" >"$STATE"

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
title="$sev_emoji $sev_word — $total security event$([ "$total" -ne 1 ] && echo s)"

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
