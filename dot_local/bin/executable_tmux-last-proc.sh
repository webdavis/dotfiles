#!/usr/bin/env bash
# tmux2k custom plugin body: prints "<prev_session>:<window_name> <emoji>"
# for the right-side plugin slot named `last-proc` (see dot_tmux.conf §3.1).
#
# Reads @prev-session set by the client-session-changed hook in §21.3.
# Silent no-op if @prev-session is unset or the session is gone.
#
# **Installation:** tmux2k discovers plugin scripts at
# ~/.tmux/plugins/tmux2k/scripts/<name>.sh. Since tmux2k is tpm-managed (not
# chezmoi-managed), either copy or symlink this file to
# ~/.tmux/plugins/tmux2k/scripts/last-proc.sh after `prefix + I`.

prev=$(tmux show-option -gv @prev-session 2>/dev/null)
[[ -z $prev ]] && exit 0
tmux has-session -t "$prev" 2>/dev/null || exit 0

win_idx=$(tmux display-message -p -t "$prev:" '#{window_index}' 2>/dev/null)
win_name=$(tmux display-message -p -t "$prev:" '#{window_name}' 2>/dev/null)
emoji=$("$HOME/.local/bin/tmux-window-emoji.sh" "$prev:$win_idx")

printf '%s:%s %s' "$prev" "$win_name" "$emoji"
