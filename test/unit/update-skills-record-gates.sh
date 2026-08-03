#!/usr/bin/env bash
#
# update-skills.sh: the three gates on __update_skills_record, tested at the
# function itself.
#
# Each gate is checked here rather than only through the script because two of
# them are defence in depth: today no --dry-run path reaches a record call site,
# so a whole-script test cannot tell a working dry-run gate from a deleted one.
# That is exactly the situation where a guard rots into a comment. Testing the
# function directly makes each gate a live assertion, so a future call site added
# on a preview path cannot start posting unnoticed.
#
# The gates, and what each prevents:
#   --scheduled  a hand-run on a Wednesday would make a dead LaunchAgent look
#                alive, inverting the one signal the record carries.
#   --dry-run    a preview must have no side effects, and a push reaches a
#                channel.
#   week guard   24 hourly Monday slots would otherwise become 24 messages.
#
# Unit test: UPDATE_SKILLS_LIB_ONLY=1 sourcing, a stub relay, no sleeps.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"

fail() {
  printf 'update-skills-record-gates: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -r $SCRIPT ]] || fail "not readable: $SCRIPT"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.agents/skills" "$HOME/.local/bin"

RELAY_CALL_LOG="$tmp/relay.log"
export RELAY_CALL_LOG
: >"$RELAY_CALL_LOG"
cat >"$tmp/relay-stub.sh" <<'STUB'
#!/usr/bin/env bash
printf 'ARGV %s\n' "$(printf '%s ' "$@" | tr '\n' ' ')" >>"$RELAY_CALL_LOG"
if { : >&9; } 2>/dev/null; then
  printf 'FD9 inherited\n' >>"$RELAY_CALL_LOG"
else
  printf 'FD9 closed\n' >>"$RELAY_CALL_LOG"
fi
printf 'relay: posted HTTP 200\n'
STUB
chmod +x "$tmp/relay-stub.sh"
# __update_skills_alert resolves relay by absolute path under $HOME, not through
# UNATTENDED_LOG_RELAY, so the alert path needs its own copy of the same stub.
cp "$tmp/relay-stub.sh" "$HOME/.local/bin/relay.sh"
export UNATTENDED_LOG_RELAY="$tmp/relay-stub.sh"
export UNATTENDED_LOG_HERMES_URL="http://hermes.test/webhooks/unattended-upgrades"

export UPDATE_SKILLS_LIB_ONLY=1
# shellcheck source=dot_local/bin/executable_update-skills.sh
source "$SCRIPT"

[[ -n ${UNATTENDED_LOG_AVAILABLE:-} ]] ||
  fail "the record library did not load; every gate below would pass vacuously"

# posted_count -- how many entries reached the stub since the last reset.
reset_calls() {
  : >"$RELAY_CALL_LOG"
  rm -f "$LOG_WEEK_GUARD"
}
posted_count() { grep -c '^ARGV ' "$RELAY_CALL_LOG" 2>/dev/null || true; }

# ── CONTROL. With every gate satisfied the entry is posted. Without this the
#    gate assertions below would all pass on a function that never posts. ─────
SCHEDULED=1
DRYRUN=""
reset_calls
__update_skills_record completed "control body" || fail "the control record exited non-zero"
[[ "$(posted_count)" -eq 1 ]] ||
  fail "the control posted $(posted_count) entries, want 1: $(cat "$RELAY_CALL_LOG")"
grep -qF -- '--state completed' "$RELAY_CALL_LOG" || fail "the control entry lost its class"
grep -qF 'control body' "$RELAY_CALL_LOG" || fail "the control entry lost its body"
grep -qE 'run at [0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z' "$RELAY_CALL_LOG" ||
  fail "the control entry carries no ISO 8601 UTC run timestamp"
grep -qF 'last successful run' "$RELAY_CALL_LOG" ||
  fail "the control entry carries no gap line; an entry that does not state its own gap measures nothing"

# ── GATE 1: not scheduled -> nothing posted. ────────────────────────────────
SCHEDULED=""
DRYRUN=""
reset_calls
__update_skills_record completed "manual body" || fail "an unscheduled record exited non-zero"
[[ "$(posted_count)" -eq 0 ]] ||
  fail "a MANUAL run posted a weekly record: $(cat "$RELAY_CALL_LOG")"

# ── GATE 2: dry run -> nothing posted, and nothing written either. A preview
#    that consumed the week would suppress the real entry. ───────────────────
SCHEDULED=1
DRYRUN="--dry-run"
reset_calls
__update_skills_record completed "preview body" || fail "a dry-run record exited non-zero"
[[ "$(posted_count)" -eq 0 ]] ||
  fail "--dry-run posted a weekly record: $(cat "$RELAY_CALL_LOG")"
[[ ! -e $LOG_WEEK_GUARD ]] ||
  fail "--dry-run claimed the week; the real entry for that week would be suppressed"

# ── GATE 3: one entry per ISO week, with exactly one permitted upgrade. ─────
SCHEDULED=1
DRYRUN=""
reset_calls
__update_skills_record deferred "first slot"
__update_skills_record deferred "second slot"
[[ "$(posted_count)" -eq 1 ]] ||
  fail "two deferrals in one week posted $(posted_count) entries, want 1"
__update_skills_record completed "the run that finished"
[[ "$(posted_count)" -eq 2 ]] ||
  fail "a completed run was suppressed by the earlier deferral, leaving a finished week reading as stuck"
__update_skills_record deferred "a later slot"
[[ "$(posted_count)" -eq 2 ]] ||
  fail "a deferral after the completed entry posted, burying the truer message"

# ── BOTH relay call sites close fd 9. The run holds its serialize lock as a
#    kernel flock on fd 9; relay detaches channels that outlive the run, and a
#    flock is released only when the LAST copy of the fd closes. An inherited
#    copy in a detached curl therefore keeps the lock held after this process
#    exited, and every later slot defers over a competing run that does not
#    exist. Both of these wrappers are reached from UNDER that lock (the
#    lock-failure, roster-refusal and exhaustion paths), and this repo has
#    already shipped the same leak twice, so neither `9>&-` is left to trust. ──
assert_fd9_closed() { # <label> <command...>
  local label="$1"
  shift
  reset_calls
  (
    exec 9>>"$tmp/fd9.lock"
    "$@" >/dev/null 2>&1
  )
  grep -qF 'FD9 closed' "$RELAY_CALL_LOG" ||
    fail "$label handed relay the run's serialize-lock fd; a detached child would hold the lock after the run exited: $(cat "$RELAY_CALL_LOG")"
}
SCHEDULED=1
DRYRUN=""
assert_fd9_closed "the weekly record" __update_skills_record completed "fd probe body"
assert_fd9_closed "the alert wrapper" __update_skills_alert "fd probe alert"

printf 'update-skills-record-gates: OK\n'
