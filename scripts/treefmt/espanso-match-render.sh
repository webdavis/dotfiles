#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=scripts/treefmt/lib-render-context.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib-render-context.sh"

standard_input_is_valid_yaml() {
  yq eval '.' - >/dev/null
}

rendered_match_file_is_valid_yaml() {
  local file=$1
  render_template "$file" | standard_input_is_valid_yaml
}

report_invalid_rendered_match_file() {
  local file=$1
  printf 'espanso-match-render: rendered match file is not valid YAML: %s\n' "$file" >&2
}

validate_every_rendered_match_file() {
  local file validation_status=0
  for file in "$@"; do
    if ! rendered_match_file_is_valid_yaml "$file"; then
      report_invalid_rendered_match_file "$file"
      validation_status=1
    fi
  done
  return "$validation_status"
}

main() {
  create_render_context || exit 1
  validate_every_rendered_match_file "$@" || exit 1
}

main "$@"
