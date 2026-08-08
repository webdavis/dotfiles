#!/usr/bin/env bats
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
# Both are pure sourced functions, so every test below sources the helper and
# calls it in-process. No flows, no clocks, no sleeps.

REPO_ROOT="${BATS_TEST_DIRNAME%/test/unit}"
ALERTER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter"

setup_file() {
  # A jq that always fails, for the digest tier's best-effort contract. The
  # helper suppresses jq's own stderr, so a shim that writes to stderr also
  # proves the diagnostic under test is the helper's own line, not jq's.
  mkdir -p "$BATS_FILE_TMPDIR/failing-jq"
  printf '#!/usr/bin/env bash\nprintf "jq shim: simulated digest append failure\\n" >&2\nexit 5\n' \
    >"$BATS_FILE_TMPDIR/failing-jq/jq"
  chmod +x "$BATS_FILE_TMPDIR/failing-jq/jq"
}

setup() {
  source "$ALERTER/normalize.sh"
  source "$ALERTER/digest-store.sh"
  SPOOL="$BATS_TEST_TMPDIR/state/osquery-digest-spool/digest.ndjson"
  # shellcheck disable=SC2034 # digest_append reads it out of the sourced helper.
  OSQUERY_DIGEST_STORE="$SPOOL"
}

# perms_of <path>: octal permission bits. GNU stat first (Linux), BSD stat as the
# fallback (macOS), the order the repo's stat-portability rule requires.
perms_of() { stat -c '%a' "$1" 2>/dev/null || stat -f '%Lp' "$1"; }

# --- normalize.sh: shaping ---------------------------------------------------

@test "a packed row reaches the routing stage under its bare query name, with its columns and action intact" {
  output="$(normalize_findings <<<'{"name":"pack_intrusion-detection_suid_bin_unexpected","action":"added","columns":{"path":"/tmp/x"}}')"
  [[ $output == '{"q":"suid_bin_unexpected","act":"added","cols":{"path":"/tmp/x"},"ep":"/tmp/x"}' ]]
}

@test "only the pack segment is stripped, so a hyphenated pack name leaves the query's own underscores alone" {
  output="$(normalize_findings <<<'{"name":"pack_agent-attack-surface_agent_exposure_changed","action":"added","columns":{}}')"
  [[ $output == '{"q":"agent_exposure_changed","act":"added","cols":{},"ep":""}' ]]
}

@test "a row that omits its action is normalized to changed, so no later stage special-cases a null" {
  output="$(normalize_findings <<<'{"name":"new_admin_user","columns":{"username":"bob"}}')"
  [[ $output == '{"q":"new_admin_user","act":"changed","cols":{"username":"bob"},"ep":""}' ]]
}

@test "a snapshot-action row stays one finding instead of fanning out its snapshot array" {
  output="$(normalize_findings <<<'{"name":"pack_security-policy-regression_filevault_state","action":"snapshot","snapshot":[{"path":"/a"},{"path":"/b"}]}')"
  [[ $output == '{"q":"filevault_state","act":"snapshot","cols":{},"ep":""}' ]]
}

@test "a malformed line drops out without taking the rest of the batch with it" {
  local expected='{"q":"new_admin_user","act":"added","cols":{"username":"alice"},"ep":""}'
  output="$(
    normalize_findings <<'EOF'
this is not json
{"name":"new_admin_user","action":"added","columns":{"username":"alice"}}
EOF
  )"
  [[ $output == "$expected" ]]
}

# --- normalize.sh: admission -------------------------------------------------

@test "an unrecognized query name never becomes a finding, whether it arrives packed or top-level" {
  local expected
  expected='{"q":"new_admin_user","act":"added","cols":{},"ep":""}
{"q":"agent_secretfile_changed","act":"added","cols":{},"ep":""}
{"q":"filevault_off","act":"added","cols":{},"ep":""}'
  output="$(
    normalize_findings <<'EOF'
{"name":"new_admin_user","action":"added","columns":{}}
{"name":"pack_foo_bar","action":"added","columns":{}}
{"name":"pack_agent-attack-surface_agent_secretfile_changed","action":"added","columns":{}}
{"name":"totally_bogus_query","action":"added","columns":{}}
{"name":"pack_security-policy-regression_filevault_off","action":"added","columns":{}}
EOF
  )"
  [[ $output == "$expected" ]]
}

@test "the heartbeat canary is dropped defensively, so a stray liveness row can never generate noise" {
  output="$(normalize_findings <<<'{"name":"heartbeat_canary","action":"snapshot","columns":{}}')"
  [[ -z $output ]]
}

@test "renameio atomic-write churn is dropped while a real file event on the same query survives" {
  local expected='{"q":"file_events_recent","act":"added","cols":{"target_path":"/Users/x/.ssh/authorized_keys"},"ep":"/Users/x/.ssh/authorized_keys"}'
  output="$(
    normalize_findings <<'EOF'
{"name":"file_events_recent","action":"added","columns":{"target_path":"/Users/x/.config/foo/.renameio-TempDir-abc/bar"}}
{"name":"file_events_recent","action":"added","columns":{"target_path":"/Users/x/.ssh/authorized_keys"}}
EOF
  )"
  [[ $output == "$expected" ]]
}

@test "a counter==0 membership baseline is discarded while counter>0 and counter-absent rows survive" {
  local expected
  expected='{"q":"new_admin_user","act":"added","cols":{"username":"eve"},"ep":""}
{"q":"new_admin_user","act":"added","cols":{"username":"mallory"},"ep":""}'
  output="$(
    normalize_findings <<'EOF'
{"name":"new_admin_user","action":"added","counter":0,"columns":{"username":"root"}}
{"name":"new_admin_user","action":"added","counter":1,"columns":{"username":"eve"}}
{"name":"new_admin_user","action":"added","columns":{"username":"mallory"}}
EOF
  )"
  [[ $output == "$expected" ]]
}

@test "the three absolute-state queries keep their counter==0 row, so an already-unsafe state pages on first observation" {
  local expected
  expected='{"q":"filevault_off","act":"added","cols":{},"ep":""}
{"q":"remote_access_sharing_state","act":"added","cols":{},"ep":""}
{"q":"agent_exposure_changed","act":"added","cols":{},"ep":""}'
  output="$(
    normalize_findings <<'EOF'
{"name":"pack_security-policy-regression_filevault_off","action":"added","counter":0,"columns":{}}
{"name":"pack_security-policy-regression_remote_access_sharing_state","action":"added","counter":0,"columns":{}}
{"name":"pack_agent-attack-surface_agent_exposure_changed","action":"added","counter":0,"columns":{}}
EOF
  )"
  [[ $output == "$expected" ]]
}

# --- normalize.sh: the enrich path -------------------------------------------

@test "the enrich path names the exact file each query type hands the enricher, and is empty where signing does not apply" {
  local expected
  expected='{"q":"es_launchd_writes","act":"added","cols":{"path":"/usr/bin/foo"},"ep":"/usr/bin/foo"}
{"q":"file_events_recent","act":"added","cols":{"target_path":"/Users/x/.ssh/authorized_keys"},"ep":"/Users/x/.ssh/authorized_keys"}
{"q":"persistence_launchd","act":"added","cols":{"path":"/Library/LaunchAgents/com.example.plist","label":"com.example"},"ep":"/Library/LaunchAgents/com.example.plist"}
{"q":"system_extensions_new","act":"added","cols":{"bundle_path":"/Applications/X.app","path":"/ignored"},"ep":"/Applications/X.app"}
{"q":"suid_bin_unexpected","act":"added","cols":{"path":"/tmp/suid"},"ep":"/tmp/suid"}
{"q":"new_admin_user","act":"added","cols":{"username":"eve"},"ep":""}'
  output="$(
    normalize_findings <<'EOF'
{"name":"es_launchd_writes","action":"added","columns":{"path":"/usr/bin/foo"}}
{"name":"file_events_recent","action":"added","columns":{"target_path":"/Users/x/.ssh/authorized_keys"}}
{"name":"pack_intrusion-detection_persistence_launchd","action":"added","columns":{"path":"/Library/LaunchAgents/com.example.plist","label":"com.example"}}
{"name":"pack_intrusion-detection_system_extensions_new","action":"added","columns":{"bundle_path":"/Applications/X.app","path":"/ignored"}}
{"name":"pack_intrusion-detection_suid_bin_unexpected","action":"added","columns":{"path":"/tmp/suid"}}
{"name":"new_admin_user","action":"added","columns":{"username":"eve"}}
EOF
  )"
  [[ $output == "$expected" ]]
}

@test "a tab inside a path is squashed to a space, so the enrich path stays one renderable token" {
  output="$(normalize_findings <<<'{"name":"es_launchd_writes","action":"added","columns":{"path":"/usr/bin/foo\tbar"}}')"
  [[ $output == '{"q":"es_launchd_writes","act":"added","cols":{"path":"/usr/bin/foo\tbar"},"ep":"/usr/bin/foo bar"}' ]]
}

# --- digest-store.sh: the recorded line --------------------------------------

@test "one append records a single line of derived triage fields and nothing else" {
  local line lines derived_line_pattern
  derived_line_pattern='^\{"timestamp":"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z"'
  derived_line_pattern+=',"detector":"system_extensions_new","category":"","identity":"com.example.ext"'
  derived_line_pattern+=',"action":"added","summary":"system_extensions_new com.example.ext"\}$'
  digest_append '{"q":"system_extensions_new","act":"added","cols":{"identifier":"com.example.ext","team":"TEAMID"},"ep":""}'
  mapfile -t lines <"$SPOOL"
  [[ ${#lines[@]} -eq 1 ]]
  line="${lines[0]}"
  [[ $line =~ $derived_line_pattern ]]
}

@test "appends accumulate, one line per finding, so the daily digest sees every one" {
  local lines
  digest_append '{"q":"system_extensions_new","act":"added","cols":{"identifier":"com.example.ext"},"ep":""}'
  digest_append '{"q":"new_admin_user","act":"added","cols":{"username":"eve"},"ep":""}'
  digest_append '{"q":"suid_bin_unexpected","act":"added","cols":{"path":"/tmp/suid"},"ep":""}'
  mapfile -t lines <"$SPOOL"
  [[ ${#lines[@]} -eq 3 ]]
}

@test "the spool is private: a 700 directory and a 600 file" {
  digest_append '{"q":"new_admin_user","act":"added","cols":{"username":"eve"},"ep":""}'
  [[ "$(perms_of "${SPOOL%/*}")" == 700 ]]
  [[ "$(perms_of "$SPOOL")" == 600 ]]
}

@test "a listening-port finding is identified by name, address and port together" {
  local line
  digest_append '{"q":"listening_ports_non_loopback","act":"added","cols":{"name":"nc","address":"0.0.0.0","port":"4444"},"ep":""}'
  IFS= read -r line <"$SPOOL"
  [[ $line == *'"identity":"nc 0.0.0.0:4444"'* ]]
}

@test "a finding's raw hash and secret column never reach the spool, only its path" {
  local line
  digest_append '{"q":"agent_authfile_changed","act":"added","cols":{"path":"/Users/x/.codex/config.toml","sha256":"deadbeefdeadbeef","secret_value":"SUPERSECRETTOKEN"},"ep":""}'
  IFS= read -r line <"$SPOOL"
  [[ $line == *'"identity":"/Users/x/.codex/config.toml"'* ]]
  [[ $line != *sha256* ]]
  [[ $line != *deadbeef* ]]
  [[ $line != *SUPERSECRETTOKEN* ]]
}

# --- digest-store.sh: a failed append ----------------------------------------

@test "a failed append never aborts the detection path" {
  # shellcheck disable=SC2030,SC2031 # a PATH local to this test is the point:
  # only the helper under test sees the failing jq.
  PATH="$BATS_FILE_TMPDIR/failing-jq:$PATH"
  run digest_append '{"q":"new_admin_user","act":"added","cols":{"username":"eve"},"ep":""}'
  [[ $status -eq 0 ]]
}

@test "a failed append says so on stderr, naming the spool it could not write" {
  local diagnostic
  # shellcheck disable=SC2030,SC2031 # a PATH local to this test is the point:
  # only the helper under test sees the failing jq.
  PATH="$BATS_FILE_TMPDIR/failing-jq:$PATH"
  digest_append '{"q":"new_admin_user","act":"added","cols":{"username":"eve"},"ep":""}' 2>"$BATS_TEST_TMPDIR/stderr"
  IFS= read -r diagnostic <"$BATS_TEST_TMPDIR/stderr"
  [[ $diagnostic == *digest-store* ]]
  [[ $diagnostic == *"$SPOOL"* ]]
}

@test "a failed append leaves no partial line behind" {
  # shellcheck disable=SC2030,SC2031 # a PATH local to this test is the point:
  # only the helper under test sees the failing jq.
  PATH="$BATS_FILE_TMPDIR/failing-jq:$PATH"
  digest_append '{"q":"new_admin_user","act":"added","cols":{"username":"eve"},"ep":""}' 2>/dev/null
  [[ ! -s $SPOOL ]]
}
