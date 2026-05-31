#!/usr/bin/env bash
#
# osquery-send-alert.sh — sourced helper, not run directly. Provides
# send_alert(), which dispatches one finding to BOTH the local macOS notifier
# (alerter) and the hermes "#osquery" Discord webhook. Signing and dual-channel
# delivery live here so osquery-results-notify.sh and osquery-posture-watch.sh
# share one implementation.
#
# Usage (from a sourcing script):
#   source "$HOME/.local/bin/osquery-send-alert.sh"
#   send_alert "Firewall disabled" "alf global_state 1 -> 0" Sosumi

OSQUERY_HERMES_URL="${OSQUERY_HERMES_URL:-http://127.0.0.1:8644/webhooks/osquery}"
OSQUERY_HERMES_ENV="${OSQUERY_HERMES_ENV:-$HOME/.hermes/.env}"

# send_alert <title> <detail> [sound]
# Local notification always fires; the Discord POST is best-effort and never
# fails the caller (so a down gateway can't break the local alert). An empty
# sound argument means a silent notification (used for the low INFO tier).
send_alert() {
  local title="$1" detail="$2" sound="${3-}"

  # 1) Local macOS notification (alerter, AppleScript fallback). Pass --sound
  #    only when one is given so INFO-tier alerts are visible but silent.
  if command -v alerter >/dev/null 2>&1; then
    if [ -n "$sound" ]; then
      alerter --timeout 60 --title "$title" --message "$detail" --sound "$sound" >/dev/null 2>&1 &
    else
      alerter --timeout 60 --title "$title" --message "$detail" >/dev/null 2>&1 &
    fi
  else
    local escaped=${detail//\"/\\\"}
    if [ -n "$sound" ]; then
      osascript -e "display notification \"$escaped\" with title \"$title\" sound name \"$sound\"" >/dev/null 2>&1 || true
    else
      osascript -e "display notification \"$escaped\" with title \"$title\"" >/dev/null 2>&1 || true
    fi
  fi

  # 2) Discord via the hermes webhook. The HMAC key is the inlined route secret
  #    read from hermes's .env; strip surrounding quotes so the bytes match what
  #    python-dotenv loaded into the gateway.
  local secret
  secret=$(grep -m1 '^OSQUERY_WEBHOOK_SECRET=' "$OSQUERY_HERMES_ENV" 2>/dev/null |
    cut -d= -f2- | sed -e 's/^"//' -e 's/"$//' -e "s/^'//" -e "s/'\$//")
  [ -n "$secret" ] || return 0

  local body sig
  body=$(jq -cn --arg t "$title" --arg d "$detail" \
    '{event_type:"osquery.alert", alert:{title:$t, detail:$d}}')
  sig=$(printf '%s' "$body" | openssl dgst -sha256 -hmac "$secret" | awk '{print $NF}')
  curl -s -o /dev/null --max-time 5 \
    -X POST "$OSQUERY_HERMES_URL" \
    -H 'Content-Type: application/json' \
    -H "X-Webhook-Signature: $sig" \
    -H "X-Request-ID: osquery-$(date +%s)-${RANDOM}" \
    --data "$body" || true
}
