#!/usr/bin/env bash

set -euo pipefail

macos_defaults_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
dotfiles_directory="$(cd "$macos_defaults_directory/../.." && pwd)"

# shellcheck source=.chezmoitemplates/cli-print-style-lib.sh.tmpl
source "$dotfiles_directory/.chezmoitemplates/cli-print-style-lib.sh.tmpl"
# shellcheck source=scripts/macos-defaults/helpers/defaults-records.sh
source "$macos_defaults_directory/helpers/defaults-records.sh"

readonly exit_already_tracked=0
readonly exit_setting_not_set=1
readonly exit_unsupported_type=1
readonly exit_malformed_arguments=3
readonly exit_tracked_value_differs=4
readonly fewest_arguments=2
readonly most_arguments=6
readonly safe_identifier_pattern='^[a-zA-Z0-9._-]+$'

domain=""
key=""
host=""
scope="user"
capture_temp_file=""

print_usage() {
  report_line "usage: macos-defaults-capture.sh <domain> <key> [--host current] [--scope user|system]" >&2
}

argument_count_is_supported() {
  local argument_count=$1
  ((argument_count >= fewest_arguments && argument_count <= most_arguments))
}

arguments_remain() {
  local remaining_argument_count=$1
  ((remaining_argument_count > 0))
}

option_value_follows() {
  local remaining_argument_count=$1
  ((remaining_argument_count >= 2))
}

host_option_names_the_current_host() {
  local host_option_value=$1
  [[ $host_option_value == current ]]
}

parse_options() {
  while arguments_remain "$#"; do
    case "$1" in
      --host=current)
        host=current
        shift
        ;;
      --host)
        if ! option_value_follows "$#" || ! host_option_names_the_current_host "$2"; then
          return 1
        fi
        host=current
        shift 2
        ;;
      --scope=*)
        scope=${1#*=}
        shift
        ;;
      --scope)
        if ! option_value_follows "$#"; then
          return 1
        fi
        scope=$2
        shift 2
        ;;
      *) return 1 ;;
    esac
  done
}

parse_arguments() {
  if ! argument_count_is_supported "$#"; then
    return 1
  fi
  domain=$1
  key=$2
  shift 2
  parse_options "$@"
}

require_valid_scope() {
  if validate_record_scope "$scope" "$host" "" >/dev/null; then
    return 0
  fi
  report_line "error[invalid-scope]: --scope '$scope' cannot be combined with --host '$host'." >&2
  return 1
}

identifier_is_safe() {
  local identifier=$1
  [[ $identifier =~ $safe_identifier_pattern ]]
}

require_safe_identifier() {
  local label=$1 identifier=$2
  if identifier_is_safe "$identifier"; then
    return 0
  fi
  report_line "error[invalid-$label]: '$identifier' contains characters outside letters, digits, dot, underscore and dash." >&2
  return 1
}

run_defaults_for_the_captured_host() {
  if record_targets_current_host "$host"; then
    defaults -currentHost "$@"
  else
    defaults "$@"
  fi
}

read_live_type_description() {
  local target=$1 key=$2
  run_defaults_for_the_captured_host read-type "$target" "$key"
}

read_live_value() {
  local target=$1 key=$2
  run_defaults_for_the_captured_host read "$target" "$key"
}

defaults_read_target() {
  if record_scope_is_system "$scope"; then
    resolve_system_plist_path "$domain" ""
  else
    printf '%s' "$domain"
  fi
}

yaml_type_for_live_type() {
  local live_type_description=$1
  case "$live_type_description" in
    *boolean*) printf 'bool' ;;
    *integer*) printf 'int' ;;
    *float*) printf 'float' ;;
    *string*) printf 'string' ;;
    *) return 1 ;;
  esac
}

yaml_type_is_string() {
  local yaml_type=$1
  [[ $yaml_type == string ]]
}

live_bool_as_yaml() {
  local live_value=$1
  case "$live_value" in
    1) printf 'true' ;;
    0) printf 'false' ;;
    *) printf '%s' "$live_value" ;;
  esac
}

live_string_as_yaml() {
  local live_value=$1
  printf '"%s"' "${live_value//\"/\\\"}"
}

live_value_as_yaml() {
  local yaml_type=$1 live_value=$2
  case "$yaml_type" in
    bool) live_bool_as_yaml "$live_value" ;;
    string) live_string_as_yaml "$live_value" ;;
    *) printf '%s' "$live_value" ;;
  esac
}

live_value_as_yq_prints_it() {
  local yaml_type=$1 live_value=$2 yaml_value=$3
  if yaml_type_is_string "$yaml_type"; then
    printf '%s' "$live_value"
  else
    printf '%s' "$yaml_value"
  fi
}

tracked_setting_selector() {
  printf '.macos.defaults[] | select(.domain == "%s" and .key == "%s" and ((.host // "") == "%s") and ((.scope // "user") == "%s"))' \
    "$domain" "$key" "$host" "$scope"
}

read_tracked_value() {
  local data_file=$1
  yq eval -r "$(tracked_setting_selector) | .value" "$data_file"
}

setting_is_tracked() {
  local tracked_value=$1
  ! text_is_empty "$tracked_value"
}

tracked_value_matches_live_value() {
  local tracked_value=$1 yaml_type=$2 live_value=$3 yaml_value=$4
  [[ $tracked_value == "$(live_value_as_yq_prints_it "$yaml_type" "$live_value" "$yaml_value")" ]]
}

optional_record_fields() {
  local optional_fields=""
  if record_targets_current_host "$host"; then
    optional_fields+=", \"host\": \"$host\""
  fi
  if record_scope_is_system "$scope"; then
    optional_fields+=', "scope": "system"'
  fi
  printf '%s' "$optional_fields"
}

new_record_expression() {
  local yaml_type=$1 yaml_value=$2
  printf '.macos.defaults += [{"domain": "%s", "key": "%s", "type": "%s", "value": %s, "tier": "enforce"%s}]' \
    "$domain" "$key" "$yaml_type" "$yaml_value" "$(optional_record_fields)"
}

captured_record_details() {
  local yaml_type=$1
  local details="type $yaml_type"
  if record_targets_current_host "$host"; then
    details+=", host $host"
  fi
  if record_scope_is_system "$scope"; then
    details+=", scope system"
  fi
  printf '%s' "$details"
}

print_data_file_with_new_record() {
  local data_file=$1 yaml_type=$2 yaml_value=$3
  yq eval "$(new_record_expression "$yaml_type" "$yaml_value")" "$data_file"
}

append_record() {
  local data_file=$1 yaml_type=$2 yaml_value=$3
  capture_temp_file="$(mktemp "$data_file.XXXXXX")" || return
  trap 'rm -f "$capture_temp_file"' EXIT
  print_data_file_with_new_record "$data_file" "$yaml_type" "$yaml_value" >"$capture_temp_file" || return
  mv "$capture_temp_file" "$data_file" || return
  trap - EXIT
}

main() {
  if ! parse_arguments "$@"; then
    print_usage
    exit "$exit_malformed_arguments"
  fi
  require_valid_scope || exit "$exit_malformed_arguments"
  require_safe_identifier domain "$domain" || exit "$exit_malformed_arguments"
  require_safe_identifier key "$key" || exit "$exit_malformed_arguments"

  report_section 'macos-defaults' "capture $domain $key"

  local data_file target live_type_description yaml_type live_value yaml_value tracked_value
  data_file="$(macos_defaults_data_file)" || exit $?
  require_readable_data_file "$data_file" || exit $?
  target="$(defaults_read_target)" || exit $?

  if ! live_type_description="$(read_live_type_description "$target" "$key" 2>/dev/null)"; then
    report_line "error[not-set]: $domain $key is not currently set on this Mac." >&2
    exit "$exit_setting_not_set"
  fi
  if ! yaml_type="$(yaml_type_for_live_type "$live_type_description")"; then
    report_line "error[unsupported-type]: $domain $key has type '$live_type_description'; only bool, int, float and string are supported." >&2
    exit "$exit_unsupported_type"
  fi
  live_value="$(read_live_value "$target" "$key")" || exit $?
  yaml_value="$(live_value_as_yaml "$yaml_type" "$live_value")"

  tracked_value="$(read_tracked_value "$data_file")" || exit $?
  if setting_is_tracked "$tracked_value"; then
    if tracked_value_matches_live_value "$tracked_value" "$yaml_type" "$live_value" "$yaml_value"; then
      report_line "already tracked: $domain $key = $tracked_value"
      exit "$exit_already_tracked"
    fi
    report_line "error[tracked-value-differs]: $domain $key is $tracked_value in macos_defaults.yaml but $live_value on this Mac; run just macos-defaults-apply to restore it, or edit the YAML to keep it." >&2
    exit "$exit_tracked_value_differs"
  fi

  report_line "capturing $domain $key..."
  append_record "$data_file" "$yaml_type" "$yaml_value" || exit $?
  report_line "captured $domain $key = $live_value ($(captured_record_details "$yaml_type"))."
}

main "$@"
