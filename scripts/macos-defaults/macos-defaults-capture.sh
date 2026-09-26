#!/usr/bin/env bash

set -euo pipefail

macos_defaults_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
dotfiles_dir="$(cd "$macos_defaults_dir/../.." && pwd)"

# shellcheck source=.chezmoitemplates/cli-print-style-lib.sh.tmpl
source "$dotfiles_dir/.chezmoitemplates/cli-print-style-lib.sh.tmpl"
# shellcheck source=scripts/macos-defaults/helpers/defaults-records.sh
source "$macos_defaults_dir/helpers/defaults-records.sh"

readonly exit_captured_or_already_tracked=0
readonly exit_setting_not_set=1
readonly exit_malformed_arguments=3
readonly exit_tracked_value_differs=4
readonly safe_identifier_pattern='^[a-zA-Z0-9._-]+$'

domain=""
key=""
host=""
scope="user"
capture_temp_file=""

exit_with_usage() {
  report_line "usage: macos-defaults-capture.sh <domain> <key> [--host current] [--scope user|system]" >&2
  exit "$exit_malformed_arguments"
}

parse_arguments() {
  if (($# < 2 || $# > 6)); then
    exit_with_usage
  fi
  domain=$1
  key=$2
  shift 2
  while (($# > 0)); do
    case "$1" in
      --host=current)
        host=current
        shift
        ;;
      --host)
        if (($# < 2)) || [[ $2 != current ]]; then
          exit_with_usage
        fi
        host=current
        shift 2
        ;;
      --scope=*)
        scope=${1#*=}
        shift
        ;;
      --scope)
        if (($# < 2)); then
          exit_with_usage
        fi
        scope=$2
        shift 2
        ;;
      *) exit_with_usage ;;
    esac
  done
}

require_valid_scope() {
  if ! validate_record_scope "$scope" "$host" "" >/dev/null; then
    report_line "error[invalid-scope]: --scope '$scope' cannot be combined with --host '$host'." >&2
    exit "$exit_malformed_arguments"
  fi
}

require_safe_identifier() {
  local label=$1 identifier=$2
  if [[ ! $identifier =~ $safe_identifier_pattern ]]; then
    report_line "error[invalid-$label]: '$identifier' contains characters outside letters, digits, dot, underscore and dash." >&2
    exit "$exit_malformed_arguments"
  fi
}

defaults_command() {
  if [[ -n $host ]]; then
    defaults -currentHost "$@"
  else
    defaults "$@"
  fi
}

read_target() {
  if [[ $scope == system ]]; then
    resolve_system_plist_path "$domain" ""
  else
    printf '%s' "$domain"
  fi
}

schema_type_for() {
  local defaults_type=$1
  case "$defaults_type" in
    *boolean*) printf 'bool' ;;
    *integer*) printf 'int' ;;
    *float*) printf 'float' ;;
    *string*) printf 'string' ;;
    *) return 1 ;;
  esac
}

yaml_value_for() {
  local schema_type=$1 live_value=$2
  case "$schema_type" in
    bool)
      case "$live_value" in
        1) printf 'true' ;;
        0) printf 'false' ;;
        *) printf '%s' "$live_value" ;;
      esac
      ;;
    string) printf '"%s"' "${live_value//\"/\\\"}" ;;
    *) printf '%s' "$live_value" ;;
  esac
}

comparable_live_value() {
  local schema_type=$1 live_value=$2 yaml_value=$3
  if [[ $schema_type == string ]]; then
    printf '%s' "$live_value"
  else
    printf '%s' "$yaml_value"
  fi
}

tracked_value() {
  local data_file=$1
  yq eval -r \
    ".macos.defaults[] | select(.domain == \"$domain\" and .key == \"$key\" and ((.host // \"\") == \"$host\") and ((.scope // \"user\") == \"$scope\")) | .value" \
    "$data_file"
}

new_record_expression() {
  local schema_type=$1 yaml_value=$2
  local optional_fields=""
  if [[ -n $host ]]; then
    optional_fields+=", \"host\": \"$host\""
  fi
  if [[ $scope == system ]]; then
    optional_fields+=', "scope": "system"'
  fi
  printf '.macos.defaults += [{"domain": "%s", "key": "%s", "type": "%s", "value": %s, "tier": "enforce"%s}]' \
    "$domain" "$key" "$schema_type" "$yaml_value" "$optional_fields"
}

captured_record_details() {
  local schema_type=$1
  local details="type $schema_type"
  if [[ -n $host ]]; then
    details+=", host $host"
  fi
  if [[ $scope == system ]]; then
    details+=", scope system"
  fi
  printf '%s' "$details"
}

append_record() {
  local data_file=$1 schema_type=$2 yaml_value=$3
  capture_temp_file="$(mktemp "$data_file.XXXXXX")"
  trap 'rm -f "$capture_temp_file"' EXIT
  yq eval "$(new_record_expression "$schema_type" "$yaml_value")" "$data_file" >"$capture_temp_file"
  mv "$capture_temp_file" "$data_file"
  trap - EXIT
}

main() {
  parse_arguments "$@"
  require_valid_scope
  require_safe_identifier domain "$domain"
  require_safe_identifier key "$key"

  report_section 'macos-defaults' "capture $domain $key"

  local data_file target defaults_type schema_type live_value yaml_value existing_value
  data_file="$(macos_defaults_data_file)" || exit $?
  require_readable_data_file "$data_file" || exit $?
  target="$(read_target)"

  if ! defaults_type="$(defaults_command read-type "$target" "$key" 2>/dev/null)"; then
    report_line "error[not-set]: $domain $key is not currently set on this Mac." >&2
    exit "$exit_setting_not_set"
  fi
  if ! schema_type="$(schema_type_for "$defaults_type")"; then
    report_line "error[unsupported-type]: $domain $key has type '$defaults_type'; only bool, int, float and string are supported." >&2
    exit "$exit_setting_not_set"
  fi
  live_value="$(defaults_command read "$target" "$key")"
  yaml_value="$(yaml_value_for "$schema_type" "$live_value")"

  existing_value="$(tracked_value "$data_file")"
  if [[ -n $existing_value ]]; then
    if [[ $existing_value == "$(comparable_live_value "$schema_type" "$live_value" "$yaml_value")" ]]; then
      report_line "already tracked: $domain $key = $existing_value"
      exit "$exit_captured_or_already_tracked"
    fi
    report_line "error[tracked-value-differs]: $domain $key is $existing_value in macos_defaults.yaml but $live_value on this Mac; run just macos-defaults-apply to restore it, or edit the YAML to keep it." >&2
    exit "$exit_tracked_value_differs"
  fi

  report_line "capturing $domain $key..."
  append_record "$data_file" "$schema_type" "$yaml_value"
  report_line "captured $domain $key = $live_value ($(captured_record_details "$schema_type"))."
}

main "$@"
