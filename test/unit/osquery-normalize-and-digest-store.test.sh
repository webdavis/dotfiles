#!/usr/bin/env bash
# The two sourced helpers of results-alerter.sh that shape a finding before it
# is judged, and record the ones that never page.
#
# normalize.sh turns the raw osquery results-log tail into normalized finding
# NDJSON, one {q, act, cols, ep} object per surviving row. It owns the admission
# rules (the known-query allowlist, the renameio exclusion, the counter==0
# baseline discard) and the shaping rules (pack-prefix strip, action default,
# enrich path).
#
# digest-store.sh owns the digest tier's write side: a suspicious-but-ambiguous
# finding that does not page accumulates in a private local spool as one derived
# NDJSON line. It is best-effort (a spool failure must never abort detection) and
# privacy-bound (a raw hash or a secret column must never reach the spool).
#
# Both are pure sourced functions, so every test below calls them in-process. No
# flows, no clocks, no sleeps.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit: both belong to a script that runs on its own, and either one
# here would reach into the runner's own shell. test/validate-tests.sh pins the
# shape, and bashunit runs each test body in a subshell, so the two helpers are
# sourced ONCE at file scope and every test still gets its own copy of whatever
# it changes.
#
# assert_same, never assert_equals: assert_equals normalizes away control
# characters before comparing, and a tab surviving into an enrich path is
# exactly what one test below exists to catch.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ALERTER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter"

# shellcheck source=dot_local/libexec/osquery/results-alerter/normalize.sh
source "$ALERTER/normalize.sh"
# shellcheck source=dot_local/libexec/osquery/results-alerter/digest-store.sh
source "$ALERTER/digest-store.sh"

# The file fixture: a jq that always fails, for the digest tier's best-effort
# contract. The helper suppresses jq's own stderr, so a shim that writes to
# stderr also proves the diagnostic under test is the helper's own line, not
# jq's. Built once, because nothing about it is per-test.
set_up_before_script() {
  FILE_FIXTURE="$(mktemp -d)"
  mkdir -p "$FILE_FIXTURE/failing-jq"
  printf '#!/usr/bin/env bash\nprintf "jq shim: simulated digest append failure\\n" >&2\nexit 5\n' \
    >"$FILE_FIXTURE/failing-jq/jq"
  chmod +x "$FILE_FIXTURE/failing-jq/jq"
}

tear_down_after_script() { discard_fixture "$FILE_FIXTURE"; }

set_up() {
  TEST_FIXTURE="$(mktemp -d)"
  SPOOL="$TEST_FIXTURE/state/osquery-digest-spool/digest.ndjson"
  # shellcheck disable=SC2034 # digest_append reads it out of the sourced helper.
  OSQUERY_DIGEST_STORE="$SPOOL"
}

tear_down() { discard_fixture "$TEST_FIXTURE"; }

# discard_fixture <path>: remove one mktemp -d this file created, and nothing
# else. Plain rm -rf, the convention every other test in this repo uses: the
# suite runs dozens of times a day and on a CI host with no Trash, so routing
# fixture teardown through `trash` would both fill the operator's Trash and add
# a fork per test to a suite held to a one-second-per-test bar.
discard_fixture() {
  [[ -n ${1:-} && -d $1 ]] || return 0
  rm -rf "$1"
}

# --- normalize.sh: shaping ---------------------------------------------------

function test_a_packed_row_reaches_routing_under_its_bare_query_name_with_columns_and_action_intact() {
  local normalized
  normalized="$(normalize_findings <<<'{"name":"pack_intrusion-detection_suid_bin_unexpected","action":"added","columns":{"path":"/tmp/x"}}')"
  assert_same '{"q":"suid_bin_unexpected","act":"added","cols":{"path":"/tmp/x"},"ep":"/tmp/x"}' "$normalized"
}

function test_only_the_pack_segment_is_stripped_so_a_hyphenated_pack_name_leaves_the_querys_underscores_alone() {
  local normalized
  normalized="$(normalize_findings <<<'{"name":"pack_agent-attack-surface_agent_exposure_changed","action":"added","columns":{}}')"
  assert_same '{"q":"agent_exposure_changed","act":"added","cols":{},"ep":""}' "$normalized"
}

function test_a_row_that_omits_its_action_is_normalized_to_changed_so_no_later_stage_special_cases_a_null() {
  local normalized
  normalized="$(normalize_findings <<<'{"name":"new_admin_user","columns":{"username":"bob"}}')"
  assert_same '{"q":"new_admin_user","act":"changed","cols":{"username":"bob"},"ep":""}' "$normalized"
}

function test_a_snapshot_action_row_stays_one_finding_instead_of_fanning_out_its_snapshot_array() {
  local normalized
  normalized="$(normalize_findings <<<'{"name":"pack_security-policy-regression_filevault_state","action":"snapshot","snapshot":[{"path":"/a"},{"path":"/b"}]}')"
  assert_same '{"q":"filevault_state","act":"snapshot","cols":{},"ep":""}' "$normalized"
}

function test_a_malformed_line_drops_out_without_taking_the_rest_of_the_batch_with_it() {
  local expected='{"q":"new_admin_user","act":"added","cols":{"username":"alice"},"ep":""}' normalized
  normalized="$(
    normalize_findings <<'EOF'
this is not json
{"name":"new_admin_user","action":"added","columns":{"username":"alice"}}
EOF
  )"
  assert_same "$expected" "$normalized"
}

# --- normalize.sh: admission -------------------------------------------------

function test_an_unrecognized_query_name_never_becomes_a_finding_packed_or_top_level() {
  local expected normalized
  expected='{"q":"new_admin_user","act":"added","cols":{},"ep":""}
{"q":"agent_secretfile_changed","act":"added","cols":{},"ep":""}
{"q":"filevault_off","act":"added","cols":{},"ep":""}'
  normalized="$(
    normalize_findings <<'EOF'
{"name":"new_admin_user","action":"added","columns":{}}
{"name":"pack_foo_bar","action":"added","columns":{}}
{"name":"pack_agent-attack-surface_agent_secretfile_changed","action":"added","columns":{}}
{"name":"totally_bogus_query","action":"added","columns":{}}
{"name":"pack_security-policy-regression_filevault_off","action":"added","columns":{}}
EOF
  )"
  assert_same "$expected" "$normalized"
}

function test_the_heartbeat_canary_is_dropped_defensively_so_a_stray_liveness_row_generates_no_noise() {
  local normalized
  normalized="$(normalize_findings <<<'{"name":"heartbeat_canary","action":"snapshot","columns":{}}')"
  assert_empty "$normalized"
}

function test_renameio_atomic_write_churn_is_dropped_while_a_real_file_event_on_the_same_query_survives() {
  local expected='{"q":"file_events_recent","act":"added","cols":{"target_path":"/Users/x/.ssh/authorized_keys"},"ep":"/Users/x/.ssh/authorized_keys"}' normalized
  normalized="$(
    normalize_findings <<'EOF'
{"name":"file_events_recent","action":"added","columns":{"target_path":"/Users/x/.config/foo/.renameio-TempDir-abc/bar"}}
{"name":"file_events_recent","action":"added","columns":{"target_path":"/Users/x/.ssh/authorized_keys"}}
EOF
  )"
  assert_same "$expected" "$normalized"
}

function test_a_counter_zero_membership_baseline_is_discarded_while_counter_positive_and_counter_absent_rows_survive() {
  local expected normalized
  expected='{"q":"new_admin_user","act":"added","cols":{"username":"eve"},"ep":""}
{"q":"new_admin_user","act":"added","cols":{"username":"mallory"},"ep":""}'
  normalized="$(
    normalize_findings <<'EOF'
{"name":"new_admin_user","action":"added","counter":0,"columns":{"username":"root"}}
{"name":"new_admin_user","action":"added","counter":1,"columns":{"username":"eve"}}
{"name":"new_admin_user","action":"added","columns":{"username":"mallory"}}
EOF
  )"
  assert_same "$expected" "$normalized"
}

function test_the_three_absolute_state_queries_keep_their_counter_zero_row_so_an_already_unsafe_state_pages_on_first_observation() {
  local expected normalized
  expected='{"q":"filevault_off","act":"added","cols":{},"ep":""}
{"q":"remote_access_sharing_state","act":"added","cols":{},"ep":""}
{"q":"agent_exposure_changed","act":"added","cols":{},"ep":""}'
  normalized="$(
    normalize_findings <<'EOF'
{"name":"pack_security-policy-regression_filevault_off","action":"added","counter":0,"columns":{}}
{"name":"pack_security-policy-regression_remote_access_sharing_state","action":"added","counter":0,"columns":{}}
{"name":"pack_agent-attack-surface_agent_exposure_changed","action":"added","counter":0,"columns":{}}
EOF
  )"
  assert_same "$expected" "$normalized"
}

# --- normalize.sh: the enrich path -------------------------------------------

function test_the_enrich_path_names_the_exact_file_each_query_type_hands_the_enricher_and_is_empty_where_signing_does_not_apply() {
  local expected normalized
  expected='{"q":"es_launchd_writes","act":"added","cols":{"path":"/usr/bin/foo"},"ep":"/usr/bin/foo"}
{"q":"file_events_recent","act":"added","cols":{"target_path":"/Users/x/.ssh/authorized_keys"},"ep":"/Users/x/.ssh/authorized_keys"}
{"q":"persistence_launchd","act":"added","cols":{"path":"/Library/LaunchAgents/com.example.plist","label":"com.example"},"ep":"/Library/LaunchAgents/com.example.plist"}
{"q":"system_extensions_new","act":"added","cols":{"bundle_path":"/Applications/X.app","path":"/ignored"},"ep":"/Applications/X.app"}
{"q":"suid_bin_unexpected","act":"added","cols":{"path":"/tmp/suid"},"ep":"/tmp/suid"}
{"q":"new_admin_user","act":"added","cols":{"username":"eve"},"ep":""}'
  normalized="$(
    normalize_findings <<'EOF'
{"name":"es_launchd_writes","action":"added","columns":{"path":"/usr/bin/foo"}}
{"name":"file_events_recent","action":"added","columns":{"target_path":"/Users/x/.ssh/authorized_keys"}}
{"name":"pack_intrusion-detection_persistence_launchd","action":"added","columns":{"path":"/Library/LaunchAgents/com.example.plist","label":"com.example"}}
{"name":"pack_intrusion-detection_system_extensions_new","action":"added","columns":{"bundle_path":"/Applications/X.app","path":"/ignored"}}
{"name":"pack_intrusion-detection_suid_bin_unexpected","action":"added","columns":{"path":"/tmp/suid"}}
{"name":"new_admin_user","action":"added","columns":{"username":"eve"}}
EOF
  )"
  assert_same "$expected" "$normalized"
}

function test_a_tab_inside_a_path_is_squashed_to_a_space_so_the_enrich_path_stays_one_renderable_token() {
  local normalized
  normalized="$(normalize_findings <<<'{"name":"es_launchd_writes","action":"added","columns":{"path":"/usr/bin/foo\tbar"}}')"
  assert_same '{"q":"es_launchd_writes","act":"added","cols":{"path":"/usr/bin/foo\tbar"},"ep":"/usr/bin/foo bar"}' "$normalized"
}

# --- digest-store.sh: the recorded line --------------------------------------

function test_one_append_records_a_single_line_of_derived_triage_fields_and_nothing_else() {
  local lines derived_line_pattern
  derived_line_pattern='^\{"timestamp":"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z"'
  derived_line_pattern+=',"detector":"system_extensions_new","category":"","identity":"com.example.ext"'
  derived_line_pattern+=',"action":"added","summary":"system_extensions_new com.example.ext"\}$'
  digest_append '{"q":"system_extensions_new","act":"added","cols":{"identifier":"com.example.ext","team":"TEAMID"},"ep":""}'
  mapfile -t lines <"$SPOOL"
  assert_same 1 "${#lines[@]}"
  assert_matches "$derived_line_pattern" "${lines[0]}"
}

function test_appends_accumulate_one_line_per_finding_so_the_daily_digest_sees_every_one() {
  local lines
  digest_append '{"q":"system_extensions_new","act":"added","cols":{"identifier":"com.example.ext"},"ep":""}'
  digest_append '{"q":"new_admin_user","act":"added","cols":{"username":"eve"},"ep":""}'
  digest_append '{"q":"suid_bin_unexpected","act":"added","cols":{"path":"/tmp/suid"},"ep":""}'
  mapfile -t lines <"$SPOOL"
  assert_same 3 "${#lines[@]}"
}

function test_the_spool_is_private_a_700_directory_and_a_600_file() {
  digest_append '{"q":"new_admin_user","act":"added","cols":{"username":"eve"},"ep":""}'
  assert_file_permissions 700 "${SPOOL%/*}"
  assert_file_permissions 600 "$SPOOL"
}

function test_a_listening_port_finding_is_identified_by_name_address_and_port_together() {
  local line
  digest_append '{"q":"listening_ports_non_loopback","act":"added","cols":{"name":"nc","address":"0.0.0.0","port":"4444"},"ep":""}'
  IFS= read -r line <"$SPOOL"
  assert_contains '"identity":"nc 0.0.0.0:4444"' "$line"
}

function test_a_findings_raw_hash_and_secret_column_never_reach_the_spool_only_its_path() {
  local line
  digest_append '{"q":"agent_authfile_changed","act":"added","cols":{"path":"/Users/x/.codex/config.toml","sha256":"deadbeefdeadbeef","secret_value":"SUPERSECRETTOKEN"},"ep":""}'
  IFS= read -r line <"$SPOOL"
  assert_contains '"identity":"/Users/x/.codex/config.toml"' "$line"
  assert_not_contains sha256 "$line"
  assert_not_contains deadbeef "$line"
  assert_not_contains SUPERSECRETTOKEN "$line"
}

# --- digest-store.sh: a failed append ----------------------------------------

function test_a_failed_append_never_aborts_the_detection_path() {
  # A PATH local to this test is the point: bashunit runs each body in its own
  # subshell, so only the helper under test sees the failing jq.
  PATH="$FILE_FIXTURE/failing-jq:$PATH"
  digest_append '{"q":"new_admin_user","act":"added","cols":{"username":"eve"},"ep":""}' 2>/dev/null
  assert_successful_code
}

function test_a_failed_append_says_so_on_stderr_naming_the_spool_it_could_not_write() {
  local diagnostic
  PATH="$FILE_FIXTURE/failing-jq:$PATH"
  digest_append '{"q":"new_admin_user","act":"added","cols":{"username":"eve"},"ep":""}' 2>"$TEST_FIXTURE/stderr"
  IFS= read -r diagnostic <"$TEST_FIXTURE/stderr"
  assert_contains digest-store "$diagnostic"
  assert_contains "$SPOOL" "$diagnostic"
}

function test_a_failed_append_leaves_no_partial_line_behind() {
  PATH="$FILE_FIXTURE/failing-jq:$PATH"
  digest_append '{"q":"new_admin_user","act":"added","cols":{"username":"eve"},"ep":""}' 2>/dev/null
  assert_is_file_empty "$SPOOL"
}
