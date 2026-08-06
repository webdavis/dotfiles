#!/usr/bin/env bash
# treefmt validator: `jq empty` per file, non-zero on any JSON parse error.
# Ported verbatim from the old treefmt.nix jqValidate writeShellApplication.
set -uo pipefail
status=0
for file; do
  jq empty <"$file" || {
    echo "jq-validate: invalid JSON: $file" >&2
    status=1
  }
done
exit "$status"
