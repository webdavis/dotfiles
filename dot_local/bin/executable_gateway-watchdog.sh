#!/usr/bin/env bash
# Gateway watchdog — persistent daemon
# Monitors Bob (port 18789) and Butters (port 19789)
# Notifies Discord on outage/recovery with bot-specific messages

set_globals() {
  BOB_STATE="unknown"
  BUTTERS_STATE="unknown"
  BOB_CRASH_COUNT=0
  BOB_CRASH_WINDOW_START=0
  BOB_CRASH_LOOP_NOTIFIED=false
  CHECK_INTERVAL=5
  MAX_CRASHES=3
  CRASH_WINDOW=300 # 5 minutes
}

get_config() {
  python3 -c "
import json, os
c = json.load(open(os.path.expanduser('~/.openclaw/openclaw.json')))
token = c.get('channels', {}).get('discord', {}).get('token', '')
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
  local token channel
  token=$(echo "$config" | sed -n '1p')
  channel=$(echo "$config" | sed -n '2p')
  [ -z "$token" ] || [ -z "$channel" ] && return 1
  curl -s -X POST "https://discord.com/api/v10/channels/$channel/messages" \
    -H "Authorization: Bot $token" \
    -H "Content-Type: application/json" \
    -d "{\"content\":\"$msg\"}" >/dev/null 2>&1
}

check_bob() {
  if curl -sf --max-time 3 http://127.0.0.1:18789/ >/dev/null 2>&1; then
    if [ "$BOB_STATE" = "down" ]; then
      BOB_CRASH_COUNT=$((BOB_CRASH_COUNT + 1))
      NOW=$(date +%s)
      if [ $((NOW - BOB_CRASH_WINDOW_START)) -gt "$CRASH_WINDOW" ]; then
        BOB_CRASH_COUNT=1
        BOB_CRASH_WINDOW_START=$NOW
        BOB_CRASH_LOOP_NOTIFIED=false
      fi
      if [ "$BOB_CRASH_COUNT" -ge "$MAX_CRASHES" ] && [ "$BOB_CRASH_LOOP_NOTIFIED" = false ]; then
        logger -t "openclaw-watchdog" "Bob crash loop detected ($BOB_CRASH_COUNT crashes)"
        send_discord "🔴 **Bob crash loop detected** — restarted $BOB_CRASH_COUNT times in 5 minutes. Something is wrong."
        BOB_CRASH_LOOP_NOTIFIED=true
      else
        logger -t "openclaw-watchdog" "Bob recovered (crash #$BOB_CRASH_COUNT)"
        send_discord "✅ Bob is back online."
      fi
    elif [ "$BOB_STATE" = "unknown" ]; then
      logger -t "openclaw-watchdog" "Watchdog started, Bob is up"
      BOB_CRASH_WINDOW_START=$(date +%s)
    fi
    BOB_STATE="up"
  else
    if [ "$BOB_STATE" = "up" ] || [ "$BOB_STATE" = "unknown" ]; then
      logger -t "openclaw-watchdog" "Bob went offline"
      send_discord "⚠️ Bob is going offline. Should be back in ~15 seconds."
    fi
    BOB_STATE="down"
  fi
}

check_butters() {
  if curl -sf --max-time 3 http://127.0.0.1:19789/ >/dev/null 2>&1; then
    if [ "$BUTTERS_STATE" = "down" ]; then
      logger -t "openclaw-watchdog" "Butters recovered"
      send_discord "✅ Butters is back online."
    elif [ "$BUTTERS_STATE" = "unknown" ]; then
      logger -t "openclaw-watchdog" "Watchdog started, Butters is up"
    fi
    BUTTERS_STATE="up"
  else
    if [ "$BUTTERS_STATE" = "up" ] || [ "$BUTTERS_STATE" = "unknown" ]; then
      logger -t "openclaw-watchdog" "Butters went offline"
      send_discord "⚠️ Butters is going offline."
    fi
    BUTTERS_STATE="down"
  fi
}

watch() {
  while true; do
    check_bob
    check_butters
    sleep "$CHECK_INTERVAL"
  done
}

main() {
  set_globals
  watch
}

main
