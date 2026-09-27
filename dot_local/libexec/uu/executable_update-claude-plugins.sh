#!/usr/bin/env bash

set -euo pipefail

readonly exit_needs_you=100
readonly exit_failed=1

installed_plugin_versions() {
  claude plugin list --json </dev/null |
    jq -r '.[] | select(.scope == "user") | "\(.id)\t\(.version // "unknown")"'
}

version_of() {
  local versions=$1 plugin=$2
  awk -F '\t' -v plugin="$plugin" '$1 == plugin { print $2 }' <<<"$versions"
}

update_marketplaces() {
  claude plugin marketplace update </dev/null
}

update_plugin() {
  local plugin=$1
  claude plugin update "$plugin" --scope user --json </dev/null 2>/dev/null | tail -n 1
}

update_succeeded() {
  local result=$1
  jq -e '.outcome == "ok"' >/dev/null 2>&1 <<<"$result"
}

update_needs_an_accepted_command() {
  local result=$1
  jq -e '.shownCommand.sha256 != null' >/dev/null 2>&1 <<<"$result"
}

accepted_command_hash() {
  local result=$1
  jq -r '.shownCommand.sha256' <<<"$result"
}

failure_message() {
  local result=$1
  local message
  message="$(jq -r '.message // empty' 2>/dev/null <<<"$result")" || message=$result
  printf '%s' "${message:-claude gave no result}"
}

print_changes() {
  local before=$1 after=$2
  local plugin new old
  while IFS=$'\t' read -r plugin new; do
    old="$(version_of "$before" "$plugin")"
    if [[ -n $old && $old != "$new" ]]; then
      printf '%s: %s → %s\n' "$plugin" "$old" "$new"
    fi
  done <<<"$after"
}

main() {
  local before after output plugin result failed=no needs_you=no
  if ! before="$(installed_plugin_versions)"; then
    printf 'error[list-failed]: claude plugin list did not answer.\n' >&2
    exit "$exit_failed"
  fi

  if ! output="$(update_marketplaces 2>&1)"; then
    printf 'error[marketplace-update-failed]: the plugin marketplaces could not be refreshed:\n%s\n' "$output" >&2
    failed=yes
  fi

  while IFS=$'\t' read -r plugin _; do
    [[ -n $plugin ]] || continue
    result="$(update_plugin "$plugin" || true)"
    if update_succeeded "$result"; then
      continue
    elif update_needs_an_accepted_command "$result"; then
      printf 'error[needs-approval]: %s wants to run a new install command; review it, then run: claude plugin update %s --accept-command %s\n' \
        "$plugin" "$plugin" "$(accepted_command_hash "$result")" >&2
      needs_you=yes
    else
      printf 'error[update-failed]: %s could not be updated: %s\n' "$plugin" "$(failure_message "$result")" >&2
      failed=yes
    fi
  done <<<"$before"

  if ! after="$(installed_plugin_versions)"; then
    printf 'error[list-failed]: claude plugin list did not answer after the updates.\n' >&2
    exit "$exit_failed"
  fi
  print_changes "$before" "$after"

  if [[ $failed == yes ]]; then
    exit "$exit_failed"
  fi
  if [[ $needs_you == yes ]]; then
    exit "$exit_needs_you"
  fi
}

main "$@"
