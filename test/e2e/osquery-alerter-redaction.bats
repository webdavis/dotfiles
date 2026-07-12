#!/usr/bin/env bats
# End-to-end redaction (H2): a credential finding driven through the REAL alerter AND
# the REAL dispatcher, with delivery forced to fail so the page spools to disk. The
# on-disk spool body (base64) must carry the BASENAME only — never the full home-dir
# path or the raw sha256 (invariant #4). The H1 T-SEC-redaction test asserts the page
# body in memory; this asserts the one place a body is actually written to disk and
# later replayed, so a future change that spooled a pre-redaction body would fail here.

load ../fixtures/osquery-alerter-lib

setup() { setup_redaction_h2_harness; }
teardown() { teardown_harness; }

@test "T-SEC-redaction-spool: a spooled credential page carries the basename only, no path/sha256" {
  run_redaction_h2 "$(row agent_authfile_changed added 1 '{"path":"/Users/stephen/.paseo/daemon-keypair.json","sha256":"DEADBEEFCAFE1234"}')"
  assert_spool_count 1
  local body
  body=$(cut -f4 "$(find "$OSQUERY_SPOOL_DIR" -type f | head -1)" | base64 -d)
  [[ "$body" == *daemon-keypair.json* ]]       # the basename survives (the page did flow)
  [[ "$body" != *DEADBEEFCAFE1234* ]]          # the raw sha256 never reaches the spool
  [[ "$body" != *"/Users/stephen/.paseo"* ]]   # the full home-dir path is redacted
  run grep -rqF "DEADBEEFCAFE1234" "$OSQUERY_DELIVERY_LOG" # delivery log = metadata only
  [ "$status" -ne 0 ]
}

@test "T-CAP-overlong-spool: an over-long page spools UNDER the 2000-char cap and then drains (FX8)" {
  # A 4000-char field would render a >3000-char body; without a final cap the POST is
  # rejected and re-spooled forever (retry never shrinks it). The SPOOLED body must be
  # the truncated one (< 2000), and it must deliver once the gateway recovers.
  local big
  big=$(printf 'A%.0s' $(seq 1 4000))
  run_redaction_h2 "$(row new_admin_user added 1 "$(jq -cn --arg u "$big" '{username:$u,uid:"503"}')")"
  assert_spool_count 1
  local detail
  detail=$(cut -f4 "$(find "$OSQUERY_SPOOL_DIR" -type f | head -1)" | base64 -d | jq -r '.alert.detail')
  [ "${#detail}" -lt 2000 ] # the spooled body is the capped one
  run_h2_drain
  assert_spool_count 0 # recovers and clears
}
