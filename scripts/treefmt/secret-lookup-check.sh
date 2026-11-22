#!/usr/bin/env bash

set -euo pipefail

readonly direct_vault_call_pattern='(^|[^A-Za-z0-9_])keepassxc(Attribute|Attachment)? +[("$]'

file_calls_the_vault_directly() {
  local file=$1
  grep -q -E "$direct_vault_call_pattern" "$file"
}

report_direct_vault_calls_in() {
  local file=$1
  printf 'secret-lookup-check: %s reads the vault directly; name the entry in .chezmoidata/secrets.yaml and read it through secret.tmpl:\n' "$file" >&2
  grep -n -E "$direct_vault_call_pattern" "$file" >&2
}

check_every_template() {
  local file check_status=0
  for file in "$@"; do
    if file_calls_the_vault_directly "$file"; then
      report_direct_vault_calls_in "$file"
      check_status=1
    fi
  done
  return "$check_status"
}

main() {
  check_every_template "$@" || exit 1
}

main "$@"
