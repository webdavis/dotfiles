#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=scripts/treefmt/lib-shellcheck-rendered-template.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib-shellcheck-rendered-template.sh"
# shellcheck source=scripts/treefmt/lib-render-context.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib-render-context.sh"

readonly leading_lines_that_decide_the_shell=10
readonly chezmoi_partials_directory='.chezmoitemplates'

line_is_blank() {
  local line=$1
  [[ $line =~ ^[[:space:]]*$ ]]
}

line_is_only_a_template_directive() {
  local line=$1
  [[ $line =~ ^\{\{.*\}\}[[:space:]]*$ ]]
}

line_can_decide_the_shell() {
  local line=$1
  ! line_is_blank "$line" && ! line_is_only_a_template_directive "$line"
}

line_is_a_shebang() {
  local line=$1
  [[ $line =~ ^#! ]]
}

shebang_names_a_shell() {
  local shebang=$1
  [[ $shebang =~ sh ]]
}

line_is_a_shellcheck_shell_directive() {
  local line=$1
  [[ $line =~ ^#[[:space:]]*shellcheck[[:space:]]+shell= ]]
}

line_names_a_shell() {
  local line=$1
  if line_is_a_shebang "$line"; then
    shebang_names_a_shell "$line"
  else
    line_is_a_shellcheck_shell_directive "$line"
  fi
}

print_leading_lines() {
  local file=$1
  head -n "$leading_lines_that_decide_the_shell" "$file"
}

find_the_line_that_decides_the_shell() {
  local template=$1
  local line
  while IFS= read -r line; do
    if line_can_decide_the_shell "$line"; then
      printf '%s\n' "$line"
      return 0
    fi
  done < <(print_leading_lines "$template")
  return 1
}

template_is_a_shell_script() {
  local template=$1
  local deciding_line
  deciding_line="$(find_the_line_that_decides_the_shell "$template")" || return 1
  line_names_a_shell "$deciding_line"
}

file_is_readable() {
  local file=$1
  [[ -r $file ]]
}

file_mentions_keepassxc() {
  local file=$1
  grep -q 'keepassxc' "$file"
}

list_partial_names_included_by() {
  local template=$1
  grep -o 'includeTemplate "[^"]*"' "$template" | sed 's/includeTemplate "//; s/"$//'
}

templates_are_the_same() {
  local first_template=$1 second_template=$2
  [[ $first_template == "$second_template" ]]
}

template_is_in_an_include_loop() {
  local template=$1
  shift
  local -a including_templates=("$@")
  local including_template
  for including_template in "${including_templates[@]}"; do
    if templates_are_the_same "$template" "$including_template"; then
      return 0
    fi
  done
  return 1
}

list_files_that_make_up() {
  local template=$1
  shift
  local -a including_templates=("$@")
  local partial_name
  if template_is_in_an_include_loop "$template" "${including_templates[@]}"; then
    return 0
  fi
  if ! file_is_readable "$template"; then
    return 0
  fi
  printf '%s\n' "$template"
  while IFS= read -r partial_name; do
    list_files_that_make_up "$chezmoi_partials_directory/$partial_name" "$template" "${including_templates[@]}"
  done < <(list_partial_names_included_by "$template")
}

any_file_mentions_keepassxc() {
  local file
  for file in "$@"; do
    if file_mentions_keepassxc "$file"; then
      return 0
    fi
  done
  return 1
}

template_needs_keepassxc() {
  local template=$1
  local -a files_that_make_up_the_template
  mapfile -t files_that_make_up_the_template < <(list_files_that_make_up "$template")
  any_file_mentions_keepassxc "${files_that_make_up_the_template[@]}"
}

template_is_eligible_for_shellcheck() {
  local template=$1
  template_is_a_shell_script "$template" && ! template_needs_keepassxc "$template"
}

shellcheck_every_eligible_template() {
  local template shellcheck_status=0
  for template in "$@"; do
    if template_is_eligible_for_shellcheck "$template"; then
      shellcheck_rendered_template "$template" || shellcheck_status=1
    fi
  done
  return "$shellcheck_status"
}

main() {
  create_render_context || exit 1
  shellcheck_every_eligible_template "$@" || exit 1
}

main "$@"
