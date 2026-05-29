#!/usr/bin/env bash

# Strict mode.
set -euo pipefail

results_log="${OSQUERY_RESULTS_LOG:-$HOME/.local/log/osquery/osqueryd.snapshots.log}"
report_dir="${OSQUERY_REPORT_DIR:-$HOME/workspaces/Ivy/security/osquery}"
report_file="$report_dir/$(date +%Y-%m-%d).md"

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

# Get the most recent snapshot for a given query name from the results log.
get_latest_snapshot() {
  tail -r "$results_log" | grep -m 1 "\"name\":\"$1\"" || true
}

# ---------------------------------------------------------------------------
# Renderers — one per scheduled query. Each takes the snapshot line (JSON) as
# $1 and writes a markdown section body to stdout. Sections are dispatched
# via render_section(); to add a new query, write a renderer and add a
# render_section line in the build block at the bottom of the script.
# ---------------------------------------------------------------------------

render_sip_state() {
  local enabled
  enabled=$(jq -r '.snapshot[0].enabled' <<<"$1")
  if [[ $enabled == "1" ]]; then
    echo "- Status: **enabled**"
  else
    echo "- Status: **disabled** (intentional on this host)"
  fi
}

render_firewall_state() {
  local global stealth logging version state_text
  global=$(jq -r '.snapshot[0].global_state' <<<"$1")
  stealth=$(jq -r '.snapshot[0].stealth_enabled' <<<"$1")
  logging=$(jq -r '.snapshot[0].logging_enabled' <<<"$1")
  version=$(jq -r '.snapshot[0].version' <<<"$1")
  case "$global" in
    0) state_text="**OFF**" ;;
    1) state_text="on (allow signed apps)" ;;
    2) state_text="on (block all)" ;;
    *) state_text="unknown ($global)" ;;
  esac
  echo "- State: $state_text"
  echo "- Stealth mode: $stealth"
  echo "- Logging: $logging"
  echo "- alf version: $version"
}

render_gatekeeper_state() {
  local assess devid version
  assess=$(jq -r '.snapshot[0].assessments_enabled' <<<"$1")
  devid=$(jq -r '.snapshot[0].dev_id_enabled' <<<"$1")
  version=$(jq -r '.snapshot[0].version' <<<"$1")
  if [[ $assess == "1" ]]; then
    echo "- Assessments: **enabled**"
  else
    echo "- Assessments: **DISABLED**"
  fi
  echo "- Dev-ID: $devid"
  echo "- Version: $version"
}

render_listening_non_localhost() {
  local rows
  rows=$(jq -r '.snapshot | length' <<<"$1")
  echo "Count: $rows"
  if [[ $rows == "0" ]]; then return; fi
  echo ""
  echo "| Process | PID | Address | Port | Proto |"
  echo "|---------|-----|---------|------|-------|"
  jq -r '.snapshot[] | "| \(.process_name // "?") | \(.pid) | \(.address) | \(.port) | \(.protocol) |"' <<<"$1"
}

render_startup_items() {
  local rows
  rows=$(jq -r '.snapshot | length' <<<"$1")
  if [[ $rows == "0" ]]; then
    echo "_None._"
    return
  fi
  echo "| Name | Type | Path | User |"
  echo "|------|------|------|------|"
  jq -r '.snapshot[] | "| \(.name) | \(.type) | \(.path) | \(.username // "?") |"' <<<"$1"
}

render_non_apple_launchd() {
  local rows
  rows=$(jq -r '.snapshot | length' <<<"$1")
  echo "Count: $rows"
  if [[ $rows == "0" ]]; then return; fi
  echo ""
  echo "| Label | RunAtLoad | KeepAlive |"
  echo "|-------|-----------|-----------|"
  jq -r '.snapshot[] | "| \(.label) | \(.run_at_load // "—") | \(.keep_alive // "—") |"' <<<"$1"
}

render_launchd_overrides() {
  local rows
  rows=$(jq -r '.snapshot | length' <<<"$1")
  if [[ $rows == "0" ]]; then
    echo "_None._"
    return
  fi
  echo "| Label | Key | Value |"
  echo "|-------|-----|-------|"
  jq -r '.snapshot[] | "| \(.label) | \(.key) | \(.value) |"' <<<"$1"
}

render_ssh_authorized_keys() {
  local rows
  rows=$(jq -r '.snapshot | length' <<<"$1")
  echo "Count: $rows"
  if [[ $rows == "0" ]]; then return; fi
  echo ""
  echo "| User | Algorithm | Key File | Key Prefix |"
  echo "|------|-----------|----------|------------|"
  jq -r '.snapshot[] | "| \(.username) | \(.algorithm) | \(.key_file) | `\(.key_prefix)…` |"' <<<"$1"
}

render_sudoers_rules() {
  local rows
  rows=$(jq -r '.snapshot | length' <<<"$1")
  echo "Count: $rows"
  if [[ $rows == "0" ]]; then return; fi
  echo ""
  echo "| Header | Rule |"
  echo "|--------|------|"
  jq -r '.snapshot[] | "| \(.header) | \(.rule_details) |"' <<<"$1"
}

render_system_extensions() {
  local rows
  rows=$(jq -r '.snapshot | length' <<<"$1")
  echo "Count: $rows"
  if [[ $rows == "0" ]]; then return; fi
  echo ""
  echo "| Identifier | Category | MDM | State |"
  echo "|------------|----------|-----|-------|"
  jq -r '.snapshot[] | "| \(.identifier) | \(.category) | \(.mdm_managed) | \(.state // "?") |"' <<<"$1"
}

# Dispatcher: render_section <query_name> <section_title> <renderer_func>.
# Skips silently if no snapshot for that query is in the log yet.
render_section() {
  local name="$1" title="$2" renderer="$3"
  local line
  line=$(get_latest_snapshot "$name")
  [[ -z $line ]] && return 0
  echo ""
  echo "### $title"
  echo ""
  "$renderer" "$line"
  echo ""
}

# Idempotency: anchor on firewall_state (always present in this pack).
fw_line=$(get_latest_snapshot "firewall_state")
if [[ -z $fw_line ]]; then
  echo "No firewall_state snapshot found; daemon may not have run since config change"
  exit 1
fi
unix_time=$(jq -r '.unixTime' <<<"$fw_line")
if grep -qF "<!-- ts:$unix_time -->" "$report_file"; then
  echo "Report for timestamp $unix_time already exists, skipping"
  exit 0
fi

timestamp=$(date -r "$unix_time" '+%Y-%m-%d %H:%M:%S')

{
  echo ""
  echo "## $timestamp <!-- ts:$unix_time -->"

  render_section sip_state "SIP" render_sip_state
  render_section firewall_state "Application Firewall" render_firewall_state
  render_section gatekeeper_state "Gatekeeper" render_gatekeeper_state
  render_section listening_non_localhost "Listening Ports (non-loopback)" render_listening_non_localhost
  render_section startup_items "Startup Items" render_startup_items
  render_section non_apple_launchd "Non-Apple launchd Jobs" render_non_apple_launchd
  render_section launchd_overrides "launchd Overrides" render_launchd_overrides
  render_section ssh_authorized_keys "SSH Authorized Keys" render_ssh_authorized_keys
  render_section sudoers_rules "Sudoers Rules" render_sudoers_rules
  render_section system_extensions "System Extensions" render_system_extensions
} >>"$report_file"

echo "Report appended to $report_file"

# ---------------------------------------------------------------------------
# Alert tier — paging-worthy states. SIP intentionally excluded (off on this
# host by design). FileVault excluded because the disk_encryption table is
# unreliable on Apple Silicon (always reports encrypted=0 even when fdesetup
# status says On); the homelab fleet design will pick up FileVault state via
# MDM, not osquery.
# ---------------------------------------------------------------------------

alert_msg=""
fw_global=$(jq -r '.snapshot[0].global_state' <<<"$fw_line")
if [[ $fw_global == "0" ]]; then
  alert_msg="Application Firewall is OFF"
fi

gk_line=$(get_latest_snapshot "gatekeeper_state")
if [[ -n $gk_line ]]; then
  gk_assess=$(jq -r '.snapshot[0].assessments_enabled' <<<"$gk_line")
  if [[ $gk_assess == "0" ]]; then
    if [[ -n $alert_msg ]]; then
      alert_msg="${alert_msg}; "
    fi
    alert_msg="${alert_msg}Gatekeeper assessments DISABLED"
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
