#!/usr/bin/env bash
#
# osquery-alert-dispatch.sh — sourced helper, not run directly. Provides
# send_alert(), which dispatches one finding to BOTH the local macOS notifier
# (alerter) and a hermes Discord webhook, routing by severity (CRIT -> #priority,
# else -> #osquery). Signing and dual-channel delivery live here so the three
# producers — osquery-results-alerter.sh, osquery-firewall-gatekeeper-monitor.sh,
# and osquery-uptime-watchdog.sh — share one implementation.
#
# Usage (from a sourcing script):
#   source "$HOME/.local/bin/osquery-alert-dispatch.sh"
#   send_alert CRIT "Firewall disabled" "alf global_state 1 -> 0" Sosumi

# One Discord route: the #priority channel (the one channel the user watches),
# signed with the osquery HMAC key below. v2 has NO #osquery channel — only a
# confirmed CRIT page is POSTed; any other severity does the local notification
# only. There is deliberately no non-priority URL for a producer to leak to.
OSQUERY_HERMES_PRIORITY_URL="${OSQUERY_HERMES_PRIORITY_URL:-http://127.0.0.1:8644/webhooks/osquery-priority}"
# The notifier signs with its OWN copy of the HMAC key, read from its own secret
# file — NOT from hermes's .env. HMAC is symmetric so the value must match the
# gateway's, but the signer must not reach into the verifier's credential store;
# each side owns its own copy. Single-value file, mode 600, runtime (not tracked).
OSQUERY_WEBHOOK_SECRET_FILE="${OSQUERY_WEBHOOK_SECRET_FILE:-$HOME/.config/osquery/webhook-secret}"
OSQUERY_DELIVERY_LOG="${OSQUERY_DELIVERY_LOG:-$HOME/.local/log/osquery/webhook-delivery.log}"
# Undelivered pages spool here — one mode-600 file per page in a mode-700 dir — so a
# transient gateway outage never loses a page (a lost page is indistinguishable from
# "all clear"). The drain (alerter startup + watchdog) replays them.
OSQUERY_SPOOL_DIR="${OSQUERY_SPOOL_DIR:-$HOME/.local/state/osquery-spool}"

# Append a timestamped line to the delivery log (best-effort; never fails caller).
# Only metadata is ever logged — never the body or the HMAC secret.
_osquery_log() {
  mkdir -p "$(dirname "$OSQUERY_DELIVERY_LOG")" 2>/dev/null || true
  printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$1" >>"$OSQUERY_DELIVERY_LOG" 2>/dev/null || true
}

# Spool one undelivered page (best-effort; never fails the caller). The file is named
# by the content-stable request_id, so re-spooling the same page is idempotent.
# Line: <unix_ts>\t<request_id>\t<url>\t<base64(body)>.
_spool_page() {
  local request_id="$1" url="$2" body="$3" spool_file
  mkdir -p "$OSQUERY_SPOOL_DIR" 2>/dev/null || return 0
  chmod 700 "$OSQUERY_SPOOL_DIR" 2>/dev/null || true
  spool_file="$OSQUERY_SPOOL_DIR/$request_id"
  printf '%s\t%s\t%s\t%s\n' \
    "$(date -u +%s)" "$request_id" "$url" "$(printf '%s' "$body" | base64 | tr -d '\n')" \
    >"$spool_file" 2>/dev/null || return 0
  chmod 600 "$spool_file" 2>/dev/null || true
}

# Replay spooled pages: re-POST each (stored request_id verbatim → idempotent at the
# gateway; signature recomputed from the stored body), remove on delivery, leave on
# failure for the next drain. Localhost only — a tampered/off-box url is skipped,
# never sent. Fully set -e-safe: a malformed entry or empty dir must NEVER abort the
# caller (a delivery feature must not cause a detection outage).
_drain_spool() {
  [ -d "$OSQUERY_SPOOL_DIR" ] || return 0
  local secret spool_file request_id url body signature http
  secret="${OSQUERY_WEBHOOK_SECRET:-}"
  if [ -z "$secret" ] && [ -r "$OSQUERY_WEBHOOK_SECRET_FILE" ]; then
    IFS= read -r secret <"$OSQUERY_WEBHOOK_SECRET_FILE" || true
    secret=$(printf '%s' "$secret" | tr -d '\r')
  fi
  [ -n "$secret" ] || return 0
  for spool_file in "$OSQUERY_SPOOL_DIR"/*; do
    [ -f "$spool_file" ] || continue
    IFS=$'\t' read -r _ request_id url body <"$spool_file" || continue
    if [ -z "$request_id" ] || [ -z "$url" ] || [ -z "$body" ]; then continue; fi
    case "$url" in
      http://127.0.0.1:8644/*) ;;
      *) continue ;;
    esac
    body=$(printf '%s' "$body" | base64 -d 2>/dev/null) || continue
    [ -n "$body" ] || continue
    signature=$(printf '%s' "$body" | openssl dgst -sha256 -hmac "$secret" | awk '{print $NF}')
    http=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 \
      -X POST "$url" \
      -H 'Content-Type: application/json' \
      -H "X-Webhook-Signature: $signature" \
      -H "X-Request-ID: $request_id" \
      --data "$body") || http=000
    case "$http" in
      2*) rm -f "$spool_file" ;;
      *) : ;;
    esac
  done
  return 0
}

# send_alert <severity> <title> <detail> [sound]
# Only a CRIT page is delivered to Discord (#priority); any other severity does the
# local notification and returns (v2 has no #osquery channel). The Discord POST is
# best-effort and never fails the caller (so a down gateway can't break the local
# alert). An empty sound argument means a silent notification (the digest's tier).
send_alert() {
  local severity="$1" title="$2" detail="$3" sound="${4-}"

  # The local notifier renders plain text, so strip Discord markdown (**bold**,
  # `code`) for it; the webhook POST below keeps the markdown intact.
  local plain_title plain_detail
  plain_title=$(printf '%s' "$title" | sed -e 's/\*\*//g' -e 's/`//g')
  plain_detail=$(printf '%s' "$detail" | sed -e 's/\*\*//g' -e 's/`//g')

  # 1) Local macOS notification (alerter, AppleScript fallback). Pass --sound
  #    only when one is given so INFO-tier alerts are visible but silent.
  if command -v alerter >/dev/null 2>&1; then
    if [ -n "$sound" ]; then
      alerter --timeout 60 --title "$plain_title" --message "$plain_detail" --sound "$sound" >/dev/null 2>&1 &
    else
      alerter --timeout 60 --title "$plain_title" --message "$plain_detail" >/dev/null 2>&1 &
    fi
  else
    local escaped=${plain_detail//\"/\\\"}
    if [ -n "$sound" ]; then
      osascript -e "display notification \"$escaped\" with title \"$plain_title\" sound name \"$sound\"" >/dev/null 2>&1 || true
    else
      osascript -e "display notification \"$escaped\" with title \"$plain_title\"" >/dev/null 2>&1 || true
    fi
  fi

  # v2: only a CRIT page is delivered to Discord. Any other severity stops here,
  # after the local notification — there is no #osquery channel to POST to.
  [ "$severity" = "CRIT" ] || return 0
  local url="$OSQUERY_HERMES_PRIORITY_URL"

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

  local body signature request_id http attempt
  # host is INSIDE the signed body (and thus the request_id) — the spec's body shape
  # and the documented multi-host migration seam both require {event_type, host, alert}.
  body=$(jq -cn --arg h "$(hostname -s)" --arg t "$title" --arg d "$detail" \
    '{event_type:"osquery.alert", host:$h, alert:{title:$t, detail:$d}}')
  signature=$(printf '%s' "$body" | openssl dgst -sha256 -hmac "$secret" | awk '{print $NF}')
  # Content-stable request id: a retry or double-fire of the SAME alert dedups
  # at the gateway (it honours X-Request-ID for 1h) instead of double-posting.
  request_id="osquery-$(printf '%s' "$body" | openssl dgst -sha256 | awk '{print $NF}' | cut -c1-32)"

  for attempt in 1 2 3; do
    http=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 \
      -X POST "$url" \
      -H 'Content-Type: application/json' \
      -H "X-Webhook-Signature: $signature" \
      -H "X-Request-ID: $request_id" \
      --data "$body") || http=000
    case "$http" in
      2*) return 0 ;;  # delivered
      429 | 5?? | 000) # transient → back off and retry (base overridable for tests)
        if [ "$attempt" -lt 3 ]; then sleep "$((attempt * ${OSQUERY_RETRY_BACKOFF_BASE:-1}))"; fi ;;
      *) break ;; # 401/413/etc — retry won't help
    esac
  done
  # Delivery failed: spool the page so it is never silently lost; the drain replays
  # it. Log the request_id only — never the body or the secret.
  _spool_page "$request_id" "$url" "$body"
  _osquery_log "SPOOLED webhook delivery: request_id=$request_id http=$http"
  return 0
}
