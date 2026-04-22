#!/usr/bin/env bash
# tmux2k custom plugin: right-side status for the previously-active session.
#
# While a non-shell is running in the prev session's active pane:
#   uriel:sleep ⏳     (sleep running)
#   uriel:cargo 🔨     (cargo building)
#   uriel:claude 🤖    (claude running)
#
# After the process finishes and the pane returns to a shell, switch to
# "session:window" and keep a completion glyph until you switch sessions:
#   uriel:chezmoi ✅   (last command exited 0)
#   uriel:chezmoi ❌   (last command exited non-zero)
#
# If the shell hasn't run anything yet (no state file), still show the
# session:window so the slot never goes blank:
#   uriel:chezmoi
#
# Reads:
#   @prev-session                                — set by the client-session-changed hook
#   /tmp/tmux-last-proc-$UID/<pane_id>           — "<exit_code>" written by the
#                                                   bashrc precmd __tmux_last_proc_precmd

main() {
  prev=$(tmux show-option -gv @prev-session 2>/dev/null)
  [[ -z $prev ]] && exit 0
  tmux has-session -t "$prev" 2>/dev/null || exit 0

  IFS='|' read -r cmd pane_id win_name < <(
    tmux display-message -p -t "$prev:" '#{pane_current_command}|#{pane_id}|#{window_name}' 2>/dev/null
  )
  [[ -z $cmd ]] && exit 0

  case "$cmd" in
    bash | zsh | fish | sh | dash)
      # Shell at prompt — show session:window plus completion glyph if known.
      local state_file="/tmp/tmux-last-proc-${UID}/${pane_id}"
      local exit_code=""
      [[ -f $state_file ]] && read -r exit_code _ <"$state_file" 2>/dev/null
      if [[ -z $exit_code ]]; then
        printf '%s:%s' "$prev" "$win_name"
      elif [[ $exit_code == "0" ]]; then
        printf '%s:%s ✅' "$prev" "$win_name"
      else
        printf '%s:%s ❌' "$prev" "$win_name"
      fi
      ;;
    *)
      # Non-shell command is running — show session:process with matching emoji.
      emoji=$("$HOME/.local/bin/tmux-window-emoji.sh" "$prev:")
      printf '%s:%s %s' "$prev" "$cmd" "$emoji"
      ;;
  esac
}

main
