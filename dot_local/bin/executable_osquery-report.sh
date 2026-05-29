#!/usr/bin/env bash

# Strict mode.
set -euo pipefail

results_log="${OSQUERY_RESULTS_LOG:-$HOME/.local/log/osquery/osqueryd.snapshots.log}"
events_log="${OSQUERY_EVENTS_LOG:-$HOME/.local/log/osquery/osqueryd.results.log}"
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

# ---------------------------------------------------------------------------
# Snapshot fetchers — pull the Nth-most-recent line for a query from the
# snapshots log. N=1 is the current tick, N=2 is the previous tick. Returns
# the .snapshot JSON array (or "[]" if no snapshot exists).
# ---------------------------------------------------------------------------

get_snapshot_line() {
  local name="$1" nth="${2:-1}"
  tail -r "$results_log" | grep "\"name\":\"$name\"" | sed -n "${nth}p" || true
}

get_snapshot_rows() {
  local line
  line=$(get_snapshot_line "$1" "${2:-1}")
  if [[ -z $line ]]; then
    echo "[]"
  else
    jq -c '.snapshot // []' <<<"$line"
  fi
}

# Row count for a query at the Nth-most-recent snapshot.
count_rows() {
  jq 'length' <<<"$(get_snapshot_rows "$1" "${2:-1}")"
}

# Render "(+N)" / "(-N)" / "" suffix based on delta vs the previous tick.
count_delta_suffix() {
  local name="$1" cur prev
  cur=$(count_rows "$name" 1)
  prev=$(count_rows "$name" 2)
  if [[ $prev -eq 0 ]] && [[ $cur -eq 0 ]]; then
    return
  fi
  local delta=$((cur - prev))
  if [[ $delta -gt 0 ]]; then
    printf ' (+%d)' "$delta"
  elif [[ $delta -lt 0 ]]; then
    printf ' (%d)' "$delta"
  fi
}

# ---------------------------------------------------------------------------
# Diff helpers — emit markdown delta blocks for one inventory query. Each
# function:
#   - returns 0 with no output when nothing changed,
#   - returns 0 with a block of "**Title (+a, -r):**\n- ➕ ...\n- ➖ ..."
#     when changes exist.
# The composite-key string is the row identity for set-diff purposes.
# ---------------------------------------------------------------------------

# Per-query diff renderers. Each takes no args; reads current+previous snapshots
# internally and prints the markdown delta block (or nothing).

diff_sudoers_rules() {
  local cur prev added removed n_add n_rem
  cur=$(get_snapshot_rows sudoers_rules 1)
  prev=$(get_snapshot_rows sudoers_rules 2)
  added=$(jq -c --argjson p "$prev" '
    [.[] | . as $r |
      select(($p | map("\(.header)|\(.rule_details)") | index("\($r.header)|\($r.rule_details)")) | not)
    ]' <<<"$cur")
  removed=$(jq -c --argjson c "$cur" '
    [.[] | . as $r |
      select(($c | map("\(.header)|\(.rule_details)") | index("\($r.header)|\($r.rule_details)")) | not)
    ]' <<<"$prev")
  n_add=$(jq 'length' <<<"$added")
  n_rem=$(jq 'length' <<<"$removed")
  [[ $n_add -eq 0 && $n_rem -eq 0 ]] && return 0
  echo "**Sudoers rules (+$n_add, -$n_rem):**"
  echo ""
  jq -r '.[] | "- ➕ `\(.header) \(.rule_details)`"' <<<"$added"
  jq -r '.[] | "- ➖ `\(.header) \(.rule_details)`"' <<<"$removed"
  echo ""
}

diff_ssh_authorized_keys() {
  local cur prev added removed n_add n_rem
  cur=$(get_snapshot_rows ssh_authorized_keys 1)
  prev=$(get_snapshot_rows ssh_authorized_keys 2)
  added=$(jq -c --argjson p "$prev" '
    [.[] | . as $r |
      select(($p | map("\(.username)|\(.algorithm)|\(.key_prefix)") | index("\($r.username)|\($r.algorithm)|\($r.key_prefix)")) | not)
    ]' <<<"$cur")
  removed=$(jq -c --argjson c "$cur" '
    [.[] | . as $r |
      select(($c | map("\(.username)|\(.algorithm)|\(.key_prefix)") | index("\($r.username)|\($r.algorithm)|\($r.key_prefix)")) | not)
    ]' <<<"$prev")
  n_add=$(jq 'length' <<<"$added")
  n_rem=$(jq 'length' <<<"$removed")
  [[ $n_add -eq 0 && $n_rem -eq 0 ]] && return 0
  echo "**SSH authorized keys (+$n_add, -$n_rem):**"
  echo ""
  jq -r '.[] | "- ➕ `\(.username) \(.algorithm) \(.key_prefix)… in \(.key_file)`"' <<<"$added"
  jq -r '.[] | "- ➖ `\(.username) \(.algorithm) \(.key_prefix)… in \(.key_file)`"' <<<"$removed"
  echo ""
}

diff_non_apple_launchd() {
  local cur prev added removed n_add n_rem
  cur=$(get_snapshot_rows non_apple_launchd 1)
  prev=$(get_snapshot_rows non_apple_launchd 2)
  added=$(jq -c --argjson p "$prev" '
    [.[] | . as $r | select(($p | map(.label) | index($r.label)) | not)]' <<<"$cur")
  removed=$(jq -c --argjson c "$cur" '
    [.[] | . as $r | select(($c | map(.label) | index($r.label)) | not)]' <<<"$prev")
  n_add=$(jq 'length' <<<"$added")
  n_rem=$(jq 'length' <<<"$removed")
  [[ $n_add -eq 0 && $n_rem -eq 0 ]] && return 0
  echo "**Non-Apple launchd jobs (+$n_add, -$n_rem):**"
  echo ""
  jq -r '.[] | "- ➕ `\(.label)`"' <<<"$added"
  jq -r '.[] | "- ➖ `\(.label)`"' <<<"$removed"
  echo ""
}

diff_launchd_overrides() {
  local cur prev added removed n_add n_rem
  cur=$(get_snapshot_rows launchd_overrides 1)
  prev=$(get_snapshot_rows launchd_overrides 2)
  added=$(jq -c --argjson p "$prev" '
    [.[] | . as $r |
      select(($p | map("\(.label)|\(.key)|\(.value)") | index("\($r.label)|\($r.key)|\($r.value)")) | not)
    ]' <<<"$cur")
  removed=$(jq -c --argjson c "$cur" '
    [.[] | . as $r |
      select(($c | map("\(.label)|\(.key)|\(.value)") | index("\($r.label)|\($r.key)|\($r.value)")) | not)
    ]' <<<"$prev")
  n_add=$(jq 'length' <<<"$added")
  n_rem=$(jq 'length' <<<"$removed")
  [[ $n_add -eq 0 && $n_rem -eq 0 ]] && return 0
  echo "**launchd overrides (+$n_add, -$n_rem):**"
  echo ""
  jq -r '.[] | "- ➕ `\(.label) \(.key)=\(.value)`"' <<<"$added"
  jq -r '.[] | "- ➖ `\(.label) \(.key)=\(.value)`"' <<<"$removed"
  echo ""
}

diff_system_extensions() {
  local cur prev added removed n_add n_rem
  cur=$(get_snapshot_rows system_extensions 1)
  prev=$(get_snapshot_rows system_extensions 2)
  added=$(jq -c --argjson p "$prev" '
    [.[] | . as $r |
      select(($p | map("\(.identifier)|\(.state // "")") | index("\($r.identifier)|\($r.state // "")")) | not)
    ]' <<<"$cur")
  removed=$(jq -c --argjson c "$cur" '
    [.[] | . as $r |
      select(($c | map("\(.identifier)|\(.state // "")") | index("\($r.identifier)|\($r.state // "")")) | not)
    ]' <<<"$prev")
  n_add=$(jq 'length' <<<"$added")
  n_rem=$(jq 'length' <<<"$removed")
  [[ $n_add -eq 0 && $n_rem -eq 0 ]] && return 0
  echo "**System extensions (+$n_add, -$n_rem):**"
  echo ""
  jq -r '.[] | "- ➕ `\(.identifier) (\(.state // "?"))`"' <<<"$added"
  jq -r '.[] | "- ➖ `\(.identifier) (\(.state // "?"))`"' <<<"$removed"
  echo ""
}

diff_startup_items() {
  local cur prev added removed n_add n_rem
  cur=$(get_snapshot_rows startup_items 1)
  prev=$(get_snapshot_rows startup_items 2)
  added=$(jq -c --argjson p "$prev" '
    [.[] | . as $r |
      select(($p | map("\(.path)|\(.username // "")") | index("\($r.path)|\($r.username // "")")) | not)
    ]' <<<"$cur")
  removed=$(jq -c --argjson c "$cur" '
    [.[] | . as $r |
      select(($c | map("\(.path)|\(.username // "")") | index("\($r.path)|\($r.username // "")")) | not)
    ]' <<<"$prev")
  n_add=$(jq 'length' <<<"$added")
  n_rem=$(jq 'length' <<<"$removed")
  [[ $n_add -eq 0 && $n_rem -eq 0 ]] && return 0
  echo "**Startup items (+$n_add, -$n_rem):**"
  echo ""
  jq -r '.[] | "- ➕ `\(.name) — \(.path) [\(.username // "?")]`"' <<<"$added"
  jq -r '.[] | "- ➖ `\(.name) — \(.path) [\(.username // "?")]`"' <<<"$removed"
  echo ""
}

# Posture changes — value-level rather than set-level. Returns a block of
# "**Posture changed:**\n- firewall: X → Y\n..." when any value differs.
diff_posture() {
  local cur_fw prev_fw cur_gk prev_gk cur_sip prev_sip
  cur_fw=$(jq -r '.[0].global_state // ""' <<<"$(get_snapshot_rows firewall_state 1)")
  prev_fw=$(jq -r '.[0].global_state // ""' <<<"$(get_snapshot_rows firewall_state 2)")
  cur_gk=$(jq -r '.[0].assessments_enabled // ""' <<<"$(get_snapshot_rows gatekeeper_state 1)")
  prev_gk=$(jq -r '.[0].assessments_enabled // ""' <<<"$(get_snapshot_rows gatekeeper_state 2)")
  cur_sip=$(jq -r '.[0].enabled // ""' <<<"$(get_snapshot_rows sip_state 1)")
  prev_sip=$(jq -r '.[0].enabled // ""' <<<"$(get_snapshot_rows sip_state 2)")

  local has_any=0
  local out=""
  [[ -n $prev_fw && $prev_fw != "$cur_fw" ]] && {
    out+="- firewall: \`$prev_fw\` → \`$cur_fw\`"$'\n'
    has_any=1
  }
  [[ -n $prev_gk && $prev_gk != "$cur_gk" ]] && {
    out+="- gatekeeper: \`$prev_gk\` → \`$cur_gk\`"$'\n'
    has_any=1
  }
  [[ -n $prev_sip && $prev_sip != "$cur_sip" ]] && {
    out+="- SIP: \`$prev_sip\` → \`$cur_sip\`"$'\n'
    has_any=1
  }
  [[ $has_any -eq 0 ]] && return 0
  echo "**Posture changed:**"
  echo ""
  printf '%s' "$out"
  echo ""
}

# FIM events block: events in [$1, $2] grouped by category. Returns no
# output when zero events.
diff_file_events() {
  local since="$1" until="$2"
  [[ ! -s $events_log ]] && return 0
  local rows
  rows=$(jq -c --argjson s "$since" --argjson u "$until" \
    'select(.name == "file_events_recent" and (.unixTime | tonumber) >= $s and (.unixTime | tonumber) <= $u)' \
    "$events_log" 2>/dev/null || true)
  [[ -z $rows ]] && return 0
  echo "**File integrity events:**"
  echo ""
  printf '%s\n' "$rows" |
    jq -r '.columns.category' |
    sort | uniq -c |
    awk '{printf "- %s: %d\n", $2, $1}'
  echo ""
}

# Count file events in a window — used for the status card.
count_file_events() {
  local since="$1" until="$2"
  [[ ! -s $events_log ]] && {
    echo 0
    return
  }
  jq -c --argjson s "$since" --argjson u "$until" \
    'select(.name == "file_events_recent" and (.unixTime | tonumber) >= $s and (.unixTime | tonumber) <= $u)' \
    "$events_log" 2>/dev/null | wc -l | tr -d ' '
}

# ---------------------------------------------------------------------------
# Idempotency anchor: firewall_state is in every pack tick.
# ---------------------------------------------------------------------------

fw_line=$(get_snapshot_line firewall_state 1)
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

# Window bounds for FIM and "previous snapshot" sense.
prev_fw_line=$(get_snapshot_line firewall_state 2)
if [[ -n $prev_fw_line ]]; then
  prev_unix=$(jq -r '.unixTime' <<<"$prev_fw_line")
  prev_time_short=$(date -r "$prev_unix" '+%H:%M')
else
  prev_unix=$((unix_time - 21600))
  prev_time_short="—"
fi

# ---------------------------------------------------------------------------
# Status-card values + emoji selection.
# ---------------------------------------------------------------------------

fw_global=$(jq -r '.snapshot[0].global_state' <<<"$fw_line")
gk_line=$(get_snapshot_line gatekeeper_state 1)
sip_line=$(get_snapshot_line sip_state 1)
gk_assess=$(jq -r '.snapshot[0].assessments_enabled // ""' <<<"$gk_line")
sip_enabled=$(jq -r '.snapshot[0].enabled // ""' <<<"$sip_line")

case "$fw_global" in
  1) fw_cell="✅ on" ;;
  2) fw_cell="✅ block-all" ;;
  0) fw_cell="❌ OFF" ;;
  *) fw_cell="? ($fw_global)" ;;
esac
case "$gk_assess" in
  1) gk_cell="✅ on" ;;
  0) gk_cell="❌ OFF" ;;
  *) gk_cell="?" ;;
esac
case "$sip_enabled" in
  1) sip_cell="✅ on" ;;
  0) sip_cell="🔵 off (intentional)" ;;
  *) sip_cell="?" ;;
esac
fim_count=$(count_file_events "$prev_unix" "$unix_time")

# Counts line with deltas. Listening ports get a count-delta only (no diff in
# changes section — would be flooded by mDNS noise).
ssh_count=$(count_rows ssh_authorized_keys 1)
ssh_delta=$(count_delta_suffix ssh_authorized_keys)
sudo_count=$(count_rows sudoers_rules 1)
sudo_delta=$(count_delta_suffix sudoers_rules)
ld_count=$(count_rows non_apple_launchd 1)
ld_delta=$(count_delta_suffix non_apple_launchd)
sx_count=$(count_rows system_extensions 1)
sx_delta=$(count_delta_suffix system_extensions)
lp_count=$(count_rows listening_non_localhost 1)
lp_delta=$(count_delta_suffix listening_non_localhost)

# Compute total change count for the header tag.
deltas_block=$(
  diff_posture
  diff_sudoers_rules
  diff_ssh_authorized_keys
  diff_non_apple_launchd
  diff_launchd_overrides
  diff_system_extensions
  diff_startup_items
  diff_file_events "$prev_unix" "$unix_time"
)
change_count=$(printf '%s' "$deltas_block" | grep -cE '^- (➕|➖|firewall:|gatekeeper:|SIP:)' || true)

if [[ -z $prev_fw_line ]]; then
  header_tag="📍 baseline (no previous snapshot)"
elif [[ $change_count -eq 0 ]]; then
  header_tag="✅ ALL CLEAR"
else
  header_tag="⚠ $change_count change(s) since $prev_time_short"
fi

# ---------------------------------------------------------------------------
# Build and append the snapshot section.
# ---------------------------------------------------------------------------

{
  echo ""
  echo "## $timestamp — $header_tag <!-- ts:$unix_time -->"
  echo ""
  echo "| FW | GK | SIP | FIM (6h) |"
  echo "|----|----|-----|----------|"
  echo "| $fw_cell | $gk_cell | $sip_cell | $fim_count |"
  echo ""
  printf '📊 %d SSH%s · %d sudoers%s · %d launchd%s · %d sysext%s · %d listening%s\n' \
    "$ssh_count" "$ssh_delta" \
    "$sudo_count" "$sudo_delta" \
    "$ld_count" "$ld_delta" \
    "$sx_count" "$sx_delta" \
    "$lp_count" "$lp_delta"
  echo ""
  echo "### Changes since previous snapshot"
  echo ""
  if [[ -z $prev_fw_line ]]; then
    echo "_Baseline snapshot — no previous tick to compare against._"
  elif [[ $change_count -eq 0 ]]; then
    echo "_None._"
  else
    printf '%s' "$deltas_block"
  fi
  echo ""
} >>"$report_file"

echo "Report appended to $report_file"

# ---------------------------------------------------------------------------
# Alert tier: firewall OFF or gatekeeper DISABLED.
# ---------------------------------------------------------------------------

alert_msg=""
if [[ $fw_global == "0" ]]; then
  alert_msg="Application Firewall is OFF"
fi
if [[ $gk_assess == "0" ]]; then
  if [[ -n $alert_msg ]]; then
    alert_msg="${alert_msg}; "
  fi
  alert_msg="${alert_msg}Gatekeeper assessments DISABLED"
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
