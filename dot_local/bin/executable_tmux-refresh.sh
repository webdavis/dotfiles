#!/usr/bin/env bash

# ┏ Requirements (tools) ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
# ┃                                                    ┃
# ┃  ∙ https://github.com/tmux/tmux                    ┃
# ┃  ∙ https://github.com/tmuxinator/tmuxinator        ┃
# ┃  ∙ https://github.com/tmux-plugins/tmux-resurrect  ┃
# ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛

# Exit immediately if a command fails.
set -e

declare_globals() {
  # Settings:
  declare -g TMUXINATOR_PRESETS_DIR TMUX_RESURRECT_DIR
  TMUXINATOR_PRESETS_DIR="${HOME}/.config/tmuxinator"
  TMUX_RESURRECT_DIR="${HOME}/.tmux/resurrect"
}

block_if_in_tmux() {
  # Protect against running this script from inside Tmux.
  if [[ -n $TMUX ]]; then
    print_process "critical" "ERROR: You are running this script inside tmux — cannot safely kill the tmux server you're attached to."
    return 1
  fi
}

trap_error() {
  local lineno=${1:-?}    # Line number where the error occurred
  local cmd=${2:-?}       # Command that triggered the error
  local exit_code=${3:-1} # Exit code of the command

  print_process "critical" "Error: Command '${cmd}' failed with exit code ${exit_code} at line ${lineno}" true true
  exit "$exit_code"
}

trap_interrupt() {
  print_process "warning" "Interrupt received (Ctrl-C). Exiting gracefully..." true true
  exit 130 # 130 is the standard exit code for script terminated by Ctrl-C
}

trap_init() {
  trap 'trap_error ${LINENO} "$BASH_COMMAND" $?' ERR
  trap 'trap_interrupt' SIGINT
}

print_process_helper() {
  local message="$1"
  local newline="${2:-true}"
  local color="${3:-}"
  local stderr="${4:-false}"
  local reset="\033[0m"

  # Select the output stream
  local fd=1 # default stdout
  $stderr && fd=2

  message="${color}${message}${reset}"

  if $newline; then
    printf "%b\n" "$message" >&$fd
  else
    printf "%b" "$message" >&$fd
  fi
}

print_process() {
  local type="${1:-}"
  local message="${2:-}"
  local newline="${3:-true}"
  local stderr="${4:-false}"

  local cyan="\033[0;36m"
  local green="\033[0;32m"
  local yellow="\033[0;33m"
  local red="\033[0;31m"

  case "$type" in
    success) print_process_helper "$message" "$newline" "$green" ;;
    warning) print_process_helper "$message" "$newline" "$yellow" ;;
    critical) print_process_helper "$message" "$newline" "$red" "$stderr" ;;
    error)
      print_process_helper "Error: $message" "$newline" "$red" "$stderr"
      exit 1
      ;;
    info) print_process_helper "$message" "$newline" "$cyan" ;;
    *) print_process_helper "$message" "$newline" ;;
  esac
}

print_process_start_indicator() {
  local message="$1"
  local newline="${2:-true}"
  local red="\033[0;31m"
  print_process_helper "$message" "$newline" "$red"
}

print_process_finished_indicator() {
  local message="$1"
  local newline="${2:-true}"
  local green="\033[0;32m"
  print_process_helper "$message" "$newline" "$green"
}

print_process_info_indicator() {
  local message="$1"
  local newline="${2:-true}"
  local cyan="\033[0;36m"
  print_process_helper "$message" "$newline" "$cyan"
}

help_message() {
  local bold normal
  bold=$(tput bold)
  normal=$(tput sgr0)

  local script_name
  script_name="${BASH_SOURCE[0]##*/}"

  printf '%s\n' "\

${bold}DESCRIPTION${normal}
   This script can kill all of existing Tmux sessions, purge their tmux-resurrect data, and then recreate them using tmuxinator presets, all in one command.

   All session presets can be found in ${TMUXINATOR_PRESETS_DIR}

${bold}USAGE${normal}
   ${script_name} [-skpth] [any combination]

${bold}OPTIONS${normal}
   -k   Kill all existing Tmux sessions before starting new ones.
   -p   Purge all tmux-resurrect data before starting sessions.
   -t   Start all Tmux sessions via Tmuxinator using presets located in ${TMUXINATOR_PRESETS_DIR} (default behavior)
   -h   Prints this help text.

${bold}DEFAULT${normal}
   If no flags are provided, the script will perform all actions:
     1. ${bold}kill${normal}
     2. ${bold}purge${normal}
     3. ${bold}start${normal}

${bold}EXAMPLES${normal}
   ${script_name}            (Default) Kill, purge, and start all sessions
   ${script_name} -t         Start all tmux sessions
   ${script_name} -k         Kill all tmux sessions
   ${script_name} -p         Purge all tmux-resurrect data
   ${script_name} -k -p      Kill sessions and purge tmux-resurrect data
   ${script_name} -k -t      Kill existing sessions, then start new ones (preserves tmux-resurrect data)
   ${script_name} -p -t      Purge tmux-resurrect data, then start sessions
   ${script_name} -k -p -t   Kill, purge, and start all sessions"
}

kill_sessions_flag=false
purge_tmux_resurrect_data_flag=false
launch_all_tmux_sessions_flag=false

optstring=':kpth'
while getopts "$optstring" option; do
  case "$option" in
    k)
      kill_sessions_flag=true
      ;;
    p)
      purge_tmux_resurrect_data_flag=true
      ;;
    t)
      launch_all_tmux_sessions_flag=true
      ;;
    h)
      help_message
      exit 0
      ;;
    *)
      print_process "error" "invalid option '${OPTARG}'" true true
      ;;
  esac
done
unset -v option

verify_tool() {
  local tool="$1"

  if [[ ! -x "$(builtin command -v "$tool")" ]]; then
    print_process "warning" "\
Oops! Looks like you don't have ${tool} installed.

Please install it and then try again (e.g. brew install ${tool})" "true"
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

if_no_flags_activate_all() {
  # Default behavior: If no flags are provided, do everything by default.

  if ! $kill_sessions_flag && ! $purge_tmux_resurrect_data_flag && ! $launch_all_tmux_sessions_flag; then
    kill_sessions_flag=true
    purge_tmux_resurrect_data_flag=true
    launch_all_tmux_sessions_flag=true
  fi
}

kill_tmux_sessions() {
  # Description: Kills all existing Tmux sessions.

  block_if_in_tmux || return 1

  if tmux info &>/dev/null; then
    print_process "critical" "Kill tmux server..." false

    tmux kill-server 2>/dev/null || true

    rm -rf /tmp/tmux-"$(id -u)"/*

    print_process "success" " Done."
  else
    print_process "warning" "No tmux server running."
  fi
}

wait_for_resurrect_session_and_data_removal() {
  # Enable nullglob so the glob expands to empty if there are no files.
  shopt -s nullglob
  local files
  while true; do
    files=("${TMUX_RESURRECT_DIR}"/*)
    [[ ${#files[@]} -eq 0 ]] && break
    sleep 0.1
  done
  shopt -u nullglob
}

purge_tmux_resurrect_data() {
  # Description: Deletes Tmux session data tracked by tmux_resurrect.
  # Ref: https://github.com/tmux-plugins/tmux-resurrect
  print_process "critical" "Purging all tmux-resurrect data..." false
  rm -rf "${TMUX_RESURRECT_DIR:?}"/*
  wait_for_resurrect_session_and_data_removal
  print_process "success" " Done."
}

launch_tmux_session() {
  # Description: Spawns a Tmux session preset using Tmuxinator.
  # Ref: https://github.com/tmuxinator/tmuxinator

  local project="$1"
  local file="$2"

  print_process "info" "Starting ${project}..." false

  export HOME="$HOME"
  export XDG_DATA_HOME="$HOME/.local/share"
  export XDG_CONFIG_HOME="$HOME/.config"
  TMUX_ENV_CMD="env HOME=$HOME XDG_DATA_HOME=$HOME/.local/share XDG_CONFIG_HOME=$HOME/.config"

  $purge_tmux_resurrect_data_flag && wait_for_resurrect_session_and_data_removal

  if $TMUX_ENV_CMD tmuxinator start "$project" --config "$file" --no-attach; then
    print_process_finished_indicator " Done."
  fi
}

get_tmuxinator_projects() {
  local files=("$TMUXINATOR_PRESETS_DIR"/*)
  local project file
  local found=false

  for file in "${files[@]}"; do
    [[ -f $file ]] || continue
    found=true

    # Strip path.
    project="${file##*/}"

    # Strips file extension.
    project="${project%%.*}"

    printf "%s:%s\n" "$project" "$file"
  done

  if ! $found; then
    print_process "error" "Could not find any tmuxinator config files in ${TMUXINATOR_PRESETS_DIR}" true true
  fi
}

launch_all_tmux_sessions() {
  # Description: Iterates over all Tmuxinator preset files and launches each session.

  local entry entries project file

  mapfile -t entries < <(get_tmuxinator_projects)

  for entry in "${entries[@]}"; do
    project="${entry%%:*}"
    file="${entry##*:}"
    launch_tmux_session "$project" "$file"
  done
}

perform_actions() {
  $kill_sessions_flag && kill_tmux_sessions
  $purge_tmux_resurrect_data_flag && purge_tmux_resurrect_data
  $launch_all_tmux_sessions_flag && launch_all_tmux_sessions
}

main() {
  trap_init
  declare_globals
  verify_required_tools "tmux" "tmuxinator"
  if_no_flags_activate_all
  perform_actions
}

main "$@"
