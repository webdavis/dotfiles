#!/usr/bin/env bash

# Exit immediately if any command fails (including compound commands).
set -euo pipefail

# Change to project root.
cd "$(git rev-parse --show-toplevel)"

# Store all shell scripts and shell-based dotfiles in an array.
mapfile -d '' files < <(find . -type f \( -name "*.sh" -o -name "*.bash" -o -name "dot_bash*" ! -name "*.tmpl" -o -name "dot_profile" \) -print0)

if [[ ${#files[@]} -eq 0 ]]; then
  echo "No shell scripts or shell-based dotfiles found."
  exit 0
fi

echo "✅ Found ${#files[@]} file(s) to lint:"
for f in "${files[@]}"; do
  echo "$f"
done

# Run shellcheck on all files.
for file in "${files[@]}"; do
  echo "Linting: $file"

  shellcheck "$file"
done
