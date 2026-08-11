#!/usr/bin/env bash
# The weekly jobs post their records through the engine, and they run under
# launchd where nothing is on PATH and nobody is watching. Their resolution is
# the same transitional rule the hooks use, held once in log-entries.sh, so
# this pins the rule and the three jobs that read it.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

# A home with a space, because these paths are interpolated in several places.
HOME="$scratch/home dir"
export HOME
mkdir -p "$HOME/.local/libexec/pns/helpers"
cp "$REPO_ROOT/dot_local/libexec/pns/helpers/engine-path.sh" \
  "$HOME/.local/libexec/pns/helpers/"

binary="$HOME/.local/libexec/pns/pns"
relay="$HOME/.local/libexec/pns/relay.sh"

fail() {
  echo "$1" >&2
  exit 1
}

# shellcheck source=dot_local/libexec/unattended-upgrades/helpers/log-entries.sh
source "$REPO_ROOT/dot_local/libexec/unattended-upgrades/helpers/log-entries.sh"

# --- the rule --------------------------------------------------------------
printf '#!/usr/bin/env bash\n' >"$relay"
chmod +x "$relay"
[[ "$(unattended_engine)" == "$relay" ]] ||
  fail "before the binary exists the weekly jobs keep the bash engine, got: $(unattended_engine)"

printf '#!/usr/bin/env bash\n' >"$binary"
chmod +x "$binary"
[[ "$(unattended_engine)" == "$binary" ]] ||
  fail "once installed the binary carries the weekly records, got: $(unattended_engine)"

[[ "$(UNATTENDED_LOG_RELAY=/custom/engine unattended_engine)" == /custom/engine ]] ||
  fail "an explicit override must win outright"

# A machine without the pns tree at all: the jobs still resolve something and
# report it, rather than aborting mid-record.
(
  PNS_HELPERS_DIR="$scratch/absent"
  export PNS_HELPERS_DIR
  [[ "$(unattended_engine)" == "$relay" ]] ||
    fail "a missing resolver degrades to the bash engine, got: $(unattended_engine)"
)

# --- the record actually goes to the resolved engine -----------------------
# unattended_log_post is what every weekly job calls; it must hand the record
# to the binary once that exists.
printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$@" >"%s/posted"\nprintf "relay: posted HTTP 200\\n"\n' \
  "$scratch" >"$binary"
chmod +x "$binary"
UNATTENDED_LOG_STATE_DIR="$scratch/state" unattended_log_post \
  weekly-test 'done' project 'a detail' >/dev/null 2>&1 ||
  fail "an accepted post must return success"
[[ -f "$scratch/posted" ]] ||
  fail "the record must reach the resolved engine"
grep -qx -- '--remote-only' "$scratch/posted" ||
  fail "the weekly record is the durable log path, so it must stay --remote-only"

# --- the three jobs resolve through the guard, none keeps a default --------
# The path may appear inside weekly_engine's terminal fallback and nowhere
# else. grep -q is never placed downstream of a pipe: its early exit closes
# the pipe, the upstream grep dies of SIGPIPE on any file larger than the
# buffer, and under pipefail the whole check reads as "no match": a guard
# that cannot fail on exactly the biggest file.
for job in \
  "dot_local/libexec/unattended-upgrades/executable_homebrew-weekly-upgrade.sh" \
  "dot_local/libexec/unattended-upgrades/claude/executable_report-plugin-updates.sh" \
  "dot_local/libexec/unattended-upgrades/agent-skills/executable_update-skills.sh"; do
  grep -q 'weekly_engine()' "$REPO_ROOT/$job" ||
    fail "$job must define the guarded resolution"
  assignments="$(grep -nE '(RELAY|relay_script)=' "$REPO_ROOT/$job" | grep -v '^[0-9]*: *#' || true)"
  if grep 'libexec/pns/relay\.sh' <<<"$assignments" >/dev/null; then
    fail "$job assigns the bash engine directly instead of weekly_engine: $assignments"
  fi
done

# --- a present-but-not-executable engine is refused with the stated lines --
# An interrupted install leaves exactly this; the refusal and its wording are
# the operator's only clue in a launchd log.
# BOTH candidates dead, because a non-executable binary alone correctly
# falls back to the bash engine: the refusal is for the machine where the
# terminal resolution itself cannot run.
chmod -x "$binary" "$relay"
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
chmod +x "$binary" "$relay"

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
resolver_err="$(PNS_HELPERS_DIR="$scratch/absent" unattended_engine 2>&1 >/dev/null)"
[[ -z $resolver_err ]] ||
  fail "resolution must not leak shell noise into the record, got: $resolver_err"

# --- the resolution runs where its function exists --------------------------
# The regression this pins: RELAY= called unattended_engine forty lines above
# the source that defines it, so every scheduled run died 127 at load. The
# seed run proves the script gets PAST resolution and into its real work (the
# inventory stage), whatever that stage then says.
seed_home="$scratch/seed home"
mkdir -p "$seed_home/.claude/plugins"
printf '{"plugins":{}}' >"$seed_home/.claude/plugins/installed_plugins.json"
set +e
seed_err="$(HOME="$seed_home" \
  "$REPO_ROOT/dot_local/libexec/unattended-upgrades/claude/executable_report-plugin-updates.sh" \
  --seed-baseline 2>&1)"
seed_rc=$?
set -e
[[ $seed_rc -ne 127 ]] ||
  fail "the job died before its resolution function existed: $seed_err"
if grep -q 'command not found' <<<"$seed_err"; then
  fail "the resolution ran before its function existed: $seed_err"
fi
grep -q 'installed-plugin inventory' <<<"$seed_err" ||
  fail "the run must reach the inventory stage past the resolution: $seed_err"

exit 0
