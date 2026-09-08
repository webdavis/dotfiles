#!/usr/bin/env bash
# The weekly jobs post their records through the engine, and they run under
# launchd where nothing is on PATH and nobody is watching. Their resolution is
# the same transitional rule the hooks use, held once in log-entries.sh, so
# this pins the rule and the remaining job that read it.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

# A home with a space, because these paths are interpolated in several places.
HOME="$scratch/home dir"
export HOME
mkdir -p "$HOME/.local/libexec/pns"

binary="$HOME/.local/libexec/pns/pns"

fail() {
  echo "$1" >&2
  exit 1
}

# shellcheck source=dot_local/libexec/unattended-upgrades/helpers/log-entries.sh
source "$REPO_ROOT/dot_local/libexec/unattended-upgrades/helpers/log-entries.sh"

# --- the rule --------------------------------------------------------------
printf '#!/usr/bin/env bash\n' >"$binary"
chmod +x "$binary"
[[ "$(unattended_engine)" == "$binary" ]] ||
  fail "the binary is the engine, got: $(unattended_engine)"

[[ "$(UNATTENDED_LOG_ENGINE=/custom/engine unattended_engine)" == /custom/engine ]] ||
  fail "an explicit override must win outright"

# --- the record actually goes to the resolved engine -----------------------
# unattended_log_post is what every weekly job calls; it must hand the record
# to the binary once that exists.
printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$@" >"%s/posted"\nprintf "pns: posted HTTP 200\\n"\n' \
  "$scratch" >"$binary"
chmod +x "$binary"
UNATTENDED_LOG_STATE_DIR="$scratch/state" unattended_log_post \
  weekly-test 'done' project 'a detail' >/dev/null 2>&1 ||
  fail "an accepted post must return success"
[[ -f "$scratch/posted" ]] ||
  fail "the record must reach the resolved engine"
grep -qx -- '--remote-only' "$scratch/posted" ||
  fail "the weekly record is the durable log path, so it must stay --remote-only"
{ grep -qx -- '--channel' "$scratch/posted" && grep -qx -- 'unattended-upgrades' "$scratch/posted"; } ||
  fail "the record names its route (--channel unattended-upgrades); a raw URL override is the retired way"

# --- a present-but-not-executable engine is refused with the stated lines --
# An interrupted install leaves exactly this; the refusal and its wording are
# the operator's only clue in a launchd log.
# A dead binary: the refusal is for the machine where the engine cannot run.
chmod -x "$binary"
set +e
post_out="$(UNATTENDED_LOG_STATE_DIR="$scratch/state" unattended_log_post \
  weekly-test 'done' project 'a detail' 2>&1)"
post_rc=$?
set -e
[[ $post_rc -ne 0 ]] ||
  fail "a not-executable engine must fail the post so the caller can react"
grep -q 'no executable pns engine at .*NOT delivered' <<<"$post_out" ||
  fail "the refusal must speak the stated line, got: $post_out"
alert_out="$(unattended_log_alert_delivery_failure "$scratch/guard" weekly-test 2>&1)" ||
  fail "the alert path never fails its caller"
grep -q 'stays unclaimed so a later run retries it' <<<"$alert_out" ||
  fail "the alert refusal must speak the stated line, got: $alert_out"
chmod +x "$binary"

# --- the week claim releases for retry -------------------------------------
# A claim without a delivery must not burn the week: the first failing slot
# would otherwise leave no record and no retry for seven days.
guard_dir="$scratch/claims"
unattended_log_claim_week "$guard_dir" completed ||
  fail "a fresh week must claim"
unattended_log_claim_week "$guard_dir" completed &&
  fail "a claimed week must refuse a second claim"
unattended_log_release_week "$guard_dir" completed
unattended_log_claim_week "$guard_dir" completed ||
  fail "a released week must claim again, or a failed slot burns the week"

# --- a missing resolver degrades in silence --------------------------------
# The quiet-degrade promise: no raw shell errors in a launchd record on a
# machine without the pns tree.
resolver_err="$(unattended_engine 2>&1 >/dev/null)"
[[ -z $resolver_err ]] ||
  fail "resolution must not leak shell noise into the record, got: $resolver_err"

exit 0
