#!/usr/bin/env bash
# treefmt validator: `yq eval` per file, non-zero on any YAML parse error.
# Ported verbatim from the old treefmt.nix yqValidate writeShellApplication.
set -uo pipefail
status=0
for file; do
  yq eval '.' "$file" >/dev/null || {
    echo "yq-validate: invalid YAML: $file" >&2
    status=1
  }
done
exit "$status"
