#!/usr/bin/env bash
#
# osquery-alert-dispatch.sh — sourced helper, not run directly. Provides
# send_alert(), which always fires the local macOS notifier (alerter) and, for a
# CRIT severity ONLY, POSTs the page to the hermes #priority Discord webhook. v2
# has NO #osquery channel — there is deliberately no non-priority route for a
# producer to leak to. Signing + spooled delivery live here so every producer —
# osquery-results-alerter.sh, osquery-firewall-gatekeeper-monitor.sh,
# osquery-uptime-watchdog.sh, osquery-digest.sh, osquery-tailscale-monitor.sh —
# shares one implementation.
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

# Spool one undelivered page and REPORT persistence success (R2-6). The file is named by
# the occurrence-unique request_id (R2-4), so re-spooling the same occurrence is idempotent
# while two DISTINCT occurrences never collide. Written through a checked temp file + atomic
# rename so a reader never sees a torn entry, and RETURNS NONZERO on any persistence failure
# so the caller treats a failed spool as a HARD delivery failure (never "spooled" when the
# file does not exist). Line: <unix_ts>\t<request_id>\t<url>\t<base64(body)>.
_spool_page() {
  local request_id="$1" url="$2" body="$3" spool_file tmp
  mkdir -p "$OSQUERY_SPOOL_DIR" 2>/dev/null || return 1
  chmod 700 "$OSQUERY_SPOOL_DIR" 2>/dev/null || true
  spool_file="$OSQUERY_SPOOL_DIR/$request_id"
  tmp="$spool_file.tmp.$$"
  if ! printf '%s\t%s\t%s\t%s\n' \
    "$(date -u +%s)" "$request_id" "$url" "$(printf '%s' "$body" | base64 | tr -d '\n')" \
    >"$tmp" 2>/dev/null; then
    rm -f "$tmp" 2>/dev/null || true
    return 1
  fi
  chmod 600 "$tmp" 2>/dev/null || true
  mv -f "$tmp" "$spool_file" 2>/dev/null || {
    rm -f "$tmp" 2>/dev/null || true
    return 1
  }
  return 0
}

# Fire ONE loud, interruptive local notification. Used when a page can be neither delivered
# NOR durably spooled (R2-6): the operator MUST learn that a CRITICAL alert was lost, since a
# silently dropped page is indistinguishable from "all clear". Best-effort, never fails caller.
_loud_local() {
  local t="$1" m="$2"
  if command -v alerter >/dev/null 2>&1; then
    alerter --timeout 60 --title "$t" --message "$m" --sound Funk >/dev/null 2>&1 &
  else
    local escaped=${m//\"/\\\"}
    osascript -e "display notification \"$escaped\" with title \"$t\" sound name \"Funk\"" >/dev/null 2>&1 || true
  fi
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
    case "$spool_file" in *.tmp.*) continue ;; esac # skip an in-flight temp from a crashed write
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

# send_alert <severity> <title> <detail> [sound] [occurrence_id]
# Only a CRIT page is delivered to Discord (#priority); any other severity does the local
# notification and returns (v2 has no #osquery channel). The empty sound argument means a
# silent notification (the digest/heartbeat tier) — which now ALSO threads tier=muted into
# the POST body (R2-11) so a muted message is distinguishable from a real page on the wire.
# occurrence_id (optional, R2-4) identifies THIS occurrence so its request_id/spool filename
# are occurrence-unique (distinct incidents survive) yet stable across a retry of the same
# occurrence (the gateway dedups it). Absent → a per-call unique id.
#
# Return contract (R2-6): 0 when the page was DELIVERED or durably SPOOLED; NONZERO only on a
# HARD failure (neither delivered nor stored), after firing a loud local alert. A monotonic
# per-process sequence guarantees the fallback id is unique across calls in one process.
_OSQUERY_ALERT_SEQ=0
send_alert() {
  local severity="$1" title="$2" detail="$3" sound="${4-}" occurrence="${5-}"

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

  local body request_id signature http attempt tier id_seed
  # tier (R2-11): a page is loud (a sound was requested), a digest/heartbeat is muted (no
  # sound). Both severities are CRIT and both POST; tier lets the Hermes adapter suppress the
  # notification for muted traffic instead of pinging it like a page. host is INSIDE the
  # signed body — the spec's body shape and the multi-host migration seam both require
  # {event_type, host, tier, alert}.
  if [ -n "$sound" ]; then tier="page"; else tier="muted"; fi
  body=$(jq -cn --arg h "$(hostname -s)" --arg t "$title" --arg d "$detail" --arg tier "$tier" \
    '{event_type:"osquery.alert", host:$h, tier:$tier, alert:{title:$t, detail:$d}}')
  # request_id from OCCURRENCE IDENTITY, not body content (R2-4). Two distinct incidents that
  # happen to render the same body get DISTINCT ids (both survive the spool, both deliver);
  # a retry of the SAME occurrence reuses one id (gateway dedups it for 1h via X-Request-ID,
  # spool filename is idempotent). No occurrence supplied → a per-call unique seed (the
  # monotonic sequence + pid + time + RANDOM) so each distinct call still spools uniquely.
  # Built BEFORE reading the secret so a missing-secret page can spool with the SAME id and
  # the drain later signs and delivers it verbatim.
  if [ -n "$occurrence" ]; then
    id_seed="$occurrence"
  else
    _OSQUERY_ALERT_SEQ=$((_OSQUERY_ALERT_SEQ + 1))
    id_seed="fallback|$(date -u +%s)|$$|${_OSQUERY_ALERT_SEQ}|${RANDOM}|$body"
  fi
  request_id="osquery-$(printf '%s' "$id_seed" | openssl dgst -sha256 | awk '{print $NF}' | cut -c1-32)"

  # 2) Discord via the hermes webhook (best-effort, bounded retry). Read the HMAC
  #    key from the notifier's own secret file (env override allowed for tests);
  #    strip CR so a CRLF file can't corrupt the key.
  local secret="${OSQUERY_WEBHOOK_SECRET:-}"
  if [ -z "$secret" ] && [ -r "$OSQUERY_WEBHOOK_SECRET_FILE" ]; then
    IFS= read -r secret <"$OSQUERY_WEBHOOK_SECRET_FILE" || true
    secret=$(printf '%s' "$secret" | tr -d '\r')
  fi
  if [ -z "$secret" ]; then
    # FX4: a missing secret must NOT silently degrade a critical to local-only. Spool the
    # page durably (unsigned; the drain signs it once the secret returns) and fire a LOUD
    # local notification NAMING the broken channel. R2-6: if the spool ALSO fails, the page
    # is neither delivered nor stored — a HARD failure that must be loud AND return nonzero,
    # never a bare success that drops the page.
    if _spool_page "$request_id" "$url" "$body"; then
      _osquery_log "SPOOLED-NOSECRET Discord delivery degraded: request_id=$request_id (no secret in $OSQUERY_WEBHOOK_SECRET_FILE)"
      _loud_local "⚠️ osquery Discord paging BROKEN" \
        "No webhook secret — this CRITICAL page was spooled locally and delivers when the secret is restored."
      return 0
    fi
    _osquery_log "SPOOL-FAILED-NOSECRET request_id=$request_id (no secret AND spool unwritable: $OSQUERY_SPOOL_DIR)"
    _loud_local "⚠️ osquery paging FAILED — page LOST" \
      "No webhook secret AND the page could not be stored locally — this CRITICAL alert is lost. Fix $OSQUERY_SPOOL_DIR."
    return 1
  fi

  signature=$(printf '%s' "$body" | openssl dgst -sha256 -hmac "$secret" | awk '{print $NF}')

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
  # Delivery failed: spool the page so it is never silently lost; the drain replays it. R2-6:
  # a spool failure here means the page is neither delivered NOR stored — a HARD failure that
  # returns nonzero and fires a loud local alert, so the caller does NOT advance its cursor/
  # state past a page that was actually lost. Log the request_id only — never body or secret.
  if _spool_page "$request_id" "$url" "$body"; then
    _osquery_log "SPOOLED webhook delivery: request_id=$request_id http=$http"
    return 0
  fi
  _osquery_log "SPOOL-FAILED webhook delivery: request_id=$request_id http=$http (spool unwritable: $OSQUERY_SPOOL_DIR)"
  _loud_local "⚠️ osquery paging FAILED — page LOST" \
    "Discord POST failed (http=$http) AND the page could not be stored locally — this CRITICAL alert is lost. Fix $OSQUERY_SPOOL_DIR."
  return 1
}
