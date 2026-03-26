#!/bin/bash
# Gateway watchdog — persistent daemon
# Checks gateway health, notifies on outage/recovery, detects crash loops

# Read config dynamically each cycle to pick up token rotations
get_config() {
  python3 -c "
import json, os
c = json.load(open(os.path.expanduser('~/.openclaw/openclaw.json')))
token = c.get('channels', {}).get('discord', {}).get('token', '')
# Read channel ID from watchdog.conf (set during setup)
conf_path = os.path.expanduser('~/.local/share/openclaw-watchdog/watchdog.conf')
channel = ''
if os.path.exists(conf_path):
    for line in open(conf_path):
        if line.startswith('DISCORD_CHANNEL_ID='):
            channel = line.strip().split('=', 1)[1]
print(token)
print(channel)
" 2>/dev/null
}

send_discord() {
  local msg="$1"
  local config
  config=$(get_config)
  local token=$(echo "$config" | sed -n '1p')
  local channel=$(echo "$config" | sed -n '2p')
  [ -z "$token" ] || [ -z "$channel" ] && return 1
  curl -s -X POST "https://discord.com/api/v10/channels/$channel/messages" \
    -H "Authorization: Bot $token" \
    -H "Content-Type: application/json" \
    -d "{\"content\":\"$msg\"}" >/dev/null 2>&1
}

STATE="unknown"
CRASH_COUNT=0
CRASH_WINDOW_START=0
CRASH_LOOP_NOTIFIED=false
CHECK_INTERVAL=5
MAX_CRASHES=3
CRASH_WINDOW=300 # 5 minutes

while true; do
  if curl -sf --max-time 3 http://127.0.0.1:18789/ >/dev/null 2>&1; then
    # Gateway is up
    if [ "$STATE" = "down" ]; then
      CRASH_COUNT=$((CRASH_COUNT + 1))
      NOW=$(date +%s)

      # Reset crash window if it's been long enough
      if [ $((NOW - CRASH_WINDOW_START)) -gt $CRASH_WINDOW ]; then
        CRASH_COUNT=1
        CRASH_WINDOW_START=$NOW
        CRASH_LOOP_NOTIFIED=false
      fi

      if [ "$CRASH_COUNT" -ge "$MAX_CRASHES" ] && [ "$CRASH_LOOP_NOTIFIED" = false ]; then
        logger -t "openclaw-watchdog" "Crash loop detected ($CRASH_COUNT crashes in ${CRASH_WINDOW}s)"
        send_discord "🔴 **CRASH LOOP DETECTED** — Gateway has restarted $CRASH_COUNT times in the last 5 minutes. Something is wrong. Check macOS system log: \`log show --predicate 'subsystem == "openclaw-watchdog"' --last 1h\`"
        CRASH_LOOP_NOTIFIED=true
      else
        logger -t "openclaw-watchdog" "Gateway recovered (crash #$CRASH_COUNT in window)"
        send_discord "✅ Back online."
      fi
    elif [ "$STATE" = "unknown" ]; then
      logger -t "openclaw-watchdog" "Watchdog started, gateway is up"
      CRASH_WINDOW_START=$(date +%s)
    fi
    STATE="up"
  else
    # Gateway is down
    if [ "$STATE" = "up" ] || [ "$STATE" = "unknown" ]; then
      logger -t "openclaw-watchdog" "Gateway went down"
      send_discord "⚠️ **Gateway down.** macOS is restarting it — should be back in ~15 seconds."
    fi
    STATE="down"
  fi
  sleep $CHECK_INTERVAL
done
