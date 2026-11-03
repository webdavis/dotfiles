#!/usr/bin/env bash

set -euo pipefail

# Each entry is "<key> <annotate action> <label>"; a lone "-" starts a new group.
readonly -a menu_entries=(
  "c capture        comment on the selected text"
  "o open           review this folder"
  "g gate           review this folder and approve"
  "t terminal       review this pane's output"
  "l last           review one of the agent's replies"
  "n last-newest    review the agent's newest reply"
  "-"
  "m manage         manage saved comments"
  "y copy-context   copy saved comments"
  "Y copy-archive   copy saved comments, then archive"
  "p paste-archive  paste saved comments into the prompt, then archive"
  "s send-archive   send saved comments to the agent, then archive"
)

bold=$(tput bold)
dim=$(tput dim)
red=$(tput setaf 1)
magenta=$(tput setaf 5)
normal=$(tput sgr0)

print_menu() {
  local entry key action label
  echo "${bold}annotate${normal}"
  echo
  for entry in "${menu_entries[@]}"; do
    if [[ $entry == "-" ]]; then
      echo
      continue
    fi
    read -r key action label <<<"$entry"
    printf '  %s%s%s  %-14s %s%s%s\n' "$magenta" "$key" "$normal" "$action" "$dim" "$label" "$normal"
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
  output=$(herdr plugin action invoke "annotate.${action}" 2>&1) && return 0
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
