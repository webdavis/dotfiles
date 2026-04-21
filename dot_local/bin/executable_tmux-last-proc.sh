#!/usr/bin/env bash
# tmux2k custom plugin: prints "<prev_session>:<window_name> <emoji>"
# for the right-side plugin slot named `last-proc` (see dot_tmux.conf §3.1).
#
# Reads @prev-session set by the client-session-changed hook in tmux.conf.
# Silent no-op if @prev-session is unset or the session is gone.
#
# Installation (one-time per machine):
#   1. Symlink this file into tmux2k's plugins directory with the exact
#      plugin name. tmux2k requires the filename to match the name in
#      @tmux2k-right-plugins:
#
#        ln -sf ~/.local/bin/tmux-last-proc.sh \
#          ~/.tmux/plugins/tmux2k/plugins/last-proc.sh
#
#   2. dot_tmux.conf already sets:
#        set-option -g @tmux2k-right-plugins "last-proc network ram"
#        set-option -g @tmux2k-last-proc-colors "cyan black"
#
# Colors work without editing tmux2k's main.sh because get_plugin_colors
# falls back to the user-set @tmux2k-<name>-colors tmux option.
#
# Why a symlink rather than a copy: tmux2k's install dir is managed by tpm
# (not chezmoi), but the symlink target stays under chezmoi's control so
# updates to this script automatically take effect after a tmux reload.

main() {
  prev=$(tmux show-option -gv @prev-session 2>/dev/null)
  [[ -z $prev ]] && exit 0
  tmux has-session -t "$prev" 2>/dev/null || exit 0

  win_idx=$(tmux display-message -p -t "$prev:" '#{window_index}' 2>/dev/null)
  win_name=$(tmux display-message -p -t "$prev:" '#{window_name}' 2>/dev/null)
  emoji=$("$HOME/.local/bin/tmux-window-emoji.sh" "$prev:$win_idx")

  printf '%s:%s %s' "$prev" "$win_name" "$emoji"
}

main
