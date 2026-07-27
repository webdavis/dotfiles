#!/usr/bin/env bash
# macos-posture-controls-agreement.sh -- the poller's monitored set and the
# repo's declared record set are ENUMERATED AND DIFFED (slice 6): a control
# declared in .chezmoidata/macos_posture_controls.yaml but never read, or read
# but never declared, FAILS here rather than passing quietly. Completeness
# guards beat count guards: this is set equality, not a length check.
#
# How: convert the real YAML records to the JSON the poller consumes (the
# render test pins that the chezmoi template produces exactly this), program
# every stub healthy FROM the records themselves, run the real poller once, and
# require the persisted baseline's key set to equal {firewall, gatekeeper,
# screenlock} plus exactly the declared ids. A record the poller drops leaves a
# missing key; a read the poller hardcodes adds an extra one; either diffs.
#
# A record naming a reader this test cannot map fails LOUDLY: slices 9 and 10
# add records here, and each new reader must land with a poller function, a
# harness stub, and a mapping below, or this gate refuses.
set -euo pipefail

# Scrubbed at SCRIPT scope. Git exports GIT_DIR to every hook it runs and this
# suite runs from the pre-push hook.
unset MACOS_DEFAULTS_SOURCE_DIR GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DATA_FILE="$REPO_ROOT/.chezmoidata/macos_posture_controls.yaml"

# shellcheck source=../fixtures/osquery-poller-lib.bash
source "$REPO_ROOT/test/fixtures/osquery-poller-lib.bash"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

for tool in yq jq; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'SKIP: %s not on PATH; cannot exercise the declared-controls agreement\n' "$tool"
    exit 0
  }
done
[[ -f $DATA_FILE ]] || fail "missing data file: $DATA_FILE"

records_json="$(yq -o=json '.macos.posture_controls' "$DATA_FILE")" ||
  fail "could not convert the declared records to JSON"
jq -e 'type == "array" and length > 0' <<<"$records_json" >/dev/null ||
  fail "macos_posture_controls.yaml must declare a non-empty record list"

# Every shipped record is verify; anything else has no business in a file the
# poller only reads from.
bad_tiers="$(jq -r '.[] | select(.tier != "verify") | .id' <<<"$records_json")"
[[ -z $bad_tiers ]] || fail "non-verify tier(s) in the shipped data: $bad_tiers"

setup_poller_harness
trap 'teardown_poller_harness' EXIT

set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
set_posture_controls "$records_json"

# Program every stub HEALTHY from the record's own reader and expect, so the
# run is silent and the baseline seeds. An unmapped reader fails the gate.
while IFS=$'\t' read -r reader expect; do
  case "$reader" in
    fdesetup_status)
      if [[ $expect == "on" ]]; then
        export POLLER_FDESETUP_OUTPUT="FileVault is On."
      else
        export POLLER_FDESETUP_OUTPUT="FileVault is Off."
      fi
      ;;
    csrutil_status)
      export POLLER_CSRUTIL_OUTPUT="System Integrity Protection status: ${expect}."
      ;;
    defaults_autologin)
      if [[ $expect == "on" ]]; then
        export POLLER_DEFAULTS_AUTOLOGIN_MODE=present
      else
        export POLLER_DEFAULTS_AUTOLOGIN_MODE=absent
      fi
      ;;
    sysadminctl_guest)
      export POLLER_SYSADMINCTL_GUEST_OUTPUT="stub sysadminctl Guest account ${expect}."
      ;;
    *)
      fail "record reader '$reader' has no mapping here: add the poller reader, the harness stub, and this mapping together"
      ;;
  esac
done < <(jq -r '.[] | [.reader, .expect] | @tsv' <<<"$records_json")

poller_status=0
poller_output="$(run_poller 2>&1)" || poller_status=$?
[[ $poller_status -eq 0 ]] ||
  fail "the poller must exit 0 on an all-healthy declared set, got $poller_status: $poller_output"
[[ ! -s $POLLER_SEND_ALERT_LOG ]] ||
  fail "an all-healthy declared set must page nothing; send_alert saw: $(cat "$POLLER_SEND_ALERT_LOG")"
[[ -f $OSQUERY_POSTURE_STATE ]] ||
  fail "the healthy run must seed a baseline"

# THE DIFF: baseline keys == built-in fields + declared ids, exactly.
expected_keys="$POLLER_HOME/expected-keys"
actual_keys="$POLLER_HOME/actual-keys"
{
  printf '%s\n' firewall gatekeeper screenlock
  jq -r '.[].id' <<<"$records_json"
  # One recorded-declaration field per control: the poller persists the expect
  # each value was recorded under, so a changed declaration re-arms the control
  # instead of reading as a silent steady-deviant.
  jq -r '.[].id + ":expect"' <<<"$records_json"
} | LC_ALL=C sort >"$expected_keys"
jq -r 'keys_unsorted[]' "$OSQUERY_POSTURE_STATE" | LC_ALL=C sort >"$actual_keys"
diff -u "$expected_keys" "$actual_keys" >&2 ||
  fail "monitored set != declared set: a control declared but never read (missing key) or read but never declared (extra key)"

printf 'ok: the poller monitors exactly the declared posture controls\n'
