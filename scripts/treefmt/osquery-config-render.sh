#!/usr/bin/env bash
# treefmt validator: osquery's JSON-bodied .conf templates render via
# includeTemplate, so the plain *.json validator never sees them. Render each
# and jq-validate the result. Ported from the old treefmt.nix
# osqueryConfigRender writeShellApplication.
set -uo pipefail
HOME="$(mktemp -d)"
export HOME
status=0
for file; do
  tmpl="${file#./}"
  tmpl="${tmpl#.chezmoitemplates/}"
  CI=1 chezmoi --source "$PWD" execute-template --no-tty \
    "{{ includeTemplate \"${tmpl}\" . }}" | jq empty || {
    echo "osquery-config-render: rendered config is not valid JSON: $file" >&2
    status=1
  }
done
exit "$status"
