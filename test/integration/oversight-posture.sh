#!/usr/bin/env bash
# oversight-posture.sh -- slice 9: OverSight's two posture records.
#
#   1. A verify-tier control asserting the OverSight monitor PROCESS is
#      actually running, read by the security-posture poller through the
#      pgrep_oversight reader. The check is a live process-table probe,
#      deliberately NOT the presence of /Applications/OverSight.app on disk:
#      the bundle sits on disk whether or not the monitor is watching
#      anything, so presence-on-disk would report healthy for a quit monitor.
#      Every behaviour below drives the REAL poller against a STUBBED pgrep,
#      so the suite passes regardless of whether OverSight is running (or even
#      installed) on the machine running it; the stopped-lifecycle behaviour
#      is itself the proof that the state comes from the probe and not the
#      disk, because on the development machine the bundle exists while the
#      stub says stopped, and the poller pages anyway.
#   2. A manual record for OverSight's Notification Center delivery
#      (interactive-only; no supported command line writes it), declared in
#      macos_system_setup.yaml with a runbook pointer whose section must
#      exist. Deliberately NOT a microphone or camera permission record: the
#      installed bundle declares no usage-description keys and no
#      entitlements, so macOS never offers those grants and OverSight never
#      asks (it observes device activation events, not device content).
#
# The behaviours, each against the REAL repo record (extracted from
# .chezmoidata/macos_posture_controls.yaml, so a fixture cannot drift from
# what ships):
#
#   B1 healthy: monitor running -> silent tick, exactly one exact-argv pgrep
#      probe, the baseline gains oversight/oversight:expect.
#   B2 lifecycle: stopped pages EXACTLY ONE CRIT naming the control and its
#      remedy, stays quiet while stopped, silently re-arms on restore, and a
#      LATER stop pages again.
#   B3 indeterminate: a probe that exits nonzero is INDETERMINATE, never
#      running (and never stopped): pid-looking output with a failed status,
#      the no-match status that still printed, and a zero exit with no output
#      all gap; the baseline is byte-for-byte untouched.
#   B4 tier guard: the shipped record is tier: verify; flipped to enforce it
#      is refused by the render template AND by the poller before any probe
#      runs.
#   B5 manual record: the notification-delivery record exists in
#      macos_system_setup.yaml at tier: manual with no mutating payload, the
#      runbook section it names exists, the section directs the operator at
#      the Notifications pane, and neither the record nor the section directs
#      the ungrantable microphone or camera privacy step.
set -euo pipefail

# Scrubbed at SCRIPT scope. Git exports GIT_DIR to every hook it runs and this
# suite runs from the pre-push hook.
unset MACOS_DEFAULTS_SOURCE_DIR GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DATA_FILE="$REPO_ROOT/.chezmoidata/macos_posture_controls.yaml"
SETUP_FILE="$REPO_ROOT/.chezmoidata/macos_system_setup.yaml"
TEMPLATE="$REPO_ROOT/dot_local/libexec/osquery/posture-controls.json.tmpl"
RUNBOOK="$REPO_ROOT/docs/runbooks/macos-fresh-machine-quickstart.md"

# shellcheck source=../fixtures/osquery-poller-lib.bash
source "$REPO_ROOT/test/fixtures/osquery-poller-lib.bash"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

for tool in yq jq chezmoi; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'SKIP: %s not on PATH; cannot exercise the OverSight posture records\n' "$tool"
    exit 0
  }
done
for required_file in "$DATA_FILE" "$SETUP_FILE" "$TEMPLATE" "$RUNBOOK"; do
  [[ -f $required_file ]] || fail "missing file: $required_file"
done

# ---- the shipped record, extracted so no fixture can drift from it ----------

oversight_records="$(yq -o=json '[.macos.posture_controls[] | select(.id == "oversight")]' "$DATA_FILE")" ||
  fail "could not read the oversight record from $DATA_FILE"
jq -e 'length == 1' <<<"$oversight_records" >/dev/null ||
  fail "exactly one oversight record must be declared in macos_posture_controls.yaml"
oversight_tier="$(jq -r '.[0].tier' <<<"$oversight_records")"
oversight_reader="$(jq -r '.[0].reader' <<<"$oversight_records")"
oversight_expect="$(jq -r '.[0].expect' <<<"$oversight_records")"
oversight_description="$(jq -r '.[0].description' <<<"$oversight_records")"
oversight_remedy="$(jq -r '.[0].remedy // empty' <<<"$oversight_records")"

[[ $oversight_tier == "verify" ]] ||
  fail "the oversight record must be tier: verify (an apply-time runner must never restart an operator-quit login item); got '$oversight_tier'"
[[ $oversight_reader == "pgrep_oversight" ]] ||
  fail "the oversight record must be read by the pgrep_oversight process probe; got '$oversight_reader'"
[[ $oversight_expect == "running" ]] ||
  fail "the oversight record must expect 'running'; got '$oversight_expect'"
[[ -n $oversight_remedy ]] ||
  fail "the oversight record must carry a remedy (the page's fix-it line)"

healthy_seed='{"firewall":"1","gatekeeper":"1","screenlock":"1","oversight":"running","oversight:expect":"running"}'
probe_argv="pgrep -x -U $(id -u) OverSight"

# ---- B1: healthy tick -- a live process probe, silent, baselined -------------

setup_poller_harness
trap 'teardown_poller_harness' EXIT
set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
set_posture_controls "$oversight_records"
export POLLER_PGREP_MODE=running

poller_status=0
poller_output="$(run_poller 2>&1)" || poller_status=$?
[[ $poller_status -eq 0 ]] ||
  fail "B1: a healthy tick must exit 0, got $poller_status: $poller_output"
assert_no_page || fail "B1: a running monitor must page nothing"
# The probe is the process table, by exact argv: the user-scoped exact-name
# pgrep match on the pinned process identity, exactly once. A bundle-presence
# check would never invoke pgrep at all.
assert_probe_argv "$probe_argv" 1 ||
  fail "B1: the poller must probe the process table exactly once, with the pinned identity"
assert_baseline_scalar oversight running ||
  fail "B1: the baseline must record the oversight control as running"
assert_no_mutation_attempt || fail "B1: a posture read must never mutate"
teardown_poller_harness
trap - EXIT

# ---- B2: lifecycle -- one page per stop, quiet while down, re-arm on restore --

setup_poller_harness
trap 'teardown_poller_harness' EXIT
set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
set_posture_controls "$oversight_records"
seed_baseline "$healthy_seed"

export POLLER_PGREP_MODE=stopped
poller_status=0
run_poller >/dev/null 2>&1 || poller_status=$?
[[ $poller_status -eq 0 ]] || fail "B2 tick 1: expected exit 0, got $poller_status"
assert_page_count 1 || fail "B2 tick 1: the stop must page exactly once"
assert_page_severity_is CRIT || fail "B2 tick 1: the stop page must be CRIT"
assert_page_body_has "\`$oversight_description\`: now stopped, declared running" ||
  fail "B2 tick 1: the page must name the control and the stopped-vs-declared state"
assert_page_body_has "$oversight_remedy" ||
  fail "B2 tick 1: the page must carry the declared remedy"
# Notify-before-persist: at page time the baseline still held the prior state.
assert_page_saw_baseline "$healthy_seed" ||
  fail "B2 tick 1: the baseline must not advance before the page is queued"
assert_baseline_scalar oversight stopped ||
  fail "B2 tick 1: after the queued page the baseline must advance to stopped"

run_poller >/dev/null 2>&1 || fail "B2 tick 2: expected exit 0"
assert_page_count 1 || fail "B2 tick 2: an ONGOING stop must stay quiet (page-once)"

export POLLER_PGREP_MODE=running
run_poller >/dev/null 2>&1 || fail "B2 tick 3: expected exit 0"
assert_page_count 1 || fail "B2 tick 3: a restore is silent recovery, not a page"
assert_baseline_scalar oversight running ||
  fail "B2 tick 3: the restore must clear the marker (baseline back to running)"

export POLLER_PGREP_MODE=stopped
run_poller >/dev/null 2>&1 || fail "B2 tick 4: expected exit 0"
assert_page_count 2 || fail "B2 tick 4: a LATER stop must page again"
assert_no_mutation_attempt || fail "B2: the lifecycle must never mutate"
teardown_poller_harness
trap - EXIT

# ---- B3: probe failures are indeterminate, never running, never stopped ------

# <label> <env-assignments...>: run one tick under the given probe form and
# require a monitoring-gap page naming the control, with the baseline
# byte-for-byte untouched (nothing was believed, nothing advanced).
run_indeterminate_case() {
  local label="$1"
  shift
  local poller_status=0 env_assignment

  setup_poller_harness
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
  set_posture_controls "$oversight_records"
  seed_baseline "$healthy_seed"
  snapshot_baseline
  for env_assignment in "$@"; do
    export "${env_assignment?}"
  done

  run_poller >/dev/null 2>&1 || poller_status=$?
  [[ $poller_status -eq 0 ]] ||
    fail "B3 $label: expected exit 0 after paging the gap, got $poller_status"
  assert_page_count 1 || fail "B3 $label: an unreadable probe must page a monitoring gap"
  assert_page_severity_is CRIT || fail "B3 $label: the gap page must be CRIT"
  assert_page_body_has 'monitoring gap' || fail "B3 $label: the page must name the gap"
  assert_page_body_has 'oversight' || fail "B3 $label: the gap page must name the control"
  assert_baseline_unchanged ||
    fail "B3 $label: an indeterminate read must never advance the baseline"
  assert_no_mutation_attempt || fail "B3 $label: the failure path must never mutate"

  for env_assignment in "$@"; do
    unset "${env_assignment%%=*}"
  done
  teardown_poller_harness
}

# A pid on stdout with a failed exit: running-looking output is never believed.
run_indeterminate_case 'failure-with-pid-output' POLLER_PGREP_MODE=error
# The no-match status that still printed something: not the documented silent
# no-match, so never classified stopped.
run_indeterminate_case 'exit-1-with-output' POLLER_PGREP_OUTPUT=3424 POLLER_PGREP_EXIT=1
# A zero exit that printed nothing: pgrep always prints the matched pids on
# success, so an empty success is unreadable, not running.
run_indeterminate_case 'exit-0-without-output' POLLER_PGREP_OUTPUT= POLLER_PGREP_EXIT=0

# ---- B4: the tier guard refuses the record flipped to enforce ----------------

# (a) at render time: the template aborts, naming the record and the tier.
sandbox="$(cd "$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$sandbox"' EXIT
render_home="$sandbox/render-home"
mkdir -p "$render_home"
flip_source="$sandbox/flip-src"
mkdir -p "$flip_source/.chezmoidata"
cp "$DATA_FILE" "$flip_source/.chezmoidata/macos_posture_controls.yaml"
yq -i '(.macos.posture_controls[] | select(.id == "oversight")).tier = "enforce"' \
  "$flip_source/.chezmoidata/macos_posture_controls.yaml" ||
  fail "B4: could not flip the oversight tier in the fixture copy"
render_status=0
HOME="$render_home" chezmoi --source "$flip_source" execute-template --no-tty \
  <"$TEMPLATE" >"$sandbox/render.out" 2>"$sandbox/render.err" || render_status=$?
[[ $render_status -ne 0 ]] ||
  fail "B4: the render must refuse the oversight record at tier enforce (rendered: $(cat "$sandbox/render.out"))"
grep -qF 'oversight' "$sandbox/render.err" ||
  fail "B4: the render refusal must name the record (stderr: $(cat "$sandbox/render.err"))"
grep -qF 'tier "enforce"' "$sandbox/render.err" ||
  fail "B4: the render refusal must name the offending tier (stderr: $(cat "$sandbox/render.err"))"

# (b) at runtime: the poller refuses the deployed file BEFORE any probe runs.
setup_poller_harness
set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
set_posture_controls "$(jq 'map(.tier = "enforce")' <<<"$oversight_records")"
seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
poller_status=0
run_poller >/dev/null 2>&1 || poller_status=$?
[[ $poller_status -eq 0 ]] ||
  fail "B4: expected exit 0 after paging the refused file, got $poller_status"
assert_page_count 1 || fail "B4: a mis-tiered deployed file must page a monitoring gap"
assert_page_body_has 'oversight' || fail "B4: the refusal page must name the record"
assert_no_probe_calls || fail "B4: a refused file must be rejected BEFORE any probe runs"
teardown_poller_harness

# ---- B5: the manual notification-delivery record and its runbook section -----

# The interactive dependency is Notification Center delivery, NOT a
# microphone or camera privacy grant. Measured on the installed 2.4.0 bundle:
# no NS*UsageDescription keys in Info.plist and an empty entitlements dict
# under the hardened runtime, so macOS never presents a Microphone or Camera
# grant for it (its TCC table is empty on a working install); it observes
# device ACTIVATION EVENTS through CoreMediaIO property listeners, never
# device content. Its only output is a notification: with delivery denied the
# monitor observes correctly and tells nobody, a dead alert channel that
# reads as healthy. So the record must direct the operator at notification
# delivery, and the runbook section must name the pane an operator can
# actually use, never the Privacy & Security step no reader can complete.

manual_records="$(yq -o=json '[.macos.system_setup[] | select(.tier == "manual" and (.description | contains("OverSight")))]' "$SETUP_FILE")" ||
  fail "B5: could not read macos_system_setup.yaml"
jq -e 'length == 1' <<<"$manual_records" >/dev/null ||
  fail "B5: exactly one manual OverSight record must be declared in macos_system_setup.yaml"
echo "$manual_records" | jq -e '.[0] | (has("command") or has("sudo")) | not' >/dev/null ||
  fail "B5: the manual record must carry no mutating payload (the authorization is interactive-only)"
manual_description="$(jq -r '.[0].description' <<<"$manual_records")"
grep -qiE 'notification' <<<"$manual_description" ||
  fail "B5: the manual record must direct the operator at notification delivery (the monitor's only output channel); got '$manual_description'"
if grep -qiE 'microphone|camera' <<<"$manual_description"; then
  fail "B5: the manual record must not describe a microphone or camera grant; macOS never presents one for OverSight (no usage-description keys, no entitlements) and it needs none; got '$manual_description'"
fi
manual_runbook="$(jq -r '.[0].runbook // empty' <<<"$manual_records")"
[[ -n $manual_runbook ]] ||
  fail "B5: the manual record must name its runbook section"
grep -qxF "### $manual_runbook" "$RUNBOOK" ||
  fail "B5: the runbook section '### $manual_runbook' must exist in $RUNBOOK"
manual_runbook_body="$(awk -v heading="### $manual_runbook" '
  $0 == heading { in_section = 1; next }
  in_section && /^### / { exit }
  in_section { print }
' "$RUNBOOK")"
[[ -n ${manual_runbook_body//[[:space:]]/} ]] ||
  fail "B5: the runbook section '### $manual_runbook' has an empty body"
grep -qF 'System Settings → Notifications' <<<"$manual_runbook_body" ||
  fail "B5: the runbook section must name the concrete pane the operator uses (System Settings → Notifications)"
if grep -qE 'Privacy & Security → (Microphone|Camera)' <<<"$manual_runbook_body"; then
  fail "B5: the runbook section must not direct a Privacy & Security microphone or camera grant; macOS never presents that pane entry for OverSight, so the step cannot be completed"
fi
# The runbook's running check must be the poller's own probe, user-scoped:
# an un-scoped pgrep would count another user's OverSight, so an operator
# following the runbook could read running while this user's monitor is
# stopped and the poller pages.
grep -qF 'pgrep -x -U "$(id -u)" OverSight' <<<"$manual_runbook_body" ||
  fail 'B5: the runbook running check must be user-scoped exactly like the poller probe: pgrep -x -U "$(id -u)" OverSight'
if grep -qE 'pgrep -x OverSight' <<<"$manual_runbook_body"; then
  fail "B5: the runbook section still carries the un-scoped running check (pgrep -x OverSight), which another user's OverSight can satisfy"
fi

printf 'ok: OverSight posture records (process probe, page-once lifecycle, indeterminate discipline, tier guard, manual notification delivery)\n'
