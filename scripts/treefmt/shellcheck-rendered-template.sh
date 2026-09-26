#!/usr/bin/env bash

set -uo pipefail

treefmt_scripts_dir="$(dirname "${BASH_SOURCE[0]}")"

# shellcheck source=scripts/treefmt/lib-shellcheck-rendered-template.sh
source "$treefmt_scripts_dir/lib-shellcheck-rendered-template.sh"
# shellcheck source=scripts/treefmt/lib-render-context.sh
source "$treefmt_scripts_dir/lib-render-context.sh"

readonly leading_lines_that_decide_the_shell=10
readonly chezmoi_partials_dir='.chezmoitemplates'

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

file_mentions_keepassxc() {
  local file=$1
  grep -q 'keepassxc' "$file"
}

partials_included_by() {
  local file=$1
  grep -o 'includeTemplate "[^"]*"' "$file" | sed 's/includeTemplate "//; s/"$//'
}

template_or_its_partials_use_keepassxc() {
  local file=$1
  local -a unvisited=("$file") visited=()
  local current visited_file partial_name
  while ((${#unvisited[@]})); do
    current="${unvisited[0]}"
    unvisited=("${unvisited[@]:1}")
    for visited_file in "${visited[@]:-}"; do
      [[ $visited_file == "$current" ]] && continue 2
    done
    visited+=("$current")
    [[ -r $current ]] || continue
    if file_mentions_keepassxc "$current"; then
      return 0
    fi
    while IFS= read -r partial_name; do
      unvisited+=("$chezmoi_partials_dir/$partial_name")
    done < <(partials_included_by "$current")
  done
  return 1
}

main() {
  local file status=0
  create_render_context || exit 1
  for file in "$@"; do
    template_is_a_shell_script "$file" || continue
    template_or_its_partials_use_keepassxc "$file" && continue
    shellcheck_rendered_template "$file" || status=1
  done
  exit "$status"
}

main "$@"
