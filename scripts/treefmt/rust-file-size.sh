#!/usr/bin/env bash
set -euo pipefail

status=0
for file; do
  if ! awk -v file="$file" '
    END {
      if (NR > 500) {
        printf "rust-file-size: %s has %d physical lines (limit 500)\n", file, NR
        exit 1
      }
    }
  ' <"$file" >&2; then
    status=1
  fi
done
exit "$status"
