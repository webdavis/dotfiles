#!/usr/bin/env bash

set -euo pipefail

macos_defaults_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
dotfiles_dir="$(cd "$macos_defaults_dir/../.." && pwd)"

# shellcheck source=.chezmoitemplates/cli-print-style-lib.sh.tmpl
source "$dotfiles_dir/.chezmoitemplates/cli-print-style-lib.sh.tmpl"
# shellcheck source=scripts/macos-defaults/helpers/defaults-records.sh
source "$macos_defaults_dir/helpers/defaults-records.sh"

readonly exit_invalid_data=2
readonly field_separator=$'\x1f'

planned_writes=()

quit_system_settings() {
  osascript -e 'tell application "System Settings" to quit' 2>/dev/null || true
}

tier_is_known() {
  local tier=$1
  [[ $tier == enforce || $tier == verify || $tier == manual ]]
}

tier_is_applied() {
  local tier=$1
  [[ $tier == enforce ]]
}

join_planned_write() {
  local kind=$1 target=$2 key=$3 type=$4 value=$5
  printf '%s' "$kind$field_separator$target$field_separator$key$field_separator$type$field_separator$value"
}

resolve_permitted_system_plist_path() {
  local domain=$1 plist_path=$2
  local resolved_plist_path
  resolved_plist_path="$(resolve_system_plist_path "$domain" "$plist_path")" || return "$exit_invalid_data"
  require_system_plist_path_permitted "$resolved_plist_path" || return "$exit_invalid_data"
  printf '%s' "$resolved_plist_path"
}

plan_write() {
  local domain=$1 key=$2 value_type=$3 value=$4 host=$5 scope=$6 plist_path=$7
  local resolved_plist_path
  if [[ $scope == system ]]; then
    resolved_plist_path="$(resolve_permitted_system_plist_path "$domain" "$plist_path")" || exit "$exit_invalid_data"
    planned_writes+=("$(join_planned_write system "$resolved_plist_path" "$key" "$value_type" "$value")")
  elif [[ -n $host ]]; then
    planned_writes+=("$(join_planned_write currenthost "$domain" "$key" "$value_type" "$value")")
  else
    planned_writes+=("$(join_planned_write user "$domain" "$key" "$value_type" "$value")")
  fi
}

plan_record() {
  local domain=$1 key=$2 value_type=$3 value=$4 host=$5 scope=$6 plist_path=$7 tier=$8
  if ! tier_is_known "$tier"; then
    report_line "error[unknown-tier]: $domain $key has tier '$tier'; refusing to write anything." >&2
    exit "$exit_invalid_data"
  fi
  if ! tier_is_applied "$tier"; then
    return
  fi
  validate_defaults_record "$domain" "$key" "$value_type" "$value" "$host" "$scope" "$plist_path" "$tier" ||
    exit "$exit_invalid_data"
  plan_write "$domain" "$key" "$value_type" "$value" "$host" "$scope" "$plist_path"
}

plan_all_records() {
  local record_stream=$1
  local record_line domain key value_type value host scope plist_path tier
  while IFS= read -r record_line; do
    [[ -z $record_line ]] && continue
    IFS=$field_separator read -r domain key value_type value host scope plist_path tier <<<"$record_line"
    plan_record "$domain" "$key" "$value_type" "$value" "$host" "$scope" "$plist_path" "$tier"
  done <<<"$record_stream"
}

run_planned_write() {
  local planned_write=$1
  local kind target key value_type value
  IFS=$field_separator read -r kind target key value_type value <<<"$planned_write"
  case "$kind" in
    user) defaults write "$target" "$key" "-$value_type" "$value" ;;
    currenthost) defaults -currentHost write "$target" "$key" "-$value_type" "$value" ;;
    system) system_defaults_write "$target" "$key" "$value_type" "$value" ;;
    *)
      report_line "error[unknown-write-kind]: planned write has kind '$kind'; refusing to write it." >&2
      exit "$exit_invalid_data"
      ;;
  esac
}

run_all_planned_writes() {
  local planned_write
  if ((${#planned_writes[@]} == 0)); then
    return
  fi
  for planned_write in "${planned_writes[@]}"; do
    run_planned_write "$planned_write"
  done
}

restart_affected_processes() {
  local data_file=$1
  local process_name
  yq eval -r '.macos.killall[]' "$data_file" |
    while read -r process_name; do
      [[ -z $process_name ]] && continue
      killall "$process_name" 2>/dev/null || true
    done
}

main() {
  report_section 'macos-defaults' 'apply'

  local data_file record_stream
  data_file="$(macos_defaults_data_file)" || exit $?
  require_readable_data_file "$data_file" || exit $?

  quit_system_settings
  record_stream="$(read_validated_records "$data_file")" || exit $?

  report_line "validating every record before writing anything..."
  plan_all_records "$record_stream"

  report_line "writing ${#planned_writes[@]} setting(s)..."
  run_all_planned_writes
  restart_affected_processes "$data_file"
  report_line "applied ${#planned_writes[@]} setting(s)."
}

main "$@"
