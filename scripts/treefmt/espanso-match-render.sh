#!/usr/bin/env bash

set -uo pipefail

# shellcheck source=scripts/treefmt/lib-render-context.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib-render-context.sh"

rendered_match_file_is_valid_yaml() {
  local file=$1
  render_template "$file" | yq eval '.' - >/dev/null
}

main() {
  local file status=0
  init_render_context || exit 1
  for file in "$@"; do
    if ! rendered_match_file_is_valid_yaml "$file"; then
      printf 'espanso-match-render: rendered match file is not valid YAML: %s\n' "$file" >&2
      status=1
    fi
  done
  exit "$status"
}

main "$@"
