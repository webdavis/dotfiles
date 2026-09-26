#!/usr/bin/env bash

set -euo pipefail

macos_defaults_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
dotfiles_directory="$(cd "$macos_defaults_directory/../.." && pwd)"

# shellcheck source=.chezmoitemplates/cli-print-style-lib.sh.tmpl
source "$dotfiles_directory/.chezmoitemplates/cli-print-style-lib.sh.tmpl"
# shellcheck source=scripts/macos-defaults/helpers/defaults-records.sh
source "$macos_defaults_directory/helpers/defaults-records.sh"

readonly exit_invalid_data=2
readonly user_destination='user'
readonly current_host_destination='current-host'
readonly system_destination='system'

planned_writes=()

quit_system_settings() {
  osascript -e 'tell application "System Settings" to quit' 2>/dev/null || true
}

require_known_record_tier() {
  local domain=$1 key=$2 tier=$3
  if record_tier_is_known "$tier"; then
    return 0
  fi
  report_line "error[unknown-tier]: $domain $key has tier '$tier'; refusing to write anything." >&2
  return 1
}

resolve_permitted_system_plist_path() {
  local domain=$1 plist_path=$2
  local resolved_plist_path
  resolved_plist_path="$(resolve_system_plist_path "$domain" "$plist_path")" || return
  require_system_plist_path_permitted "$resolved_plist_path" || return
  printf '%s' "$resolved_plist_path"
}

join_planned_write() {
  local destination=$1 target=$2 key=$3 value_type=$4 value=$5
  local separator=$DEFAULTS_RECORD_FIELD_SEPARATOR
  printf '%s' "$destination$separator$target$separator$key$separator$value_type$separator$value"
}

plan_write() {
  local destination=$1 target=$2 key=$3 value_type=$4 value=$5
  planned_writes+=("$(join_planned_write "$destination" "$target" "$key" "$value_type" "$value")")
}

plan_write_to_its_destination() {
  local domain=$1 key=$2 value_type=$3 value=$4 host=$5 scope=$6 plist_path=$7
  local system_plist_path
  if record_scope_is_system "$scope"; then
    system_plist_path="$(resolve_permitted_system_plist_path "$domain" "$plist_path")" || return
    plan_write "$system_destination" "$system_plist_path" "$key" "$value_type" "$value"
  elif record_targets_current_host "$host"; then
    plan_write "$current_host_destination" "$domain" "$key" "$value_type" "$value"
  else
    plan_write "$user_destination" "$domain" "$key" "$value_type" "$value"
  fi
}

plan_record() {
  local domain=$1 key=$2 value_type=$3 value=$4 host=$5 scope=$6 plist_path=$7 tier=$8
  require_known_record_tier "$domain" "$key" "$tier" || return
  if ! record_tier_is_enforced "$tier"; then
    return 0
  fi
  validate_defaults_record "$domain" "$key" "$value_type" "$value" "$host" "$scope" "$plist_path" "$tier" || return
  plan_write_to_its_destination "$domain" "$key" "$value_type" "$value" "$host" "$scope" "$plist_path"
}

plan_every_record() {
  local record_lines=$1
  local domain key value_type value host scope plist_path tier
  if text_is_empty "$record_lines"; then
    return 0
  fi
  while IFS=$DEFAULTS_RECORD_FIELD_SEPARATOR read -r domain key value_type value host scope plist_path tier; do
    plan_record "$domain" "$key" "$value_type" "$value" "$host" "$scope" "$plist_path" "$tier" || return
  done <<<"$record_lines"
}

write_user_setting() {
  local domain=$1 key=$2 value_type=$3 value=$4
  defaults write "$domain" "$key" "-$value_type" "$value"
}

write_current_host_setting() {
  local domain=$1 key=$2 value_type=$3 value=$4
  defaults -currentHost write "$domain" "$key" "-$value_type" "$value"
}

write_system_setting() {
  local plist_path=$1 key=$2 value_type=$3 value=$4
  system_defaults_write "$plist_path" "$key" "$value_type" "$value"
}

write_planned_setting_to_its_destination() {
  local planned_write=$1
  local destination target key value_type value
  IFS=$DEFAULTS_RECORD_FIELD_SEPARATOR read -r destination target key value_type value <<<"$planned_write"
  case "$destination" in
    "$user_destination") write_user_setting "$target" "$key" "$value_type" "$value" ;;
    "$current_host_destination") write_current_host_setting "$target" "$key" "$value_type" "$value" ;;
    "$system_destination") write_system_setting "$target" "$key" "$value_type" "$value" ;;
    *)
      report_line "error[unknown-write-destination]: planned write has destination '$destination'; refusing to write it." >&2
      return "$exit_invalid_data"
      ;;
  esac
}

no_writes_are_planned() {
  ((${#planned_writes[@]} == 0))
}

write_every_planned_setting() {
  local planned_write
  if no_writes_are_planned; then
    return 0
  fi
  for planned_write in "${planned_writes[@]}"; do
    write_planned_setting_to_its_destination "$planned_write"
  done
}

read_killall_process_names() {
  local data_file=$1
  yq eval -r '.macos.killall[]' "$data_file"
}

restart_process() {
  local process_name=$1
  killall "$process_name" 2>/dev/null || true
}

restart_affected_processes() {
  local data_file=$1
  local process_name
  read_killall_process_names "$data_file" |
    while read -r process_name; do
      text_is_empty "$process_name" && continue
      restart_process "$process_name"
    done
}

main() {
  report_section 'macos-defaults' 'apply'

  local data_file record_lines
  data_file="$(macos_defaults_data_file)" || exit $?
  require_readable_data_file "$data_file" || exit $?

  quit_system_settings
  record_lines="$(read_validated_records "$data_file")" || exit $?

  report_line "validating every record before writing anything..."
  plan_every_record "$record_lines" || exit "$exit_invalid_data"

  report_line "writing ${#planned_writes[@]} setting(s)..."
  write_every_planned_setting
  restart_affected_processes "$data_file"
  report_line "applied ${#planned_writes[@]} setting(s)."
}

main "$@"
