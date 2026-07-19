#!/usr/bin/env bash
#
# alert-dispatch.sh, sourced helper, not run directly. Provides
# send_alert(), which always fires the local macOS notifier (alerter) and, for a
# CRIT severity ONLY, POSTs the page to the hermes #priority Discord webhook. v2
# has NO #osquery channel: there is deliberately no non-priority route for a
# producer to leak to. Signing and durable handling of an undelivered page live
# here so the three producers (results-alerter.sh, firewall-gatekeeper-monitor.sh,
# and uptime-watchdog.sh) share one implementation.
#
# The undelivered-alerts store is WRITE-AHEAD: send_alert persists the page to
# disk BEFORE the first network attempt and deletes it only after a confirmed 2xx.
# A crash or kill anywhere between persist and success therefore leaves a
# recoverable record for the next drain (a lost page is indistinguishable from
# "all clear").
#
# Usage (from a sourcing script):
#   source "$HOME/.local/libexec/osquery/alert-dispatch.sh"
#   send_alert CRIT "Firewall disabled" "alf global_state 1 -> 0" Sosumi

# One Discord route: the #priority channel (the one channel the user watches),
# signed with the osquery HMAC key below. v2 has NO #osquery channel, only a
# confirmed CRIT page is POSTed; any other severity does the local notification
# only. There is deliberately no non-priority URL for a producer to leak to.
OSQUERY_HERMES_PRIORITY_URL="${OSQUERY_HERMES_PRIORITY_URL:-http://127.0.0.1:8644/webhooks/osquery-priority}"
# The notifier signs with its OWN copy of the HMAC key, read from its own secret
# file, NOT from hermes's .env. HMAC is symmetric so the value must match the
# gateway's, but the signer must not reach into the verifier's credential store;
# each side owns its own copy. Single-value file, mode 600, runtime (not tracked).
OSQUERY_WEBHOOK_SECRET_FILE="${OSQUERY_WEBHOOK_SECRET_FILE:-$HOME/.config/osquery/webhook-secret}"
OSQUERY_DELIVERY_LOG="${OSQUERY_DELIVERY_LOG:-$HOME/.local/log/osquery/webhook-delivery.log}"
# Undelivered pages are stored here, one mode-600 file per page in a mode-700
# dir, so a transient gateway outage never loses a page. retry_undelivered_alerts
# replays them in occurrence-time order.
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

# HMAC-SHA256 of the message on STDIN under the key in $1, hex digest on stdout.
# The key is a FUNCTION argument (it lives in this shell's memory, never a child
# process argv), and it is never passed to openssl: openssl only ever hashes
# bytes on stdin (`openssl dgst -sha256`), so the secret cannot appear in any
# `ps` output. HMAC is built by hand from SHA-256 (the standard
# H((K'^opad)||H((K'^ipad)||m)) construction); a unit test pins it byte-identical
# to `openssl dgst -hmac`. openssl `dgst` is used (not the OpenSSL-3-only `mac`)
# so it works with the host's LibreSSL too.
_hmac_sha256_hex() {
  local key="$1" block_size=64 key_hex byte_hex ipad_format="" opad_format="" inner_hex inner_format="" i
  # Key to hex bytes; if longer than the block size, replace it with its digest.
  key_hex="$(printf '%s' "$key" | od -v -A n -t x1 | tr -d ' \n')"
  if ((${#key_hex} > block_size * 2)); then
    key_hex="$(printf '%s' "$key" | openssl dgst -sha256 | awk '{print $NF}')"
  fi
  while ((${#key_hex} < block_size * 2)); do key_hex+="00"; done
  # Build the padded-key-XOR-pad byte strings as printf \xHH format specifiers.
  for ((i = 0; i < block_size; i++)); do
    byte_hex="${key_hex:i*2:2}"
    printf -v ipad_format '%s\\x%02x' "$ipad_format" "$((16#$byte_hex ^ 0x36))"
    printf -v opad_format '%s\\x%02x' "$opad_format" "$((16#$byte_hex ^ 0x5c))"
  done
  # inner = SHA256( (K' xor ipad) || message ); the message streams from stdin.
  # shellcheck disable=SC2059 # the format is a fixed \xHH byte string, no user data
  inner_hex="$({
    printf "$ipad_format"
    cat
  } | openssl dgst -sha256 | awk '{print $NF}')"
  for ((i = 0; i < ${#inner_hex}; i += 2)); do inner_format+="\\x${inner_hex:i:2}"; done
  # outer = SHA256( (K' xor opad) || inner ).
  # shellcheck disable=SC2059 # the format is a fixed \xHH byte string, no user data
  printf "$opad_format$inner_format" | openssl dgst -sha256 | awk '{print $NF}'
}

# Store one undelivered page and REPORT persistence success. The file is named by
# the occurrence-unique request_id, so re-storing the same occurrence is
# idempotent while two DISTINCT occurrences never collide. The encoded body and
# the occurrence timestamp are computed and VALIDATED in checked assignments
# BEFORE the record file is opened, so an encoder failure fails the store instead
# of writing an empty body that a later drain would silently skip. Written through
# a checked temp file, flushed, then atomically renamed, and it RETURNS NONZERO on
# any persistence failure so the caller treats a failed store as a hard delivery
# failure (never "stored" when the file does not exist).
# Line: <occurrence_ts>\t<request_id>\t<url>\t<base64(body)>.
_store_undelivered_alert() {
  local occurrence_ts="$1" request_id="$2" url="$3" body="$4" stored_file temp_file encoded_body
  [[ $occurrence_ts =~ ^[0-9]+$ ]] || return 1
  [[ -n $request_id ]] || return 1
  # pipefail inside the subshell so a base64 failure is the assignment's status,
  # not masked by the trailing tr; then reject an empty result defensively.
  if ! encoded_body="$(
    set -o pipefail
    printf '%s' "$body" | base64 | tr -d '\n'
  )"; then
    return 1
  fi
  [[ -n $encoded_body ]] || return 1
  mkdir -p "$OSQUERY_UNDELIVERED_ALERTS_DIR" 2>/dev/null || return 1
  chmod 700 "$OSQUERY_UNDELIVERED_ALERTS_DIR" 2>/dev/null || true
  stored_file="$OSQUERY_UNDELIVERED_ALERTS_DIR/$request_id"
  temp_file="$stored_file.tmp.$$"
  if ! printf '%s\t%s\t%s\t%s\n' "$occurrence_ts" "$request_id" "$url" "$encoded_body" >"$temp_file" 2>/dev/null; then
    rm -f "$temp_file" 2>/dev/null || true
    return 1
  fi
  chmod 600 "$temp_file" 2>/dev/null || true
  # Flush the record's bytes before the rename so a crash right after the rename
  # cannot leave a present-but-empty file. GNU `sync FILE` fsyncs just this file;
  # BSD sync takes no argument and flushes all. Best-effort either way.
  sync "$temp_file" 2>/dev/null || sync 2>/dev/null || true
  if ! mv -f "$temp_file" "$stored_file" 2>/dev/null; then
    rm -f "$temp_file" 2>/dev/null || true
    return 1
  fi
  return 0
}

# Move any crashed *.tmp.* partial (a temp whose writer process is gone) OUT of
# the store, into a sibling quarantine dir, so a torn write is never replayed and
# never skipped forever. A temp whose writer pid is still alive is an in-flight
# write and is left alone. Best-effort; never fails the caller.
_quarantine_stale_partials() {
  [[ -d $OSQUERY_UNDELIVERED_ALERTS_DIR ]] || return 0
  local temp_file pid quarantine_dir
  quarantine_dir="${OSQUERY_UNDELIVERED_ALERTS_DIR%/}.quarantine"
  for temp_file in "$OSQUERY_UNDELIVERED_ALERTS_DIR"/*.tmp.*; do
    [[ -f $temp_file ]] || continue
    pid="${temp_file##*.tmp.}"
    if [[ $pid =~ ^[0-9]+$ ]] && kill -0 "$pid" 2>/dev/null; then
      continue # a live writer still owns this temp
    fi
    mkdir -p "$quarantine_dir" 2>/dev/null || continue
    mv -f "$temp_file" "$quarantine_dir/${temp_file##*/}" 2>/dev/null || true
  done
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

# Deliver ONE stored record: read it, skip a non-localhost url (never send an
# off-box page), decode and sign the stored body, POST it (stored request_id
# verbatim so the gateway dedups), and delete the record only on a confirmed 2xx.
# Fully set -e-safe: any malformed field returns 0 so the drain continues.
_deliver_stored_record() {
  local stored_file="$1" secret="$2" ts request_id url body signature http_status
  IFS=$'\t' read -r ts request_id url body <"$stored_file" || return 0
  [[ -n $request_id && -n $url && -n $body ]] || return 0
  case "$url" in
    http://127.0.0.1:8644/*) ;;
    *) return 0 ;;
  esac
  body="$(printf '%s' "$body" | base64 -d 2>/dev/null)" || return 0
  [[ -n $body ]] || return 0
  signature="$(printf '%s' "$body" | _hmac_sha256_hex "$secret")"
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
  return 0
}

# Replay stored undelivered pages in OCCURRENCE-TIME order (earliest first, with
# the file path as a deterministic tie-breaker), so a backlog delivers in the
# order events happened, not in SHA-derived filename order. Crashed partials are
# quarantined first. Localhost only. Fully set -e-safe: a malformed entry or empty
# dir must NEVER abort the caller (a delivery feature must not cause a detection
# outage).
retry_undelivered_alerts() {
  [[ -d $OSQUERY_UNDELIVERED_ALERTS_DIR ]] || return 0
  _quarantine_stale_partials
  local secret
  secret="${OSQUERY_WEBHOOK_SECRET:-}"
  if [[ -z $secret && -r $OSQUERY_WEBHOOK_SECRET_FILE ]]; then
    IFS= read -r secret <"$OSQUERY_WEBHOOK_SECRET_FILE" || true
    secret="$(printf '%s' "$secret" | tr -d '\r')"
  fi
  [[ -n $secret ]] || return 0

  # Collect "<occurrence_ts>\t<file>" for every well-formed record, then sort by
  # occurrence time (numeric) with the path as the tie-breaker.
  local records="" stored_file ts
  for stored_file in "$OSQUERY_UNDELIVERED_ALERTS_DIR"/*; do
    [[ -f $stored_file ]] || continue
    case "$stored_file" in
      *.tmp.*) continue ;; # an in-flight temp; quarantine handles crashed ones
    esac
    IFS=$'\t' read -r ts _ <"$stored_file" || continue
    [[ $ts =~ ^[0-9]+$ ]] || continue
    records+="$ts	$stored_file"$'\n'
  done
  [[ -n $records ]] || return 0

  local sorted
  sorted="$(printf '%s' "$records" | sort -t$'\t' -k1,1n -k2,2)" || return 0
  while IFS=$'\t' read -r _ stored_file; do
    [[ -n $stored_file ]] || continue
    _deliver_stored_record "$stored_file" "$secret"
  done <<<"$sorted"
  return 0
}

# send_alert <severity> <title> <detail> [sound] [occurrence_id]
# severity is CRIT | NOTICE | INFO. Only CRIT is delivered to Discord (#priority);
# any other severity does the local notification and returns. The local
# notification always fires regardless. An empty sound argument means a silent
# notification (used for the low INFO tier). occurrence_id (optional) identifies
# THIS occurrence so its request_id and stored filename are occurrence-unique
# (distinct incidents survive) yet stable across a retry of the same occurrence
# (the gateway dedups it). Absent means a per-call unique id.
#
# Delivery is WRITE-AHEAD: the page is persisted BEFORE the first network attempt
# and deleted only after a confirmed 2xx. Return contract: 0 when the page was
# DELIVERED or durably STORED; NONZERO only on a HARD failure (the write-ahead
# persist itself failed, so the page is neither delivered nor stored), after
# firing a loud local alert, so the caller does not advance its cursor past a page
# that was lost.
send_alert() {
  local severity="$1" title="$2" detail="$3" sound="${4-}" occurrence="${5-}"

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

  # v2: only a CRIT page is delivered to Discord. Any other severity stops here,
  # after the local notification, there is no #osquery channel to POST to.
  [[ $severity == "CRIT" ]] || return 0
  local url="$OSQUERY_HERMES_PRIORITY_URL"

  # Occurrence time: when this page was raised. It is the drain's ordering key AND
  # part of the signed body. A clock glitch never blocks a page (fall back to 0).
  local occurrence_ts
  occurrence_ts="$(date -u +%s)"
  [[ $occurrence_ts =~ ^[0-9]+$ ]] || occurrence_ts=0

  # Build the webhook body and an occurrence-stable request id. tier: a page is
  # loud (a sound was requested), a digest/heartbeat is muted (no sound). Both
  # severities are CRIT and both POST; tier lets the Hermes adapter suppress the
  # notification for muted traffic instead of pinging it like a page. host and the
  # occurrence ts live INSIDE the signed body, the spec's body shape and the
  # multi-host migration seam both require {event_type, host, tier, ts, alert}.
  #
  # The request id derives from OCCURRENCE IDENTITY (threaded from the caller)
  # when present, so two distinct incidents that render the same body get distinct
  # ids (both stored, both delivered) while a retry of the same occurrence reuses
  # one id (the gateway dedups it, the stored filename is idempotent). No
  # occurrence means a per-call unique seed.
  local body request_id id_seed tier
  if [[ -n $sound ]]; then tier="page"; else tier="muted"; fi
  body="$(jq -cn --arg h "$(hostname -s)" --arg t "$title" --arg d "$detail" \
    --arg tier "$tier" --argjson ts "$occurrence_ts" \
    '{event_type:"osquery.alert", host:$h, tier:$tier, ts:$ts, alert:{title:$t, detail:$d}}')"
  if [[ -n $occurrence ]]; then
    id_seed="$occurrence"
  else
    _OSQUERY_ALERT_SEQUENCE=$((_OSQUERY_ALERT_SEQUENCE + 1))
    id_seed="fallback|$occurrence_ts|$$|${_OSQUERY_ALERT_SEQUENCE}|${RANDOM}|$body"
  fi
  request_id="osquery-$(printf '%s' "$id_seed" | openssl dgst -sha256 | awk '{print $NF}' | cut -c1-32)"

  # WRITE-AHEAD: persist the page BEFORE the first network attempt, so a crash or
  # kill before a confirmed success leaves a recoverable record. A persist failure
  # is a HARD failure (loud + nonzero): the page can be neither delivered nor
  # stored, and the caller must not advance its cursor past it.
  if ! _store_undelivered_alert "$occurrence_ts" "$request_id" "$url" "$body"; then
    _osquery_log "STORE-FAILED write-ahead persist: request_id=$request_id (storage unwritable: $OSQUERY_UNDELIVERED_ALERTS_DIR)"
    _loud_local "osquery paging FAILED, page LOST" \
      "The page could not be stored locally. Fix $OSQUERY_UNDELIVERED_ALERTS_DIR."
    return 1
  fi
  local stored_file="$OSQUERY_UNDELIVERED_ALERTS_DIR/$request_id"

  # 2) Discord via the hermes webhook (best-effort, bounded retry). Read the HMAC
  #    key from the notifier's own secret file (env override allowed for tests);
  #    strip CR so a CRLF file can't corrupt the key.
  local secret="${OSQUERY_WEBHOOK_SECRET:-}"
  if [[ -z $secret && -r $OSQUERY_WEBHOOK_SECRET_FILE ]]; then
    IFS= read -r secret <"$OSQUERY_WEBHOOK_SECRET_FILE" || true
    secret="$(printf '%s' "$secret" | tr -d '\r')"
  fi
  if [[ -z $secret ]]; then
    # The page is already persisted (write-ahead); the drain signs and sends it
    # once the secret returns. Name the broken channel loudly.
    _osquery_log "STORED-NOSECRET Discord delivery degraded: request_id=$request_id (no secret in $OSQUERY_WEBHOOK_SECRET_FILE)"
    _loud_local "osquery Discord paging BROKEN" \
      "No webhook secret. This page was stored locally and delivers when the secret is restored."
    return 0
  fi

  local signature http_status attempt
  signature="$(printf '%s' "$body" | _hmac_sha256_hex "$secret")"

  for attempt in 1 2 3; do
    http_status="$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 \
      -X POST "$url" \
      -H 'Content-Type: application/json' \
      -H "X-Webhook-Signature: $signature" \
      -H "X-Request-ID: $request_id" \
      --data "$body")" || http_status=000
    case "$http_status" in
      2*)
        rm -f "$stored_file" # delete ONLY after a confirmed 2xx
        return 0
        ;;
      429 | 5?? | 000) # transient, back off and retry (base overridable for tests)
        if [[ $attempt -lt 3 ]]; then sleep "$((attempt * ${OSQUERY_RETRY_BACKOFF_BASE:-1}))"; fi ;;
      *) break ;; # 401/413/etc, retry won't help
    esac
  done
  # Delivery failed after retries: the write-ahead record REMAINS for the next
  # drain. Best-effort, a down gateway never fails the caller.
  _osquery_log "STORED webhook delivery pending: request_id=$request_id http=$http_status (retained for retry)"
  return 0
}
