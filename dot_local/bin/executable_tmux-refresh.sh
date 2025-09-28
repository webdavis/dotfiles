#!/usr/bin/env bash

# ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
# ┃    Required Tools                                  ┃
# ┃     ̅ ̅ ̅ ̅ ̅ ̅ ̅ ̅ ̅ ̅ ̅ ̅ ̅ ̅                                  ┃
# ┃  ∙ https://github.com/tmux/tmux                    ┃
# ┃  ∙ https://github.com/tmuxinator/tmuxinator        ┃
# ┃  ∙ https://github.com/tmux-plugins/tmux-resurrect  ┃
# ┃                                                    ┃
# ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛

# Exit immediately if a command fails.
set -e

# Settings:
TMUXINATOR_PRESETS_DIR="${HOME}/.config/tmuxinator"
TMUX_RESURRECT_DIR="${HOME}/.tmux/resurrect"
TMUX_RESURRECT_LAST_FILE="${TMUX_RESURRECT_DIR}/last"
TMUX_RESURRECT_DATA_FILES="${TMUX_RESURRECT_DIR}/tmux-resurrect*"

# Print colors:
export GREEN="\033[0;32m"
export CYAN="\033[0;36m"
export RED="\033[0;31m"
export RESET="\033[0m"

help_message() {
  local bold normal
  bold=$(tput bold)
  normal=$(tput sgr0)

  local script_name
  script_name="${BASH_SOURCE[0]##*/}"

  printf "%s\\n" "\

${bold}DESCRIPTION${normal}
   This script can kill all of existing Tmux sessions, purge their tmux-resurrect data, and then recreate them using tmuxinator presets, all in one command.

   All session presets can be found in ${TMUXINATOR_PRESETS_DIR}

${bold}USAGE${normal}
   ${script_name} [-hkpt] [any combination]

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
    echo "Error: invalid option '$OPTARG'"
    exit 1
    ;;
  esac
done
unset -v option

verify_tool() {
  local tool="$1"

  if [[ ! -x "$(builtin command -v "$tool")" ]]; then
    printf "%s\n\n" "\
Oops! Looks like you don't have ${tool} installed.

Please install it and then try again (e.g. brew install ${tool})" 2>&1

    exit 1
  fi
}

verify_required_tools() {
  local required_tools=(tmux tmuxinator)
  local tool

  for tool in "${required_tools[@]}"; do
    verify_tool "$tool"
  done
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

  echo -e "${RED}Killing all existing tmux sessions...${RESET}"

  local session

  while IFS= read -r session; do
    tmux kill-session -t "$session"
  done < <(tmux list-sessions -F "#{session_name}" 2>/dev/null)
}

purge_tmux_resurrect_data() {
  # Description: Deletes Tmux session data tracked by tmux_resurrect.
  # Ref: https://github.com/tmux-plugins/tmux-resurrect

  echo -e "${RED}Purging all tmux-resurrect data...${RESET}"
  rm -f "$TMUX_RESURRECT_LAST_FILE"
  rm -f "$TMUX_RESURRECT_DATA_FILES"
}

launch_tmux_session() {
  # Description: Spawns a Tmux session preset using Tmuxinator.
  # Ref: https://github.com/tmuxinator/tmuxinator

  local project="$1"
  local file="$2"

  echo -en "\n${CYAN}Starting ${project}...${RESET}"
  # echo -en "Starting ${project}..."

  if tmuxinator start "$project" --config "$file" --no-attach; then
    echo -e "${GREEN} Done.${RESET}"
  fi
}

get_tmuxinator_projects() {
  local files=("$TMUXINATOR_PRESETS_DIR"/*)

  local project file

  for file in "${files[@]}"; do
    # Strip path.
    project="${file##*/}"

    # Strips file extension.
    project="${project%%.*}"

    echo "$project:$file"
  done
}

launch_all_tmux_sessions() {
  # Description: Iterates over all Tmuxinator preset files and launches each session.

  # Export the function for parallel.
  export -f launch_tmux_session
  get_tmuxinator_projects | parallel --colsep ':' --group launch_tmux_session '{1}' '{2}'
}

perform_actions() {
  $kill_sessions_flag && kill_tmux_sessions
  $purge_tmux_resurrect_data_flag && purge_tmux_resurrect_data
  $launch_all_tmux_sessions_flag && launch_all_tmux_sessions
}

main() {
  verify_required_tools
  if_no_flags_activate_all
  perform_actions
}

main "$@"
