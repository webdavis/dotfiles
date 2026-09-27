#!/usr/bin/env bash

set -euo pipefail

readonly exit_failed=1
readonly hermes_plugins="$HOME/.hermes/plugins"

installed_git_plugins() {
  local git_directory
  for git_directory in "$hermes_plugins"/*/.git; do
    if [[ -e $git_directory ]]; then
      basename "$(dirname "$git_directory")"
    fi
  done
}

plugin_revision() {
  local plugin=$1
  git -C "$hermes_plugins/$plugin" rev-parse --short HEAD
}

update_plugin() {
  local plugin=$1
  hermes plugins update "$plugin" </dev/null
}

main() {
  local failed=no plugin old new output
  while IFS= read -r plugin; do
    [[ -n $plugin ]] || continue
    old="$(plugin_revision "$plugin")"
    if ! output="$(update_plugin "$plugin" 2>&1)"; then
      printf 'error[update-failed]: %s could not be updated:\n%s\n' "$plugin" "$output" >&2
      failed=yes
      continue
    fi
    new="$(plugin_revision "$plugin")"
    if [[ $old != "$new" ]]; then
      printf '%s: %s → %s\n' "$plugin" "$old" "$new"
    fi
  done < <(installed_git_plugins)

  if [[ $failed == yes ]]; then
    exit "$exit_failed"
  fi
}

main "$@"
