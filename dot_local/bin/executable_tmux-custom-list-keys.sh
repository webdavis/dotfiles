#!/usr/bin/env bash

# Exit immediately if a command fails.
set -e

declare_globals() {
  declare -g \
    TMUX_PREFIX_KEY \
    TMUX_SESSIONIZER_MODE_PREFIX \
    TMUX_RESIZE_MODE_PREFIX

  # Tmux Mode Prefix Keys:
  TMUX_PREFIX_KEY="$(tmux show-option -gqv prefix)"
  TMUX_SESSIONIZER_MODE_PREFIX='C-o'
  TMUX_RESIZE_MODE_PREFIX='r'

  # Tmux Key-tables used by `tmux list-keys -T <table>`.
  declare -Ag TMUX_KEY_TABLES
  TMUX_KEY_TABLES=(
    [root]="0:"
    [prefix]="1:$TMUX_PREFIX_KEY"
    [TMUX_SESSIONIZER]="2:$TMUX_PREFIX_KEY $TMUX_SESSIONIZER_MODE_PREFIX"
    [RESIZE]="3:$TMUX_PREFIX_KEY $TMUX_RESIZE_MODE_PREFIX"
  )
}

help_message() {
  local bold normal
  bold=$(tput bold)
  normal=$(tput sgr0)

  local script_name
  script_name="${BASH_SOURCE[0]##*/}"

  table_list=$(
    while IFS= read -r table; do
      printf "    %s\n" "$table"
    done < <(get_sorted_key_tables)
  )

  printf "%s\\n" "\

${bold}DESCRIPTION${normal}
   This script lists all Tmux key-table bindings with their exact bindings paths.

   Key-tables included:

${table_list}

${bold}EXAMPLE OUTPUT${normal}
    === TMUX_SESSIONIZER ===
    C-d C-o d   Tmux Sessionizer Mode: Go-to Dotfiles
    C-d C-o e   Tmux Sessionizer Mode: Go-to essential-feed-case-study

    === RESIZE ===
    C-d r H Resize Mode: Resize Left (by 5)
    C-d r J Resize Mode: Resize Down (by 5)

${bold}USAGE${normal}
   ${script_name} [-h]

${bold}OPTIONS${normal}
   -h   Prints this help text.

${bold}EXAMPLES${normal}
   ${script_name}        List all key-tables and their bindings.
   ${script_name} -h     Display this help text."
}

parse_command_line_arguments() {
  local option
  local optstring=':h'
  while getopts "$optstring" option; do
    case "$option" in
      h)
        help_message
        exit 0
        ;;
      *)
        echo "Error: invalid option '$OPTARG'"
        exit 1
        ;;
    esac
  done
}

print_key_table() {
  local table="$1"
  local prefix="${TMUX_KEY_TABLES[$table]#*:}"
  [[ -n "$prefix" ]] && prefix="$prefix "
  echo -e "\n=== $table ==="
  tmux list-keys -Na -P "$prefix" -T "$table"
}

get_sorted_key_tables() {
  local table

  for table in "${!TMUX_KEY_TABLES[@]}"; do
    echo "$table:${TMUX_KEY_TABLES[$table]}"
  done | sort -t: -k2n | cut -d: -f1
}

print_all_key_tables() {
  local table

  while IFS= read -r table; do
    print_key_table "$table"
  done < <(get_sorted_key_tables)
}

verify_tool() {
  local tool="$1"

  if [[ ! -x "$(builtin command -v "$tool")" ]]; then
    printf "%s\n\n" "\
Oops! Looks like you don't have ${tool} installed.

Please install it and then try again (e.g. brew install ${tool})" 2>&1

    return 1
  fi
  return 0
}

verify_required_tools() {
  local required_tools=("$@")

  local tool
  local missing_tool=false

  for tool in "${required_tools[@]}"; do
    if ! verify_tool "$tool"; then
      missing_tool=true
    fi
  done

  if $missing_tool; then
    exit 1
  fi
}

main() {
  declare_globals
  verify_required_tools tmux
  parse_command_line_arguments "$@"
  print_all_key_tables
}

main "$@"
