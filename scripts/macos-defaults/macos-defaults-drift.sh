#!/usr/bin/env bash

set -euo pipefail

macos_defaults_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
dotfiles_directory="$(cd "$macos_defaults_directory/../.." && pwd)"

# shellcheck source=.chezmoitemplates/cli-print-style-lib.sh.tmpl
source "$dotfiles_directory/.chezmoitemplates/cli-print-style-lib.sh.tmpl"
# shellcheck source=scripts/macos-defaults/helpers/defaults-records.sh
source "$macos_defaults_directory/helpers/defaults-records.sh"

readonly exit_every_setting_matches=0
readonly exit_drift_detected=1
readonly exit_invalid_data=2
readonly exit_some_settings_unreadable=3
readonly unset_marker='<unset>'
readonly unreadable_marker='<unreadable>'

checked_count=0
drift_count=0
unreadable_count=0
drift_table_started=0

value_type_is_bool() {
  local value_type=$1
  [[ $value_type == bool ]]
}

bool_as_one_or_zero() {
  local value=$1
  case "$value" in
    true | yes | 1) printf '1' ;;
    false | no | 0) printf '0' ;;
    *) printf '%s' "$value" ;;
  esac
}

expected_value_as_defaults_prints_it() {
  local value_type=$1 value=$2
  if value_type_is_bool "$value_type"; then
    bool_as_one_or_zero "$value"
  else
    printf '%s' "$value"
  fi
}

drift_table_has_started() {
  ((drift_table_started == 1))
}

start_drift_table() {
  printf 'DOMAIN\tKEY\tEXPECTED\tACTUAL\n'
  drift_table_started=1
}

add_drift_table_row() {
  local domain=$1 key=$2 expected_value=$3 live_value=$4
  if ! drift_table_has_started; then
    start_drift_table
  fi
  printf '%s\t%s\t%s\t%s\n' "$domain" "$key" "$expected_value" "$live_value"
}

count_checked_setting() {
  checked_count=$((checked_count + 1))
}

count_drifted_setting() {
  drift_count=$((drift_count + 1))
}

count_unreadable_setting() {
  unreadable_count=$((unreadable_count + 1))
}

setting_has_drifted() {
  local expected_value=$1 live_value=$2
  [[ $expected_value != "$live_value" ]]
}

list_setting_if_it_drifted() {
  local domain=$1 key=$2 expected_value=$3 live_value=$4
  if setting_has_drifted "$expected_value" "$live_value"; then
    add_drift_table_row "$domain" "$key" "$expected_value" "$live_value"
    count_drifted_setting
  fi
}

list_unreadable_setting() {
  local domain=$1 key=$2 expected_value=$3
  add_drift_table_row "$domain" "$key" "$expected_value" "$unreadable_marker"
  count_unreadable_setting
}

print_unset_marker() {
  printf '%s' "$unset_marker"
}

read_any_host_setting() {
  local domain=$1 key=$2
  defaults read "$domain" "$key" 2>/dev/null
}

read_current_host_setting() {
  local domain=$1 key=$2
  defaults -currentHost read "$domain" "$key" 2>/dev/null
}

read_user_setting() {
  local domain=$1 key=$2 host=$3
  if record_targets_current_host "$host"; then
    read_current_host_setting "$domain" "$key" || print_unset_marker
  else
    read_any_host_setting "$domain" "$key" || print_unset_marker
  fi
}

read_system_setting() {
  local plist_path=$1 key=$2
  local live_value read_status=0
  live_value="$(system_defaults_read_actual "$plist_path" "$key")" || read_status=$?
  if read_status_means_unset "$read_status"; then
    print_unset_marker
    return 0
  fi
  printf '%s' "$live_value"
  return "$read_status"
}

check_user_setting() {
  local domain=$1 key=$2 expected_value=$3 host=$4
  local live_value
  live_value="$(read_user_setting "$domain" "$key" "$host")"
  list_setting_if_it_drifted "$domain" "$key" "$expected_value" "$live_value"
}

check_system_setting() {
  local domain=$1 key=$2 expected_value=$3 plist_path=$4
  local system_plist_path live_value read_status=0
  system_plist_path="$(resolve_system_plist_path "$domain" "$plist_path")" || return
  live_value="$(read_system_setting "$system_plist_path" "$key")" || read_status=$?
  if read_status_means_unreadable "$read_status"; then
    list_unreadable_setting "$domain" "$key" "$expected_value"
    return 0
  fi
  list_setting_if_it_drifted "$domain" "$key" "$expected_value" "$live_value"
}

check_live_setting() {
  local domain=$1 key=$2 expected_value=$3 host=$4 scope=$5 plist_path=$6
  if record_scope_is_system "$scope"; then
    check_system_setting "$domain" "$key" "$expected_value" "$plist_path"
  else
    check_user_setting "$domain" "$key" "$expected_value" "$host"
  fi
}

require_known_record_tier() {
  local domain=$1 key=$2 tier=$3
  if record_tier_is_known "$tier"; then
    return 0
  fi
  report_line "error[unknown-tier]: $domain $key has tier '$tier'; refusing to check it." >&2
  return 1
}

check_record() {
  local domain=$1 key=$2 value_type=$3 value=$4 host=$5 scope=$6 plist_path=$7 tier=$8
  local expected_value
  validate_record_identity "$domain" "$key" || return
  require_known_record_tier "$domain" "$key" "$tier" || return
  if ! record_tier_has_an_expected_value "$tier"; then
    return 0
  fi
  validate_record_scope "$scope" "$host" "$plist_path" >/dev/null || return
  expected_value="$(expected_value_as_defaults_prints_it "$value_type" "$value")"
  check_live_setting "$domain" "$key" "$expected_value" "$host" "$scope" "$plist_path" || return
  count_checked_setting
}

check_every_record() {
  local record_lines=$1
  local domain key value_type value host scope plist_path tier
  if text_is_empty "$record_lines"; then
    return 0
  fi
  while IFS=$DEFAULTS_RECORD_FIELD_SEPARATOR read -r domain key value_type value host scope plist_path tier; do
    check_record "$domain" "$key" "$value_type" "$value" "$host" "$scope" "$plist_path" "$tier" || return
  done <<<"$record_lines"
}

some_settings_drifted() {
  ((drift_count > 0))
}

some_settings_were_unreadable() {
  ((unreadable_count > 0))
}

report_unreadable_settings() {
  report_line "error[unreadable]: $unreadable_count setting(s) could not be read; they are not drift, and not passing either." >&2
}

report_drifted_settings() {
  report_line "error[drift]: $drift_count of $checked_count setting(s) differ from macos_defaults.yaml." >&2
}

main() {
  report_section 'macos-defaults' 'drift check'

  local data_file record_lines
  data_file="$(macos_defaults_data_file)" || exit $?
  require_readable_data_file "$data_file" || exit $?

  report_line "checking tracked settings..."
  record_lines="$(read_validated_records "$data_file")" || exit $?
  check_every_record "$record_lines" || exit "$exit_invalid_data"

  if some_settings_were_unreadable; then
    report_unreadable_settings
  fi
  if some_settings_drifted; then
    report_drifted_settings
    exit "$exit_drift_detected"
  fi
  if some_settings_were_unreadable; then
    exit "$exit_some_settings_unreadable"
  fi
  report_line "all $checked_count tracked setting(s) match."
  exit "$exit_every_setting_matches"
}

main "$@"
