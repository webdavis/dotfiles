#!/usr/bin/env bash

# Exit immediately if any command fails (including compound commands).
set -euo pipefail

get_project_root() {
  # Change to project root.
  git rev-parse --show-toplevel
}

change_to_project_root() {
  # Ensure this script runs from the project root.

  local project_root="$1"

  if [[ -z ${project_root:-} ]]; then
    echo "Error: could not determine project root directory (are you in a Git repository?)" >&2
    exit 1
  fi

  if ! cd "$project_root"; then
    echo "Error: could not change into project root directory (${project_root##*/})" >&2
    exit 1
  fi
}

run_shellcheck() {
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
}

main() {
  change_to_project_root "$(get_project_root)"
  run_shellcheck
}

main "$@"
