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

# New differential rows. Exclude chezmoi's renameio temp-file churn. Render
# each as "<query without pack prefix>: <added|removed> <best identifier>".
new_lines=$(tail -c "+$((prev_offset + 1))" "$LOG")
findings=$(printf '%s\n' "$new_lines" | jq -r '
  select(.name != null and (.name | startswith("pack_") or . == "file_events_recent"))
  | select((.columns.target_path // "") | test("/\\.renameio-TempDir") | not)
  | (.name | sub("^pack_[^_]+_"; "")) as $q
  | (.action // "changed") as $act
  | (.columns.target_path // .columns.label // .columns.identifier
     // .columns.name // .columns.path // .columns.username
     // (.columns | to_entries | map("\(.key)=\(.value)") | join(", "))) as $id
  | "\($q): \($act) \($id)"
' 2>/dev/null || true)

# Advance the offset before notifying so a slow/failed dispatch never re-fires
# the same batch on the next WatchPaths trigger.
echo "$size" >"$STATE"

[[ -z $findings ]] && exit 0

count=$(printf '%s\n' "$findings" | grep -c '' || true)
sample=$(printf '%s\n' "$findings" | head -8)

title="osquery: $count change$([ "$count" -ne 1 ] && echo s)"
if [[ $count -le 8 ]]; then
  detail="$sample"
else
  detail="$sample"$'\n'"… and $((count - 8)) more"
fi

# Use the attention sound when a security-policy-regression row is in the batch
# (a protection may have flipped); otherwise the default.
sound="Glass"
printf '%s\n' "$findings" | grep -qiE 'firewall_state|gatekeeper_state|sip_state|filevault_state|screenlock_state|remote_access' && sound="Sosumi"

send_alert "$title" "$detail" "$sound"
