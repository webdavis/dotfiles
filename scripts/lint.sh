#!/usr/bin/env bash
# Prevent Shellcheck from complaining about named references:
# shellcheck disable=SC2034

# Exit immediately if any variables are empty.
set -u

get_project_root() {
  git rev-parse --show-toplevel
}

change_to_project_root() {
  # Ensure this script runs from the project root.

  local project_root="${1:-}"

  if [[ -z ${project_root:-} ]]; then
    echo "Error: could not determine project root directory (are you in a Git repository?)" >&2
    exit 1
  fi

  if ! cd "$project_root"; then
    echo "Error: could not change into project root directory (${project_root})" >&2
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
  local script_name="${1:-}"

  local message="Error: ${script_name} must be run inside a Nix flake development shell.

To enter the flake shell, run:
  $ nix develop
  $ ./${script_name}

Alternatively, you can run this script ad hoc without entering the shell:
  $ nix develop .#adhoc --command ./${script_name}"

  printf "%s\n" "$message" >&2
}

assert_in_nix_shell_or_exit() {
  local script_name="${1:-}"

  in_nix_dev_shell && return 0

  print_nix_shell_error "$script_name"

  exit 1
}

find_shell_files() {
  find . -type f \( \
    -name "*.sh" \
    -o -name "*.bash" \
    -o -name "dot_bash*" ! -name "*.tmpl" \
    -o -name "dot_profile" \
  \) -print0
}

assert_files_found() {
  local tool="$1"; shift 1
  local files=("$@")

  if [[ ${#files[@]} -eq 0 ]]; then
    echo "⚠️ No files were found for $tool - skipping." >&2
    return 1
  fi
  return 0
}

print_runner_header() {
  local tool="${1:-}"
  shift 1
  local files=("$@")

  echo " 🛠️ Checking ${#files[@]} file(s) with $tool"
  echo "———————————————————————————————————————————"
}

execute_runner() {
  local finder="$1"; shift 1
  local runner=("$@")

  local -a files=()
  mapfile -d '' files < <("$finder")

  local tool="${runner[0]%%_*}"
  assert_files_found "$tool" "${files[@]}" || return 1

  local status=0

  print_runner_header "$tool" "${files[@]}"

  local file
  for file in "${files[@]}"; do
    echo "Processing ${tool}: ${file}"
    "${runner[@]}" "$file" || ((status=status==0 ? $? : status))
  done
  echo

  return "$status"
}

run_shellcheck() {
  execute_runner find_shell_files shellcheck || return "$?"
}

shfmt_runner() {
  local file="$1"
  shfmt -i 2 -ci -s --diff "$file"
  shfmt -i 2 -ci -s --write "$file"
}

run_shfmt() {
  execute_runner find_shell_files shfmt_runner || return "$?"
}

parse_cli_options() {
  local -n pco_runners="${1:-runners}"
  shift 1
  local cli_options=("$@")

  local optstring=":sS"
  while getopts "$optstring" option "${cli_options[@]}"; do
    case "$option" in
      s) pco_runners+=("run_shellcheck") ;;
      S) pco_runners+=("run_shfmt") ;;
      *) echo "Error: invalid option '$OPTARG'" >&2; exit 1 ;;
    esac
  done
}

execute_runners() {
  local -n er_runners="${1:-runners}"

  local status=0
  local runner
  for runner in "${er_runners[@]}"; do
    $runner || ((status=status==0 ? $? : status))
  done

  return "$status"
}

get_all_runners() {
  declare -F | awk '{print $3}' | grep '^run_'
}

main() {
  change_to_project_root "$(get_project_root)"
  assert_in_nix_shell_or_exit "$(get_script_path)"

  local -a runners=()
  parse_cli_options "runners" "$@"

  (( ${#runners[@]} == 0 )) && mapfile -t runners < <(get_all_runners)

  execute_runners "runners" || return "$?"
}

main "$@"
