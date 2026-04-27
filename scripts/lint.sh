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
    [mdformat]=0
    [nixfmt]=0
    [taplo]=0
    [jq]=0
    [yq]=0
  )
}

change_to_project_root() {
  # Ensure this script runs from the project root.

  local project_root="${1:-}"

  if [[ -z ${project_root:-} ]]; then
    printf "%s\n" "Error: could not determine project root directory (are you in a Git repository?)" >&2
    exit 1
  fi

  if ! cd "$project_root"; then
    printf "%s\n" "Error: could not change into project root directory (${project_root})" >&2
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

assert_files_found() {
  local tool="$1"
  shift 1
  local files=("$@")

  if [[ ${#files[@]} -eq 0 ]]; then
    printf "%s\n" "⚠️ No files were found for $tool - skipping." >&2
    return 1
  fi
  return 0
}

print_runner_header() {
  local tool="${1:-}"
  shift 1
  local files=("$@")

  local yellow="\e[33m"
  local bold="\e[1m"
  local reset="\e[0m"

  printf "\n%s%b%s%b\n" " 🛠️ Checking ${#files[@]} file(s) with " "${yellow}" "$tool" "$reset"
  printf "%s\n" "———————————————————————————————————————————"
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
    printf "%s\n" "Processing ${tool}: ${file}"
    "${runner[@]}" "$file" || ((status = status == 0 ? $? : status))
  done
  printf "\n"

  ((status)) && EXIT_CODES[$tool]="$status"

  return "$status"
}

find_shell_files() {
  find . -type f \( \
    -name "*.sh" \
    -o -name "*.bash" \
    -o -name "dot_bash*" ! -name "*.tmpl" \
    -o -name "dot_profile" \
    \) -print0
}

run_10_shellcheck() {
  execute_runner find_shell_files shellcheck || return "$?"
}

find_shell_templates() {
  find . -type f \( \
    -name "dot_bashrc.tmpl" \
    \) -print0
}

shellcheck_rendered_template_runner() {
  local template_file="$1"
  CI=1 chezmoi execute-template --no-tty <"$template_file" | shellcheck - || return "$?"
}

run_11_shellcheck_templates() {
  execute_runner find_shell_templates shellcheck_rendered_template_runner || return "$?"
}

shfmt_runner() {
  local file="$1"
  # Show what will change (informational, non-fatal).
  shfmt -i 2 -ci -s --diff "$file" || true
  # Format in-place. Exit code reflects write success, not pre-format diff.
  shfmt -i 2 -ci -s --write "$file" || return "$?"
}

run_20_shfmt() {
  execute_runner find_shell_files shfmt_runner || return "$?"
}

find_markdown_files() {
  # Skip files with YAML frontmatter that mdformat can't preserve without the
  # mdformat-frontmatter plugin. Claude Code skills/agents/commands rely on
  # `---\nkey: value\n---` metadata blocks; running mdformat on them mangles
  # the frontmatter into an HR + H2 heading.
  # docs/research/ holds verbatim deep-research output — don't reformat
  # third-party content (some files use markdown extensions whose HTML
  # output mdformat's strict round-trip validator rejects).
  find . \
    -type d \( -name ".git" -o -regex ".*/\.?vendor" \
    -o -path "./private_dot_claude/skills" \
    -o -path "./private_dot_claude/agents" \
    -o -path "./private_dot_claude/commands" \
    -o -path "./docs/research" \) -prune \
    -o -type f -name "*.md" \
    -print0
}

mdformat_runner() {
  local file="$1"
  # Report formatting status (informational, non-fatal).
  mdformat --check "$file" || true
  # Format in-place. Exit code reflects format success, not pre-format check.
  mdformat "$file" || return "$?"
}

run_40_mdformat() {
  execute_runner find_markdown_files mdformat_runner || return "$?"
}

find_nix_files() {
  find . \
    -type d \( -name ".git" -o -name ".direnv" -o -regex ".*/\.?vendor" \) -prune \
    -o -type f -name "*.nix" \
    -print0
}

nixfmt_runner() {
  local file="$1"
  nix fmt -- --quiet "$file"
  nix fmt -- --ci --quiet "$file" || return "$?"
}

run_50_nixfmt() {
  execute_runner find_nix_files nixfmt_runner || return "$?"
}

find_toml_files() {
  # dot_aerospace.toml uses user-preferred visual alignment that taplo's
  # default formatter strips; skip it so the user's style is preserved.
  find . \
    -type d \( -name ".git" -o -name ".direnv" -o -regex ".*/\.?vendor" \) -prune \
    -o -type f -name "*.toml" ! -name "dot_aerospace.toml" \
    -print0
}

taplo_runner() {
  local file="$1"
  # Format in place, matching shfmt/mdformat behavior elsewhere in lint.sh.
  # A non-zero exit here means taplo couldn't parse the file (real syntax
  # error), not that it would change formatting.
  taplo format "$file" >/dev/null 2>&1 || return "$?"
}

run_60_taplo() {
  execute_runner find_toml_files taplo_runner || return "$?"
}

find_json_files() {
  # Exclude chezmoi modify_ templates: they share the .json extension of their
  # target file but contain Go template directives, so jq can't parse them.
  find . \
    -type d \( -name ".git" -o -name ".direnv" -o -name "node_modules" -o -regex ".*/\.?vendor" \) -prune \
    -o -type f -name "*.json" -not -name 'modify_*' \
    -print0
}

jq_runner() {
  local file="$1"
  jq empty <"$file" || return "$?"
}

run_70_jq() {
  execute_runner find_json_files jq_runner || return "$?"
}

find_yaml_files() {
  find .chezmoidata -type f \( -name "*.yaml" -o -name "*.yml" \) -print0 2>/dev/null
}

yq_runner() {
  local file="$1"
  yq eval '.' "$file" >/dev/null || return "$?"
}

run_80_yq() {
  execute_runner find_yaml_files yq_runner || return "$?"
}

parse_cli_options() {
  local -n pco_runners="${1:-runners}"
  shift 1
  local cli_options=("$@")

  local ci_mode=false

  local optstring=":csSrmntjy"
  while getopts "$optstring" option "${cli_options[@]}"; do
    case "$option" in
      c) ci_mode=true ;;
      s) pco_runners+=("run_10_shellcheck" "run_11_shellcheck_templates") ;;
      S) pco_runners+=("run_20_shfmt") ;;
      m) pco_runners+=("run_40_mdformat") ;;
      n) pco_runners+=("run_50_nixfmt") ;;
      t) pco_runners+=("run_60_taplo") ;;
      j) pco_runners+=("run_70_jq") ;;
      y) pco_runners+=("run_80_yq") ;;
      *)
        printf "%s\n" "Error: invalid option '$OPTARG'" >&2
        exit 1
        ;;
    esac
  done

  printf "%b" "$ci_mode"
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

  local blue="\e[34m"
  local bold="\e[1m"
  local reset="\e[0m"
  local header="
╭─────────────────╮
│     SUMMARY     │
╰─────────────────╯
"
  printf "%b%b%s%b\n" "${bold}" "${blue}" "$header" "$reset"

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
  local summary="${1:-}"
  {
    printf "%s\n" "### 📝 Lint／Format Summary"
    printf "\n"
    printf "%b" "$summary"
  } >>"${GITHUB_STEP_SUMMARY:-}"
}

summarize_in_ci() {
  local -n si_ci_fields="$1"
  local -n si_ci_results="$2"

  local format="| %s | %s |"
  local divider="| --- | --- |"
  write_to_github_step_summary "$(build_summary "si_ci_fields" "si_ci_results" "$format" "$divider")"
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
