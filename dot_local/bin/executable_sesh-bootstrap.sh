#!/usr/bin/env bash
# Bootstrap the three default sesh sessions in parallel. Called from:
#   - dot_bashrc.tmpl tmux auto-startup
#   - dot_local/bin/executable_tmux-refresh.sh
#   - Library/LaunchAgents/com.claude.code.plist (via claude-restart.sh)
# `sesh connect` is idempotent — reconnects to an existing session or creates it.

set -euo pipefail

for session in uriel openclaw homelab; do
  sesh connect "$session" 2>/dev/null &
done
wait
