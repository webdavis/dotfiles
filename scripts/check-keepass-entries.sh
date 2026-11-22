#!/usr/bin/env bash

set -euo pipefail

dotfiles_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# shellcheck source=.chezmoitemplates/cli-print-style-lib.sh.tmpl
source "$dotfiles_directory/.chezmoitemplates/cli-print-style-lib.sh.tmpl"

readonly chezmoi_config_template="$dotfiles_directory/.chezmoi.toml.tmpl"
readonly secrets_list="$dotfiles_directory/.chezmoidata/secrets.yaml"
readonly exit_all_found=0
readonly exit_entries_missing=1
readonly exit_could_not_check=2

if [[ ${REPORT_LIB_PLAIN:-} == 1 || -n ${NO_COLOR:-} ]]; then
  readonly red='' green='' yellow='' faint='' italic='' reset=''
else
  readonly red=$'\033[38;5;203m' green=$'\033[38;5;42m' yellow=$'\033[38;5;221m' faint=$'\033[38;5;244m' italic=$'\033[3m' reset=$'\033[0m'
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

titles_in_the_secrets_list() {
  yq -r '.secrets[] | (select(kind == "map") | .title) // .' "$secrets_list" | sort -u
}

parts_in_the_secrets_list_as_title_kind_and_name() {
  local part_kind
  for part_kind in attributes attachments; do
    part_kind=$part_kind yq -r '.secrets[] | select(kind == "map" and has(strenv(part_kind))) | .[strenv(part_kind)][] | [(parent | parent | .title), strenv(part_kind), .] | @tsv' "$secrets_list" || return 1
  done
}

ask_for_the_database_password() {
  local database=$1
  read -rs -p "Enter password to unlock $database: " database_password </dev/tty
  printf '\n' >&2
}

run_keepassxc_cli_unlocked() {
  printf '%s\n' "$database_password" | keepassxc-cli "$@"
}

where_each_title_is_found() {
  local entry_names=$1 titles=$2
  awk -F '\t' '
    NR == FNR {
      if ($0 ~ /\/$/ || $0 ~ /^Recycle Bin\//) next
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

path_of_the_entry_titled() {
  local entry_names=$1 title=$2
  printf '%s\n' "$entry_names" | awk -v title="$title" '
    /^Recycle Bin\// { next }
    { name = $0; sub(/.*\//, "", name); if (name == title) { print; exit } }
  '
}

entry_holds_the_part() {
  local database=$1 path=$2 kind=$3 name=$4
  if [[ $kind == attributes ]]; then
    run_keepassxc_cli_unlocked show --quiet --attributes "$name" "$database" "$path" >/dev/null 2>&1
  else
    run_keepassxc_cli_unlocked attachment-export --quiet --stdout "$database" "$path" "$name" >/dev/null 2>&1
  fi
}

where_each_part_is_missing() {
  local database=$1 entry_names=$2 parts=$3 title kind name path
  while IFS=$'\t' read -r title kind name; do
    [[ -n $title ]] || continue
    path="$(path_of_the_entry_titled "$entry_names" "$title")"
    [[ -n $path ]] || continue
    entry_holds_the_part "$database" "$path" "$kind" "$name" || printf '%s\t✗ no %s named %s\n' "$title" "${kind%s}" "$name"
  done <<<"$parts"
}

widest_title() {
  local titles=$1
  printf '%s\n' "$titles" | awk '{ if (length($0) > width) width = length($0) } END { print (width > 50 ? 50 : width) + 0 }'
}

readonly bullet='•'
readonly found_column_width=28

rule_of_width() {
  printf '%*s' "$1" '' | sed 's/ /─/g'
}

print_column_headings() {
  local width=$1
  printf '  %s%-*s  %s%s\n' "$faint" "$width" 'expected' 'found' "$reset"
  printf '  %s%s  %s%s\n' "$faint" "$(rule_of_width "$width")" "$(rule_of_width "$found_column_width")" "$reset"
}

print_row() {
  local width=$1 expected=$2 found=$3
  printf '%s %-*s  %s\n' "$bullet" "$width" "$expected" "$found"
}

main() {
  local database titles parts entry_names results missing_parts expected found width title_count issue_count=0
  database="$(database_path)"
  [[ -n $database ]] || fail_to_check no-database "no keepassxc database is set in $chezmoi_config_template"
  titles="$(titles_in_the_secrets_list)" || fail_to_check secrets-list "could not read $secrets_list"
  [[ -n $titles ]] || fail_to_check no-secrets "$secrets_list names no entries"
  parts="$(parts_in_the_secrets_list_as_title_kind_and_name)" || fail_to_check secrets-list "could not read $secrets_list"
  ask_for_the_database_password "$database"
  entry_names="$(run_keepassxc_cli_unlocked ls --quiet --recursive --flatten "$database")" || fail_to_check keepassxc "could not list $database"
  [[ -n $entry_names ]] || fail_to_check empty-database "keepassxc-cli listed no entries in $database"
  results="$(where_each_title_is_found "$entry_names" "$titles")"
  missing_parts="$(where_each_part_is_missing "$database" "$entry_names" "$parts")"
  unset entry_names database_password
  if [[ -n $missing_parts ]]; then
    results="${results:+$results$'\n'}$missing_parts"
  fi
  title_count="$(printf '%s\n' "$titles" | wc -l | tr -d ' ')"
  width="$(widest_title "$titles")"

  report_section 'KeePass entry check' "$title_count entries the secrets list names"
  if [[ -z $results ]]; then
    printf '  %s%s%s\n' "$green" "success: all $title_count found" "$reset"
    exit "$exit_all_found"
  fi
  printf '\n'
  print_column_headings "$width"
  while IFS=$'\t' read -r expected found; do
    print_in_red "$(print_row "$width" "$expected" "$found")"
    issue_count=$((issue_count + 1))
  done < <(printf '%s\n' "$results")
  printf '\n'
  printf '  %s%s%s%s\n' "$yellow" "$italic" "oops! $issue_count of $title_count needs fixing!" "$reset"
  exit "$exit_entries_missing"
}

main "$@"
