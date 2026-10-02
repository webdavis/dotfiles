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
  readonly red='' reset=''
else
  readonly red=$'\033[31m' reset=$'\033[0m'
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
  git -C "$dotfiles_directory" grep -o -E "$keepassxc_reference_pattern" -- ':!docs' |
    sed -E 's/^([^:]+):keepassxc[A-Za-z]* +"(.*)"$/\1\t\2/'
}

titles_in() {
  local references=$1
  printf '%s\n' "$references" | cut -f2 | sort -u
}

files_using() {
  local references=$1 title=$2
  printf '%s\n' "$references" | awk -F '\t' -v title="$title" '$2 == title { print $1 }' | sort -u
}

where_used() {
  local references=$1 title=$2 files count
  files="$(files_using "$references" "$title")"
  count="$(printf '%s\n' "$files" | wc -l | tr -d ' ')"
  if ((count == 1)); then
    printf '%s' "$files"
  else
    printf '%s (+%d more)' "$(printf '%s\n' "$files" | head -n 1)" "$((count - 1))"
  fi
}

entry_in_a_group_named() {
  local entry_names=$1 title=$2
  printf '%s\n' "$entry_names" | awk -v suffix="/$title" 'substr($0, length($0) - length(suffix) + 1) == suffix { print; exit }'
}

widest_title() {
  local titles=$1
  printf '%s\n' "$titles" | awk '{ if (length($0) > width) width = length($0) } END { print (width > 50 ? 50 : width) + 0 }'
}

report_missing_title() {
  local references=$1 entry_names=$2 title=$3 width=$4 grouped
  grouped="$(entry_in_a_group_named "$entry_names" "$title")"
  if [[ -n $grouped ]]; then
    print_in_red "$(printf '  ✗ %-*s  found only as "%s"' "$width" "$title" "$grouped")"
  else
    print_in_red "$(printf '  ✗ %-*s  used in %s' "$width" "$title" "$(where_used "$references" "$title")")"
  fi
}

main() {
  local database references titles entry_names missing title width title_count missing_count
  database="$(database_path)"
  [[ -n $database ]] || fail_to_check no-database "no keepassxc database is set in $chezmoi_config_template"
  references="$(references_as_file_and_title)" || fail_to_check git-grep "could not read the templates"
  [[ -n $references ]] || fail_to_check no-references "found no keepassxc references in the templates"
  titles="$(titles_in "$references")"
  entry_names="$(keepassxc-cli ls --recursive --flatten "$database")" || fail_to_check keepassxc "could not list $database"
  missing="$(comm -23 <(printf '%s\n' "$titles") <(printf '%s\n' "$entry_names" | sort -u))"
  title_count="$(printf '%s\n' "$titles" | wc -l | tr -d ' ')"

  report_section 'keepass' "$title_count entries the templates read"
  if [[ -z $missing ]]; then
    report_line "  ✓ all $title_count are in the database"
    exit "$exit_all_found"
  fi
  missing_count="$(printf '%s\n' "$missing" | wc -l | tr -d ' ')"
  width="$(widest_title "$missing")"
  while IFS= read -r title; do
    report_missing_title "$references" "$entry_names" "$title" "$width"
  done < <(printf '%s\n' "$missing")
  print_in_red "  $missing_count missing, $((title_count - missing_count)) found"
  exit "$exit_entries_missing"
}

main "$@"
