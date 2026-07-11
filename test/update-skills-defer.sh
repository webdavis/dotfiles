#!/usr/bin/env bash
# update-skills-defer.sh: the weekly idle-gate judges recent ACTIVITY, not mere
# process existence, so the updater runs UNATTENDED. On the daily driver a
# `claude --remote-control` bridge is always up; the round-2 gate deferred on any
# such process forever and forced a manual run. The gate now works in two steps:
#
#   1. If NO process resolves to an agent harness (claude/codex/hermes) via the
#      existing effective-program logic, resolved through any interpreter front,
#      PROCEED (unchanged fast path; evidence probes are never touched).
#   2. If an agent process exists, judge idleness by ACTIVITY. Every harness
#      PRESENT on the machine (its activity dir exists) is probed for the newest
#      file mtime; if EVERY present harness is older than IDLE_THRESHOLD, PROCEED;
#      if ANY is fresh, DEFER as before.
#   3. Fail closed: an unreadable process table, or an activity probe that errors
#      (unreadable dir), counts as ACTIVE and DEFERS.
#   4. UPDATE_SKILLS_FORCE=1 still bypasses everything (not re-tested here; the
#      other suites cover it).
#
# The gate is exercised through the REAL script (no FORCE, that bypasses the
# gate). PATH shims present a controllable process world (ps, which can also
# simulate a read failure) and pin the clock (date) so the slot-aware exhaustion
# branch is deterministic; the activity evidence is stubbed as real files with
# controlled mtimes inside the tmp HOME, pointed at via the env vars the script
# reads with sane $HOME-relative defaults. A minimal lock with empty tables makes
# every network pass a no-op, so a proceeding full run reaches the final
# `[update-skills] done`.
set -euo pipefail

# git hooks (this runs under pre-commit) leak GIT_DIR/GIT_INDEX_FILE, unset so
# no child git command reaches the outer repo.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

tmp="$(mktemp -d)"
cleanup() {
  chmod -R u+rwx "$tmp" 2>/dev/null || true # an unreadable-dir test may leave a 000 dir
  rm -rf "$tmp"
}
trap cleanup EXIT

HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.agents/skills"

# Minimal lock: every table empty, so install/update/hermes/fork passes no-op
# and a proceeding full run reaches `[update-skills] done`.
cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{
  "version": 2,
  "tiers": {},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": {},
  "clawhubTracked": {},
  "forks": {}
}
EOF

stub_dir="$tmp/stubs"
mkdir -p "$stub_dir"
ALERTER_LOG="$tmp/alerter.log"
NPX_LOG="$tmp/npx.log"
: >"$ALERTER_LOG"

# ps stub: prints the simulated process world ($FAKE_PS), or simulates a read
# failure (exit non-zero) when FAKE_PS_FAIL is set, the gate must fail closed.
cat >"$stub_dir/ps" <<'EOF'
#!/usr/bin/env bash
if [[ -n ${FAKE_PS_FAIL:-} ]]; then
  echo "ps: simulated read failure" >&2
  exit 1
fi
printf '%s\n' "${FAKE_PS:-}"
EOF

# alerter stub: record every invocation so the "last slot" LOUD-alert path is
# observable.
cat >"$stub_dir/alerter" <<EOF
#!/usr/bin/env bash
printf 'alerter %s\n' "\$*" >>"$ALERTER_LOG"
EOF

# date stub: pin the ISO week, the hour, and the weekday (1 = Monday) so the
# slot-aware alert branch is deterministic; everything else (notably the +%s the
# activity probe reads) falls through to the real date.
cat >"$stub_dir/date" <<'EOF'
#!/usr/bin/env bash
case "$1" in
  +%H) printf '%s\n' "${FAKE_HOUR:-04}" ;;
  +%u) printf '%s\n' "${FAKE_DOW:-1}" ;;
  +%G-%V) printf '%s\n' "${FAKE_WEEK:-2026-28}" ;;
  *) exec /bin/date "$@" ;;
esac
EOF

# npx stub: a proceeding full run runs `npx skills update`; log and succeed.
cat >"$stub_dir/npx" <<EOF
#!/usr/bin/env bash
printf 'npx %s\n' "\$*" >>"$NPX_LOG"
echo "stub npx"
EOF

chmod +x "$stub_dir"/*
export PATH="$stub_dir:$PATH"
export FAKE_WEEK="2026-28"

# ── Activity evidence surface. The three probe dirs are pointed at the tmp HOME
#    via the env vars the script reads (defaults are $HOME/.claude/projects,
#    $HOME/.codex/sessions, $HOME/.hermes/logs). Threshold default 900s; tests
#    override it for the boundary case. ────────────────────────────────────────
ACT_CLAUDE="$HOME/act/claude"
ACT_CODEX="$HOME/act/codex"
ACT_HERMES="$HOME/act/hermes"
export UPDATE_SKILLS_CLAUDE_ACTIVITY_DIR="$ACT_CLAUDE"
export UPDATE_SKILLS_CODEX_ACTIVITY_DIR="$ACT_CODEX"
export UPDATE_SKILLS_HERMES_ACTIVITY_DIR="$ACT_HERMES"
export UPDATE_SKILLS_IDLE_THRESHOLD=900

# harness evidence-state helpers, each on one probe dir.
reset_activity() {
  chmod -R u+rwx "$HOME/act" 2>/dev/null || true
  rm -rf "$HOME/act"
}
harness_absent() { rm -rf "$1"; } # no dir at all → not installed
harness_active() {                # a file whose mtime is NOW → within the window
  mkdir -p "$1"
  : >"$1/live.jsonl"
}
harness_stale() { # a file aged well past the window → no recent activity
  local age_ago
  age_ago="$(/bin/date -r "$(($(/bin/date +%s) - 3600))" +%Y%m%d%H%M.%S)"
  mkdir -p "$1"
  : >"$1/old.jsonl"
  touch -t "$age_ago" "$1/old.jsonl"
}
harness_unreadable() { # present but unreadable → probe error → fail closed
  mkdir -p "$1"
  : >"$1/x.jsonl"
  chmod 000 "$1"
}
# a file aged a controlled NUMBER of seconds into the past (boundary test).
harness_file_aged() {
  local dir="$1" seconds="$2" ts
  ts="$(/bin/date -r "$(($(/bin/date +%s) - seconds))" +%Y%m%d%H%M.%S)"
  mkdir -p "$dir"
  : >"$dir/f.jsonl"
  touch -t "$ts" "$dir/f.jsonl"
}

# run_gate <fake_ps> [fake_hour] [fake_ps_fail] [fake_dow] [sched], reset per-run
# state, run the real script with the given world, capture combined output. No
# FORCE: the gate is under test. The 5th arg, when "--scheduled", marks the run
# as a LaunchAgent (scheduled) run; otherwise the run is manual. The activity
# evidence is whatever the caller staged on disk beforehand.
GATE_OUTPUT=""
run_gate() {
  local fake_ps="$1" fake_hour="${2:-04}" fake_ps_fail="${3:-}" fake_dow="${4:-1}" sched="${5:-}"
  local -a run_args=()
  [[ $sched == "--scheduled" ]] && run_args=(--scheduled)
  rm -rf "$HOME/.local/state"
  : >"$ALERTER_LOG"
  GATE_OUTPUT="$(FAKE_PS="$fake_ps" FAKE_HOUR="$fake_hour" FAKE_PS_FAIL="$fake_ps_fail" FAKE_DOW="$fake_dow" bash "$SCRIPT" "${run_args[@]}" 2>&1)" ||
    fail "script exited non-zero (gate should always exit 0): $GATE_OUTPUT"
}
run_gate_sched() { run_gate "$1" "${2:-04}" "${3:-}" "${4:-1}" --scheduled; }

proceeded() { printf '%s\n' "$GATE_OUTPUT" | grep -qF '[update-skills] done'; }
deferred() { printf '%s\n' "$GATE_OUTPUT" | grep -qiF 'deferring'; }
early_exited() { printf '%s\n' "$GATE_OUTPUT" | grep -qiF 'already succeeded'; }
alerted() { [[ -s $ALERTER_LOG ]]; }

# argv fixtures, real dresden shapes plus hostile interpreter fronts.
HERMES_GATEWAY='/Users/x/.hermes/hermes-agent/venv/bin/python -m hermes_cli.main gateway run --replace'
HERMES_SESSION='/Users/x/.hermes/hermes-agent/venv/bin/python /Users/x/.hermes/hermes-agent/venv/bin/hermes -c Do a thing'
CLAUDE_REMOTE='/opt/homebrew/bin/claude --remote-control'
CLAUDE_BG='/opt/homebrew/Caskroom/claude-code@latest/2.1.200/claude --bg-spare /tmp/x.sock'
CODEX_SERVER='/Applications/Codex.app/Contents/Resources/codex app-server --analytics-default-enabled'
CODEX_SESSION='codex resume 019f4a8f-c990-7441-b02f-086e4bd16e87'
CODEX_PLAIN='codex'
UNRELATED_PYTHON='/usr/bin/python3 /usr/local/bin/some-tool.py --flag'
# Hostile interpreter-front bypasses: options between the interpreter and the -m
# module / script operand must be skipped, and `env`/node fronts resolved. Each
# still resolves to an agent harness and, paired with a fresh evidence file,
# MUST defer.
HERMES_U='/Users/x/venv/bin/python3 -u /Users/x/venv/bin/hermes -c go'
HERMES_I_M='/Users/x/venv/bin/python3 -I -m hermes_cli.main gateway run'
HERMES_X_M='/Users/x/venv/bin/python3 -X utf8 -m hermes_cli.main gateway run'
HERMES_DASHDASH='/Users/x/venv/bin/python3 -- /Users/x/venv/bin/hermes -c go'
HERMES_ENV='/usr/bin/env python3 -m hermes_cli.main gateway run'
CLAUDE_NODE='/opt/homebrew/bin/node /Users/x/.local/share/npm/bin/claude --remote-control'

# ── Item 1 of the brief: bridge process present + ALL evidence stale → PROCEED.
#    This is the unattended headline: the always-up `claude --remote-control`
#    bridge no longer defers the weekly run when nothing has happened lately. ───
reset_activity
harness_stale "$ACT_CLAUDE"
harness_stale "$ACT_CODEX"
harness_stale "$ACT_HERMES"
run_gate "$CLAUDE_REMOTE"
proceeded || fail "bridge present + all evidence stale did not PROCEED (unattended run): $GATE_OUTPUT"
deferred && fail "bridge present + all evidence stale wrongly deferred: $GATE_OUTPUT"

# Bridge present + ALL evidence dirs absent (no transcripts at all) → PROCEED:
# a harness with no install contributes no evidence and does not block.
reset_activity
run_gate "$CLAUDE_REMOTE"
proceeded || fail "bridge present + no evidence dirs did not PROCEED (absent = no block): $GATE_OUTPUT"

# ── Item 2: bridge process present + ONE harness active (fresh mtime) → DEFER.
#    Cross-harness: the fresh harness need not be the one whose process is up. ──
reset_activity
harness_stale "$ACT_CLAUDE"
harness_stale "$ACT_CODEX"
harness_active "$ACT_HERMES" # a live hermes turn, while the claude bridge idles
run_gate "$CLAUDE_REMOTE"
deferred || fail "bridge present + one harness active did not DEFER: $GATE_OUTPUT"
proceeded && fail "bridge present + one harness active wrongly proceeded: $GATE_OUTPUT"

# The active harness matching its own process also defers.
reset_activity
harness_active "$ACT_CLAUDE"
run_gate "$CLAUDE_REMOTE"
deferred || fail "an active claude session did not DEFER: $GATE_OUTPUT"

# ── Item 3: NO agent process → PROCEED without touching evidence probes. Prove
#    it by staging ALL dirs ACTIVE yet still expecting PROCEED (the fast path
#    short-circuits before any probe). ─────────────────────────────────────────
reset_activity
harness_active "$ACT_CLAUDE"
harness_active "$ACT_CODEX"
harness_active "$ACT_HERMES"
run_gate "$UNRELATED_PYTHON"
proceeded || fail "an agent-free world did not PROCEED even with fresh evidence (fast path): $GATE_OUTPUT"

# ── Item 4: evidence probe error (unreadable dir) → DEFER (fail closed). ───────
reset_activity
harness_stale "$ACT_CLAUDE"
harness_unreadable "$ACT_CODEX" # chmod 000 → find errors → ACTIVE
run_gate "$CLAUDE_REMOTE"
chmod 755 "$ACT_CODEX" 2>/dev/null || true
deferred || fail "an unreadable activity dir did not fail closed (must DEFER): $GATE_OUTPUT"
proceeded && fail "an unreadable activity dir wrongly proceeded: $GATE_OUTPUT"

# ── Item 5: harness absent (no evidence dir) does not block. A present-but-stale
#    harness alongside two ABSENT ones → PROCEED. ──────────────────────────────
reset_activity
harness_stale "$ACT_CLAUDE"
harness_absent "$ACT_CODEX"
harness_absent "$ACT_HERMES"
run_gate "$CLAUDE_REMOTE"
proceeded || fail "stale-plus-absent harnesses did not PROCEED (absent must not block): $GATE_OUTPUT"

# ── Item 6: threshold boundary. With a small window, a file just INSIDE the
#    window is ACTIVE (DEFER) and one just OUTSIDE is STALE (PROCEED). ──────────
export UPDATE_SKILLS_IDLE_THRESHOLD=100
reset_activity
harness_file_aged "$ACT_CLAUDE" 40 # 40s old, inside the 100s window → ACTIVE
run_gate "$CLAUDE_REMOTE"
deferred || fail "a file just inside IDLE_THRESHOLD did not DEFER: $GATE_OUTPUT"
reset_activity
harness_file_aged "$ACT_CLAUDE" 400 # 400s old, well outside 100s → STALE
run_gate "$CLAUDE_REMOTE"
proceeded || fail "a file just outside IDLE_THRESHOLD did not PROCEED: $GATE_OUTPUT"
export UPDATE_SKILLS_IDLE_THRESHOLD=900

# ── ps read failure → DEFER (fail closed): an unreadable process table means we
#    cannot prove idleness, so we never swap. ──────────────────────────────────
reset_activity # even with no evidence, a ps failure must defer
run_gate "$HERMES_GATEWAY" 04 1
deferred || fail "ps read failure did not fail closed (must defer): $GATE_OUTPUT"

# ── Interpreter-front recognition, under the new semantics: each resolves to an
#    agent harness, so paired with a fresh evidence file it MUST defer. A NON-
#    harness front paired with the same fresh file PROCEEDS (proving the front is
#    what is recognized, not the file). ────────────────────────────────────────
for world in "$HERMES_U" "$HERMES_I_M" "$HERMES_X_M" "$HERMES_DASHDASH" "$HERMES_ENV" \
  "$CLAUDE_NODE" "$HERMES_GATEWAY" "$HERMES_SESSION" "$CLAUDE_BG" "$CODEX_SERVER" \
  "$CODEX_SESSION" "$CODEX_PLAIN"; do
  reset_activity
  harness_active "$ACT_CLAUDE"
  run_gate "$world"
  deferred || fail "an interpreter-fronted agent + fresh evidence did not DEFER: [$world]: $GATE_OUTPUT"
done
# The non-harness python front is NOT recognized: same fresh file, but PROCEED.
reset_activity
harness_active "$ACT_CLAUDE"
run_gate "$UNRELATED_PYTHON"
proceeded || fail "unrelated python front was treated as a harness: $GATE_OUTPUT"

# A live agent hiding among unrelated processes, with fresh evidence → DEFER.
reset_activity
harness_active "$ACT_CODEX"
run_gate "$(printf '%s\n%s' "$UNRELATED_PYTHON" "$CODEX_SESSION")" 04
deferred || fail "a live agent among unrelated processes did not defer: $GATE_OUTPUT"

# ── Weekly success stamp + alert (last-slot). These semantics are unchanged; the
#    deferral they rest on is now activity-driven, so each stages a fresh
#    harness. ──────────────────────────────────────────────────────────────────
# A non-last slot deferral must NOT fire the LOUD alert.
reset_activity
harness_active "$ACT_CODEX"
run_gate "$CODEX_PLAIN" 04
deferred || fail "an active session did not defer: $GATE_OUTPUT"
alerted && fail "a non-last slot (04:00) fired the LOUD alert, only the last slot should: $(cat "$ALERTER_LOG")"

# Weekly success stamp present → EARLY-EXIT before any work (no evidence needed).
# The stamp is <week> <custom-lock-hash> <updater-hash>; the early-exit only
# fires when all three match the current desired state, so the fixture stamp is
# built from the real hashes (a roster or updater change un-stamps the week).
reset_activity
mkdir -p "$HOME/.local/state/update-skills"
lock_hash="$(shasum -a 256 "$HOME/.agents/custom-skill-lock.json" | awk '{print $1}')"
updater_hash="$(shasum -a 256 "$SCRIPT" | awk '{print $1}')"
printf '%s %s %s' "$FAKE_WEEK" "$lock_hash" "$updater_hash" >"$HOME/.local/state/update-skills/last-success"
: >"$ALERTER_LOG"
GATE_OUTPUT="$(FAKE_PS="$UNRELATED_PYTHON" FAKE_HOUR=08 bash "$SCRIPT" 2>&1)" ||
  fail "stamped run exited non-zero: $GATE_OUTPUT"
early_exited || fail "a run whose week already succeeded did not early-exit: $GATE_OUTPUT"
proceeded && fail "a stamped week re-ran the full pass instead of early-exiting: $GATE_OUTPUT"

# A stamp for the same week but a DIFFERENT roster hash must NOT early-exit (a
# roster change un-stamps the week).
printf '%s %s %s' "$FAKE_WEEK" "deadbeef" "$updater_hash" >"$HOME/.local/state/update-skills/last-success"
GATE_OUTPUT="$(FAKE_PS="$UNRELATED_PYTHON" FAKE_HOUR=08 bash "$SCRIPT" 2>&1)" ||
  fail "roster-changed run exited non-zero: $GATE_OUTPUT"
early_exited && fail "a stamp with a stale roster hash early-exited instead of rebuilding: $GATE_OUTPUT"
rm -rf "$HOME/.local/state"

# A proceeding full run (agent-free world) WRITES the week stamp (so the next
# slot early-exits).
reset_activity
run_gate "$UNRELATED_PYTHON" 04
[[ -f "$HOME/.local/state/update-skills/last-success" ]] ||
  fail "a successful full run did not write the weekly success stamp"
[[ "$(<"$HOME/.local/state/update-skills/last-success")" == "$FAKE_WEEK "* ]] ||
  fail "the success stamp does not begin with the current ISO week: $(<"$HOME/.local/state/update-skills/last-success")"

# ── Slot-aware exhaustion via the --scheduled marker. Exhaustion is claimed ONLY
#    for a SCHEDULED run with no later slot remaining this week, and now means
#    "the machine had agent activity at every scheduled slot". Each stages a
#    fresh harness so the run defers. ──────────────────────────────────────────
# SCHEDULED last Monday slot (23:00) still deferring → LOUD alert + log line.
reset_activity
harness_active "$ACT_CODEX"
run_gate_sched "$CODEX_PLAIN" 23 "" 1
deferred || fail "scheduled last-slot world did not defer: $GATE_OUTPUT"
alerted || fail "a scheduled last slot (Monday 23:00) deferral did not fire the LOUD alerter notification"
grep -qiE 'exhaust|budget|last|activity' <<<"$GATE_OUTPUT" ||
  fail "scheduled last-slot deferral did not emit an exhausted-budget log line: $GATE_OUTPUT"
[[ -f "$HOME/.local/state/update-skills/last-scheduled-week" ]] ||
  fail "a scheduled run did not record its ISO week in the scheduled-attempt state file"

# SCHEDULED EARLY Monday slot (04:00) deferring → NO alert (later slots remain).
reset_activity
harness_active "$ACT_CODEX"
run_gate_sched "$CODEX_PLAIN" 04 "" 1
deferred || fail "scheduled early-slot world did not defer: $GATE_OUTPUT"
alerted && fail "an early scheduled slot (04:00) claimed exhaustion; later slots remain: $(cat "$ALERTER_LOG")"

# SCHEDULED coalesced catch-up on a LATER weekday (Wed 10:00) → alert (the Monday
# slots are spent; launchd delivered the missed event late).
reset_activity
harness_active "$ACT_CODEX"
run_gate_sched "$CODEX_PLAIN" 10 "" 3
deferred || fail "scheduled catch-up world did not defer: $GATE_OUTPUT"
alerted || fail "a scheduled catch-up on a later day did not claim exhaustion (no later slot this week): $GATE_OUTPUT"

# MANUAL run on Monday 23:00 (the last slot hour) → DEFERS but NEVER claims
# scheduled-budget exhaustion (a manual run is not part of the scheduled cycle).
reset_activity
harness_active "$ACT_CODEX"
run_gate "$CODEX_PLAIN" 23 "" 1
deferred || fail "manual Monday-23:00 world did not defer: $GATE_OUTPUT"
alerted && fail "a manual Monday-23:00 run claimed scheduled-budget exhaustion: $(cat "$ALERTER_LOG")"

# MANUAL non-Monday run → DEFERS, no alert (unchanged).
reset_activity
harness_active "$ACT_CODEX"
run_gate "$CODEX_PLAIN" 23 "" 3
deferred || fail "manual non-Monday world did not defer: $GATE_OUTPUT"
alerted && fail "a manual non-Monday deferral alerted: $(cat "$ALERTER_LOG")"

# ── The plist declares EXACTLY 24 hourly Monday retry slots, each a full
#    {Weekday=1, Hour in 0..23, Minute=0} tuple, AND passes --scheduled in
#    ProgramArguments. Parse the rendered plist as real plist data (plutil ->
#    json -> jq) so dropping Weekday or Minute (which launchd then treats as a
#    wildcard, firing far more often) fails this test. The expected hour set is
#    generated programmatically (0..23) rather than hand-listed. ───────────────
PLIST="$REPO_ROOT/Library/LaunchAgents/com.webdavis.update-skills.plist.tmpl"
rendered_plist="$(CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$PLIST")" ||
  fail "chezmoi execute-template failed on the update-skills plist"
plist_json="$tmp/plist.json"
printf '%s' "$rendered_plist" | plutil -convert json -o "$plist_json" - 2>/dev/null ||
  fail "rendered plist did not parse as a plist"
slot_count="$(jq '.StartCalendarInterval | length' "$plist_json")"
[[ $slot_count -eq 24 ]] ||
  fail "expected exactly 24 StartCalendarInterval tuples, got $slot_count"
non_conforming="$(jq '[.StartCalendarInterval[] | select(.Weekday != 1 or .Minute != 0)] | length' "$plist_json")"
[[ $non_conforming -eq 0 ]] ||
  fail "a slot is missing Weekday=1 or Minute=0 (launchd would treat the missing key as a wildcard)"
slot_hours="$(jq -c '[.StartCalendarInterval[].Hour] | sort' "$plist_json")"
expected_hours="$(jq -cn '[range(0;24)]')"
[[ $slot_hours == "$expected_hours" ]] ||
  fail "slot hours are not exactly 0..23: $slot_hours"
prog_scheduled="$(jq -r '[.ProgramArguments[] | select(. == "--scheduled")] | length' "$plist_json")"
[[ $prog_scheduled == "1" ]] ||
  fail "the plist ProgramArguments does not pass exactly one --scheduled marker (slot-aware exhaustion needs it)"

echo "update-skills-defer: OK"
