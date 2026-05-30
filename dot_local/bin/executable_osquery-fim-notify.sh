#!/usr/bin/env bash
#
# osquery-fim-notify.sh — fired by launchd whenever
# ~/.local/log/osquery/osqueryd.results.log changes. Reads new lines since the
# last invocation (tracked via a byte-offset state file), filters for
# file_events_recent rows, and fires alerter with a batched summary so a
# single FIM burst becomes one notification rather than N.

set -euo pipefail

LOG="${OSQUERY_EVENTS_LOG:-$HOME/.local/log/osquery/osqueryd.results.log}"
STATE="${OSQUERY_FIM_STATE:-$HOME/.local/state/osquery-fim-offset}"

mkdir -p "$(dirname "$STATE")"

# Nothing to do if the log doesn't exist yet.
[[ -f $LOG ]] || exit 0

size=$(stat -f %z "$LOG")
prev_offset=$(cat "$STATE" 2>/dev/null || echo 0)

# Log was rotated or truncated — reset to current EOF and don't notify on
# whatever's already there (avoids a notification storm when the agent is
# first installed or when osqueryd recycles its log).
if [[ $size -lt $prev_offset ]]; then
  prev_offset=$size
fi

# First-ever run: seed the offset at EOF and exit. This means we won't fire
# notifications for events that already happened before the agent was loaded.
if [[ ! -f $STATE ]]; then
  echo "$size" >"$STATE"
  exit 0
fi

# Nothing new since last fire.
if [[ $size -eq $prev_offset ]]; then
  exit 0
fi

# Slurp new bytes. Parse each line as JSON; collect file_events_recent rows.
# chezmoi's atomic-write library (renameio) creates short-lived /.renameio-
# TempDir*/ files inside any watched directory; filter those out so a routine
# `chezmoi apply` doesn't spam notifications.
new_lines=$(tail -c "+$((prev_offset + 1))" "$LOG")
events=$(printf '%s\n' "$new_lines" | jq -cr '
  select(.name == "file_events_recent") |
  select((.columns.target_path // "") | test("/\\.renameio-TempDir") | not)
' 2>/dev/null || true)

# Update offset to current EOF before doing anything else, so even if alerter
# fails we won't re-notify on the same batch.
echo "$size" >"$STATE"

# No matching events — exit silently. (Other query results may have appeared
# in the log; we ignore them here.)
[[ -z $events ]] && exit 0

count=$(printf '%s\n' "$events" | wc -l | tr -d ' ')

# Build a one-line summary plus up to 3 example paths for the message body.
categories=$(printf '%s\n' "$events" |
  jq -r '.columns.category' | sort -u | paste -sd, -)
sample=$(printf '%s\n' "$events" |
  jq -r '.columns | "\(.category): \(.action) \(.target_path)"' | head -3)

if [[ $count -le 3 ]]; then
  msg="$sample"
else
  msg="$count events across [$categories]"$'\n'"$(printf '%s\n' "$sample")"$'\n'"…"
fi

title="FIM ($count event$([ "$count" -ne 1 ] && echo s))"

if command -v alerter &>/dev/null; then
  alerter --timeout 30 --title "$title" --message "$msg" --sound Glass 2>/dev/null &
else
  # AppleScript fallback truncates at ~250 chars; that's fine for the summary.
  escaped=${msg//\"/\\\"}
  osascript -e "display notification \"$escaped\" with title \"$title\" sound name \"Glass\""
fi
