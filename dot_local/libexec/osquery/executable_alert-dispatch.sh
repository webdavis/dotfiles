#!/usr/bin/env bash
#
# alert-dispatch.sh, sourced helper, not run directly. Provides
# send_alert(), which dispatches one finding to BOTH the local macOS notifier
# (alerter) and a hermes Discord webhook, routing by severity (CRIT -> #priority,
# else -> #osquery). Signing, dual-channel delivery, and durable handling of an
# undelivered page all live here so the three producers (results-alerter.sh,
# firewall-gatekeeper-monitor.sh, and uptime-watchdog.sh) share one
# implementation.
#
# Usage (from a sourcing script):
#   source "$HOME/.local/libexec/osquery/alert-dispatch.sh"
#   send_alert CRIT "Firewall disabled" "alf global_state 1 -> 0" Sosumi

# Two routes, same app (osquery), each signed with the one osquery key below.
# CRIT findings go to the #priority channel (the one channel the user watches);
# everything else goes to the quiet #osquery log channel. send_alert picks the
# URL from its severity argument.
OSQUERY_HERMES_URL="${OSQUERY_HERMES_URL:-http://127.0.0.1:8644/webhooks/osquery}"
OSQUERY_HERMES_PRIORITY_URL="${OSQUERY_HERMES_PRIORITY_URL:-http://127.0.0.1:8644/webhooks/osquery-priority}"
# The notifier signs with its OWN copy of the HMAC key, read from its own secret
# file, NOT from hermes's .env. HMAC is symmetric so the value must match the
# gateway's, but the signer must not reach into the verifier's credential store;
# each side owns its own copy. Single-value file, mode 600, runtime (not tracked).
OSQUERY_WEBHOOK_SECRET_FILE="${OSQUERY_WEBHOOK_SECRET_FILE:-$HOME/.config/osquery/webhook-secret}"
OSQUERY_DELIVERY_LOG="${OSQUERY_DELIVERY_LOG:-$HOME/.local/log/osquery/webhook-delivery.log}"
# Undelivered pages are stored here, one mode-600 file per page in a mode-700
# dir, so a transient gateway outage never loses a page (a lost page is
# indistinguishable from "all clear"). retry_undelivered_alerts replays them.
OSQUERY_UNDELIVERED_ALERTS_DIR="${OSQUERY_UNDELIVERED_ALERTS_DIR:-$HOME/.local/state/osquery-undelivered-alerts}"

# A monotonic per-process sequence so the fallback request id (when no occurrence
# identity is threaded) is unique across calls in one process.
_OSQUERY_ALERT_SEQUENCE=0

# Append a timestamped line to the delivery log (best-effort; never fails caller).
# Only metadata is ever logged, never the body or the HMAC secret.
_osquery_log() {
  mkdir -p "$(dirname "$OSQUERY_DELIVERY_LOG")" 2>/dev/null || true
  printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$1" >>"$OSQUERY_DELIVERY_LOG" 2>/dev/null || true
}

# Store one undelivered page and REPORT persistence success. The file is named by
# the occurrence-unique request_id, so re-storing the same occurrence is
# idempotent while two DISTINCT occurrences never collide. Written through a
# checked temp file then an atomic rename so a reader never sees a torn entry, and
# it RETURNS NONZERO on any persistence failure so the caller treats a failed
# store as a hard delivery failure (never "stored" when the file does not exist).
# Line: <unix_ts>\t<request_id>\t<url>\t<base64(body)>.
_store_undelivered_alert() {
  local request_id="$1" url="$2" body="$3" stored_file temp_file
  mkdir -p "$OSQUERY_UNDELIVERED_ALERTS_DIR" 2>/dev/null || return 1
  chmod 700 "$OSQUERY_UNDELIVERED_ALERTS_DIR" 2>/dev/null || true
  stored_file="$OSQUERY_UNDELIVERED_ALERTS_DIR/$request_id"
  temp_file="$stored_file.tmp.$$"
  if ! printf '%s\t%s\t%s\t%s\n' \
    "$(date -u +%s)" "$request_id" "$url" "$(printf '%s' "$body" | base64 | tr -d '\n')" \
    >"$temp_file" 2>/dev/null; then
    rm -f "$temp_file" 2>/dev/null || true
    return 1
  fi
  chmod 600 "$temp_file" 2>/dev/null || true
  if ! mv -f "$temp_file" "$stored_file" 2>/dev/null; then
    rm -f "$temp_file" 2>/dev/null || true
    return 1
  fi
  return 0
}

# Fire ONE loud, interruptive local notification. Used when a page can be neither
# delivered NOR durably stored: the operator MUST learn that a CRITICAL alert was
# lost, since a silently dropped page is indistinguishable from "all clear".
# Best-effort, never fails caller.
_loud_local() {
  local title="$1" message="$2"
  if command -v alerter >/dev/null 2>&1; then
    alerter --timeout 60 --title "$title" --message "$message" --sound Funk >/dev/null 2>&1 &
  else
    local escaped=${message//\"/\\\"}
    osascript -e "display notification \"$escaped\" with title \"$title\" sound name \"Funk\"" >/dev/null 2>&1 || true
  fi
}

# Replay stored undelivered pages: re-POST each (stored request_id verbatim so the
# gateway dedups; signature recomputed from the stored body), remove on delivery,
# leave on failure for the next run. Localhost only, a tampered or off-box url is
# skipped, never sent. Fully set -e-safe: a malformed entry or empty dir must
# NEVER abort the caller (a delivery feature must not cause a detection outage).
retry_undelivered_alerts() {
  [[ -d $OSQUERY_UNDELIVERED_ALERTS_DIR ]] || return 0
  local secret stored_file request_id url body signature http_status
  secret="${OSQUERY_WEBHOOK_SECRET:-}"
  if [[ -z $secret && -r $OSQUERY_WEBHOOK_SECRET_FILE ]]; then
    IFS= read -r secret <"$OSQUERY_WEBHOOK_SECRET_FILE" || true
    secret="$(printf '%s' "$secret" | tr -d '\r')"
  fi
  [[ -n $secret ]] || return 0
  for stored_file in "$OSQUERY_UNDELIVERED_ALERTS_DIR"/*; do
    [[ -f $stored_file ]] || continue
    case "$stored_file" in
      *.tmp.*) continue ;; # skip an in-flight temp from a crashed write
    esac
    IFS=$'\t' read -r _ request_id url body <"$stored_file" || continue
    if [[ -z $request_id || -z $url || -z $body ]]; then continue; fi
    case "$url" in
      http://127.0.0.1:8644/*) ;;
      *) continue ;;
    esac
    body="$(printf '%s' "$body" | base64 -d 2>/dev/null)" || continue
    [[ -n $body ]] || continue
    signature="$(printf '%s' "$body" | openssl dgst -sha256 -hmac "$secret" | awk '{print $NF}')"
    http_status="$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 \
      -X POST "$url" \
      -H 'Content-Type: application/json' \
      -H "X-Webhook-Signature: $signature" \
      -H "X-Request-ID: $request_id" \
      --data "$body")" || http_status=000
    case "$http_status" in
      2*) rm -f "$stored_file" ;;
      *) : ;;
    esac
  done
  return 0
}

# send_alert <severity> <title> <detail> [sound] [occurrence_id]
# severity is CRIT | NOTICE | INFO. CRIT routes the Discord POST to the #priority
# channel; NOTICE/INFO route to the quiet #osquery channel. The local
# notification always fires regardless. An empty sound argument means a silent
# notification (used for the low INFO tier). occurrence_id (optional) identifies
# THIS occurrence so its request_id and stored filename are occurrence-unique
# (distinct incidents survive) yet stable across a retry of the same occurrence
# (the gateway dedups it). Absent means a per-call unique id.
#
# Return contract: 0 when the page was DELIVERED or durably STORED; NONZERO only
# on a HARD failure (neither delivered nor stored), after firing a loud local
# alert, so the caller does not advance its cursor past a page that was lost.
send_alert() {
  local severity="$1" title="$2" detail="$3" sound="${4-}" occurrence="${5-}"

  # Route by severity: only CRIT reaches #priority.
  local webhook_url="$OSQUERY_HERMES_URL"
  [[ $severity == "CRIT" ]] && webhook_url="$OSQUERY_HERMES_PRIORITY_URL"

  # The local notifier renders plain text, so strip Discord markdown (**bold**,
  # `code`) for it; the webhook POST below keeps the markdown intact.
  local plain_title plain_detail
  plain_title="$(printf '%s' "$title" | sed -e 's/\*\*//g' -e 's/`//g')"
  plain_detail="$(printf '%s' "$detail" | sed -e 's/\*\*//g' -e 's/`//g')"

  # 1) Local macOS notification (alerter, AppleScript fallback). Pass --sound
  #    only when one is given so INFO-tier alerts are visible but silent.
  if command -v alerter >/dev/null 2>&1; then
    if [[ -n $sound ]]; then
      alerter --timeout 60 --title "$plain_title" --message "$plain_detail" --sound "$sound" >/dev/null 2>&1 &
    else
      alerter --timeout 60 --title "$plain_title" --message "$plain_detail" >/dev/null 2>&1 &
    fi
  else
    local escaped_detail=${plain_detail//\"/\\\"}
    if [[ -n $sound ]]; then
      osascript -e "display notification \"$escaped_detail\" with title \"$plain_title\" sound name \"$sound\"" >/dev/null 2>&1 || true
    else
      osascript -e "display notification \"$escaped_detail\" with title \"$plain_title\"" >/dev/null 2>&1 || true
    fi
  fi

  # Build the webhook body and an occurrence-stable request id. The request id
  # derives from OCCURRENCE IDENTITY (threaded from the caller) when present, so
  # two distinct incidents that render the same body get distinct ids (both
  # stored, both delivered) while a retry of the same occurrence reuses one id
  # (the gateway dedups it, the stored filename is idempotent). No occurrence
  # means a per-call unique seed. Built BEFORE reading the secret so a
  # missing-secret page stores with the same id the drain later signs and sends.
  local body request_id id_seed
  body="$(jq -cn --arg t "$title" --arg d "$detail" \
    '{event_type:"osquery.alert", alert:{title:$t, detail:$d}}')"
  if [[ -n $occurrence ]]; then
    id_seed="$occurrence"
  else
    _OSQUERY_ALERT_SEQUENCE=$((_OSQUERY_ALERT_SEQUENCE + 1))
    id_seed="fallback|$(date -u +%s)|$$|${_OSQUERY_ALERT_SEQUENCE}|${RANDOM}|$body"
  fi
  request_id="osquery-$(printf '%s' "$id_seed" | openssl dgst -sha256 | awk '{print $NF}' | cut -c1-32)"

  # 2) Discord via the hermes webhook (best-effort, bounded retry). Read the HMAC
  #    key from the notifier's own secret file (env override allowed for tests);
  #    strip CR so a CRLF file can't corrupt the key.
  local secret="${OSQUERY_WEBHOOK_SECRET:-}"
  if [[ -z $secret && -r $OSQUERY_WEBHOOK_SECRET_FILE ]]; then
    IFS= read -r secret <"$OSQUERY_WEBHOOK_SECRET_FILE" || true
    secret="$(printf '%s' "$secret" | tr -d '\r')"
  fi
  if [[ -z $secret ]]; then
    # A missing secret must NOT silently degrade a page to local-only. Store it
    # durably (unsigned; the drain signs it once the secret returns) and fire a
    # LOUD local notice naming the broken channel. If storing ALSO fails, the
    # page is neither delivered nor stored, a hard failure that must be loud AND
    # return nonzero, never a bare success that drops the page.
    if _store_undelivered_alert "$request_id" "$webhook_url" "$body"; then
      _osquery_log "STORED-NOSECRET Discord delivery degraded: request_id=$request_id (no secret in $OSQUERY_WEBHOOK_SECRET_FILE)"
      _loud_local "osquery Discord paging BROKEN" \
        "No webhook secret. This page was stored locally and delivers when the secret is restored."
      return 0
    fi
    _osquery_log "STORE-FAILED-NOSECRET request_id=$request_id (no secret AND storage unwritable: $OSQUERY_UNDELIVERED_ALERTS_DIR)"
    _loud_local "osquery paging FAILED, page LOST" \
      "No webhook secret AND the page could not be stored locally. Fix $OSQUERY_UNDELIVERED_ALERTS_DIR."
    return 1
  fi

  local signature http_status attempt
  signature="$(printf '%s' "$body" | openssl dgst -sha256 -hmac "$secret" | awk '{print $NF}')"

  for attempt in 1 2 3; do
    http_status="$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 \
      -X POST "$webhook_url" \
      -H 'Content-Type: application/json' \
      -H "X-Webhook-Signature: $signature" \
      -H "X-Request-ID: $request_id" \
      --data "$body")" || http_status=000
    case "$http_status" in
      2*) return 0 ;;  # delivered
      429 | 5?? | 000) # transient, back off and retry (base overridable for tests)
        if [[ $attempt -lt 3 ]]; then sleep "$((attempt * ${OSQUERY_RETRY_BACKOFF_BASE:-1}))"; fi ;;
      *) break ;; # 401/413/etc, retry won't help
    esac
  done
  # Delivery failed after retries: store the page so it is never silently lost;
  # the drain replays it. If storage ALSO fails, the page is neither delivered
  # nor stored, a hard failure that returns nonzero and fires a loud local alert,
  # so the caller does NOT advance its cursor past a page that was actually lost.
  if _store_undelivered_alert "$request_id" "$webhook_url" "$body"; then
    _osquery_log "STORED webhook delivery: request_id=$request_id http=$http_status"
    return 0
  fi
  _osquery_log "STORE-FAILED webhook delivery: request_id=$request_id http=$http_status (storage unwritable: $OSQUERY_UNDELIVERED_ALERTS_DIR)"
  _loud_local "osquery paging FAILED, page LOST" \
    "Discord POST failed (http=$http_status) AND the page could not be stored locally. Fix $OSQUERY_UNDELIVERED_ALERTS_DIR."
  return 1
}
