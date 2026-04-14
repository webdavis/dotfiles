#!/usr/bin/env bash

# claude-restart.sh
#
# Description:
#   Long-running supervisor for a persistent Claude Code session inside tmux.
#   Called by the com.claude.code LaunchAgent.
#
#   The LaunchAgent invokes this under `/opt/homebrew/bin/bash -l` so
#   the tmux server we start inherits the user's full shell environment
#   (PATH, ~/.local/bin, JAVA_HOME, HOMEBREW_*, etc.).
#
#   This script stays in the foreground and periodically checks that
#   the `claude` tmux session is alive. If it disappears (crash, manual
#   kill, etc.), the script recreates it. launchd's KeepAlive supervises
#   this script; this script supervises the tmux session.
#
#   On SIGTERM (sent by launchd on stop/restart), the script exits
#   cleanly without killing the tmux session.

set -euo pipefail

TMUX_BIN="/opt/homebrew/bin/tmux"
CLAUDE_BIN="/opt/homebrew/bin/claude"
CAFFEINATE_BIN="/usr/bin/caffeinate"

SESSION_NAME="claude"
WORK_DIR="$HOME"
CHECK_INTERVAL=30

# --- Signal handling ---
RUNNING=true
trap 'RUNNING=false' SIGTERM SIGINT SIGHUP

log() {
  echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*"
}

create_session() {
  log "Creating tmux session '$SESSION_NAME'"
  $TMUX_BIN new-session -d -s "$SESSION_NAME" -c "$WORK_DIR"

  # Launch Claude Code via send-keys (session survives Claude crashes).
  $TMUX_BIN send-keys -t "$SESSION_NAME" \
    "unset CLAUDECODE && $CAFFEINATE_BIN -s $CLAUDE_BIN --remote-control --name '$(hostname)'" Enter

  # Wait for the trust prompt to appear, then accept it.
  sleep 5
  $TMUX_BIN send-keys -t "$SESSION_NAME" Enter

  log "Session '$SESSION_NAME' created and Claude started"
}

# --- Initial setup ---
if ! $TMUX_BIN has-session -t "$SESSION_NAME" 2>/dev/null; then
  create_session
else
  log "Session '$SESSION_NAME' already exists, entering supervision loop"
fi

# --- Supervision loop ---
log "Supervisor started (PID $$), checking every ${CHECK_INTERVAL}s"

while $RUNNING; do
  sleep "$CHECK_INTERVAL" &
  wait $! 2>/dev/null || true # interrupted sleep returns non-zero

  if ! $RUNNING; then
    break
  fi

  if ! $TMUX_BIN has-session -t "$SESSION_NAME" 2>/dev/null; then
    log "Session '$SESSION_NAME' disappeared, recreating..."
    create_session
  fi
done

log "Supervisor shutting down (received signal)"
exit 0
