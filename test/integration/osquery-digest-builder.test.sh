#!/usr/bin/env bash
# Every single-quoted literal below is an EXPECTED string, not a command: the
# digest renders markdown, and a markdown code span is written in backticks, so
# SC2016 fires on each expected bullet while there is nothing here that should
# ever expand. Disabled for the file rather than on fifteen separate lines. (The
# .bats file this replaces was never shellchecked; a .test.sh is.)
# shellcheck disable=SC2016
#
# The daily digest builder (digest.sh): drains the digest spool (NDJSON, written
# by the alerter's digest_append) into ONE grouped, silent, non-paging message,
# then rotates the live store aside. This suite exercises the builder as a black
# box against a stubbed dispatch: a message-recording spy replaces the real
# send_alert, so a test asserts whether (and how) the builder dispatched without
# touching the network or the real SQLite store.
#
# Covered end to end: empty-suppression (absent/zero-byte/whitespace/all-torn); the atomic claim
# and append-restore (a build failure or a hard send failure restores the batch, preserving a
# finding appended during the build); grouped, capped, injection-safe rendering (each attacker field
# wrapped in a code span, four env-overridable caps with a non-numeric fallback, a codepoint body cap
# with an honest marker); torn-line and valid-JSON wrong-shape resilience; the silent tier=muted send
# with restore on a hard failure and rotation to .last on a stored one; and orphan recovery of a
# killed run's work file. The launchd schedule wiring is covered by the sibling launchagent test.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit: both belong to a script that runs on its own, and either one
# here would reach into the runner's own shell. The shebang stays for shellcheck
# and for editors, and is never executed. test/validate-tests.sh pins that shape;
# `just test-integration` runs it.
#
# Every check below is a real bashunit assertion. bashunit runs each test function
# under `set +euo pipefail`, so the bats file's helpers that merely `return 1` on
# failure would report nothing and pass silently; each one is now written as an
# assertion instead. Exit codes read through the bare `command; assert_*_code`
# form, which is exact precisely because errexit is off.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

function set_up() { set_up_digest_harness; }
function tear_down() { tear_down_digest_harness; }

# set_up_digest_harness (makeSUT factory) - stand up a throwaway HOME whose only
# dispatch library is a recording spy, point the builder at a temp spool path,
# and export the inputs the builder reads. Sets nothing at file-load time; every
# export happens here, called from set_up().
set_up_digest_harness() {
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
  export DIGEST_BUILDER="$REPO_ROOT/dot_local/libexec/osquery/executable_digest.sh"
}

# tear_down_digest_harness - remove ONLY a temp dir this harness created. The
# ownership marker is set after our own mktemp, so a pre-set HARNESS_HOME (marker
# unset) is left untouched.
tear_down_digest_harness() {
  [[ -n ${_DIGEST_HARNESS_OWNED_DIR:-} ]] || return 0
  rm -rf "$_DIGEST_HARNESS_OWNED_DIR"
  unset _DIGEST_HARNESS_OWNED_DIR
}

# run_digest - invoke the builder as a child process under the harness env. Its
# own output is discarded: no case here asserts on the script's stdout, only on
# what the recording spy captured, so keeping it would interleave noise into the
# bashunit report.
run_digest() { bash "$DIGEST_BUILDER" >/dev/null 2>&1; }

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
  ' digest-build-fault-injector >/dev/null 2>&1
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

# assert_no_send - the recording spy captured no send_alert call. The recorded
# calls become the assertion's "actual", so a failure names what was sent.
assert_no_send() {
  assert_same '' "$(cat "$SEND_ALERT_LOG" 2>/dev/null || true)"
}

# assert_silent_success - the B1 behavior in one intent-named assertion: the
# builder exits 0 AND sends nothing.
assert_silent_success() {
  run_digest
  assert_successful_code
  assert_no_send
}

# assert_live_store_freed - the live store was rotated aside, so a concurrent
# alerter append lands in a fresh file this run will not consume.
assert_live_store_freed() {
  assert_same '' "$(cat "$OSQUERY_DIGEST_STORE" 2>/dev/null || true)"
}

# assert_build_ran_against_work_file - the build step ran against the CLAIMED
# batch, proving the rotate happened before the build (not against the live store).
assert_build_ran_against_work_file() {
  assert_same claimed "$(cat "$DIGEST_BUILD_WITNESS" 2>/dev/null || true)"
}

# assert_live_store_restored <n> - the batch is back as the live store with <n>
# records, so the next daily run retries it.
assert_live_store_restored() {
  assert_is_file_not_empty "$OSQUERY_DIGEST_STORE"
  assert_same "$1" "$(count_records "$OSQUERY_DIGEST_STORE")"
}

# assert_no_work_file_left - no .build work file remains (the restore moved it back).
assert_no_work_file_left() {
  # An unmatched glob stays literal, so element 0 then names a path that does not
  # exist, which is exactly the passing case.
  local leftovers=("$OSQUERY_DIGEST_STORE".*.build)
  assert_file_not_exists "${leftovers[0]}"
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

# assert_body_line_count <body> <extended-regex> <n> - exactly <n> body lines match
# the regex. Named for the body rather than sharing bashunit's own
# assert_line_count, which counts every line of its input and takes different
# arguments: two functions of the same name, one shadowing the other, is a trap.
assert_body_line_count() {
  assert_same "$3" "$(grep -cE -- "$2" <<<"$1" || true)"
}

# assert_no_injected_line <body> - no body line BEGINS with a forged field marker.
# A sanitized field keeps crafted "- **Signing:**" text inline within its own
# bullet; only a sanitize regression would push it to the start of a new line.
# assert_not_matches anchors per line exactly as the grep -E it replaces did.
assert_no_injected_line() {
  assert_not_matches '^- \*\*Signing:\*\*' "$1"
}

# assert_sent_once - exactly one send_alert call was recorded.
assert_sent_once() {
  assert_same 1 "$(grep -c 'CALL' "$SEND_ALERT_LOG" 2>/dev/null || printf '0')"
}

# assert_sent_silent_crit - the recorded send is CRIT (selects the #priority route)
# with an EMPTY sound (silent/muted -> tier=muted, so Hermes suppresses the ping).
assert_sent_silent_crit() {
  assert_same CRIT "$(cat "$SEND_ALERT_SEVERITY" 2>/dev/null || true)"
  assert_same '' "$(cat "$SEND_ALERT_SOUND" 2>/dev/null || true)"
}

# assert_last_mode_600 - the .last forensic file is mode 600 (it holds full paths).
# GNU stat first (Linux, or a gnubin-fronted PATH), BSD stat second: the portable order.
assert_last_mode_600() {
  local mode
  mode=$(stat -c '%a' "$OSQUERY_DIGEST_STORE.last" 2>/dev/null || stat -f '%Lp' "$OSQUERY_DIGEST_STORE.last" 2>/dev/null)
  assert_same 600 "$mode"
}

# assert_batch_in_last <n> - the built batch was preserved as $store.last with <n> records.
assert_batch_in_last() {
  assert_is_file_not_empty "$OSQUERY_DIGEST_STORE.last"
  assert_same "$1" "$(count_records "$OSQUERY_DIGEST_STORE.last")"
}

function test_an_absent_digest_store_produces_no_message_and_exits_zero() {
  given_absent_store
  assert_silent_success
}

function test_a_zero_byte_digest_store_produces_no_message_and_exits_zero() {
  given_empty_store
  assert_silent_success
}

function test_a_whitespace_only_digest_store_produces_no_message_and_exits_zero() {
  given_whitespace_only_store
  assert_silent_success
}

function test_a_store_with_records_sends_exactly_one_silent_digest_then_rotates_the_batch_to_last() {
  seed_store \
    "$(digest_record persistence_launchd com.foo.agent 'persistence_launchd com.foo.agent')" \
    "$(digest_record sudoers /etc/sudoers.d/foo 'sudoers /etc/sudoers.d/foo')"
  run_digest
  assert_successful_code
  assert_sent_once
  assert_sent_silent_crit # CRIT route + EMPTY sound => tier=muted (non-paging)
  # The title carries the true item count (2 records).
  assert_file_contains "$SEND_ALERT_TITLE" '· 2 item(s)'
  assert_file_contains "$SEND_ALERT_BODY" '**persistence_launchd** (1)'
  assert_file_contains "$SEND_ALERT_BODY" '**sudoers** (1)'
  assert_live_store_freed  # the live store is fresh for the next run
  assert_batch_in_last 2   # the built batch is preserved for forensics
  assert_no_work_file_left # the .build is cleaned on the success path (no mv->cp leak)
  assert_last_mode_600
}

function test_a_build_failure_before_the_send_restores_the_rotated_batch_to_the_live_store() {
  seed_store \
    "$(digest_record sudoers /etc/sudoers.d/foo 'sudoers /etc/sudoers.d/foo')" \
    "$(digest_record sudoers /etc/sudoers.d/bar 'sudoers /etc/sudoers.d/bar')"
  run_digest_with_failing_build
  assert_unsuccessful_code # the forced pre-send build failure must surface
  assert_build_ran_against_work_file
  assert_live_store_restored 2
  assert_no_work_file_left
  assert_no_send
}

function test_findings_across_three_detectors_render_as_three_grouped_blocks_with_header_and_count() {
  local body
  body="$(render_body \
    "$(digest_record persistence_launchd com.foo.agent 'persistence_launchd com.foo.agent')" \
    "$(digest_record persistence_launchd com.bar.agent 'persistence_launchd com.bar.agent')" \
    "$(digest_record system_extensions_new io.tailscale 'system_extensions_new io.tailscale')" \
    "$(digest_record sudoers /etc/sudoers.d/foo 'sudoers /etc/sudoers.d/foo')")"
  assert_contains '**persistence_launchd** (2)' "$body"
  assert_contains '**system_extensions_new** (1)' "$body"
  assert_contains '**sudoers** (1)' "$body"
  assert_contains '- `com.foo.agent` - `persistence_launchd com.foo.agent`' "$body"
  assert_contains '- `io.tailscale` - `system_extensions_new io.tailscale`' "$body"
}

function test_a_detector_with_more_findings_than_the_bullet_cap_shows_capped_bullets_and_a_more_rollup() {
  local records=() i
  for i in $(seq 1 14); do
    records+=("$(digest_record persistence_launchd "com.item.$i" "summary $i")")
  done
  local body
  body="$(render_body "${records[@]}")"
  # The header counts the true total.
  assert_contains '**persistence_launchd** (14)' "$body"
  # DIGEST_MAX_BULLETS_PER_GROUP default.
  assert_body_line_count "$body" '^- `com\.item\.' 10
  assert_contains '+4 more' "$body"
}

function test_more_detector_groups_than_the_group_cap_show_capped_blocks_and_an_and_more_marker() {
  local records=() i
  for i in $(seq 1 15); do
    records+=("$(digest_record "detector_$i" "id_$i" "summary $i")")
  done
  local body
  body="$(render_body "${records[@]}")"
  assert_body_line_count "$body" '^\*\*detector_' 12 # DIGEST_MAX_GROUPS default
  assert_contains 'and 3 more detector group(s)' "$body"
}

function test_the_body_is_codepoint_capped_with_an_honest_truncation_marker() {
  local records=() i
  for i in $(seq 1 150); do
    records+=("$(printf '{"timestamp":"t","detector":"det_%s","category":"","identity":"identity_number_%s","action":"added","summary":"a summary long enough to add real bytes for finding number %s"}' "$((i % 15))" "$i" "$i")")
  done
  # Default cap: content renders, the truncation is MARKED (a silent head -c byte cut is a bug),
  # and the body stays well under Discord's 2000. Length is codepoints (jq slices codepoints).
  local body cp
  body="$(render_body "${records[@]}")"
  assert_contains '**det_' "$body"
  assert_contains '(truncated)' "$body"
  cp="$(printf '%s' "$body" | wc -m | tr -d '[:space:]')"
  assert_less_or_equal_than 1830 "$cp" # 1800 cap + marker
  # Overridable and still honest: a tighter cap is honored with the same marker.
  export DIGEST_MAX_BODY_CHARS=500
  body="$(render_body "${records[@]}")"
  assert_contains '(truncated)' "$body"
  cp="$(printf '%s' "$body" | wc -m | tr -d '[:space:]')"
  assert_less_or_equal_than 530 "$cp" # 500 cap + marker
}

function test_an_oversized_body_is_capped_inside_jq_and_sent_once_with_no_broken_pipe_failure() {
  # A body far larger than the macOS pipe buffer (which grows to ~64KB) made `jq | head -c` block
  # on write and take SIGPIPE (rc 141), which tripped the ERR trap and sent nothing on busy days.
  # Raise the field cap and seed large fields so the body far exceeds the buffer; the body cap must
  # live INSIDE jq (no pipe) so an over-cap body truncates and sends exactly once.
  local big records=() i
  big="$(printf 'x%.0s' {1..5000})"
  for i in $( # 2 detectors x 10 findings, ~10KB per bullet -> ~200KB pre-cap
    seq 1 20
  ); do
    records+=("$(digest_record "det_$((i % 2))" "id${i}_$big" "sum${i}_$big")")
  done
  export DIGEST_MAX_FIELD_CHARS=6000 # let the big fields through, so the body blows past the buffer
  seed_store "${records[@]}"
  run_digest
  assert_successful_code # the oversized body caps and sends rather than failing
  assert_sent_once
  # Capped, and the truncation is marked.
  assert_file_contains "$SEND_ALERT_BODY" '(truncated)'
}

function test_the_group_and_bullet_caps_are_env_overridable_named_constants() {
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
  assert_body_line_count "$body" '^\*\*det_' 2 # DIGEST_MAX_GROUPS honored
  assert_contains 'and 4 more detector group(s)' "$body"
  # det_1: 5 findings, 3 bullets + "+2 more" (DIGEST_MAX_BULLETS_PER_GROUP).
  assert_contains '+2 more' "$body"
}

function test_a_crafted_identity_cannot_inject_an_extra_markdown_line_into_the_digest_body() {
  local evil
  evil=$'evil\n- **Signing:** signed: Apple'
  local body
  body="$(render_body "$(digest_record persistence_launchd "$evil" 'malicious finding')")"
  # The crafted newline is squashed to a space, so the value stays inert INSIDE one bullet.
  assert_contains '- `evil - **Signing:** signed: Apple` - `malicious finding`' "$body"
  # And the forged field marker never becomes its own line.
  assert_no_injected_line "$body"
}

function test_an_attacker_controlled_field_renders_inside_a_code_span_so_a_mention_or_link_is_inert() {
  # render-page wraps every attacker-influenceable field in backticks; the digest does the
  # same, so a crafted mention or link renders as literal inline-code text, not a live
  # Discord @everyone or a clickable link. (The line/block-forging guard above is separate.)
  local body
  body="$(render_body "$(digest_record persistence_launchd '@everyone' '[click](http://evil.example)')")"
  # Both fields inside code spans.
  assert_contains '- `@everyone` - `[click](http://evil.example)`' "$body"
  # The mention is inert inline code, not bare.
  assert_contains '`@everyone`' "$body"
  # The link markdown is inert inline code too.
  assert_contains '`[click](http://evil.example)`' "$body"
}

function test_an_oversized_field_is_truncated_in_the_sanitize_chokepoint_and_cannot_crowd_out_other_groups() {
  local giant
  giant="$(printf 'x%.0s' {1..5000})" # one field far larger than the whole body cap
  local body
  body="$(render_body \
    "$(digest_record aaa_giant id_giant "$giant")" \
    "$(digest_record zzz_small id_small 'a small summary')")"
  # The oversized field is truncated in place with the per-field marker (DIGEST_MAX_FIELD_CHARS).
  assert_contains '…(truncated)' "$body"
  # ... so it cannot alone consume the whole body cap: the later detector group still renders.
  assert_contains '**zzz_small** (1)' "$body"
  assert_contains '- `id_small` - `a small summary`' "$body"
  # And the full oversized value never survives into the body.
  assert_not_contains "$giant" "$body"
}

function test_a_torn_or_malformed_spool_line_is_skipped_so_the_days_digest_still_builds() {
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
  # The two GOOD launchd findings; the torn one skipped.
  assert_contains '**persistence_launchd** (2)' "$body"
  assert_contains '- `com.good.one` - `persistence_launchd com.good.one`' "$body"
  assert_contains '- `com.good.two` - `persistence_launchd com.good.two`' "$body"
  assert_contains '**sudoers** (1)' "$body"
  assert_not_contains 'com.tor' "$body"
  # The FULL builder survives the torn line: it exits 0 (no set -e abort), so the B2 ERR
  # trap never restores the batch and the digest is not silently lost until the line ages out.
  seed_store "${records[@]}"
  run_digest
  assert_successful_code
  assert_live_store_freed
}

function test_a_valid_json_wrong_shape_line_is_coerced_not_fatal_and_the_digest_still_sends() {
  # These lines PARSE (survive try/catch) but carry a null/missing or numeric identity; a bare
  # .identity | gsub aborts jq rc=5 ("cannot be matched, not a string") -> ERR-trap restore ->
  # permanent silent digest death + an unbounded store. The field access must coerce, not abort.
  seed_store \
    "$(digest_record persistence_launchd com.good.one 'good one')" \
    '{"detector":"persistence_launchd","summary":"missing identity"}' \
    '{"detector":"persistence_launchd","identity":42,"summary":"numeric identity"}'
  run_digest
  assert_successful_code # no abort on the wrong-shape lines
  assert_sent_once
  assert_file_contains "$SEND_ALERT_BODY" '`com.good.one`' # the good finding still rendered
  assert_file_contains "$SEND_ALERT_BODY" '`?`'            # the missing-identity line coerced to ?
  assert_file_contains "$SEND_ALERT_BODY" '`42`'           # the numeric identity coerced to a string
}

function test_a_hard_send_failure_restores_the_batch_to_the_live_store_for_retry() {
  # send_alert returns nonzero ONLY when its write-ahead persist failed: the page was neither
  # delivered NOR stored ("the caller must not advance its cursor past it"), so the batch must be
  # RESTORED, not rotated to .last, and the next run retries it. Restoring cannot double-store
  # because nothing was stored.
  seed_store "$(digest_record persistence_launchd com.foo.agent 'foo')"
  export SEND_ALERT_RC=1
  run_digest
  assert_successful_code # exit 0 despite the hard send failure
  assert_sent_once
  assert_live_store_restored 1 # RESTORED for retry (the page was neither delivered nor stored)
  # No .last on a hard-fail restore.
  assert_file_not_exists "$OSQUERY_DIGEST_STORE.last"
}

function test_a_stored_send_rotates_the_batch_to_last_and_does_not_restore_it() {
  # send_alert returns 0 on every STORED outcome (delivered, stored-nosecret, stored-delivery-
  # pending): durability is delegated to its write-ahead store + drainer, so the batch is rotated
  # to .last, NOT restored (a restore would double-store and re-send a duplicate next run).
  seed_store "$(digest_record persistence_launchd com.foo.agent 'foo')"
  export SEND_ALERT_RC=0
  run_digest
  assert_successful_code
  assert_sent_once
  assert_batch_in_last 1  # rotated to .last (durability delegated to send_alert)
  assert_live_store_freed # NOT restored
}

function test_restore_preserves_a_finding_appended_to_the_fresh_store_during_the_build() {
  # The alerter can append a NEW finding to the fresh live store WHILE the build runs. A restore
  # that OVERWRITES the store (mv -f) destroys that concurrent append; an append-restore keeps it.
  # Order within a grouped digest is irrelevant, so appending is safe.
  seed_store \
    "$(digest_record persistence_launchd com.batch.a 'A')" \
    "$(digest_record persistence_launchd com.batch.b 'B')"
  local concurrent
  concurrent="$(digest_record sudoers /etc/sudoers.d/c 'C')"
  # Drive the builder with a render step that appends a concurrent finding to the fresh store, then
  # fails to force the pre-send restore.
  CONCURRENT_RECORD="$concurrent" bash -c '
    source "$DIGEST_BUILDER"
    render_digest_body() { printf "%s\n" "$CONCURRENT_RECORD" >>"$OSQUERY_DIGEST_STORE"; return 1; }
    main
  ' digest-concurrent-restore-probe >/dev/null 2>&1 || true
  local store_content
  store_content="$(cat "$OSQUERY_DIGEST_STORE" 2>/dev/null || true)"
  assert_contains 'com.batch.a' "$store_content" # the claimed batch is restored
  assert_contains 'com.batch.b' "$store_content"
  assert_contains '/etc/sudoers.d/c' "$store_content" # AND the concurrent append survives
}

function test_the_work_file_name_includes_the_pid_so_same_second_claims_do_not_collide() {
  # date +%s is per-second: two invocations in the same second would derive the same work file and
  # the second mv -f would clobber the first. The name must carry the process id too.
  local out wf pid
  out="$(bash -c 'source "$DIGEST_BUILDER"; printf "%s\n%s" "$(rotated_work_file /tmp/store)" "$$"' digest-wf-probe)"
  wf="$(printf '%s' "$out" | head -1)"
  pid="$(printf '%s' "$out" | tail -1)"
  assert_string_ends_with ".$pid.build" "$wf"
}

function test_an_orphaned_build_file_from_a_killed_run_is_swept_back_into_the_next_digest() {
  # A run killed by a signal (SIGKILL, power loss, or the SIGTERM launchd sends gui agents at
  # logout) between claim and rotate leaves a .build orphan the ERR trap could not restore (signals
  # do not fire it) and no later run would consult. The next run must sweep it back and deliver it.
  mkdir -p "$(dirname "$OSQUERY_DIGEST_STORE")"
  printf '%s\n' "$(digest_record persistence_launchd com.orphan.finding 'orphaned')" \
    >"$OSQUERY_DIGEST_STORE.1700000000.999.build"
  seed_store "$(digest_record sudoers /etc/sudoers.d/new 'current')"
  run_digest
  assert_successful_code
  assert_sent_once
  # The orphan finding recovered and sent, and the current finding too.
  assert_file_contains "$SEND_ALERT_BODY" '`com.orphan.finding`'
  assert_file_contains "$SEND_ALERT_BODY" '`/etc/sudoers.d/new`'
}

function test_a_non_numeric_cap_env_value_falls_back_to_the_default_instead_of_failing_the_render() {
  # A typo'd env cap (DIGEST_MAX_GROUPS=abc) reaches --argjson as invalid JSON and fails the render,
  # which would silently kill the daily digest. A non-integer must fall back to the default and send.
  seed_store "$(digest_record persistence_launchd com.foo.agent 'foo')"
  export DIGEST_MAX_GROUPS=abc
  run_digest
  assert_successful_code # falls back to the default rather than failing
  assert_sent_once
  assert_file_contains "$SEND_ALERT_BODY" '`com.foo.agent`'
}

function test_an_all_torn_store_renders_an_empty_body_so_it_sends_nothing_and_preserves_the_batch() {
  # Every line unparseable: Guard 2 (non-whitespace bytes) passes and the raw lines are
  # counted, but the rendered body is empty. The builder must NOT send a misleading
  # silent "N item(s)" with an empty body; it preserves the batch to .last and stays silent.
  seed_store '{"detector":"persistence_launchd","identity":"x' '{"oops'
  run_digest
  assert_successful_code
  assert_no_send
  assert_batch_in_last 2 # the unrecoverable batch is preserved for forensics
  assert_live_store_freed
}
