#!/usr/bin/env bash
# treefmt validator: osquery's JSON-bodied .conf files are the desired state
# osquery-converge.sh installs into /var/osquery, and two of them are chezmoi
# templates, so the plain *.json validator never sees any of them (wrong
# extension, and Go template actions no JSON parser can read). Render each
# through chezmoi and jq-validate the result.
#
# ONE code path for both kinds: execute-template on a file holding no template
# action renders it to itself, so a plain pack is validated by exactly the same
# command as a templated one and neither can be added to the tree unvalidated.
#
set -uo pipefail
# shellcheck source=/dev/null
source "$(dirname "${BASH_SOURCE[0]}")/lib-render-context.sh"
init_render_context || exit 1
status=0
for file; do
  render_template "$file" | jq empty || {
    printf 'osquery-config-render: rendered config is not valid JSON: %s\n' "$file" >&2
    status=1
  }
done
exit "$status"
