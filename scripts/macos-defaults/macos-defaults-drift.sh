#!/usr/bin/env bash

set -euo pipefail
shopt -s lastpipe

macos_defaults_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
dotfiles_dir="$(cd "$macos_defaults_dir/../.." && pwd)"

# shellcheck source=.chezmoitemplates/cli-print-style-lib.sh.tmpl
source "$dotfiles_dir/.chezmoitemplates/cli-print-style-lib.sh.tmpl"
# shellcheck source=scripts/macos-defaults/helpers/defaults-records.sh
source "$macos_defaults_dir/helpers/defaults-records.sh"

readonly exit_every_setting_matches=0
readonly exit_drift_detected=1
readonly exit_invalid_data=2
readonly exit_some_settings_unreadable=3

checked_count=0
drift_count=0
unreadable_count=0
table_header_printed=0

normalize_value() {
  local type=$1 value=$2
  if [[ $type != bool ]]; then
    printf '%s' "$value"
    return
  fi
  case "$value" in
    true | yes | 1) printf '1' ;;
    false | no | 0) printf '0' ;;
    *) printf '%s' "$value" ;;
  esac
}

tier_is_known() {
  local tier=$1
  [[ $tier == enforce || $tier == verify || $tier == manual ]]
}

tier_has_an_expected_value() {
  local tier=$1
  [[ $tier == enforce || $tier == verify ]]
}

print_table_header_once() {
  if ((table_header_printed == 0)); then
    printf 'DOMAIN\tKEY\tEXPECTED\tACTUAL\n'
    table_header_printed=1
  fi
}

print_table_row() {
  local domain=$1 key=$2 expected=$3 actual=$4
  print_table_header_once
  printf '%s\t%s\t%s\t%s\n' "$domain" "$key" "$expected" "$actual"
}

record_drifted_setting() {
  print_table_row "$@"
  drift_count=$((drift_count + 1))
}

record_unreadable_setting() {
  local domain=$1 key=$2 expected=$3
  print_table_row "$domain" "$key" "$expected" '<unreadable>'
  unreadable_count=$((unreadable_count + 1))
}

read_user_setting() {
  local domain=$1 key=$2 host=$3
  if [[ -n $host ]]; then
    defaults -currentHost read "$domain" "$key" 2>/dev/null || printf '<unset>'
  else
    defaults read "$domain" "$key" 2>/dev/null || printf '<unset>'
  fi
}

read_system_setting() {
  local resolved_plist_path=$1 key=$2
  local actual read_status=0
  actual="$(system_defaults_read_actual "$resolved_plist_path" "$key")" || read_status=$?
  if ((read_status == SYSTEM_READ_UNSET)); then
    printf '<unset>'
    return 0
  fi
  printf '%s' "$actual"
  return "$read_status"
}

check_setting() {
  local domain=$1 key=$2 type=$3 value=$4 host=$5 scope=$6 plist_path=$7
  local expected actual resolved_plist_path read_status=0
  expected="$(normalize_value "$type" "$value")"

  if [[ $scope == system ]]; then
    resolved_plist_path="$(resolve_system_plist_path "$domain" "$plist_path")" || exit "$exit_invalid_data"
    actual="$(read_system_setting "$resolved_plist_path" "$key")" || read_status=$?
  else
    actual="$(read_user_setting "$domain" "$key" "$host")"
  fi

  if ((read_status == SYSTEM_READ_UNREADABLE)); then
    record_unreadable_setting "$domain" "$key" "$expected"
  elif [[ $expected != "$actual" ]]; then
    record_drifted_setting "$domain" "$key" "$expected" "$actual"
  fi
}

check_record() {
  local domain=$1 key=$2 type=$3 value=$4 host=$5 scope=$6 plist_path=$7 tier=$8
  validate_record_identity "$domain" "$key" || exit "$exit_invalid_data"

  if ! tier_is_known "$tier"; then
    report_line "error[unknown-tier]: $domain $key has tier '$tier'; refusing to check it." >&2
    exit "$exit_invalid_data"
  fi
  if ! tier_has_an_expected_value "$tier"; then
    return
  fi

  scope="$(validate_record_scope "$scope" "$host" "$plist_path")" || exit "$exit_invalid_data"
  check_setting "$domain" "$key" "$type" "$value" "$host" "$scope" "$plist_path"
  checked_count=$((checked_count + 1))
}

check_all_records() {
  local data_file=$1
  local domain key type value host scope plist_path tier
  read_validated_records "$data_file" |
    while IFS=$'\x1f' read -r domain key type value host scope plist_path tier; do
      check_record "$domain" "$key" "$type" "$value" "$host" "$scope" "$plist_path" "$tier"
    done
}

report_verdict_and_exit() {
  if ((unreadable_count > 0)); then
    report_line "$unreadable_count setting(s) could not be read; they are not drift, and not passing either." >&2
  fi
  if ((drift_count > 0)); then
    report_line "error[drift]: $drift_count of $checked_count setting(s) differ from macos_defaults.yaml." >&2
    exit "$exit_drift_detected"
  fi
  if ((unreadable_count > 0)); then
    exit "$exit_some_settings_unreadable"
  fi
  report_line "all $checked_count tracked setting(s) match."
  exit "$exit_every_setting_matches"
}

main() {
  report_section 'macos-defaults' 'drift check'

  local data_file
  data_file="$(macos_defaults_data_file)" || exit $?
  require_readable_data_file "$data_file" || exit $?

  report_line "checking tracked settings..."
  check_all_records "$data_file"
  report_verdict_and_exit
}

main "$@"
