#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=.chezmoitemplates/cli-print-style-lib.sh.tmpl
source "${CHEZMOI_SOURCE_DIR:?}/.chezmoitemplates/cli-print-style-lib.sh.tmpl"

readonly settings_file="$HOME/.claude/settings.json"
readonly backup_directory="$HOME/workspaces/backups"
readonly backup_file_suffix="claude-settings-quarantined.backup.json"

settings_file_exists() {
  [[ -f $settings_file ]]
}

jq_is_installed() {
  command -v jq >/dev/null 2>&1
}

settings_file_holds_at_most_one_json_value() {
  jq -e -s 'length <= 1' <"$settings_file" >/dev/null 2>&1
}

filename_safe_utc_timestamp() {
  date -u +"%Y-%m-%dT%H-%M-%S"
}

move_settings_file_into_backups() {
  local backup_file=$1
  mkdir -p "$backup_directory" && mv "$settings_file" "$backup_file"
}

write_empty_json_object_to_settings_file() {
  printf '{}\n' >"$settings_file"
}

quarantine_settings_file() {
  local timestamp
  timestamp="$(filename_safe_utc_timestamp)"
  local backup_file="$backup_directory/$timestamp.$backup_file_suffix"

  report_line "settings.json does not parse, moving it into $backup_directory..."
  if ! move_settings_file_into_backups "$backup_file"; then
    report_line "error[move-failed]: $settings_file does not parse and could not be moved into $backup_directory." >&2
    report_line "The apply will fail in modify_settings.json until that file is repaired by hand." >&2
    return
  fi

  report_line "error[unparseable-settings]: $settings_file did not parse. It was moved to $backup_file and replaced with an empty object." >&2
  report_line "This apply rebuilds the managed fields. It does not restore per-plugin state: re-run 'claude plugin disable <id>' for anything that was disabled." >&2
  write_empty_json_object_to_settings_file
}

main() {
  report_section 'claude' 'settings.json parse check'

  if ! settings_file_exists; then
    report_line "no settings.json to check, skipping."
    return
  fi

  if ! jq_is_installed; then
    report_line "jq is not installed yet, so settings.json cannot be checked, skipping."
    return
  fi

  if settings_file_holds_at_most_one_json_value; then
    report_line "settings.json parses, skipping."
    return
  fi

  quarantine_settings_file
}

main "$@"
