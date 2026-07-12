#!/usr/bin/env bats
# Checkpoint-after-durable (R2-2 + R2-6, end to end via the REAL dispatcher). The alerter used
# to advance the cursor to EOF BEFORE dispatching, so a page that could be neither delivered
# nor spooled was lost AND the cursor moved past it. The checkpoint must advance only AFTER the
# batch is durably delivered-or-spooled; a hard delivery failure must leave the cursor put so
# the next run retries the row.

load ../fixtures/osquery-alerter-lib

setup() { setup_redaction_h2_harness; }
teardown() { teardown_harness; }

@test "T-CURSOR-hardfail-no-advance: a page that hard-fails to deliver AND spool leaves the cursor put (R2-6)" {
  # Delivery fails (curl 503) and the spool is unwritable (parent is a file), so the page is
  # neither delivered nor stored. The cursor started at offset 0; it must STILL be 0 (or absent),
  # never advanced to EOF, so the next run reprocesses the row instead of losing it.
  run_alerter_hardfail_spool "$(row pack_security-policy-regression_filevault_off added 1 '{"protection":"filevault"}')"
  local off
  off="$(cursor_offset)"
  [ "$off" = "0" ] || [ -z "$off" ]   # NOT advanced past the undelivered row
}

@test "T-CURSOR-success-advances: a delivered page advances the cursor to EOF (R2-2 positive)" {
  # The complement: when delivery succeeds, the cursor DOES advance, so a delivered batch is
  # not reprocessed. curl returns 200 here (flip the H2 shim to success).
  printf '#!/usr/bin/env bash\nprintf 200\n' >"$HARNESS_HOME/.local/bin/curl"
  chmod +x "$HARNESS_HOME/.local/bin/curl"
  local results_log="$HARNESS_HOME/.local/log/osquery/osqueryd.results.log"
  printf '%s\n' "$(row pack_security-policy-regression_filevault_off added 1 '{"protection":"filevault"}')" >"$results_log"
  printf '0 0\n' >"$HARNESS_HOME/.local/state/osquery-results-offset"
  local size
  size=$(wc -c <"$results_log"); size=${size//[[:space:]]/}
  HOME="$HARNESS_HOME" PATH="$HARNESS_HOME/.local/bin:$PATH" \
    OSQUERY_RESULTS_LOG="$results_log" \
    OSQUERY_RESULTS_OFFSET="$HARNESS_HOME/.local/state/osquery-results-offset" \
    OSQUERY_DIGEST_STORE="$OSQUERY_DIGEST_STORE" \
    OSQUERY_LAUNCHD_ALLOWLIST="$OSQUERY_LAUNCHD_ALLOWLIST" \
    OSQUERY_PIPELINE_MANIFEST="$OSQUERY_PIPELINE_MANIFEST" \
    OSQUERY_WEBHOOK_SECRET="$OSQUERY_WEBHOOK_SECRET" \
    OSQUERY_SPOOL_DIR="$OSQUERY_SPOOL_DIR" \
    OSQUERY_DELIVERY_LOG="$OSQUERY_DELIVERY_LOG" \
    OSQUERY_RETRY_BACKOFF_BASE=0 \
    bash "$ALERTER"
  [ "$(cursor_offset)" = "$size" ]   # advanced to EOF after a durable delivery
}
