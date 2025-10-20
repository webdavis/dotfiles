#!/usr/bin/env bash
# Prevent Shellcheck from complaining about named references:
# shellcheck disable=SC2034

# Exit immediately if any variables are empty.
set -u

declare -A EXIT_CODES=()

get_project_root() {
  git rev-parse --show-toplevel
}

track_runner_exit_codes() {
  EXIT_CODES=(
    [shellcheck]=0
    [shfmt]=0
  )
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
  local tool="$1"
  shift 1
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
  local finder="$1"
  shift 1
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
    "${runner[@]}" "$file" || ((status = status == 0 ? $? : status))
  done
  echo

  ((status)) && EXIT_CODES[$tool]="$status"

  return "$status"
}

shfmt_runner() {
  local file="$1"

  local status=0

  shfmt -i 2 -ci -s --diff "$file" || status="$?"
  shfmt -i 2 -ci -s --write "$file"

  return "$status"
}

run_10_shfmt() {
  execute_runner find_shell_files shfmt_runner || return "$?"
}

run_20_shellcheck() {
  execute_runner find_shell_files shellcheck || return "$?"
}

parse_cli_options() {
  local -n pco_runners="${1:-runners}"
  shift 1
  local cli_options=("$@")

  local ci_mode=false

  local optstring=":sS"
  while getopts "$optstring" option "${cli_options[@]}"; do
    case "$option" in
      s) pco_runners+=("run_shellcheck") ;;
      S) pco_runners+=("run_shfmt") ;;
      c) ci_mode=true ;;
      *)
        echo "Error: invalid option '$OPTARG'" >&2
        exit 1
        ;;
    esac
  done

  echo "$ci_mode"
}

execute_runners() {
  local -n er_runners="${1:-runners}"

  local status=0
  local runner
  for runner in "${er_runners[@]}"; do
    $runner || ((status = status == 0 ? $? : status))
  done

  return "$status"
}

get_all_runners_by_priority() {
  # List all functions starting with "run_##" and sort by priority.
  declare -F | awk '{print $3}' | grep '^run_[0-9][0-9]_' | sort
}

build_tool_results() {
  local code
  for code in "${!EXIT_CODES[@]}"; do
    if [[ ${EXIT_CODES[$code]} -eq 0 ]]; then
      printf "%s\n" "${code}:✅"
    else
      printf "%s\n" "${code}:❌"
    fi
  done
}

get_rows() {
  local format="${1:-}"
  shift 1
  local results=("$@")

  local entry tool status rows=""
  for entry in "${results[@]}"; do
    IFS=":" read -r tool status <<<"$entry"

    # shellcheck disable=SC2059
    rows+="$(printf "$format" "$tool" "$status")"$'\n'
  done

  printf "%b" "$rows"
}

build_summary() {
  local -n bs_fields="$1"
  local -n bs_results="$2"
  local format="${3:-}"
  local divider="${4:-}"

  # shellcheck disable=SC2059
  printf "%b" \
    "$(printf "$format" "${bs_fields[@]}")" \
    $'\n' \
    "${divider}" \
    $'\n' \
    "$(get_rows "$format" "${bs_results[@]}")"
}

print_to_console() {
  local summary="${1:-}"
  printf "%b\n" "$summary" | column -t -s $'\t' -c 200
}

summarize_in_console() {
  local -n si_con_fields="$1"
  local -n si_con_results="$2"

  local format="%s\t%s"
  local divider="----------\t-------"
  print_to_console "$(build_summary "si_con_fields" "si_con_results" "$format" "$divider")"
}

write_to_github_step_summary() {
  echo "test 3"
  local summary="${1:-}"
  {
    echo "### 📝 Lint／Format Summary"
    echo ""
    printf "%b" "$summary"
  } >>"${GITHUB_STEP_SUMMARY:-}"
  echo "test 4"
}

summarize_in_ci() {
  echo "test"
  local -n si_ci_fields="$1"
  local -n si_ci_results="$2"

  local format="| %s | %s |"
  local divider="| --- | --- |"
  write_to_github_step_summary "$(build_summary "si_ci_fields" "si_ci_results" "$format" "$divider")"
  echo "test 2"
}

summarize_results() {
  local ci_mode="${1:-false}"

  local -a fields=("TOOL" "STATUS")
  local -a results
  mapfile -t results < <(build_tool_results)

  summarize_in_console "fields" "results"

  if $ci_mode; then
    summarize_in_ci "fields" "results"
  fi
}

main() {
  change_to_project_root "$(get_project_root)"
  assert_in_nix_shell_or_exit "$(get_script_path)"
  track_runner_exit_codes

  local -a runners=()
  local ci_mode
  ci_mode="$(parse_cli_options "runners" "$@")"

  ((${#runners[@]} == 0)) && mapfile -t runners < <(get_all_runners_by_priority)

  local status=0
  execute_runners "runners" || status="$?"

  summarize_results "$ci_mode"

  return "$status"
}

main "$@"
