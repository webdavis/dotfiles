#!/usr/bin/env bash
#
# normalize_findings (results-alerter/normalize.sh) is the first stage of the
# alerter pipeline: it turns the raw osquery results-log tail into normalized
# finding NDJSON, one {q, act, cols, ep} object per surviving row. This suite
# pins every admission and shaping rule the stage owns:
#
#   B1 structure  - one finding per row; the query name stripped of its
#                   pack_<pack>_ prefix; an absent action defaulted to "changed";
#                   the columns object carried through; a snapshot-action row is a
#                   single finding (NO explosion, since the *_off queries are now
#                   differential); a malformed line drops out without aborting.
#   B2 select     - only recognized detector names are admitted; heartbeat_canary
#                   and any unknown pack/top-level name are dropped.
#   B3 filters    - renameio atomic-write churn (a target_path in a
#                   .renameio-TempDir) is dropped; a counter==0 differential
#                   baseline row is discarded EXCEPT for the three absolute-state
#                   queries (filevault_off, remote_access_sharing_state,
#                   agent_exposure_changed) whose first-run row must page.
#   B4 ep         - each finding carries the enrich-path the enricher must inspect
#                   (a plist/bundle/binary), per query type; empty when signing
#                   does not apply; tabs/newlines squashed to spaces.
#
# Performance: the suite batches its fixtures through ONE normalize pass per
# behavior group and one summary jq per group (not a jq spawn per row), so it
# stays inside the fast unit-admission budget. Coverage is unchanged; only the
# subprocess count is reduced.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
NORMALIZE="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/normalize.sh"

fail() {
  printf 'osquery-normalize: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $NORMALIZE ]] || fail "missing helper: $NORMALIZE"

# make_sut: run normalize_findings over the raw lines passed on stdin and print
# the normalized NDJSON to stdout. A fresh subshell per call keeps the helper's
# sourcing side-effect-free and the test order-independent.
make_sut() {
  bash -c "source '$NORMALIZE'; normalize_findings"
}

# summarize <jq-per-finding-expr> <ndjson>: reduce the normalized findings to a
# single deterministic block -- a "count=N" line then one sorted line per finding
# from the given jq expression. One jq spawn per behavior group; the whole block
# is compared at once, so every finding's shape is asserted together.
summarize() {
  local expr="$1" ndjson="$2"
  printf '%s' "$ndjson" | jq -s -r "
    (map($expr) | sort) as \$lines
    | \"count=\(\$lines | length)\" + (if (\$lines | length) > 0 then \"\n\" + (\$lines | join(\"\n\")) else \"\" end)
  "
}

# assert_block <expected> <actual> <context>: pure-bash equality, no subprocess.
assert_block() {
  local expected="$1" actual="$2" context="$3"
  [[ $actual == "$expected" ]] || fail "$context
--- expected ---
$expected
--- got ---
$actual
----------------"
}

# --- B1: structural normalization --------------------------------------------
# One pass over: a packed row (prefix strip + cols passthrough), a hyphenated
# pack (only the pack segment stripped), a bare top-level row, a row with no
# action (defaults to "changed"), a snapshot-action row (must stay ONE finding,
# not fan out its snapshot array), and a malformed line (drops out). The summary
# key is "q|act|<identity>" where identity is the row's username/path/target_path.
b1_structural_normalization() {
  local out
  out="$(
    make_sut <<'EOF'
{"name":"pack_intrusion-detection_suid_bin_unexpected","action":"added","columns":{"path":"/tmp/x"}}
{"name":"pack_agent-attack-surface_agent_exposure_changed","action":"added","columns":{}}
{"name":"new_admin_user","action":"added","columns":{"username":"alice"}}
{"name":"new_admin_user","columns":{"username":"bob"}}
{"name":"pack_security-policy-regression_filevault_state","action":"snapshot","snapshot":[{"path":"/a"},{"path":"/b"}]}
this is not json
EOF
  )"
  local got expected
  got="$(summarize '.q + "|" + .act + "|" + (.cols.username // .cols.path // .cols.target_path // "")' "$out")"
  expected='count=5
agent_exposure_changed|added|
filevault_state|snapshot|
new_admin_user|added|alice
new_admin_user|changed|bob
suid_bin_unexpected|added|/tmp/x'
  assert_block "$expected" "$got" \
    "B1: prefix strip, hyphenated-pack strip, bare top-level, action default, snapshot stays one finding, malformed drops, cols carried through"
}

# --- B2: known-query select (security allowlist) -----------------------------
# Only recognized detectors are admitted. heartbeat_canary (the liveness snapshot
# the alerter defensively drops) and any unknown pack/top-level name fall out.
b2_known_query_select() {
  local out
  out="$(
    make_sut <<'EOF'
{"name":"new_admin_user","action":"added","columns":{}}
{"name":"heartbeat_canary","action":"snapshot","columns":{}}
{"name":"pack_agent-attack-surface_agent_secretfile_changed","action":"added","columns":{}}
{"name":"pack_foo_bar","action":"added","columns":{}}
{"name":"pack_security-policy-regression_filevault_off","action":"added","columns":{}}
{"name":"totally_bogus_query","action":"added","columns":{}}
EOF
  )"
  local got expected
  got="$(summarize '.q' "$out")"
  expected='count=3
agent_secretfile_changed
filevault_off
new_admin_user'
  assert_block "$expected" "$got" \
    "B2: admit new_admin_user + agent_secretfile_changed + filevault_off; drop heartbeat_canary, pack_foo_bar, and an unknown top-level name"
}

# --- B3: renameio exclusion + counter==0 baseline discard ---------------------
# A renameio temp target is dropped; a counter==0 membership baseline (username
# root) is dropped; counter>0 (eve) and no-counter (mallory) survive; the three
# absolute-state queries keep their counter==0 first-run row.
b3_renameio_and_baseline() {
  local out
  out="$(
    make_sut <<'EOF'
{"name":"file_events_recent","action":"added","columns":{"target_path":"/Users/x/.config/foo/.renameio-TempDir-abc/bar"}}
{"name":"file_events_recent","action":"added","columns":{"target_path":"/Users/x/.ssh/authorized_keys"}}
{"name":"new_admin_user","action":"added","counter":0,"columns":{"username":"root"}}
{"name":"new_admin_user","action":"added","counter":1,"columns":{"username":"eve"}}
{"name":"new_admin_user","action":"added","columns":{"username":"mallory"}}
{"name":"pack_security-policy-regression_filevault_off","action":"added","counter":0,"columns":{}}
{"name":"pack_security-policy-regression_remote_access_sharing_state","action":"added","counter":0,"columns":{}}
{"name":"pack_agent-attack-surface_agent_exposure_changed","action":"added","counter":0,"columns":{}}
EOF
  )"
  local got expected
  got="$(summarize '.q + "|" + (.cols.username // .cols.target_path // "")' "$out")"
  expected='count=6
agent_exposure_changed|
file_events_recent|/Users/x/.ssh/authorized_keys
filevault_off|
new_admin_user|eve
new_admin_user|mallory
remote_access_sharing_state|'
  assert_block "$expected" "$got" \
    "B3: renameio temp dropped, real file survives, counter==0 membership (root) dropped, counter>0/no-counter survive, absolute-state counter==0 kept"
}

# --- B4: enrich-path (ep) per query type --------------------------------------
# ep is the exact path the enricher inspects, chosen per query: es_launchd_writes
# -> path; file_events_recent -> target_path; persistence_launchd -> path;
# system_extensions_new -> bundle_path (falling back to path); suid_bin_unexpected
# -> path; a query where signing does not apply -> "". Tabs/newlines are squashed
# to spaces (the es row carries a tab to pin that).
b4_enrich_path() {
  local out
  out="$(
    make_sut <<'EOF'
{"name":"es_launchd_writes","action":"added","columns":{"path":"/usr/bin/foo\tbar"}}
{"name":"file_events_recent","action":"added","columns":{"target_path":"/Users/x/.ssh/authorized_keys"}}
{"name":"pack_intrusion-detection_persistence_launchd","action":"added","columns":{"path":"/Library/LaunchAgents/com.example.plist","label":"com.example"}}
{"name":"pack_intrusion-detection_system_extensions_new","action":"added","columns":{"bundle_path":"/Applications/X.app","path":"/ignored"}}
{"name":"pack_intrusion-detection_suid_bin_unexpected","action":"added","columns":{"path":"/tmp/suid"}}
{"name":"new_admin_user","action":"added","columns":{"username":"eve"}}
EOF
  )"
  local got expected
  got="$(summarize '.q + "|" + (.ep // "")' "$out")"
  expected='count=6
es_launchd_writes|/usr/bin/foo bar
file_events_recent|/Users/x/.ssh/authorized_keys
new_admin_user|
persistence_launchd|/Library/LaunchAgents/com.example.plist
suid_bin_unexpected|/tmp/suid
system_extensions_new|/Applications/X.app'
  assert_block "$expected" "$got" \
    "B4: ep per query type (launchd-write path, file-event target_path, persistence path, sysext bundle_path over path, suid path, empty when signing n/a, tab squashed)"
}

b1_structural_normalization
b2_known_query_select
b3_renameio_and_baseline
b4_enrich_path

printf 'osquery-normalize: OK (B1 structure; B2 known-query select; B3 renameio + counter==0 baseline; B4 enrich-path)\n'
