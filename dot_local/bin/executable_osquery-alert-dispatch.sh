#!/usr/bin/env bash
#
# osquery-alert-dispatch.sh — sourced helper, not run directly. Provides
# send_alert(), which dispatches one finding to BOTH the local macOS notifier
# (alerter) and the hermes "#osquery" Discord webhook. Signing and dual-channel
# delivery live here so osquery-results-alerter.sh and osquery-firewall-gatekeeper-monitor.sh
# share one implementation.
#
# Usage (from a sourcing script):
#   source "$HOME/.local/bin/osquery-alert-dispatch.sh"
#   send_alert "Firewall disabled" "alf global_state 1 -> 0" Sosumi

OSQUERY_HERMES_URL="${OSQUERY_HERMES_URL:-http://127.0.0.1:8644/webhooks/osquery}"
# The notifier signs with its OWN copy of the HMAC key, read from its own secret
# file — NOT from hermes's .env. HMAC is symmetric so the value must match the
# gateway's, but the signer must not reach into the verifier's credential store;
# each side owns its own copy. Single-value file, mode 600, runtime (not tracked).
OSQUERY_WEBHOOK_SECRET_FILE="${OSQUERY_WEBHOOK_SECRET_FILE:-$HOME/.config/osquery/webhook-secret}"
OSQUERY_DELIVERY_LOG="${OSQUERY_DELIVERY_LOG:-$HOME/.local/log/osquery/webhook-delivery.log}"

# Append a timestamped line to the delivery log (best-effort; never fails caller).
_osquery_log() {
  mkdir -p "$(dirname "$OSQUERY_DELIVERY_LOG")" 2>/dev/null || true
  printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$1" >>"$OSQUERY_DELIVERY_LOG" 2>/dev/null || true
}

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

  # 2) Discord via the hermes webhook (best-effort, bounded retry). Read the HMAC
  #    key from the notifier's own secret file (env override allowed for tests);
  #    strip CR so a CRLF file can't corrupt the key. A missing/empty secret is
  #    handled gracefully (local alert already fired) rather than aborting.
  local secret="${OSQUERY_WEBHOOK_SECRET:-}"
  if [ -z "$secret" ] && [ -r "$OSQUERY_WEBHOOK_SECRET_FILE" ]; then
    IFS= read -r secret <"$OSQUERY_WEBHOOK_SECRET_FILE" || true
    secret=$(printf '%s' "$secret" | tr -d '\r')
  fi
  if [ -z "$secret" ]; then
    _osquery_log "WARN no webhook secret in $OSQUERY_WEBHOOK_SECRET_FILE — Discord delivery skipped"
    return 0
  fi

  local body sig reqid http attempt
  body=$(jq -cn --arg t "$title" --arg d "$detail" \
    '{event_type:"osquery.alert", alert:{title:$t, detail:$d}}')
  sig=$(printf '%s' "$body" | openssl dgst -sha256 -hmac "$secret" | awk '{print $NF}')
  # Content-stable request id: a retry or double-fire of the SAME alert dedups
  # at the gateway (it honours X-Request-ID for 1h) instead of double-posting.
  reqid="osquery-$(printf '%s' "$body" | openssl dgst -sha256 | awk '{print $NF}' | cut -c1-32)"

  for attempt in 1 2 3; do
    http=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 \
      -X POST "$OSQUERY_HERMES_URL" \
      -H 'Content-Type: application/json' \
      -H "X-Webhook-Signature: $sig" \
      -H "X-Request-ID: $reqid" \
      --data "$body") || http=000
    case "$http" in
      2*) return 0 ;;  # delivered
      429 | 5?? | 000) # transient → back off and retry
        if [ "$attempt" -lt 3 ]; then sleep "$attempt"; fi ;;
      *) break ;; # 401/413/etc — retry won't help
    esac
  done
  _osquery_log "ERROR webhook delivery failed: http=$http url=$OSQUERY_HERMES_URL"
  return 0
}
