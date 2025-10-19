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

in_nix_dev_shell() {
  # The IN_NIX_SHELL environment variable is only present in Nix flake dev shells.
  case "${IN_NIX_SHELL:-}" in
    pure | impure) return 0 ;;
    *) return 1 ;;
  esac
}

get_script_path() {
  git ls-files --full-name "${BASH_SOURCE[0]}"
}

print_nix_shell_error() {
  local script_name="$1"

  local message="${script_name} must be run inside a Nix flake development shell.

To enter the flake shell, run:
  $ nix develop
  $ ./${script_name}

Alternatively, you can run this script ad hoc without entering the shell:
  $ nix develop .#adhoc --command ./${script_name}"

  printf "%s\n" "$message" >&2
}

assert_in_nix_shell_or_exit() {
  local script_name="$1"

  in_nix_dev_shell && return 0

  print_nix_shell_error "$script_name"

  exit 1
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
  assert_in_nix_shell_or_exit "$(get_script_path)"
  run_shellcheck
}

main "$@"
