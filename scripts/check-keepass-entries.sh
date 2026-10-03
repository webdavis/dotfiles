#!/usr/bin/env bash

set -euo pipefail

dotfiles_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# shellcheck source=.chezmoitemplates/cli-print-style-lib.sh.tmpl
source "$dotfiles_directory/.chezmoitemplates/cli-print-style-lib.sh.tmpl"

readonly chezmoi_config_template="$dotfiles_directory/.chezmoi.toml.tmpl"
readonly keepassxc_reference_pattern='keepassxc[A-Za-z]* +"[^"$]+"'
readonly exit_all_found=0
readonly exit_entries_missing=1
readonly exit_could_not_check=2

if [[ ${REPORT_LIB_PLAIN:-} == 1 || -n ${NO_COLOR:-} ]]; then
  readonly red='' heading='' reset=''
else
  readonly red=$'\033[31m' heading=$'\033[1;4m' reset=$'\033[0m'
fi

print_in_red() {
  printf '%s%s%s\n' "$red" "$*" "$reset"
}

fail_to_check() {
  print_in_red "error[$1]: $2" >&2
  exit "$exit_could_not_check"
}

database_path() {
  sed -n "s/^  database = '\(.*\)'$/\1/p" "$chezmoi_config_template"
}

references_as_file_and_title() {
  git -C "$dotfiles_directory" grep -o -E "$keepassxc_reference_pattern" -- '*.tmpl' '**/modify_*' 'modify_*' ':!docs' |
    sed -E 's/^([^:]+):keepassxc[A-Za-z]* +"(.*)"$/\1\t\2/'
}

titles_in() {
  local references=$1
  printf '%s\n' "$references" | cut -f2 | sort -u
}

where_each_title_is_found() {
  local entry_names=$1 titles=$2
  awk -F '\t' '
    NR == FNR {
      if ($0 ~ /\/$/) next
      title = $0
      sub(/.*\//, "", title)
      count[title]++
      next
    }
    {
      if (count[$0] == 1) next
      if (count[$0] == 0) print $0 "\t✗ missing"
      else print $0 "\t✗ " count[$0] " entries share this title"
    }
  ' <(printf '%s\n' "$entry_names") <(printf '%s\n' "$titles")
}

widest_title() {
  local titles=$1
  printf '%s\n' "$titles" | awk '{ if (length($0) > width) width = length($0) } END { print (width > 50 ? 50 : width) + 0 }'
}

print_column_headings() {
  local width=$1
  printf '  %s%-*s%s  %s%s%s\n' "$heading" "$width" 'expected' "$reset" "$heading" 'found' "$reset"
}

print_row() {
  local width=$1 expected=$2 found=$3
  printf '  %-*s  %s\n' "$width" "$expected" "$found"
}

main() {
  local database references titles entry_names results expected found width title_count issue_count=0
  database="$(database_path)"
  [[ -n $database ]] || fail_to_check no-database "no keepassxc database is set in $chezmoi_config_template"
  references="$(references_as_file_and_title)" || fail_to_check git-grep "could not read the templates"
  [[ -n $references ]] || fail_to_check no-references "found no keepassxc references in the templates"
  titles="$(titles_in "$references")"
  entry_names="$(keepassxc-cli ls --recursive --flatten "$database")" || fail_to_check keepassxc "could not list $database"
  [[ -n $entry_names ]] || fail_to_check empty-database "keepassxc-cli listed no entries in $database"
  results="$(where_each_title_is_found "$entry_names" "$titles")"
  unset entry_names
  title_count="$(printf '%s\n' "$titles" | wc -l | tr -d ' ')"
  width="$(widest_title "$titles")"

  report_section 'keepass' "$title_count entries the templates read"
  if [[ -z $results ]]; then
    report_line "  all $title_count found"
    exit "$exit_all_found"
  fi
  print_column_headings "$width"
  while IFS=$'\t' read -r expected found; do
    print_in_red "$(print_row "$width" "$expected" "$found")"
    issue_count=$((issue_count + 1))
  done < <(printf '%s\n' "$results")
  print_in_red "  $issue_count of $title_count need fixing"
  exit "$exit_entries_missing"
}

main "$@"
