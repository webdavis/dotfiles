#!/usr/bin/env bash
# lulu-rule-existence-reader.sh -- the poller's LuLu rule-existence readers,
# unit-tested through the real poller against stubs (no LuLu, no live archive).
#
# LuLu's rules.plist is an NSKeyedArchiver archive of a private class: not
# hand-authorable, and not safely interpretable beyond the path strings it
# mentions. The readers therefore make an EXISTENCE-ONLY claim: the archive
# mentions the declared binary path. They read via `plutil -convert xml1 -o -`
# (a read-only conversion to stdout; the archive file is never written) and
# match the exact <string> element.
#
#   R1 lulu_rule_present: the archive mentioning the target reads present; an
#      archive not mentioning it reads absent; the plutil invocation is the
#      exact read-only argv, once per tick.
#   R2 substring honesty: a longer path that CONTAINS the target does not
#      count as the target (the closing </string> pins the element boundary).
#   R3 XML escaping: a target containing & is matched against its XML-escaped
#      form, so an escaped archive entry still reads present.
#   R4 failure discipline: a failed plutil is INDETERMINATE (a monitoring
#      gap), never absent (absent would page a deviation on a read nobody
#      completed); a zero-exit plutil that printed nothing is indeterminate
#      too (plutil always prints the converted document on success).
#   R5 lulu_rule_resolved_present: the target is resolved (readlink -f) at
#      probe time and the RESOLVED path is matched, so the control follows
#      the live binary behind a launcher symlink; an unresolvable target is
#      indeterminate, never absent.
set -euo pipefail

# Scrubbed at SCRIPT scope. Git exports GIT_DIR to every hook it runs and this
# suite runs from the pre-push hook.
unset MACOS_DEFAULTS_SOURCE_DIR GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# shellcheck source=../fixtures/osquery-poller-lib.bash
source "$REPO_ROOT/test/fixtures/osquery-poller-lib.bash"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

command -v jq >/dev/null 2>&1 || {
  printf 'SKIP: jq not on PATH; cannot exercise the rule-existence readers\n'
  exit 0
}

# archive_xml_mentioning <path>... -- a minimal plutil-shaped XML body whose
# $objects array mentions each (already-escaped) path string.
archive_xml_mentioning() {
  local body="" mentioned_path
  for mentioned_path in "$@"; do
    body+="		<string>$mentioned_path</string>"$'\n'
  done
  # shellcheck disable=SC2016 # the literal $objects key is the archive format
  printf '<?xml version="1.0" encoding="UTF-8"?>\n<plist version="1.0">\n<dict>\n	<key>$objects</key>\n	<array>\n%s	</array>\n</dict>\n</plist>\n' "$body"
}

rule_control() { # <id> <reader> <target> -- one verify record as a JSON array
  jq -cn --arg id "$1" --arg reader "$2" --arg target "$3" \
    '[{id: $id, description: ("The LuLu allow rule for " + $id), tier: "verify", reader: $reader, expect: "present", target: $target}]'
}

# run_reader_case <controls-json> -> runs one first-observation tick; the
# caller programs the stubs first and asserts pages/baseline after.
run_reader_case() {
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
  set_posture_controls "$1"
  run_poller >/dev/null 2>&1
}

# ---- R1: present / absent, via the exact read-only plutil argv ---------------

setup_poller_harness
trap 'teardown_poller_harness' EXIT
POLLER_PLUTIL_XML="$(archive_xml_mentioning /usr/local/bin/tailscaled)"
export POLLER_PLUTIL_XML
run_reader_case "$(rule_control demo_rule lulu_rule_present /usr/local/bin/tailscaled)" ||
  fail "R1 present: expected exit 0"
assert_no_page || fail "R1 present: a mentioned target must read present (silent, healthy)"
assert_baseline_scalar demo_rule present || fail "R1 present: baseline must record present"
assert_probe_argv "plutil -convert xml1 -o - $OSQUERY_POSTURE_LULU_RULES" 1 ||
  fail "R1 present: the reader must run exactly one read-only plutil conversion"
assert_no_mutation_attempt || fail "R1 present: the reader must never mutate"
teardown_poller_harness
trap - EXIT

setup_poller_harness
trap 'teardown_poller_harness' EXIT
POLLER_PLUTIL_XML="$(archive_xml_mentioning /opt/unrelated/binary)"
export POLLER_PLUTIL_XML
run_reader_case "$(rule_control demo_rule lulu_rule_present /usr/local/bin/tailscaled)" ||
  fail "R1 absent: expected exit 0"
assert_page_count 1 || fail "R1 absent: an unmentioned target is a deviation (first observation)"
assert_baseline_scalar demo_rule absent || fail "R1 absent: baseline must record absent"
teardown_poller_harness
trap - EXIT

# ---- R2: a longer path containing the target is NOT the target ---------------

setup_poller_harness
trap 'teardown_poller_harness' EXIT
POLLER_PLUTIL_XML="$(archive_xml_mentioning /usr/local/bin/tailscaled-helper)"
export POLLER_PLUTIL_XML
run_reader_case "$(rule_control demo_rule lulu_rule_present /usr/local/bin/tailscaled)" ||
  fail "R2: expected exit 0"
assert_page_count 1 || fail "R2: a superstring path must not satisfy the target (absent must page)"
assert_baseline_scalar demo_rule absent || fail "R2: baseline must record absent"
teardown_poller_harness
trap - EXIT

# ---- R3: the target is matched in its XML-escaped form -----------------------

setup_poller_harness
trap 'teardown_poller_harness' EXIT
POLLER_PLUTIL_XML="$(archive_xml_mentioning '/opt/tools/fetch &amp; sync')"
export POLLER_PLUTIL_XML
run_reader_case "$(rule_control demo_rule lulu_rule_present '/opt/tools/fetch & sync')" ||
  fail "R3: expected exit 0"
assert_no_page || fail "R3: an ampersand target must match its XML-escaped archive form"
assert_baseline_scalar demo_rule present || fail "R3: baseline must record present"
teardown_poller_harness
trap - EXIT

# ---- R4: plutil failures are indeterminate, never absent ---------------------

run_plutil_failure_case() { # <label> <env-assignment>...
  local label="$1" poller_status=0 env_assignment
  shift
  setup_poller_harness
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
  set_posture_controls "$(rule_control demo_rule lulu_rule_present /usr/local/bin/tailscaled)"
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1","demo_rule":"present","demo_rule:expect":"present","demo_rule:target":"/usr/local/bin/tailscaled"}'
  snapshot_baseline
  for env_assignment in "$@"; do
    export "${env_assignment?}"
  done
  run_poller >/dev/null 2>&1 || poller_status=$?
  [[ $poller_status -eq 0 ]] ||
    fail "R4 $label: expected exit 0 after paging the gap, got $poller_status"
  assert_page_count 1 || fail "R4 $label: a failed archive read must page a monitoring gap"
  assert_page_body_has 'monitoring gap' || fail "R4 $label: the page must name the gap"
  assert_page_body_has 'demo_rule' || fail "R4 $label: the gap page must name the control"
  assert_baseline_unchanged ||
    fail "R4 $label: an indeterminate read must never advance the baseline"
  for env_assignment in "$@"; do
    unset "${env_assignment%%=*}"
  done
  teardown_poller_harness
}

# A nonzero plutil (missing or unreadable archive, a corrupt document).
run_plutil_failure_case 'nonzero-exit' POLLER_PLUTIL_EXIT=1
# A zero exit that printed nothing: plutil prints the converted document on
# success, so an empty success is a status/output mismatch, never absent.
run_plutil_failure_case 'exit-0-without-output' POLLER_PLUTIL_XML= POLLER_PLUTIL_EXIT=0

# ---- R5: the resolved reader follows the launcher symlink --------------------

# The readlink stub resolves identically by default; program a distinct
# resolution and require the RESOLVED path, not the declared launcher, to be
# what the archive is searched for.
setup_poller_harness
trap 'teardown_poller_harness' EXIT
export POLLER_READLINK_OUTPUT='/real/interpreters/cpython-3.11/python3.11'
POLLER_PLUTIL_XML="$(archive_xml_mentioning /real/interpreters/cpython-3.11/python3.11)"
export POLLER_PLUTIL_XML
run_reader_case "$(rule_control demo_resolved lulu_rule_resolved_present /opt/venv/bin/python)" ||
  fail "R5 resolved-present: expected exit 0"
assert_no_page || fail "R5 resolved-present: the archive mentioning the RESOLVED path must read present"
assert_baseline_scalar demo_resolved present ||
  fail "R5 resolved-present: baseline must record present"
assert_probe_argv 'readlink -f /opt/venv/bin/python' 1 ||
  fail "R5 resolved-present: the reader must resolve the declared launcher exactly once"
teardown_poller_harness
trap - EXIT

# The archive mentioning only the LAUNCHER path while the resolution points
# elsewhere reads absent: the rule must cover the binary that executes.
setup_poller_harness
trap 'teardown_poller_harness' EXIT
export POLLER_READLINK_OUTPUT='/real/interpreters/cpython-3.11/python3.11'
POLLER_PLUTIL_XML="$(archive_xml_mentioning /opt/venv/bin/python)"
export POLLER_PLUTIL_XML
run_reader_case "$(rule_control demo_resolved lulu_rule_resolved_present /opt/venv/bin/python)" ||
  fail "R5 launcher-only: expected exit 0"
assert_page_count 1 ||
  fail "R5 launcher-only: a rule on the launcher path alone must read absent (the resolved binary is unruled)"
assert_baseline_scalar demo_resolved absent ||
  fail "R5 launcher-only: baseline must record absent"
teardown_poller_harness
trap - EXIT

# An unresolvable target (the launcher is gone) is indeterminate, never absent.
setup_poller_harness
trap 'teardown_poller_harness' EXIT
export POLLER_READLINK_EXIT=1
POLLER_PLUTIL_XML="$(archive_xml_mentioning /real/interpreters/cpython-3.11/python3.11)"
export POLLER_PLUTIL_XML
set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
set_posture_controls "$(rule_control demo_resolved lulu_rule_resolved_present /opt/venv/bin/python)"
seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1","demo_resolved":"present","demo_resolved:expect":"present","demo_resolved:target":"/opt/venv/bin/python"}'
snapshot_baseline
run_poller >/dev/null 2>&1 || fail "R5 unresolvable: expected exit 0 after paging the gap"
assert_page_count 1 || fail "R5 unresolvable: an unresolvable launcher must page a monitoring gap"
assert_page_body_has 'monitoring gap' || fail "R5 unresolvable: the page must name the gap"
assert_baseline_unchanged ||
  fail "R5 unresolvable: an unresolvable target must never advance the baseline"
unset POLLER_READLINK_EXIT POLLER_READLINK_OUTPUT POLLER_PLUTIL_XML
teardown_poller_harness
trap - EXIT

printf 'ok: LuLu rule-existence readers (exact-element existence claim, XML escaping, indeterminate-on-failure, resolved-target following)\n'
