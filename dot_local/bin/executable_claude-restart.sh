#!/usr/bin/env bash

# claude-restart.sh
#
# Description:
#   Starts (or restarts) a persistent Claude Code session inside tmux.
#   Called by the LaunchAgent on login, or manually after a reboot.

TMUX_BIN="/opt/homebrew/bin/tmux"
CLAUDE_BIN="/opt/homebrew/bin/claude"
CAFFEINATE_BIN="/usr/bin/caffeinate"

# Configuration.
SESSION_NAME="claude"
WORK_DIR="$HOME"

# Kill any existing session.
$TMUX_BIN kill-session -t "$SESSION_NAME" 2>/dev/null
sleep 1

# Create tmux session with a plain shell (survives if claude exits).
$TMUX_BIN new-session -d -s "$SESSION_NAME" -c "$WORK_DIR"

# Launch Claude Code via send-keys (session survives crashes).
$TMUX_BIN send-keys -t "$SESSION_NAME" \
  "unset CLAUDECODE && $CAFFEINATE_BIN -s $CLAUDE_BIN --remote-control --name '$(hostname)'" Enter

# Wait for the trust prompt to appear, then accept it.
sleep 5
$TMUX_BIN send-keys -t "$SESSION_NAME" Enter
