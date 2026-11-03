#!/usr/bin/env bash

set -euo pipefail
shopt -u patsub_replacement 2>/dev/null || true

# Each line of a menu file is "<key> <action> <label>"; a lone "-" starts a new group.
readonly menus_dir="$HOME/.config/herdr/menus"

usage() {
  echo "usage: herdr-action-menu.sh <plugin>" >&2
  exit 1
}

plugin=${1:-}
[[ -n $plugin ]] || usage
menu_file="$menus_dir/$plugin"
[[ -f $menu_file ]] || usage
bold=$(tput bold)
dim=$(tput dim)
italic=$(tput sitm)
no_italic=$(tput ritm)
red=$(tput setaf 1)
magenta=$(tput setaf 5)
normal=$(tput sgr0)

pane_folder() {
  [[ -n ${HERDR_ACTIVE_PANE_ID:-} ]] || return 0
  herdr pane get "$HERDR_ACTIVE_PANE_ID" | jq -r '.result.pane.foreground_cwd // .result.pane.cwd // empty'
}

current_branch() {
  local folder=$1
  git -C "$folder" branch --show-current 2>/dev/null || true
}

default_branch() {
  local folder=$1
  git -C "$folder" symbolic-ref --short refs/remotes/origin/HEAD 2>/dev/null | sed 's#^origin/##' || true
}

folder_name() {
  local folder=$1
  basename "$folder"
}

branch_label() {
  local branch=$1 fallback=$2
  if [[ -n $branch ]]; then
    echo "${italic}${branch}${no_italic}"
  else
    echo "the ${italic}${fallback}${no_italic}"
  fi
}

# Fills {pane_folder}, {current_branch} and {default_branch} in the labels from the pane under the popup.
load_menu() {
  local folder current default
  folder=$(pane_folder)
  folder=${folder:-$PWD}
  current=$(current_branch "$folder")
  default=$(default_branch "$folder")
  [[ $current == "$default" ]] && current=""
  local folder_label current_label default_label entry
  folder_label="${italic}$(folder_name "$folder")${no_italic}"
  current_label=$(branch_label "$current" "current branch")
  default_label=$(branch_label "$default" "default branch")
  mapfile -t menu_entries <"$menu_file"
  for entry in "${!menu_entries[@]}"; do
    menu_entries[entry]=${menu_entries[entry]//"{pane_folder}"/$folder_label}
    menu_entries[entry]=${menu_entries[entry]//"{current_branch}"/$current_label}
    menu_entries[entry]=${menu_entries[entry]//"{default_branch}"/$default_label}
  done
}

load_menu

action_column_width() {
  local entry key action label width=0
  for entry in "${menu_entries[@]}"; do
    [[ $entry == "-" ]] && continue
    read -r key action label <<<"$entry"
    ((${#action} > width)) && width=${#action}
  done
  echo "$width"
}

print_menu() {
  local entry key action label width
  width=$(action_column_width)
  echo "${bold}${plugin}${normal}"
  echo
  for entry in "${menu_entries[@]}"; do
    if [[ $entry == "-" ]]; then
      echo
      continue
    fi
    read -r key action label <<<"$entry"
    printf '  %s%s%s  %-*s %s%s%s\n' "$magenta" "$key" "$normal" "$width" "$action" "$dim" "$label" "$normal"
  done
  echo
  echo "  ${dim}esc or ctrl+c closes this menu${normal}"
}

action_for_key() {
  local pressed=$1 entry key action label
  for entry in "${menu_entries[@]}"; do
    [[ $entry == "-" ]] && continue
    read -r key action label <<<"$entry"
    [[ $key == "$pressed" ]] && echo "$action" && return 0
  done
  return 1
}

hide_cursor() {
  tput civis
  trap 'tput cnorm' EXIT
}

error_message() {
  local output=$1
  jq -r '.error.message // empty' <<<"$output" 2>/dev/null || true
}

show_error() {
  local output=$1 message width
  message=$(error_message "$output")
  width=$(($(tput cols) - 4))
  echo
  fold -s -w "$width" <<<"${message:-$output}" | sed "s/^/  ${red}/; s/\$/${normal}/"
}

run_action() {
  local action=$1 output
  output=$(herdr plugin action invoke "${plugin}.${action}" 2>&1) && return 0
  show_error "$output"
  return 1
}

main() {
  local pressed action
  hide_cursor
  print_menu
  while read -rsn1 pressed; do
    [[ $pressed == $'\e' || $pressed == $'\x03' ]] && return 0
    action=$(action_for_key "$pressed") || continue
    run_action "$action" && return 0
  done
}

main "$@"
