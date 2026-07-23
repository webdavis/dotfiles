#!/usr/bin/env bats
# The daily digest builder (digest.sh): drains the digest spool (NDJSON, written
# by the alerter's digest_append) into ONE grouped, silent, non-paging message,
# then rotates the live store aside. This suite exercises the builder as a black
# box against a stubbed dispatch: a message-recording spy replaces the real
# send_alert, so a test asserts whether (and how) the builder dispatched without
# touching the network or the real SQLite store.
#
# Behaviors covered so far:
#   B1 empty-suppression: an absent, zero-byte, or whitespace-only store produces
#      no message and no error.
#   B2 atomic rotate + ERR-restore: a store with real records is claimed into a
#      unique work file (freeing the live store for concurrent appends), and a
#      build failure BEFORE the send restores the batch so nothing is lost.
#   B3 grouped, capped, injection-safe body: findings group by detector into
#      capped Discord-safe blocks, and a crafted field cannot inject extra lines.
# The silent send and the rotation to .last land in later behaviors.

setup() { setup_digest_harness; }
teardown() { teardown_digest_harness; }

# setup_digest_harness (makeSUT factory) - stand up a throwaway HOME whose only
# dispatch library is a recording spy, point the builder at a temp spool path,
# and export the inputs the builder reads. Sets nothing at file-load time; every
# export happens here, called from setup().
setup_digest_harness() {
  HARNESS_HOME="$(mktemp -d)"
  # Record ownership only after our own mktemp, so teardown removes this path and
  # never a pre-set or inherited HARNESS_HOME.
  _DIGEST_HARNESS_OWNED_DIR="$HARNESS_HOME"
  export HOME="$HARNESS_HOME"

  # The recording spy for send_alert, at the exact libexec path the builder sources.
  # It writes one CALL marker per call to $SEND_ALERT_LOG (so a test counts calls and
  # "no dispatch" is an empty log) plus the severity/title/body/sound of the call, so
  # a test can assert HOW the builder dispatched without a real send. SEND_ALERT_RC
  # (default 0) lets a test force a hard send failure to exercise fire-and-forget.
  local dispatch_dir="$HARNESS_HOME/.local/libexec/osquery"
  mkdir -p "$dispatch_dir"
  export SEND_ALERT_LOG="$HARNESS_HOME/send-alert.log"
  export SEND_ALERT_SEVERITY="$HARNESS_HOME/send-alert.severity"
  export SEND_ALERT_TITLE="$HARNESS_HOME/send-alert.title"
  export SEND_ALERT_BODY="$HARNESS_HOME/send-alert.body"
  export SEND_ALERT_SOUND="$HARNESS_HOME/send-alert.sound"
  : >"$SEND_ALERT_LOG"
  cat >"$dispatch_dir/alert-dispatch.sh" <<'SPY'
# Recording spy for alert-dispatch.sh: capture each send_alert call so a test can
# assert whether, and how, the builder dispatched without a real send. One CALL
# marker per call (for counting) plus the severity/title/body/sound of the call.
# SEND_ALERT_RC (default 0) lets a test force a hard send failure.
send_alert() {
  printf 'CALL\n' >>"$SEND_ALERT_LOG"
  printf '%s' "${1-}" >"$SEND_ALERT_SEVERITY"
  printf '%s' "${2-}" >"$SEND_ALERT_TITLE"
  printf '%s' "${3-}" >"$SEND_ALERT_BODY"
  printf '%s' "${4-}" >"$SEND_ALERT_SOUND"
  return "${SEND_ALERT_RC:-0}"
}
SPY

  # A temp spool path the builder resolves via OSQUERY_DIGEST_STORE. Left ABSENT
  # by default so a test opts in to a zero-byte, whitespace-only, or seeded store.
  export OSQUERY_DIGEST_STORE="$HARNESS_HOME/.local/state/osquery-digest-spool/digest.ndjson"

  # A witness the fault-injection driver writes when the build step runs against
  # the CLAIMED (rotated) batch: it proves the rotate happened before the build.
  export DIGEST_BUILD_WITNESS="$HARNESS_HOME/build-witness"

  # Exported so the fault-injection driver (a child bash) can source the builder.
  export DIGEST_BUILDER="${BATS_TEST_DIRNAME}/../../dot_local/libexec/osquery/executable_digest.sh"
}

# teardown_digest_harness - remove ONLY a temp dir this harness created. The
# ownership marker is set after our own mktemp, so a pre-set HARNESS_HOME (marker
# unset) is left untouched.
teardown_digest_harness() {
  [[ -n ${_DIGEST_HARNESS_OWNED_DIR:-} ]] || return 0
  rm -rf "$_DIGEST_HARNESS_OWNED_DIR"
  unset _DIGEST_HARNESS_OWNED_DIR
}

# run_digest - invoke the builder as a child process under the harness env.
run_digest() { bash "$DIGEST_BUILDER"; }

# run_digest_with_failing_build - drive the builder to a forced PRE-SEND failure.
# A child bash sources the builder (its source-guard keeps main from auto-running),
# overrides the build step to fail (after witnessing that the batch was already
# claimed into the work file), then runs main. Sourcing is why the builder splits
# main from a source-guard: it is the seam that lets a test fault-inject one step.
run_digest_with_failing_build() {
  bash -c '
    source "$DIGEST_BUILDER"
    render_digest_body() {
      local work_file="$1"
      [[ -f $work_file ]] && printf "claimed\n" >"$DIGEST_BUILD_WITNESS"
      return 1
    }
    main
  ' digest-build-fault-injector
}

# digest_record <detector> <identity> <summary> - one NDJSON spool line in the
# shape digest_append writes (results-alerter/digest-store.sh), so the builder
# reads records identical to production.
digest_record() {
  jq -cn --arg detector "$1" --arg identity "$2" --arg summary "$3" \
    '{timestamp: "2026-07-18T00:00:00Z", detector: $detector, category: "", identity: $identity, action: "added", summary: $summary}'
}

# seed_store <record>... - write the given NDJSON records to the live store.
seed_store() {
  mkdir -p "$(dirname "$OSQUERY_DIGEST_STORE")"
  printf '%s\n' "$@" >"$OSQUERY_DIGEST_STORE"
}

# count_records <file> - number of non-blank lines (records) in a spool file.
count_records() { grep -c '[^[:space:]]' "$1" 2>/dev/null || printf '0'; }

# given_absent_store - the spool file does not exist (the default, made explicit).
given_absent_store() { rm -f "$OSQUERY_DIGEST_STORE"; }

# given_empty_store - a zero-byte spool file.
given_empty_store() {
  mkdir -p "$(dirname "$OSQUERY_DIGEST_STORE")"
  : >"$OSQUERY_DIGEST_STORE"
}

# given_whitespace_only_store - a spool with bytes but no non-whitespace content.
given_whitespace_only_store() {
  mkdir -p "$(dirname "$OSQUERY_DIGEST_STORE")"
  printf ' \t\n  \n' >"$OSQUERY_DIGEST_STORE"
}

# assert_no_send - the recording spy captured no send_alert call.
assert_no_send() {
  if [[ -s $SEND_ALERT_LOG ]]; then
    printf 'expected NO dispatch, but send_alert was called: %s\n' \
      "$(cat "$SEND_ALERT_LOG")" >&2
    return 1
  fi
}

# assert_silent_success - the B1 behavior in one intent-named assertion: the
# builder exits 0 AND sends nothing.
assert_silent_success() {
  run run_digest
  if [[ $status -ne 0 ]]; then
    printf 'expected the builder to exit 0 (silent success), got %s: %s\n' "$status" "$output" >&2
    return 1
  fi
  assert_no_send
}

# assert_live_store_freed - the live store was rotated aside, so a concurrent
# alerter append lands in a fresh file this run will not consume.
assert_live_store_freed() {
  if [[ -s $OSQUERY_DIGEST_STORE ]]; then
    printf 'expected the live store freed (rotated aside), but it still holds: %s\n' \
      "$(cat "$OSQUERY_DIGEST_STORE")" >&2
    return 1
  fi
}

# assert_build_ran_against_work_file - the build step ran against the CLAIMED
# batch, proving the rotate happened before the build (not against the live store).
assert_build_ran_against_work_file() {
  local witness
  witness="$(cat "$DIGEST_BUILD_WITNESS" 2>/dev/null || true)"
  if [[ $witness != claimed ]]; then
    printf 'expected the build step to run against the rotated work file (batch claimed first), witness=%q\n' "$witness" >&2
    return 1
  fi
}

# assert_live_store_restored <n> - the batch is back as the live store with <n>
# records, so the next daily run retries it.
assert_live_store_restored() {
  local want="$1" got
  got="$(count_records "$OSQUERY_DIGEST_STORE")"
  if [[ ! -s $OSQUERY_DIGEST_STORE || $got -ne $want ]]; then
    printf 'expected the batch restored to the live store with %s record(s), got %s (store present=%s)\n' \
      "$want" "$got" "$([[ -e $OSQUERY_DIGEST_STORE ]] && echo yes || echo no)" >&2
    return 1
  fi
}

# assert_no_work_file_left - no .build work file remains (the restore moved it back).
assert_no_work_file_left() {
  local leftovers=("$OSQUERY_DIGEST_STORE".*.build)
  if [[ -e ${leftovers[0]} ]]; then
    printf 'expected no .build work file after restore, found: %s\n' "${leftovers[*]}" >&2
    return 1
  fi
}

# render_digest_body_on <work_file> - source the builder (its source-guard keeps
# main from running) and call the build step directly on a fixture work file,
# printing the rendered body. The unit seam for the body render: B3 has no send
# yet, so a test reads the body from stdout.
render_digest_body_on() {
  bash -c 'source "$DIGEST_BUILDER"; render_digest_body "$1"' digest-render-probe "$1"
}

# render_body <record>... - build a fixture work file from the given NDJSON
# records and print the rendered digest body.
render_body() {
  local work_file="$HARNESS_HOME/fixture.build"
  printf '%s\n' "$@" >"$work_file"
  render_digest_body_on "$work_file"
}

# assert_body_has <body> <fixed-needle> - the body contains the needle.
assert_body_has() {
  local body="$1" needle="$2"
  if ! grep -qF -- "$needle" <<<"$body"; then
    printf 'expected the digest body to contain %q, body was:\n%s\n' "$needle" "$body" >&2
    return 1
  fi
}

# assert_line_count <body> <regex> <n> - exactly <n> body lines match the regex.
assert_line_count() {
  local body="$1" regex="$2" want="$3" got
  got="$(grep -cE -- "$regex" <<<"$body" || true)"
  if [[ $got -ne $want ]]; then
    printf 'expected %s line(s) matching /%s/, got %s, body was:\n%s\n' "$want" "$regex" "$got" "$body" >&2
    return 1
  fi
}

# assert_no_injected_line <body> - no body line BEGINS with a forged field marker.
# A sanitized field keeps crafted "- **Signing:**" text inline within its own
# bullet; only a sanitize regression would push it to the start of a new line.
assert_no_injected_line() {
  local body="$1"
  if grep -qE '^- \*\*Signing:\*\*' <<<"$body"; then
    printf 'INJECTION: a body line begins with a forged "- **Signing:**" marker:\n%s\n' "$body" >&2
    return 1
  fi
}

# body_byte_length <body> - the body length in bytes (the head -c cap's unit).
body_byte_length() { printf '%s' "$1" | wc -c | tr -d '[:space:]'; }

# assert_sent_once - exactly one send_alert call was recorded.
assert_sent_once() {
  local calls
  calls="$(grep -c 'CALL' "$SEND_ALERT_LOG" 2>/dev/null || printf '0')"
  if [[ $calls -ne 1 ]]; then
    printf 'expected exactly one send_alert call, got %s\n' "$calls" >&2
    return 1
  fi
}

# assert_sent_silent_crit - the recorded send is CRIT (selects the #priority route)
# with an EMPTY sound (silent/muted -> tier=muted, so Hermes suppresses the ping).
assert_sent_silent_crit() {
  local severity sound
  severity="$(cat "$SEND_ALERT_SEVERITY" 2>/dev/null || true)"
  sound="$(cat "$SEND_ALERT_SOUND" 2>/dev/null || true)"
  if [[ $severity != CRIT ]]; then
    printf 'expected CRIT severity (selects #priority), got %q\n' "$severity" >&2
    return 1
  fi
  if [[ -n $sound ]]; then
    printf 'expected an EMPTY sound (silent/muted, not a page ping), got %q\n' "$sound" >&2
    return 1
  fi
}

# assert_sent_body_has <fixed-needle> - the recorded send body contains the needle.
assert_sent_body_has() {
  local needle="$1"
  if ! grep -qF -- "$needle" "$SEND_ALERT_BODY" 2>/dev/null; then
    printf 'expected the sent body to contain %q, body was:\n%s\n' "$needle" "$(cat "$SEND_ALERT_BODY" 2>/dev/null || true)" >&2
    return 1
  fi
}

# assert_sent_title_has <fixed-needle> - the recorded send title contains the needle.
assert_sent_title_has() {
  local needle="$1"
  if ! grep -qF -- "$needle" "$SEND_ALERT_TITLE" 2>/dev/null; then
    printf 'expected the sent title to contain %q, title was:\n%s\n' "$needle" "$(cat "$SEND_ALERT_TITLE" 2>/dev/null || true)" >&2
    return 1
  fi
}

# assert_batch_in_last <n> - the built batch was preserved as $store.last with <n> records.
assert_batch_in_last() {
  local want="$1" got
  got="$(count_records "$OSQUERY_DIGEST_STORE.last")"
  if [[ ! -s $OSQUERY_DIGEST_STORE.last || $got -ne $want ]]; then
    printf 'expected the batch preserved to .last with %s record(s), got %s\n' "$want" "$got" >&2
    return 1
  fi
}

# assert_last_mode_600 - the .last forensic file is mode 600 (it holds full paths).
# GNU stat first (the nix shell), BSD stat as the fallback (the portable order).
assert_last_mode_600() {
  local mode
  mode=$(stat -c '%a' "$OSQUERY_DIGEST_STORE.last" 2>/dev/null || stat -f '%Lp' "$OSQUERY_DIGEST_STORE.last" 2>/dev/null)
  if [[ $mode != 600 ]]; then
    printf 'expected .last mode 600 (holds full paths), got %s\n' "$mode" >&2
    return 1
  fi
}

@test "an absent digest store produces no message and exits 0" {
  given_absent_store
  assert_silent_success
}

@test "a zero-byte digest store produces no message and exits 0" {
  given_empty_store
  assert_silent_success
}

@test "a whitespace-only digest store produces no message and exits 0" {
  given_whitespace_only_store
  assert_silent_success
}

@test "a store with records sends exactly one silent digest, then rotates the batch to .last" {
  seed_store \
    "$(digest_record persistence_launchd com.foo.agent 'persistence_launchd com.foo.agent')" \
    "$(digest_record sudoers /etc/sudoers.d/foo 'sudoers /etc/sudoers.d/foo')"
  run run_digest
  if [[ $status -ne 0 ]]; then
    printf 'expected exit 0, got %s: %s\n' "$status" "$output" >&2
    return 1
  fi
  assert_sent_once
  assert_sent_silent_crit             # CRIT route + EMPTY sound => tier=muted (non-paging)
  assert_sent_title_has '· 2 item(s)' # the title carries the true item count (2 records)
  assert_sent_body_has '**persistence_launchd** (1)'
  assert_sent_body_has '**sudoers** (1)'
  assert_live_store_freed  # the live store is fresh for the next run
  assert_batch_in_last 2   # the built batch is preserved for forensics
  assert_no_work_file_left # the .build is cleaned on the success path (no mv->cp leak)
  assert_last_mode_600
}

@test "a build failure before the send restores the rotated batch to the live store" {
  seed_store \
    "$(digest_record sudoers /etc/sudoers.d/foo 'sudoers /etc/sudoers.d/foo')" \
    "$(digest_record sudoers /etc/sudoers.d/bar 'sudoers /etc/sudoers.d/bar')"
  run run_digest_with_failing_build
  if [[ $status -eq 0 ]]; then
    printf 'expected a nonzero exit from the forced pre-send build failure, got 0\n' >&2
    return 1
  fi
  assert_build_ran_against_work_file
  assert_live_store_restored 2
  assert_no_work_file_left
  assert_no_send
}

@test "findings across three detectors render as three grouped blocks with header and count" {
  local body
  body="$(render_body \
    "$(digest_record persistence_launchd com.foo.agent 'persistence_launchd com.foo.agent')" \
    "$(digest_record persistence_launchd com.bar.agent 'persistence_launchd com.bar.agent')" \
    "$(digest_record system_extensions_new io.tailscale 'system_extensions_new io.tailscale')" \
    "$(digest_record sudoers /etc/sudoers.d/foo 'sudoers /etc/sudoers.d/foo')")"
  assert_body_has "$body" '**persistence_launchd** (2)'
  assert_body_has "$body" '**system_extensions_new** (1)'
  assert_body_has "$body" '**sudoers** (1)'
  assert_body_has "$body" '- `com.foo.agent` - `persistence_launchd com.foo.agent`'
  assert_body_has "$body" '- `io.tailscale` - `system_extensions_new io.tailscale`'
}

@test "a detector with more findings than the bullet cap shows N bullets and a +K more roll-up" {
  local records=() i
  for i in $(seq 1 14); do
    records+=("$(digest_record persistence_launchd "com.item.$i" "summary $i")")
  done
  local body
  body="$(render_body "${records[@]}")"
  assert_body_has "$body" '**persistence_launchd** (14)' # the header counts the true total
  assert_line_count "$body" '^- `com\.item\.' 10         # DIGEST_MAX_BULLETS_PER_GROUP default
  assert_body_has "$body" '+4 more'
}

@test "more detector groups than the group cap show M blocks and an and-K-more marker" {
  local records=() i
  for i in $(seq 1 15); do
    records+=("$(digest_record "detector_$i" "id_$i" "summary $i")")
  done
  local body
  body="$(render_body "${records[@]}")"
  assert_line_count "$body" '^\*\*detector_' 12 # DIGEST_MAX_GROUPS default
  assert_body_has "$body" 'and 3 more detector group(s)'
}

@test "the body stays under the char cap even with many findings" {
  local records=() i
  for i in $(seq 1 150); do
    records+=("$(printf '{"timestamp":"t","detector":"det_%s","category":"","identity":"identity_number_%s","action":"added","summary":"a summary long enough to add real bytes for finding number %s"}' "$((i % 15))" "$i" "$i")")
  done
  # Default cap: the body renders content yet stays well under Discord's 2000.
  local body bytes
  body="$(render_body "${records[@]}")"
  assert_body_has "$body" '**det_'
  bytes="$(body_byte_length "$body")"
  if [[ $bytes -gt 1800 ]]; then
    printf 'expected body <= the 1800 default cap (well under 2000), got %s\n' "$bytes" >&2
    return 1
  fi
  # Overridable: a tighter cap is honored, proving the hard head -c backstop and the knob.
  export DIGEST_MAX_BODY_CHARS=500
  body="$(render_body "${records[@]}")"
  bytes="$(body_byte_length "$body")"
  if [[ $bytes -gt 500 ]]; then
    printf 'expected body <= the 500 override cap, got %s\n' "$bytes" >&2
    return 1
  fi
}

@test "the group and bullet caps are env-overridable named constants" {
  local records=() i
  for i in $(seq 1 6); do
    records+=("$(digest_record "det_$i" "id_$i" "summary $i")")
  done
  for i in $(seq 1 4); do
    records+=("$(digest_record det_1 "extra_$i" "extra $i")") # det_1 gets five findings total
  done
  export DIGEST_MAX_GROUPS=2 DIGEST_MAX_BULLETS_PER_GROUP=3
  local body
  body="$(render_body "${records[@]}")"
  assert_line_count "$body" '^\*\*det_' 2 # DIGEST_MAX_GROUPS honored
  assert_body_has "$body" 'and 4 more detector group(s)'
  assert_body_has "$body" '+2 more' # det_1: 5 findings, 3 bullets + "+2 more" (DIGEST_MAX_BULLETS_PER_GROUP)
}

@test "a crafted identity cannot inject an extra markdown line into the digest body" {
  local evil
  evil=$'evil\n- **Signing:** signed: Apple'
  local body
  body="$(render_body "$(digest_record persistence_launchd "$evil" 'malicious finding')")"
  # The crafted newline is squashed to a space, so the value stays inert INSIDE one bullet.
  assert_body_has "$body" '- `evil - **Signing:** signed: Apple` - `malicious finding`'
  # And the forged field marker never becomes its own line.
  assert_no_injected_line "$body"
}

@test "an attacker-controlled field renders inside a code span, so a mention or link is inert" {
  # render-page wraps every attacker-influenceable field in backticks; the digest does the
  # same, so a crafted mention or link renders as literal inline-code text, not a live
  # Discord @everyone or a clickable link. (The line/block-forging guard above is separate.)
  local body
  body="$(render_body "$(digest_record persistence_launchd '@everyone' '[click](http://evil.example)')")"
  assert_body_has "$body" '- `@everyone` - `[click](http://evil.example)`' # both fields inside code spans
  assert_body_has "$body" '`@everyone`'                                     # the mention is inert inline code, not bare
  assert_body_has "$body" '`[click](http://evil.example)`'                  # the link markdown is inert inline code too
}

@test "an oversized field is truncated in the sanitize chokepoint and cannot crowd out other groups" {
  local giant
  giant="$(printf 'x%.0s' {1..5000})" # one field far larger than the whole body cap
  local body
  body="$(render_body \
    "$(digest_record aaa_giant id_giant "$giant")" \
    "$(digest_record zzz_small id_small 'a small summary')")"
  # The oversized field is truncated in place with the per-field marker (DIGEST_MAX_FIELD_CHARS).
  assert_body_has "$body" '…(truncated)'
  # ... so it cannot alone consume the whole body cap: the later detector group still renders.
  assert_body_has "$body" '**zzz_small** (1)'
  assert_body_has "$body" '- `id_small` - `a small summary`'
  # And the full oversized value never survives into the body.
  if grep -qF -- "$giant" <<<"$body"; then
    printf 'expected the oversized field truncated, but the full value survived\n' >&2
    return 1
  fi
}

@test "a torn or malformed spool line is skipped, so the day's digest still builds" {
  local records=(
    "$(digest_record persistence_launchd com.good.one 'persistence_launchd com.good.one')"
    '{"detector":"persistence_launchd","identity":"com.tor' # a truncated (torn) append
    'this is not json at all'                               # non-JSON garbage
    "$(digest_record persistence_launchd com.good.two 'persistence_launchd com.good.two')"
    "$(digest_record sudoers /etc/sudoers.d/foo 'sudoers /etc/sudoers.d/foo')"
  )
  # The parse drops the torn and garbage lines; the valid findings still group and render.
  local body
  body="$(render_body "${records[@]}")"
  assert_body_has "$body" '**persistence_launchd** (2)' # the two GOOD launchd findings; the torn one skipped
  assert_body_has "$body" '- `com.good.one` - `persistence_launchd com.good.one`'
  assert_body_has "$body" '- `com.good.two` - `persistence_launchd com.good.two`'
  assert_body_has "$body" '**sudoers** (1)'
  if grep -qF -- 'com.tor' <<<"$body"; then
    printf 'expected the torn line skipped, but its fragment appeared:\n%s\n' "$body" >&2
    return 1
  fi
  # The FULL builder survives the torn line: it exits 0 (no set -e abort), so the B2 ERR
  # trap never restores the batch and the digest is not silently lost until the line ages out.
  seed_store "${records[@]}"
  run run_digest
  if [[ $status -ne 0 ]]; then
    printf 'expected the build to survive the torn line (exit 0), got %s: %s\n' "$status" "$output" >&2
    return 1
  fi
  assert_live_store_freed
}

@test "a failed send still rotates the batch to .last and never restores it for a re-send" {
  seed_store "$(digest_record persistence_launchd com.foo.agent 'persistence_launchd com.foo.agent')"
  export SEND_ALERT_RC=1 # the spy simulates a HARD send failure
  run run_digest
  if [[ $status -ne 0 ]]; then
    printf 'expected fire-and-forget exit 0 despite the failed send, got %s: %s\n' "$status" "$output" >&2
    return 1
  fi
  assert_sent_once        # exactly one send ATTEMPT, no restore-and-retry loop
  assert_batch_in_last 1  # preserved to .last, NOT restored to the live store
  assert_live_store_freed # durability lives in send_alert's write-ahead store, not a batch restore
  # A second run finds a fresh (empty) store and is silent: this batch is never re-sent.
  : >"$SEND_ALERT_LOG"
  unset SEND_ALERT_RC
  run run_digest
  if [[ $status -ne 0 ]]; then
    printf 'expected the second run silent (exit 0), got %s\n' "$status" >&2
    return 1
  fi
  assert_no_send
}

@test "an all-torn store renders an empty body, so it sends nothing and preserves the batch to .last" {
  # Every line unparseable: Guard 2 (non-whitespace bytes) passes and the raw lines are
  # counted, but the rendered body is empty. The builder must NOT send a misleading
  # silent "N item(s)" with an empty body; it preserves the batch to .last and stays silent.
  seed_store '{"detector":"persistence_launchd","identity":"x' '{"oops'
  run run_digest
  if [[ $status -ne 0 ]]; then
    printf 'expected exit 0, got %s: %s\n' "$status" "$output" >&2
    return 1
  fi
  assert_no_send
  assert_batch_in_last 2  # the unrecoverable batch is preserved for forensics
  assert_live_store_freed
}
