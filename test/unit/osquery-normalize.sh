#!/usr/bin/env bash
#
# normalize_findings (results-alerter/normalize.sh) turns each raw osquery
# results-log row into exactly one JSON finding per row. This behavior pins the
# structural core: the query name is stripped of its pack_<pack>_ prefix so a
# packed query and a top-level query render under the same bare name the routing
# stage matches, and a row that omits its action defaults to "changed" so a
# downstream stage never has to special-case a null.
#
# The snapshot-explosion decision (S9): c69baab made the absolute-state *_off
# queries DIFFERENTIAL, so their off-state now arrives as an ordinary added row.
# There is therefore no snapshot array to explode; normalize emits ONE finding
# per row, and a snapshot-action row is a single finding, never fanned out.
#
# Unit test: fixture-driven. Feed representative raw lines on stdin, read the
# normalized NDJSON back, assert the shape. No filesystem, no notifier, no sleeps.
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

# assert_line_count <expected> <ndjson> <context>: exactly N non-empty findings.
assert_line_count() {
  local expected="$1" ndjson="$2" context="$3" got
  got="$(printf '%s' "$ndjson" | grep -c . || true)"
  [[ $got -eq $expected ]] ||
    fail "$context: expected $expected finding(s), got $got -- output was: $ndjson"
}

# assert_field <jq-filter> <expected> <ndjson> <context>: the filter applied to
# the (single-finding) NDJSON yields the expected value.
assert_field() {
  local filter="$1" expected="$2" ndjson="$3" context="$4" got
  got="$(printf '%s' "$ndjson" | jq -r "$filter")"
  [[ $got == "$expected" ]] ||
    fail "$context: expected '$expected', got '$got' -- output was: $ndjson"
}

# A packed row strips its pack_<pack>_ prefix to the bare query name.
a_pack_row_strips_its_prefix() {
  local out
  out="$(printf '%s\n' \
    '{"name":"pack_intrusion-detection_suid_bin_unexpected","action":"added","columns":{"path":"/tmp/x"}}' |
    make_sut)"
  assert_line_count 1 "$out" "a packed row produces one finding"
  assert_field '.q' 'suid_bin_unexpected' "$out" "the pack_<pack>_ prefix is stripped"
  assert_field '.act' 'added' "$out" "the action is carried through"
  assert_field '.cols.path' '/tmp/x' "$out" "the columns object is carried through"
}

# A pack whose name contains hyphens strips only the pack segment, not the query.
a_hyphenated_pack_row_strips_only_the_pack_segment() {
  local out
  out="$(printf '%s\n' \
    '{"name":"pack_agent-attack-surface_agent_exposure_changed","action":"added","columns":{}}' |
    make_sut)"
  assert_field '.q' 'agent_exposure_changed' "$out" "only the pack segment is stripped, the query keeps its underscores"
}

# A top-level (non-pack) row keeps its bare name unchanged.
a_top_level_row_keeps_its_bare_name() {
  local out
  out="$(printf '%s\n' \
    '{"name":"new_admin_user","action":"added","columns":{"username":"eve"}}' |
    make_sut)"
  assert_line_count 1 "$out" "a top-level row produces one finding"
  assert_field '.q' 'new_admin_user' "$out" "a bare top-level name is left intact"
}

# A row that omits its action defaults the action to "changed".
a_row_without_an_action_defaults_to_changed() {
  local out
  out="$(printf '%s\n' \
    '{"name":"new_admin_user","columns":{}}' |
    make_sut)"
  assert_field '.act' 'changed' "$out" "an absent action defaults to changed"
}

# A snapshot-action row is one finding, never exploded into one-per-snapshot-entry.
a_snapshot_row_yields_exactly_one_finding() {
  local out
  out="$(printf '%s\n' \
    '{"name":"pack_security-policy-regression_filevault_state","action":"snapshot","snapshot":[{"path":"/a"},{"path":"/b"}]}' |
    make_sut)"
  assert_line_count 1 "$out" "a snapshot row is NOT fanned out into one finding per snapshot entry"
  assert_field '.act' 'snapshot' "$out" "the snapshot action is carried through as-is"
}

# Every input row yields its own finding: three rows in, three findings out.
each_row_yields_its_own_finding() {
  local out
  out="$(printf '%s\n' \
    '{"name":"new_admin_user","action":"added","columns":{}}' \
    '{"name":"pack_intrusion-detection_suid_bin_unexpected","action":"added","columns":{}}' \
    '{"name":"pack_agent-attack-surface_agent_exposure_changed","action":"removed","columns":{}}' |
    make_sut)"
  assert_line_count 3 "$out" "one finding per input row"
}

# A malformed (non-JSON) line yields no finding for that line and never aborts
# the batch: the valid rows around it still normalize.
a_malformed_line_is_skipped_without_aborting_the_batch() {
  local out
  out="$(printf '%s\n' \
    '{"name":"new_admin_user","action":"added","columns":{}}' \
    'this is not json' \
    '{"name":"pack_intrusion-detection_suid_bin_unexpected","action":"added","columns":{}}' |
    make_sut)"
  assert_line_count 2 "$out" "the malformed line drops out, the two valid rows survive"
}

a_pack_row_strips_its_prefix
a_hyphenated_pack_row_strips_only_the_pack_segment
a_top_level_row_keeps_its_bare_name
a_row_without_an_action_defaults_to_changed
a_snapshot_row_yields_exactly_one_finding
each_row_yields_its_own_finding
a_malformed_line_is_skipped_without_aborting_the_batch

printf 'osquery-normalize: OK (prefix strip, hyphenated pack, bare top-level, action default, no snapshot explosion, one-per-row, malformed-skip)\n'
