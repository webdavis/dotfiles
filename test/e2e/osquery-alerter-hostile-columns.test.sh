#!/usr/bin/env bash
# Security: no attacker-controlled column value can be reinterpreted as record
# structure. The gate must route on the finding's ACTUAL fields, so an embedded
# separator (0x1F), newline, or tab in a crafted .cols.path/label/program can
# never shift field boundaries to make an unknown plist read as an allowlisted
# tuple (which would suppress it). Driven end-to-end through the REAL entry.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit: both belong to a script that runs on its own, and either one
# here would reach into the runner's own shell. The shebang stays for shellcheck
# and for editors, and is never executed. test/validate-tests.sh pins that shape;
# `just test-e2e` runs it.
#
# Every check below is a real bashunit assertion. bashunit runs a test function
# under `set +euo pipefail`, so a bare `grep -q` reports nothing and passes
# silently, and the bats file's refute_paged, which merely returned 1 after
# printing, was just as invisible: `set -e` never fired on it under bats either,
# which is why that helper existed. The refute keeps its diagnostic by asserting
# the matching lines are empty, so a failure shows the page that fired.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ENTRY="$REPO_ROOT/dot_local/libexec/osquery/executable_results-alerter.sh"
HELPER_SRC="$REPO_ROOT/dot_local/libexec/osquery/results-alerter"

function set_up() {
  HOME_DIR="$(mktemp -d)"
  # Record ownership only after our own mktemp, so tear_down removes this path
  # and never a pre-set or inherited HOME_DIR.
  _HOSTILE_OWNED_DIR="$HOME_DIR"
  export HOME="$HOME_DIR"
  mkdir -p "$HOME/.local/libexec/osquery/results-alerter" "$HOME/.local/state" \
    "$HOME/.local/log/osquery" "$HOME/.config/osquery" "$HOME/Library/LaunchAgents" "$HOME/bin"
  cp "$HELPER_SRC"/*.sh "$HOME/.local/libexec/osquery/results-alerter/"

  export SEND_ALERT_SPY="$HOME/send_alert.log"
  : >"$SEND_ALERT_SPY"
  cat >"$HOME/.local/libexec/osquery/alert-dispatch.sh" <<'STUB'
# shellcheck shell=bash
send_alert() {
  { printf 'CALL\tseverity=%s\ttitle=%s\n' "$1" "$2"; printf 'DETAIL-START\n%s\nDETAIL-END\n' "$3"; } >>"$SEND_ALERT_SPY"
  return "${SEND_ALERT_RC:-0}"
}
STUB
  # Trusted enricher (so a promotion never masks a suppression bug: the page must
  # come from default-deny, not from enrichment).
  cat >"$HOME/enrich-stub.sh" <<'STUB'
#!/usr/bin/env bash
printf %s "signed: Apple"
exit 0
STUB
  chmod +x "$HOME/enrich-stub.sh"
  export OSQUERY_ENRICH_SCRIPT="$HOME/enrich-stub.sh"

  # An own-agent allowlist entry the attacker will try to impersonate by injection.
  cat >"$HOME/.config/osquery/page-launchd-allowlist.txt" <<EOF
{"label":"com.good","path":"~/Library/LaunchAgents/com.good.plist","program":"~/bin/good","sha256":""}
EOF

  # Bind the verdict to a SANDBOX manifest. Left unset, pipeline-verdict.sh falls
  # back to its production default, /var/osquery/pipeline-known-good.sha256, and
  # the run then reads a real machine path this test never wrote. That made the
  # outcome a property of the host rather than of the code, in both directions:
  # on a machine without that file nothing can ever vouch for the allowlist, so
  # allowlist_verdict cannot suppress ANYTHING and each "the finding pages"
  # assertion below passed without the suppression path existing at all; on a
  # machine that has one, the same run also spends the 5-second settle loop per
  # finding. Binding it here makes suppression genuinely reachable, which is what
  # gives the injection pins something to defeat, and the HOSTILE-control test at
  # the bottom of this file is what proves that reachability on every run.
  export OSQUERY_PIPELINE_MANIFEST="$HOME/pipeline-known-good.sha256"
  export OSQUERY_PIPELINE_SETTLE_SECONDS=0
  printf 'FIXTURE PLIST\n' >"$HOME/Library/LaunchAgents/com.good.plist"
  printf 'FIXTURE PROGRAM\n' >"$HOME/bin/good"
  : >"$OSQUERY_PIPELINE_MANIFEST"
  # The allowlist FILE (the verdict's last gate) and the unpinned entry's plist
  # (its per-entry authority) are the two paths the manifest has to account for.
  chmod 600 "$HOME/.config/osquery/page-launchd-allowlist.txt"
  _bless_path "$HOME/.config/osquery/page-launchd-allowlist.txt"
  _bless_path "$HOME/Library/LaunchAgents/com.good.plist"

  export OSQUERY_RESULTS_LOG="$HOME/.local/log/osquery/osqueryd.results.log"
  export OSQUERY_RESULTS_OFFSET="$HOME/.local/state/osquery-results-offset"
  : >"$OSQUERY_RESULTS_LOG"
}

function tear_down() {
  [[ -n ${_HOSTILE_OWNED_DIR:-} ]] || return 0
  rm -rf "$_HOSTILE_OWNED_DIR"
  unset _HOSTILE_OWNED_DIR
}

# _bless_path <path> - append the path's current tuple to the sandbox manifest.
_bless_path() {
  local raw mode
  raw=$(stat -c '%a' "$1" 2>/dev/null || stat -f '%OLp' "$1" 2>/dev/null) || return 1
  mode="000$raw"
  printf '%s %s %s %s\n' \
    "$(shasum -a 256 "$1" | awk '{print $1}')" "${mode: -4}" "$(id -u)" "$1" \
    >>"$OSQUERY_PIPELINE_MANIFEST"
}

# The cursor's inode field, read the way the entry itself reads it. GNU and BSD
# stat spell the inode under different flags, so `ls -i` is the one call that
# runs on both; the entry carries the same disable, for the same reason, on the
# same call.
# shellcheck disable=SC2012  # a fixed mktemp path this file created; ls -i is safe and portable
log_inode() { ls -i "$OSQUERY_RESULTS_LOG" | awk '{print $1}'; }
seed_cursor() { printf '%s 0\n' "$(log_inode)" >"$OSQUERY_RESULTS_OFFSET"; }

# run_entry - run the alerter once and assert it exited 0, the way the bats
# `run bash "$ENTRY"; [ "$status" -eq 0 ]` pair did. The merged output is kept
# only to print it when the status is wrong, which is more than the bats form
# offered: it captured the output and then discarded it.
run_entry() {
  local status=0 output
  output="$(bash "$ENTRY" 2>&1)" || status=$?
  [[ $status -eq 0 ]] || printf 'entry exited %s:\n%s\n' "$status" "$output"
  assert_exit_code 0 "" "$status"
}

assert_paged() { assert_file_contains "$SEND_ALERT_SPY" 'severity=CRIT'; }
# The refute reports the offending lines as the assertion's actual value, keeping
# the diagnostic the bats helper printed by hand before returning 1.
refute_paged() { assert_same '' "$(grep -e 'severity=CRIT' "$SEND_ALERT_SPY" || true)"; }

# HEADLINE: a persistence_launchd finding whose crafted .cols.path embeds a 0x1F
# tuple (path\x1flabel\x1fprogram) matching the allowlisted own-agent - under an
# in-band separator this splits so allowlist_verdict reads (com.good, the-good-
# path, the-good-program) and SUPPRESSES the malicious agent. It must PAGE: the
# real label is com.attacker (unknown) -> default-deny.
function test_a_0x1f_injected_path_cannot_impersonate_an_allowlisted_tuple_and_the_finding_pages() {
  seed_cursor
  local good_path="$HOME/Library/LaunchAgents/com.good.plist" good_prog="$HOME/bin/good"
  # .cols.path = "<good_path><0x1f>com.good<0x1f><good_prog>" (0x1F as \u001f in JSON).
  printf '{"name":"pack_intrusion-detection_persistence_launchd","action":"added","columns":{"label":"com.attacker","path":"%s\\u001fcom.good\\u001f%s","program":"/attacker/mal"}}\n' \
    "$good_path" "$good_prog" >>"$OSQUERY_RESULTS_LOG"
  run_entry
  assert_paged
}

# A newline embedded in .cols.label must not truncate or split the record; the
# unknown agent still pages (default-deny), never silently lost.
function test_a_newline_in_a_column_does_not_split_the_record_and_the_finding_pages() {
  seed_cursor
  printf '{"name":"pack_intrusion-detection_persistence_launchd","action":"added","columns":{"label":"com.attacker\\ncom.good","path":"%s/Library/LaunchAgents/evil.plist","program":"%s/bin/evil"}}\n' \
    "$HOME" "$HOME" >>"$OSQUERY_RESULTS_LOG"
  run_entry
  assert_paged
}

# CONTROL for the three pins above: the GENUINE allowlisted own-agent, named with
# no injected byte at all, is suppressed. Without it this file could pass while
# allowlist_verdict was incapable of suppressing anything (its manifest missing,
# its allowlist unreadable, the helper never sourced), leaving the three "it pages"
# assertions vacuously true. This test fails the moment suppression stops working,
# which is what makes their PAGE verdicts evidence rather than a default.
function test_the_genuine_allowlisted_own_agent_is_suppressed_so_the_injection_pins_are_not_vacuous() {
  seed_cursor
  printf '{"name":"pack_intrusion-detection_persistence_launchd","action":"added","columns":{"label":"com.good","path":"%s/Library/LaunchAgents/com.good.plist","program":"%s/bin/good"}}\n' \
    "$HOME" "$HOME" >>"$OSQUERY_RESULTS_LOG"
  run_entry
  refute_paged
}

# A tab embedded in .cols.program must stay an opaque value; the unknown agent pages.
function test_a_tab_in_a_column_stays_opaque_and_the_finding_pages() {
  seed_cursor
  printf '{"name":"pack_intrusion-detection_persistence_launchd","action":"added","columns":{"label":"com.attacker","path":"%s/Library/LaunchAgents/evil.plist","program":"%s/bin/evil\\tcom.good"}}\n' \
    "$HOME" "$HOME" >>"$OSQUERY_RESULTS_LOG"
  run_entry
  assert_paged
}
