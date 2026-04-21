#!/usr/bin/env bash
# tmux2k custom plugin: prints "<prev_session>:<window_name> <emoji>"
# for the right-side plugin slot named `last-proc` (see dot_tmux.conf §3.1).
#
# Reads @prev-session set by the client-session-changed hook in tmux.conf.
# Silent no-op if @prev-session is unset or the session is gone.
#
# Installation: chezmoi handles it. The run_after script at
# .chezmoiscripts/run_after_70-link-tmux2k-last-proc.sh.tmpl symlinks
# ~/.local/bin/tmux-last-proc.sh → ~/.tmux/plugins/tmux2k/plugins/last-proc.sh
# on every `chezmoi apply`. Silent no-op until tpm has installed tmux2k
# (fresh-machine flow: chezmoi apply → start tmux → prefix+I installs
# tmux2k → next chezmoi apply links this plugin).
#
# dot_tmux.conf sets:
#   set-option -g @tmux2k-right-plugins "last-proc network ram"
#   set-option -g @tmux2k-last-proc-colors "cyan black"
#
# Colors work without editing tmux2k's main.sh because get_plugin_colors
# falls back to the user-set @tmux2k-<name>-colors tmux option.
#
# Why NOT place this file directly at dot_tmux/plugins/tmux2k/plugins/...
# in chezmoi source: tpm checks `[ -d $plugin_dir ]` before cloning and
# treats an existing directory as "already installed", which would prevent
# tmux2k from ever being cloned on a fresh machine.

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
