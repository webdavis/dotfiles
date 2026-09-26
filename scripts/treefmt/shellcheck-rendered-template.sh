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

is_shell_template() {
  local file=$1
  local line lines_read=0
  while IFS= read -r line && ((lines_read < leading_lines_that_decide_the_shell)); do
    lines_read=$((lines_read + 1))
    if line_is_blank "$line" || line_is_only_a_template_directive "$line"; then
      continue
    fi
    if line_is_a_shebang "$line"; then
      shebang_names_a_shell "$line"
      return
    fi
    line_is_a_shellcheck_shell_directive "$line"
    return
  done <"$file"
  return 1
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
    is_shell_template "$file" || continue
    template_or_its_partials_use_keepassxc "$file" && continue
    shellcheck_rendered_template "$file" || status=1
  done
  exit "$status"
}

main "$@"
