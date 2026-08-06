#!/usr/bin/env bash
# treefmt validator: render espanso .yml.tmpl match files and yq-validate the
# result. CI=1 is load-bearing: the identity template's vault reads sit behind
# `{{ if (env "CI") }}`, so this renders without KeePassXC. Ported from the old
# treefmt.nix espansoMatchRender writeShellApplication.
set -uo pipefail
HOME="$(mktemp -d)"
export HOME
status=0
for file; do
  CI=1 chezmoi --source "$PWD" execute-template --no-tty <"$file" |
    yq eval '.' - >/dev/null || {
    echo "espanso-match-render: rendered match file is not valid YAML: $file" >&2
    status=1
  }
done
exit "$status"
