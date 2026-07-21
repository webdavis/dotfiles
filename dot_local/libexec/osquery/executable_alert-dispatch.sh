#!/usr/bin/env bash
#
# alert-dispatch.sh, sourced helper, not run directly. Provides
# send_alert(), which always fires the local macOS notifier (alerter) and, for a
# CRIT severity ONLY, POSTs the page to the hermes #priority Discord webhook.
# Signing and durable handling of an undelivered page live here so the three
# producers (results-alerter.sh, firewall-gatekeeper-monitor.sh, and
# uptime-watchdog.sh) share one implementation.
#
# The undelivered-alerts store is WRITE-AHEAD: send_alert persists the page as a
# row in a local SQLite database BEFORE the first network attempt and deletes it
# only after a confirmed 2xx. A crash or kill anywhere between persist and
# success therefore leaves a recoverable row for the next drain. Durability
# matters this much because a page that vanishes leaves no trace: the operator
# sees a quiet system and has no way to tell a lost alert apart from genuinely
# good news.
#
# Usage (from a sourcing script):
#   source "$HOME/.local/libexec/osquery/alert-dispatch.sh"
#   send_alert CRIT "Firewall disabled" "alf global_state 1 -> 0" Sosumi

# One Discord route: the #priority channel (the one channel the user watches),
# signed with the osquery HMAC key below. Only a CRIT page is POSTed; any other
# severity does the local notification only.
OSQUERY_HERMES_PRIORITY_URL="${OSQUERY_HERMES_PRIORITY_URL:-http://127.0.0.1:8644/webhooks/osquery-priority}"
# The notifier signs with its OWN copy of the HMAC key, read from its own secret
# file, NOT from hermes's .env. HMAC is symmetric so the value must match the
# gateway's, but the signer must not reach into the verifier's credential store;
# each side owns its own copy. Single-value file, mode 600, runtime (not tracked).
OSQUERY_WEBHOOK_SECRET_FILE="${OSQUERY_WEBHOOK_SECRET_FILE:-$HOME/.config/osquery/webhook-secret}"
OSQUERY_DELIVERY_LOG="${OSQUERY_DELIVERY_LOG:-$HOME/.local/log/osquery/webhook-delivery.log}"
# The undelivered-alerts store: one pending_alerts row per page in a SQLite
# database, committed crash-atomically, the file mode 600 in a mode-700 parent,
# so a transient gateway outage never loses a page. retry_undelivered_alerts
# replays the rows in occurrence-time order. sqlite3 is the OS-provided binary,
# and its absence is a hard persist failure (verified fail-closed), never a
# lost page.
OSQUERY_UNDELIVERED_ALERTS_DB="${OSQUERY_UNDELIVERED_ALERTS_DB:-$HOME/.local/state/osquery-undelivered-alerts.sqlite3}"

# A monotonic per-process sequence so the fallback request id (when no occurrence
# identity is threaded) is unique across calls in one process.
_OSQUERY_ALERT_SEQUENCE=0

# Append a timestamped line to the delivery log. If the log directory cannot be
# created or the append fails, the line is dropped and the function still
# returns 0: a logging problem must never break alert delivery, so the caller
# carries on and only this log line is lost. Only metadata is ever logged, never
# the body or the HMAC secret.
_osquery_log() {
  mkdir -p "$(dirname "$OSQUERY_DELIVERY_LOG")" 2>/dev/null || true
  printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$1" >>"$OSQUERY_DELIVERY_LOG" 2>/dev/null || true
}

# Security protocol boundary: every SHA-256 hash in this library flows through
# here, and the bytes to hash arrive ONLY on stdin, never as a command-line
# argument, so nothing that gets hashed (a key, a body, a request-id seed) can
# ever appear in `ps` output. Prints the lowercase hex digest. openssl `dgst` is
# used (not the OpenSSL-3-only `mac`) so it works with the host's LibreSSL too.
_sha256_hex_of_stdin() {
  openssl dgst -sha256 | awk '{print $NF}'
}

# HMAC-SHA256 of the message on STDIN under the key in $1, hex digest on stdout.
# The key is a FUNCTION argument (it lives in this shell's memory, never a child
# process argv), and every hash goes through _sha256_hex_of_stdin, so the secret
# cannot appear in any `ps` output. HMAC is built by hand from SHA-256 (the
# standard H((K'^opad)||H((K'^ipad)||m)) construction); a test pins it
# byte-identical to `openssl dgst -hmac`.
_hmac_sha256_hex() {
  local key="$1" block_size=64 key_hex byte_hex ipad_format="" opad_format="" inner_hex inner_format="" i
  # Key to hex bytes; if longer than the block size, replace it with its digest.
  key_hex="$(printf '%s' "$key" | od -v -A n -t x1 | tr -d ' \n')"
  if ((${#key_hex} > block_size * 2)); then
    key_hex="$(printf '%s' "$key" | _sha256_hex_of_stdin)"
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
  } | _sha256_hex_of_stdin)"
  for ((i = 0; i < ${#inner_hex}; i += 2)); do inner_format+="\\x${inner_hex:i:2}"; done
  # outer = SHA256( (K' xor opad) || inner ).
  # shellcheck disable=SC2059 # the format is a fixed \xHH byte string, no user data
  printf "$opad_format$inner_format" | _sha256_hex_of_stdin
}

# Print the path to the sqlite3 CLI, preferring the OS-provided binary. The
# store is a darwin-only pipeline and macOS always ships /usr/bin/sqlite3; the
# PATH fallback keeps the library runnable from a test harness on another OS.
# Prints nothing and returns nonzero when no sqlite3 is found, so the caller can
# fail the persist closed rather than silently drop the page.
_osquery_sqlite3_bin() {
  if [[ -x /usr/bin/sqlite3 ]]; then
    printf '/usr/bin/sqlite3'
    return 0
  fi
  local found
  found="$(command -v sqlite3 2>/dev/null)" || return 1
  [[ -n $found ]] || return 1
  printf '%s' "$found"
}

# Run the SQL arriving on STDIN against the undelivered-alerts DB, applying the
# connection pragmas first: WAL for crash-atomic commits, and a busy_timeout so
# the several producers (results-alerter, firewall-gatekeeper, watchdog, digest,
# tailscale) serialize briefly under contention instead of failing outright. The
# pragmas' own chatter is routed to /dev/null so only the SQL's rows reach
# stdout. Only non-secret alert fields ever flow through here; the webhook secret
# is never passed to SQL. Returns sqlite3's exit status.
#
# One lock the busy_timeout does NOT absorb: a brand-new database's first
# conversion into WAL takes an exclusive lock, and when concurrent first-opens
# race it SQLite reports "database is locked" (SQLITE_BUSY) immediately instead
# of waiting (observed under an eight-producer stress; the busy handler is not
# consulted for that transition). A locked failure is therefore retried a few
# times with a short pause. The retry replays the WHOLE statement batch, which
# is safe because every statement this library issues is idempotent (CREATE IF
# NOT EXISTS, INSERT ON CONFLICT DO NOTHING, DELETE by key, SELECT). Query rows
# are buffered and printed only after a fully successful run, so a failed
# partial attempt can never leak rows to the caller, and `.bail on` makes an
# attempt stop cleanly at its first error.
_osquery_alerts_db_exec() {
  local sqlite3_bin sql_text query_output error_file sqlite3_status attempt
  sqlite3_bin="$(_osquery_sqlite3_bin)" || return 1
  sql_text="$(cat)"
  error_file="$(mktemp "${TMPDIR:-/tmp}/osquery-alerts-db-error.XXXXXX")" || return 1
  for attempt in 1 2 3 4 5; do
    sqlite3_status=0
    query_output="$(
      {
        printf '.bail on\n'
        printf '.output /dev/null\n'
        printf 'PRAGMA busy_timeout=5000;\n'
        printf 'PRAGMA journal_mode=WAL;\n'
        printf '.output\n'
        printf '%s\n' "$sql_text"
      } | "$sqlite3_bin" "$OSQUERY_UNDELIVERED_ALERTS_DB" 2>"$error_file"
    )" || sqlite3_status=$?
    if [[ $sqlite3_status -eq 0 ]]; then
      rm -f "$error_file" 2>/dev/null || true
      [[ -n $query_output ]] && printf '%s\n' "$query_output"
      return 0
    fi
    if [[ $attempt -eq 5 ]] || ! grep -qi 'database is locked' "$error_file"; then
      break
    fi
    sleep 0.1 2>/dev/null || sleep 1
  done
  cat "$error_file" >&2 2>/dev/null || true
  rm -f "$error_file" 2>/dev/null || true
  return "$sqlite3_status"
}

# Persist one undelivered page as a pending_alerts row and report success. The
# lazy schema bootstrap and THIS alert's INSERT run in ONE atomic sqlite3 batch
# (BEGIN IMMEDIATE ... COMMIT), so a crash or kill at any instant leaves either
# the row fully present or no committed change at all; there is no window where
# the schema exists but this alert vanished. Only a successful COMMIT reports
# success; any failure rolls back and returns nonzero so the caller treats it
# as the loud hard failure.
#
# Same occurrence re-stored is idempotent (ON CONFLICT(request_id) DO NOTHING),
# so a retry of one occurrence stays one row while two DISTINCT occurrences
# never collide. sequence_number is omitted deliberately: the AUTOINCREMENT
# primary key assigns it inside the insert, race-free under concurrent
# producers, and serves as the skew-proof drain-order tiebreaker. attempts and
# next_attempt_after are written by DR-A but consumed by DR-B; storing them now
# avoids a schema migration one slice later. The text fields are single-quoted
# with any embedded quote doubled; the values are non-secret and quote-free by
# construction (a hex request id, a base64 body, a fixed localhost url), and
# the escape keeps the statement injection-safe regardless.
_osquery_store_alert_row() {
  local occurrence_timestamp="$1" request_id="$2" url="$3" encoded_body="$4" created_at database_directory
  database_directory="$(dirname "$OSQUERY_UNDELIVERED_ALERTS_DB")"
  mkdir -p "$database_directory" 2>/dev/null || return 1
  chmod 700 "$database_directory" 2>/dev/null || true
  created_at="$(date -u +%s)"
  [[ $created_at =~ ^[0-9]+$ ]] || created_at=0
  local request_id_sql url_sql body_sql
  request_id_sql="${request_id//\'/\'\'}"
  url_sql="${url//\'/\'\'}"
  body_sql="${encoded_body//\'/\'\'}"
  if ! printf "BEGIN IMMEDIATE;
CREATE TABLE IF NOT EXISTS pending_alerts (
  sequence_number    INTEGER PRIMARY KEY AUTOINCREMENT,
  request_id         TEXT UNIQUE NOT NULL,
  occurrence_ts      INTEGER NOT NULL,
  url                TEXT NOT NULL,
  body_base64        TEXT NOT NULL,
  attempts           INTEGER NOT NULL DEFAULT 0,
  next_attempt_after INTEGER NOT NULL DEFAULT 0,
  created_at         INTEGER NOT NULL
);
INSERT INTO pending_alerts
    (request_id, occurrence_ts, url, body_base64, attempts, next_attempt_after, created_at)
  VALUES
    ('%s', %s, '%s', '%s', 0, 0, %s)
  ON CONFLICT(request_id) DO NOTHING;
COMMIT;\n" \
    "$request_id_sql" "$occurrence_timestamp" "$url_sql" "$body_sql" "$created_at" | _osquery_alerts_db_exec; then
    return 1
  fi
  [[ -f $OSQUERY_UNDELIVERED_ALERTS_DB ]] || return 1
  chmod 600 "$OSQUERY_UNDELIVERED_ALERTS_DB" 2>/dev/null || true
  return 0
}

# Record one FAILED-but-retryable delivery attempt on a pending row: attempts
# goes up by one and next_attempt_after moves into the future, so the drain
# leaves the row alone until its wait has passed instead of hammering a failing
# gateway on every tick. The wait grows with the attempt count (base seconds
# times the new attempt number; the base is overridable for tests, and DR-B T6
# adds a random wobble on top). The clock and the arithmetic both live inside
# the one UPDATE, so the read-modify-write is atomic under concurrent drains.
# A failed update is reported nonzero for the caller to log but never fail on:
# the row is still pending, and the only cost is a retry that comes sooner.
_osquery_record_transient_failure() {
  local request_id="$1" base="${OSQUERY_DRAIN_RETRY_BASE_SECONDS:-60}"
  [[ $base =~ ^[0-9]+$ ]] || base=60
  local request_id_sql="${request_id//\'/\'\'}"
  printf "UPDATE pending_alerts
  SET attempts = attempts + 1,
      next_attempt_after = CAST(strftime('%%s','now') AS INTEGER) + %s * (attempts + 1)
  WHERE request_id = '%s';\n" "$base" "$request_id_sql" | _osquery_alerts_db_exec
}

# Move one pending row into the dead_letter_alerts table, in ONE transaction:
# the insert and the delete commit together, so a crash at any instant leaves
# the record in exactly one of the two tables, never in neither. Used when
# delivery can never succeed (a permanent HTTP status; DR-B T4 adds the
# attempts/age thresholds). The table is bootstrapped lazily inside the same
# transaction, mirroring the pending_alerts bootstrap. The batch is idempotent
# (INSERT OR IGNORE plus a DELETE that fires only once the dead-letter copy
# exists), so the locked-database retry in _osquery_alerts_db_exec may replay
# it safely, and the delete can never remove a page the insert did not keep.
_osquery_dead_letter_alert_row() { # <request_id> <last_http_status> <reason>
  local request_id="$1" last_http_status="$2" reason="$3" dead_lettered_at
  dead_lettered_at="$(date -u +%s)"
  [[ $dead_lettered_at =~ ^[0-9]+$ ]] || dead_lettered_at=0
  local request_id_sql="${request_id//\'/\'\'}"
  local status_sql="${last_http_status//\'/\'\'}"
  local reason_sql="${reason//\'/\'\'}"
  printf "BEGIN IMMEDIATE;
CREATE TABLE IF NOT EXISTS dead_letter_alerts (
  sequence_number    INTEGER PRIMARY KEY,
  request_id         TEXT UNIQUE NOT NULL,
  occurrence_ts      INTEGER NOT NULL,
  url                TEXT NOT NULL,
  body_base64        TEXT NOT NULL,
  attempts           INTEGER NOT NULL,
  next_attempt_after INTEGER NOT NULL,
  created_at         INTEGER NOT NULL,
  dead_lettered_at   INTEGER NOT NULL,
  last_http_status   TEXT NOT NULL,
  reason             TEXT NOT NULL
);
INSERT OR IGNORE INTO dead_letter_alerts
    (sequence_number, request_id, occurrence_ts, url, body_base64,
     attempts, next_attempt_after, created_at,
     dead_lettered_at, last_http_status, reason)
  SELECT sequence_number, request_id, occurrence_ts, url, body_base64,
         attempts, next_attempt_after, created_at,
         %s, '%s', '%s'
    FROM pending_alerts WHERE request_id = '%s';
DELETE FROM pending_alerts
  WHERE request_id = '%s'
    AND request_id IN (SELECT request_id FROM dead_letter_alerts);
COMMIT;\n" \
    "$dead_lettered_at" "$status_sql" "$reason_sql" "$request_id_sql" "$request_id_sql" |
    _osquery_alerts_db_exec
}

# Delete one delivered page's pending_alerts row, keyed by request_id. Called
# ONLY after a confirmed 2xx (the write-ahead contract: the row is the page's
# durable copy until delivery is confirmed). A missing DB is a clean no-op
# (nothing was ever stored, so there is nothing to delete). A failed delete is
# reported nonzero for the caller to LOG but never to fail on: the page was
# delivered, and a leaked row costs one deduplicated re-post on a later drain
# (the gateway drops the duplicate by request_id), while failing the caller
# would wrongly report a delivered page as lost.
_osquery_delete_alert_row() {
  local request_id="$1"
  [[ -f $OSQUERY_UNDELIVERED_ALERTS_DB ]] || return 0
  local request_id_sql="${request_id//\'/\'\'}"
  printf "DELETE FROM pending_alerts WHERE request_id = '%s';\n" "$request_id_sql" | _osquery_alerts_db_exec
}

# Print every DUE pending row as one tab-separated line (request_id, url,
# body_base64), ordered for the drain: occurrence time first, then the
# insert-assigned sequence_number as the tiebreaker. The sequence tiebreaker is
# skew-proof: equal timestamps (or a backward clock step) still drain in insert
# order. A row whose next_attempt_after is still in the future is left out:
# its last attempt failed transiently and its retry wait has not passed yet, so
# the drain leaves it alone instead of hammering a failing gateway every tick.
# Tabs cannot appear in the fields (a hex-derived request id, a base64
# body, and a URL the localhost guard restricts), so the line format is safe.
_osquery_pending_alert_rows() {
  {
    printf '.separator "\\t"\n'
    printf "SELECT request_id, url, body_base64 FROM pending_alerts
  WHERE next_attempt_after <= CAST(strftime('%%s','now') AS INTEGER)
  ORDER BY occurrence_ts, sequence_number;\n"
  } | _osquery_alerts_db_exec
}

# Deliver ONE pending row: skip a non-localhost url (never send an off-box
# page), decode and sign the stored body, POST it (stored request_id verbatim so
# the gateway dedups), and delete the row only on a confirmed 2xx. Fully
# set -e-safe: a malformed row returns 0 so the drain continues, but NEVER
# silently: a MALFORMED-ROW log line names the stuck row and the reason, so a
# row the drain can never deliver is visible instead of invisible forever.
_deliver_pending_alert_row() {
  local request_id="$1" url="$2" encoded_body="$3" secret="$4" body signature http_status
  if [[ -z $request_id || -z $url || -z $encoded_body ]]; then
    _osquery_log "MALFORMED-ROW drain skipped an unreadable row: request_id=${request_id:-unknown} (a required field is empty; row retained)"
    return 0
  fi
  case "$url" in
    http://127.0.0.1:8644/*) ;;
    *) return 0 ;;
  esac
  if ! body="$(printf '%s' "$encoded_body" | base64 -d 2>/dev/null)" || [[ -z $body ]]; then
    _osquery_log "MALFORMED-ROW drain skipped an undecodable row: request_id=$request_id (body_base64 does not decode; row retained)"
    return 0
  fi
  # Same signing check as the send path: a failed or empty signature must never
  # go on the wire. The row stays put and a later drain retries it.
  if ! signature="$(printf '%s' "$body" | _hmac_sha256_hex "$secret")" || [[ -z $signature ]]; then
    return 0
  fi
  http_status="$(_post_alert_to_webhook "$url" "$request_id" "$signature" "$body")" || http_status=000
  case "$http_status" in
    2*)
      _osquery_delete_alert_row "$request_id" ||
        _osquery_log "ROW-DELETE-FAILED delivered page kept a pending row: request_id=$request_id (gateway dedups the re-post)"
      ;;
    401 | 403 | 404 | 413)
      # A permanent status: the gateway understood the request and refused it
      # outright, so no number of retries can ever succeed. Move the record to
      # the dead-letter table NOW, loudly, so the drain stops re-sending it
      # forever and the operator can still read the full page there.
      if _osquery_dead_letter_alert_row "$request_id" "$http_status" "permanent HTTP status $http_status"; then
        _osquery_log "DEAD-LETTERED request_id=$request_id http=$http_status (permanent status; moved to dead_letter_alerts)"
      else
        _osquery_log "DEAD-LETTER-FAILED request_id=$request_id http=$http_status (move failed; row retained in pending_alerts)"
      fi
      ;;
    *)
      # A transient failure (429, 5xx, or a transport error): count the
      # attempt and push this row's next try into the future, so the row waits
      # out its delay instead of being re-POSTed by every drain tick.
      _osquery_record_transient_failure "$request_id" ||
        _osquery_log "RETRY-BOOKKEEPING-FAILED request_id=$request_id (attempts not recorded; the row simply retries sooner)"
      ;;
  esac
  return 0
}

# Drain the SQLite store: deliver every pending row in occurrence-time order
# with the sequence tiebreaker. A query failure or an empty store is a quiet
# no-op; a malformed row is skipped. Never returns nonzero (set -e-safe).
_drain_pending_alert_rows() {
  local secret="$1" rows request_id url encoded_body
  rows="$(_osquery_pending_alert_rows)" || return 0
  [[ -n $rows ]] || return 0
  while IFS=$'\t' read -r request_id url encoded_body; do
    [[ -n $request_id ]] || continue
    _deliver_pending_alert_row "$request_id" "$url" "$encoded_body" "$secret"
  done <<<"$rows"
  return 0
}

# Store one undelivered page and REPORT persistence success. The row is keyed by
# the occurrence-unique request_id, so re-storing the same occurrence is
# idempotent while two DISTINCT occurrences never collide. The encoded body and
# the occurrence timestamp are VALIDATED in checked assignments BEFORE the row is
# written, so an encoder failure fails the store instead of persisting an empty
# body that a later drain would silently skip. RETURNS NONZERO on any persistence
# failure so the caller treats a failed store as a hard delivery failure (never
# "stored" when the row does not exist).
_store_undelivered_alert() {
  local occurrence_timestamp="$1" request_id="$2" url="$3" body="$4" encoded_body
  [[ $occurrence_timestamp =~ ^[0-9]+$ ]] || return 1
  [[ -n $request_id ]] || return 1
  # A URL carrying whitespace or a control character must never enter durable
  # storage: the drain's row export is tab-separated, so an embedded separator
  # would garble the row into an undeliverable, undiagnosed shape. Refusing at
  # persist time turns a misconfigured URL into the loud hard-fail instead of a
  # silently stuck row.
  [[ -n $url ]] || return 1
  case "$url" in
    *[[:space:][:cntrl:]]*) return 1 ;;
  esac
  # pipefail inside the subshell so a base64 failure is the assignment's status,
  # not masked by the trailing tr; then reject an empty result defensively.
  if ! encoded_body="$(
    set -o pipefail
    printf '%s' "$body" | base64 | tr -d '\n'
  )"; then
    return 1
  fi
  [[ -n $encoded_body ]] || return 1
  _osquery_store_alert_row "$occurrence_timestamp" "$request_id" "$url" "$encoded_body"
}

# Fire the ordinary local macOS notification for one alert (alerter, with an
# AppleScript fallback). The local notifier renders plain text, so Discord
# markdown (**bold**, `code`) is stripped first; the webhook body elsewhere
# keeps the markdown intact. --sound is passed only when a sound is given, so a
# silent tier stays visible but quiet. The notifier is backgrounded or its
# failure ignored: a broken notifier must never stall or fail dispatch.
_notify_locally() {
  local title="$1" detail="$2" sound="$3" plain_title plain_detail
  plain_title="$(printf '%s' "$title" | sed -e 's/\*\*//g' -e 's/`//g')"
  plain_detail="$(printf '%s' "$detail" | sed -e 's/\*\*//g' -e 's/`//g')"
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
}

# Fire ONE loud, interruptive local notification. Used when a page can be neither
# delivered NOR durably stored: the operator MUST learn that a CRITICAL alert was
# lost, because a dropped page leaves no trace and would otherwise read as good
# news. If no notifier is available or the notification itself fails, nothing
# further happens and the function still returns 0: the dispatch flow continues,
# already in its worst case, and must not break over a notifier problem.
_loud_local() {
  local title="$1" message="$2"
  if command -v alerter >/dev/null 2>&1; then
    alerter --timeout 60 --title "$title" --message "$message" --sound Funk >/dev/null 2>&1 &
  else
    local escaped=${message//\"/\\\"}
    osascript -e "display notification \"$escaped\" with title \"$title\" sound name \"Funk\"" >/dev/null 2>&1 || true
  fi
}

# Print the webhook signing key: the environment override when set, otherwise
# the first line of the notifier's own secret file with any carriage return
# stripped (so a CRLF file cannot corrupt the key). Prints nothing when no key
# is available; the caller decides what an absent key means for its path.
_read_webhook_secret() {
  local secret="${OSQUERY_WEBHOOK_SECRET:-}"
  if [[ -z $secret && -r $OSQUERY_WEBHOOK_SECRET_FILE ]]; then
    IFS= read -r secret <"$OSQUERY_WEBHOOK_SECRET_FILE" || true
    secret="$(printf '%s' "$secret" | tr -d '\r')"
  fi
  printf '%s' "$secret"
}

# The one place a webhook POST is made. Prints the HTTP status code; a transport
# failure (no connection, timeout) makes curl exit nonzero, which this function
# passes through for the caller to map to status 000. Every delivery path goes
# through here so the request shape (method, headers, timeout) lives in one spot.
_post_alert_to_webhook() { # <url> <request_id> <signature> <body>
  curl -s -o /dev/null -w '%{http_code}' --max-time 5 \
    -X POST "$1" \
    -H 'Content-Type: application/json' \
    -H "X-Webhook-Signature: $3" \
    -H "X-Request-ID: $2" \
    --data "$4"
}

# Replay stored undelivered pages in OCCURRENCE-TIME order, so a backlog
# delivers in the order events happened (ORDER BY occurrence_ts,
# sequence_number, skew-proof). A missing database means nothing was ever
# stored, a quiet no-op. Localhost only. Fully set -e-safe: a malformed entry or
# an empty store must NEVER abort the caller (a delivery feature must not cause
# a detection outage).
retry_undelivered_alerts() {
  [[ -f $OSQUERY_UNDELIVERED_ALERTS_DB ]] || return 0
  local secret
  secret="$(_read_webhook_secret)"
  [[ -n $secret ]] || return 0
  _drain_pending_alert_rows "$secret"
  return 0
}

# Build the signed webhook body for one CRIT page and print it. tier: a page is
# loud (a sound was requested), a digest/heartbeat is muted (no sound). Both
# POST; tier lets the Hermes adapter suppress the notification for muted traffic
# instead of pinging it like a page. The host and the occurrence timestamp live
# INSIDE the signed body, the spec's body shape and the multi-host migration
# seam both require {event_type, host, tier, ts, alert} (ts is the wire name of
# the occurrence timestamp).
_build_webhook_body() { # <title> <detail> <tier> <occurrence_timestamp>
  jq -cn --arg h "$(hostname -s)" --arg t "$1" --arg d "$2" \
    --arg tier "$3" --argjson ts "$4" \
    '{event_type:"osquery.alert", host:$h, tier:$tier, ts:$ts, alert:{title:$t, detail:$d}}'
}

# Derive and print the occurrence-stable request id from the seed. Two distinct
# incidents that render the same body get distinct ids (both stored, both
# delivered) while a retry of the same occurrence reuses one id (the gateway
# dedups it, and the stored filename is idempotent).
_derive_request_id() { # <request_id_seed>
  printf 'osquery-%s' "$(printf '%s' "$1" | _sha256_hex_of_stdin | cut -c1-32)"
}

# Sign the body and POST it, retrying a transient failure (429, 5xx, or a
# transport error) up to three times with growing backoff. Prints the final HTTP
# status and returns 0 exactly when a 2xx confirmed delivery; any other outcome
# returns 1 with the failing status printed. A non-transient status (401, 413,
# and the like) stops early, a retry cannot fix it.
_attempt_alert_delivery() { # <url> <request_id> <secret> <body>
  local url="$1" request_id="$2" secret="$3" body="$4" signature http_status attempt
  # This function runs inside an if condition, where errexit is suppressed, so
  # a failing signature assignment must be checked explicitly: unchecked, the
  # code would continue with an EMPTY signature, POST it, and a 2xx would
  # delete the write-ahead record. A failed or empty signature stops the
  # attempt BEFORE any POST; the record stays for a later drain.
  if ! signature="$(printf '%s' "$body" | _hmac_sha256_hex "$secret")" || [[ -z $signature ]]; then
    printf 'signing-failed'
    return 1
  fi
  for attempt in 1 2 3; do
    http_status="$(_post_alert_to_webhook "$url" "$request_id" "$signature" "$body")" || http_status=000
    case "$http_status" in
      2*)
        printf '%s' "$http_status"
        return 0
        ;;
      429 | 5?? | 000) # transient, back off and retry (base overridable for tests)
        if [[ $attempt -lt 3 ]]; then sleep "$((attempt * ${OSQUERY_RETRY_BACKOFF_BASE:-1}))"; fi ;;
      *) break ;; # 401/413/etc, retry won't help
    esac
  done
  printf '%s' "$http_status"
  return 1
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

  _notify_locally "$title" "$detail" "$sound"

  # Only a CRIT page is delivered to Discord. Any other severity stops here,
  # after the local notification.
  [[ $severity == "CRIT" ]] || return 0
  local url="$OSQUERY_HERMES_PRIORITY_URL"

  # Occurrence time: when this page was raised. It is the drain's ordering key AND
  # part of the signed body. A clock glitch never blocks a page (fall back to 0).
  local occurrence_timestamp
  occurrence_timestamp="$(date -u +%s)"
  [[ $occurrence_timestamp =~ ^[0-9]+$ ]] || occurrence_timestamp=0

  # The body, and the request id from OCCURRENCE IDENTITY (threaded from the
  # caller) when present, a per-call unique seed otherwise. The sequence
  # increment happens here, not inside a command substitution, so it survives
  # the call and keeps fallback ids unique across calls in one process.
  local tier body request_id request_id_seed
  if [[ -n $sound ]]; then tier="page"; else tier="muted"; fi
  body="$(_build_webhook_body "$title" "$detail" "$tier" "$occurrence_timestamp")"
  if [[ -n $occurrence ]]; then
    request_id_seed="$occurrence"
  else
    _OSQUERY_ALERT_SEQUENCE=$((_OSQUERY_ALERT_SEQUENCE + 1))
    request_id_seed="fallback|$occurrence_timestamp|$$|${_OSQUERY_ALERT_SEQUENCE}|${RANDOM}|$body"
  fi
  request_id="$(_derive_request_id "$request_id_seed")"

  # WRITE-AHEAD: persist the page BEFORE the first network attempt, so a crash or
  # kill before a confirmed success leaves a recoverable record. A persist failure
  # is a HARD failure (loud + nonzero): the page can be neither delivered nor
  # stored, and the caller must not advance its cursor past it.
  if ! _store_undelivered_alert "$occurrence_timestamp" "$request_id" "$url" "$body"; then
    _osquery_log "STORE-FAILED write-ahead persist: request_id=$request_id (storage unwritable: $OSQUERY_UNDELIVERED_ALERTS_DB)"
    _loud_local "osquery paging FAILED, page LOST" \
      "The page could not be stored locally. Fix $OSQUERY_UNDELIVERED_ALERTS_DB."
    return 1
  fi

  # Deliver via the hermes webhook (the page is already safe on disk, so a
  # failed delivery here costs latency, not the page). No key means no send: the
  # drain signs and sends the stored page once the secret returns.
  local secret
  secret="$(_read_webhook_secret)"
  if [[ -z $secret ]]; then
    _osquery_log "STORED-NOSECRET Discord delivery degraded: request_id=$request_id (no secret in $OSQUERY_WEBHOOK_SECRET_FILE)"
    _loud_local "osquery Discord paging BROKEN" \
      "No webhook secret. This page was stored locally and delivers when the secret is restored."
    return 0
  fi

  local http_status
  if http_status="$(_attempt_alert_delivery "$url" "$request_id" "$secret" "$body")"; then
    # Delete ONLY after a confirmed 2xx. A failed row delete is logged, not
    # fatal: the page was delivered, and the gateway dedups a leaked row's
    # re-post by request_id.
    _osquery_delete_alert_row "$request_id" ||
      _osquery_log "ROW-DELETE-FAILED delivered page kept a pending row: request_id=$request_id (gateway dedups the re-post)"
    return 0
  fi
  # Delivery failed after retries: the write-ahead row REMAINS in the store and
  # the next drain retries it, so send_alert still returns 0. A down gateway
  # delays the page; it never fails the caller and never loses the page.
  _osquery_log "STORED webhook delivery pending: request_id=$request_id http=$http_status (retained for retry)"
  return 0
}
