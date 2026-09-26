#!/usr/bin/env bash

set -uo pipefail

# shellcheck source=scripts/treefmt/lib-render-context.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib-render-context.sh"

rendered_config_is_valid_json() {
  local file=$1
  render_template "$file" | jq empty
}

main() {
  local file status=0
  create_render_context || exit 1
  for file in "$@"; do
    if ! rendered_config_is_valid_json "$file"; then
      printf 'osquery-config-render: rendered config is not valid JSON: %s\n' "$file" >&2
      status=1
    fi
  done
  exit "$status"
}

main "$@"
