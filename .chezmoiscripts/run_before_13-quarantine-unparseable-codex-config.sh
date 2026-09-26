#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=.chezmoitemplates/cli-print-style-lib.sh.tmpl
source "${CHEZMOI_SOURCE_DIR:?}/.chezmoitemplates/cli-print-style-lib.sh.tmpl"

readonly config_file="$HOME/.codex/config.toml"
readonly backup_directory="$HOME/workspaces/backups"
readonly backup_file_suffix="codex-config-quarantined.backup.toml"

config_file_exists() {
  [[ -f $config_file ]]
}

taplo_is_installed() {
  command -v taplo >/dev/null 2>&1
}

config_file_has_valid_toml_syntax() {
  taplo check --no-auto-config "$config_file" >/dev/null 2>&1
}

filename_safe_utc_timestamp() {
  date -u +"%Y-%m-%dT%H-%M-%S"
}

move_config_file_into_backups() {
  local backup_file=$1
  mkdir -p "$backup_directory" && mv "$config_file" "$backup_file"
}

create_empty_private_config_file() {
  (umask 077 && : >"$config_file")
}

quarantine_config_file() {
  local timestamp
  timestamp="$(filename_safe_utc_timestamp)"
  local backup_file="$backup_directory/$timestamp.$backup_file_suffix"

  report_line "config.toml does not parse, moving it into $backup_directory..."
  if ! move_config_file_into_backups "$backup_file"; then
    report_line "error[move-failed]: $config_file does not parse and could not be moved into $backup_directory." >&2
    report_line "The apply will fail in modify_private_config.toml until that file is repaired by hand." >&2
    return
  fi

  report_line "error[unparseable-config]: $config_file did not parse. It was moved to $backup_file and replaced with an empty file." >&2
  report_line "This apply rebuilds the managed fields. It does not restore hook trust: open Codex and run /hooks to re-approve them." >&2
  create_empty_private_config_file
}

main() {
  report_section 'codex' 'config.toml parse check'

  if ! config_file_exists; then
    report_line "no config.toml to check, skipping."
    return
  fi

  if ! taplo_is_installed; then
    report_line "taplo is not installed yet, so config.toml cannot be checked, skipping."
    return
  fi

  if config_file_has_valid_toml_syntax; then
    report_line "config.toml parses, skipping."
    return
  fi

  quarantine_config_file
}

main "$@"
