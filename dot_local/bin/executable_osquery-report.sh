#!/usr/bin/env bash

# Strict mode.
set -euo pipefail

results_log="${OSQUERY_RESULTS_LOG:-$HOME/.local/log/osquery/osqueryd.snapshots.log}"
report_dir="${OSQUERY_REPORT_DIR:-$HOME/workspaces/Ivy/security/osquery}"
report_file="$report_dir/$(date +%Y-%m-%d).md"

zombie_threshold=50
process_threshold=2132 # 80% of 2666

if [[ ! -f $results_log ]]; then
  echo "No results log found at $results_log"
  exit 1
fi

mkdir -p "$report_dir"

# Initialize new report files with vault-compliant frontmatter.
if [[ ! -f $report_file ]]; then
  today_date=$(date +%Y-%m-%d)
  today_time=$(date +%H:%M:%S)
  cat >"$report_file" <<EOF
---
createdDate: '[[$today_date]]'
createdTime: $today_time
hub: '[[osquery]]'
tags:
  - osqueryLog
status: active
startDate: $today_date
---
EOF
fi

# Get the most recent snapshot for a given query name.
# Args: $1 = query name
get_latest_snapshot() {
  tail -r "$results_log" | grep -m 1 "\"name\":\"$1\"" || true
}

zombie_line=$(get_latest_snapshot "zombie_count")
zombie_parents_line=$(get_latest_snapshot "zombie_parents")
top_procs_line=$(get_latest_snapshot "top_processes_by_count")
per_user_line=$(get_latest_snapshot "per_user_process_count")
memory_line=$(get_latest_snapshot "top_memory_hogs")

if [[ -z $zombie_line && -z $per_user_line ]]; then
  echo "No osquery snapshots found in results log"
  exit 1
fi

# Extract unixTime from the zombie_count snapshot for idempotency.
unix_time=""
if [[ -n $zombie_line ]]; then
  unix_time=$(echo "$zombie_line" | jq -r '.unixTime')
fi

# Check idempotency: skip if this timestamp is already in the report file.
if [[ -n $unix_time && -f $report_file ]]; then
  if grep -qF "<!-- ts:$unix_time -->" "$report_file"; then
    echo "Report for timestamp $unix_time already exists, skipping"
    exit 0
  fi
fi

timestamp=$(date -r "$unix_time" '+%Y-%m-%d %H:%M:%S' 2>/dev/null || date '+%Y-%m-%d %H:%M:%S')

# Build the report section.
{
  echo ""
  echo "## $timestamp <!-- ts:$unix_time -->"
  echo ""

  # Zombie count
  if [[ -n $zombie_line ]]; then
    zombie_count=$(echo "$zombie_line" | jq -r '.snapshot[0].zombie_count // "0"')
    if [[ $zombie_count -gt $zombie_threshold ]]; then
      echo "**Zombie processes: $zombie_count** (ABOVE THRESHOLD of $zombie_threshold)"
    else
      echo "**Zombie processes: $zombie_count**"
    fi
    echo ""
  fi

  # Zombie parents
  if [[ -n $zombie_parents_line ]]; then
    zombie_parents_count=$(echo "$zombie_parents_line" | jq '.snapshot | length')
    if [[ $zombie_parents_count -gt 0 ]]; then
      echo "### Zombie Parents"
      echo ""
      echo "| Zombie PID | Zombie Name | Parent PID | Parent Name |"
      echo "|------------|-------------|------------|-------------|"
      echo "$zombie_parents_line" | jq -r '.snapshot[] | "| \(.zombie_pid) | \(.zombie_name) | \(.parent_pid) | \(.parent_name) |"'
      echo ""
    fi
  fi

  # Total process count (sum of per-user counts)
  if [[ -n $per_user_line ]]; then
    total_procs=$(echo "$per_user_line" | jq '[.snapshot[].process_count | tonumber] | add // 0')
    if [[ $total_procs -gt $process_threshold ]]; then
      echo "**Total processes: $total_procs** (ABOVE THRESHOLD of $process_threshold)"
    else
      echo "**Total processes: $total_procs**"
    fi
    echo ""

    echo "### Per-User Process Count"
    echo ""
    echo "| User | Count |"
    echo "|------|-------|"
    echo "$per_user_line" | jq -r '.snapshot[] | "| \(.username // "unknown") | \(.process_count) |"'
    echo ""
  fi

  # Top processes by count
  if [[ -n $top_procs_line ]]; then
    echo "### Top Processes by Count"
    echo ""
    echo "| Name | Count |"
    echo "|------|-------|"
    echo "$top_procs_line" | jq -r '.snapshot[] | "| \(.name) | \(.count) |"'
    echo ""
  fi

  # Top memory hogs
  if [[ -n $memory_line ]]; then
    echo "### Top Memory Hogs"
    echo ""
    echo "| Name | MB |"
    echo "|------|----|"
    echo "$memory_line" | jq -r '.snapshot[] | "| \(.name) | \(.mb) |"'
    echo ""
  fi
} >>"$report_file"

echo "Report appended to $report_file"

# Alert if thresholds exceeded.
alert_msg=""
if [[ -n $zombie_line ]]; then
  zombie_count=$(echo "$zombie_line" | jq -r '.snapshot[0].zombie_count // "0"')
  if [[ $zombie_count -gt $zombie_threshold ]]; then
    alert_msg="Zombie processes: $zombie_count (threshold: $zombie_threshold)"
  fi
fi

if [[ -n $per_user_line ]]; then
  total_procs=$(echo "$per_user_line" | jq '[.snapshot[].process_count | tonumber] | add // 0')
  if [[ $total_procs -gt $process_threshold ]]; then
    if [[ -n $alert_msg ]]; then
      alert_msg="$alert_msg\n"
    fi
    alert_msg="${alert_msg}Total processes: $total_procs (threshold: $process_threshold)"
  fi
fi

if [[ -n $alert_msg ]]; then
  if command -v alerter &>/dev/null; then
    alerter --timeout 60 --title "osquery Alert" --message "$alert_msg" --sound Sosumi 2>/dev/null &
  else
    osascript -e "display notification \"$alert_msg\" with title \"osquery Alert\" sound name \"Sosumi\""
  fi
fi

# Clean up old reports (older than 30 days).
find "$report_dir" -name "*.md" -mtime +30 -delete
