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
# The throwaway HOME is chezmoi's read-source-state pre hook, which chdirs
# there; --source pins the render to this checkout.
set -uo pipefail
HOME="$(mktemp -d)"
export HOME
status=0
for file; do
  CI=1 chezmoi --source "$PWD" execute-template --no-tty <"$file" | jq empty || {
    printf 'osquery-config-render: rendered config is not valid JSON: %s\n' "$file" >&2
    status=1
  }
done
exit "$status"
