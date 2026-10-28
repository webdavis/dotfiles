#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=scripts/treefmt/lib-render-context.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib-render-context.sh"

standard_input_is_valid_json() {
  jq empty
}

rendered_config_is_valid_json() {
  local file=$1
  render_template "$file" | standard_input_is_valid_json
}

report_invalid_rendered_config() {
  local file=$1
  printf 'osquery-config-render: rendered config is not valid JSON: %s\n' "$file" >&2
}

validate_every_rendered_config() {
  local file validation_status=0
  for file in "$@"; do
    if ! rendered_config_is_valid_json "$file"; then
      report_invalid_rendered_config "$file"
      validation_status=1
    fi
  done
  return "$validation_status"
}

main() {
  create_render_context || exit 1
  validate_every_rendered_config "$@" || exit 1
}

main "$@"
