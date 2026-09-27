#!/usr/bin/env bash

set -euo pipefail

readonly exit_failed=1

installed_plugin_versions() {
  codex plugin list --json </dev/null |
    jq -r '.installed[] | "\(.pluginId)\t\(.version // "unknown")"'
}

version_of() {
  local versions=$1 plugin=$2
  awk -F '\t' -v plugin="$plugin" '$1 == plugin { print $2 }' <<<"$versions"
}

upgrade_marketplaces() {
  codex plugin marketplace upgrade </dev/null
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
  local before after output
  if ! before="$(installed_plugin_versions)"; then
    printf 'error[list-failed]: codex plugin list did not answer.\n' >&2
    exit "$exit_failed"
  fi

  if ! output="$(upgrade_marketplaces 2>&1)"; then
    printf 'error[marketplace-upgrade-failed]: the plugin marketplaces could not be upgraded:\n%s\n' "$output" >&2
    exit "$exit_failed"
  fi

  if ! after="$(installed_plugin_versions)"; then
    printf 'error[list-failed]: codex plugin list did not answer after the upgrade.\n' >&2
    exit "$exit_failed"
  fi
  print_changes "$before" "$after"
}

main "$@"
