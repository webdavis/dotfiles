#!/usr/bin/env bash
# update-skills-defer.sh, the weekly idle-gate FAILS CLOSED: it defers whenever
# ANY process whose argv EFFECTIVE program resolves to an agent harness (claude,
# codex, or hermes) is running, live session or background daemon alike. Argv
# shape cannot prove idleness here (every interactive Claude launch carries
# --remote-control; Codex app-server and the Hermes gateway host live agent
# turns in-process), so the old daemon-shape allowlist was DELETED: a busy
# machine could present only daemon-shaped argv and get its skills swapped under
# a live session. The gate now errs toward deferral, which the design tolerates
# (a deferred run just lands the updates next week). Only a machine with NO agent
# process proceeds.
#
# The discriminator keys on the argv EFFECTIVE program, resolved through any
# interpreter front (python/node/bun, optional `env` prefix), skipping the
# interpreter's own options to find the -m module or script operand. Free prompt
# text is never matched.
#
# The gate is exercised through the REAL script (no UPDATE_SKILLS_FORCE, that
# bypasses the gate). PATH shims present a controllable process world (ps, which
# can also simulate a read failure), record alerter invocations, and pin the
# clock (date) so the "last retry slot" branch is deterministic. A minimal lock
# with empty tables makes every network pass a no-op, so a proceeding full run
# runs clean to its final `[update-skills] done`.
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
trap 'rm -rf "$tmp"' EXIT

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
# slot-aware alert branch is deterministic; everything else falls through.
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

# run_gate <fake_ps> [fake_hour] [fake_ps_fail] [fake_dow] [sched], reset per-run
# state, run the real script with the given world, capture combined output. No
# FORCE: the gate is under test. The 5th arg, when "--scheduled", marks the run
# as a LaunchAgent (scheduled) run; otherwise the run is manual.
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
CLAUDE_DAEMON='/opt/homebrew/Caskroom/claude-code@latest/2.1.200/claude daemon run --origin transient'
CLAUDE_PROMPT_TRAP='/opt/homebrew/bin/claude -p restart the hermes gateway'
CODEX_SERVER='/Applications/Codex.app/Contents/Resources/codex app-server --analytics-default-enabled'
CODEX_SESSION='codex resume 019f4a8f-c990-7441-b02f-086e4bd16e87'
CODEX_PLAIN='codex'
UNRELATED_PYTHON='/usr/bin/python3 /usr/local/bin/some-tool.py --flag'
# Hostile interpreter-front bypasses (item 2): options between the interpreter
# and the -m module / script operand must be skipped, and `env`/node fronts
# resolved. Each still resolves to an agent harness and MUST defer.
HERMES_U='/Users/x/venv/bin/python3 -u /Users/x/venv/bin/hermes -c go'
HERMES_I_M='/Users/x/venv/bin/python3 -I -m hermes_cli.main gateway run'
HERMES_X_M='/Users/x/venv/bin/python3 -X utf8 -m hermes_cli.main gateway run'
HERMES_DASHDASH='/Users/x/venv/bin/python3 -- /Users/x/venv/bin/hermes -c go'
HERMES_ENV='/usr/bin/env python3 -m hermes_cli.main gateway run'
CLAUDE_NODE='/opt/homebrew/bin/node /Users/x/.local/share/npm/bin/claude --remote-control'

# ── Item 1: fail-closed idle gate. ANY agent process (session OR daemon) DEFERS;
#    only a machine with no agent process proceeds. ──────────────────────────
# The nine real dresden shapes all DEFER now (the daemon-shape allowlist is gone).
for world in "$HERMES_GATEWAY" "$HERMES_SESSION" "$CLAUDE_REMOTE" "$CLAUDE_BG" \
  "$CLAUDE_DAEMON" "$CLAUDE_PROMPT_TRAP" "$CODEX_SERVER" "$CODEX_SESSION" "$CODEX_PLAIN"; do
  run_gate "$world"
  deferred || fail "an agent process did not defer (fail-closed gate): [$world]: $GATE_OUTPUT"
  proceeded && fail "an agent process proceeded (must never swap under a live/daemon agent): [$world]: $GATE_OUTPUT"
done

# ── Item 2: hostile interpreter-front resolution. Each resolves to an agent and
#    must DEFER. ───────────────────────────────────────────────────────────
for world in "$HERMES_U" "$HERMES_I_M" "$HERMES_X_M" "$HERMES_DASHDASH" "$HERMES_ENV" "$CLAUDE_NODE"; do
  run_gate "$world"
  deferred || fail "an interpreter-fronted agent did not resolve/defer: [$world]: $GATE_OUTPUT"
done

# A non-agent interpreter process (a plain python tool) → PROCEED.
run_gate "$UNRELATED_PYTHON"
proceeded || fail "unrelated python process did not proceed: $GATE_OUTPUT"

# ps read failure → DEFER (fail closed).
run_gate "$HERMES_GATEWAY" 04 1
deferred || fail "ps read failure did not fail closed (must defer): $GATE_OUTPUT"

# A world with NO agent process (only an unrelated python tool) → PROCEED.
run_gate "$UNRELATED_PYTHON"
proceeded || fail "an agent-free world did not proceed: $GATE_OUTPUT"

# A live agent hiding among unrelated processes → DEFER.
run_gate "$(printf '%s\n%s' "$UNRELATED_PYTHON" "$CODEX_SESSION")" 04
deferred || fail "a live agent among unrelated processes did not defer: $GATE_OUTPUT"

# ── Weekly success stamp + alert (last-slot) ────────────────────────────────
# 2) A non-last slot deferral must NOT fire the LOUD alert.
run_gate "$CODEX_PLAIN" 04
deferred || fail "interactive session did not defer: $GATE_OUTPUT"
alerted && fail "a non-last slot (04:00) fired the LOUD alert, only the last slot should: $(cat "$ALERTER_LOG")"

# 3) Weekly success stamp present → EARLY-EXIT before any work.
mkdir -p "$HOME/.local/state/update-skills"
printf '%s' "$FAKE_WEEK" >"$HOME/.local/state/update-skills/last-success"
: >"$ALERTER_LOG"
GATE_OUTPUT="$(FAKE_PS="$UNRELATED_PYTHON" FAKE_HOUR=08 bash "$SCRIPT" 2>&1)" ||
  fail "stamped run exited non-zero: $GATE_OUTPUT"
early_exited || fail "a run whose week already succeeded did not early-exit: $GATE_OUTPUT"
proceeded && fail "a stamped week re-ran the full pass instead of early-exiting: $GATE_OUTPUT"
rm -rf "$HOME/.local/state"

# 3b) A proceeding full run (agent-free world) WRITES the week stamp (so the next
#     slot early-exits).
run_gate "$UNRELATED_PYTHON" 04
[[ -f "$HOME/.local/state/update-skills/last-success" ]] ||
  fail "a successful full run did not write the weekly success stamp"
[[ "$(<"$HOME/.local/state/update-skills/last-success")" == "$FAKE_WEEK" ]] ||
  fail "the success stamp is not the current ISO week: $(<"$HOME/.local/state/update-skills/last-success")"

# ── Item 6: slot-aware exhaustion via the --scheduled marker ─────────────────
# Exhaustion is claimed ONLY for a SCHEDULED run with no later slot remaining
# this week; a manual run never claims scheduled-budget exhaustion.

# 4) SCHEDULED last Monday slot (16:00) still deferring → LOUD alert + log line.
run_gate_sched "$CODEX_PLAIN" 16 "" 1
deferred || fail "scheduled last-slot world did not defer: $GATE_OUTPUT"
alerted || fail "a scheduled last slot (Monday 16:00) deferral did not fire the LOUD alerter notification"
grep -qiE 'exhaust|budget|last' <<<"$GATE_OUTPUT" ||
  fail "scheduled last-slot deferral did not emit an exhausted-budget log line: $GATE_OUTPUT"
[[ -f "$HOME/.local/state/update-skills/last-scheduled-week" ]] ||
  fail "a scheduled run did not record its ISO week in the scheduled-attempt state file"

# 4a) SCHEDULED EARLY Monday slot (04:00) deferring → NO alert (later slots remain).
run_gate_sched "$CODEX_PLAIN" 04 "" 1
deferred || fail "scheduled early-slot world did not defer: $GATE_OUTPUT"
alerted && fail "an early scheduled slot (04:00) claimed exhaustion; later slots remain: $(cat "$ALERTER_LOG")"

# 4b) SCHEDULED coalesced catch-up on a LATER weekday (Wed 10:00) → alert (the
#     Monday slots are spent; launchd delivered the missed event late).
run_gate_sched "$CODEX_PLAIN" 10 "" 3
deferred || fail "scheduled catch-up world did not defer: $GATE_OUTPUT"
alerted || fail "a scheduled catch-up on a later day did not claim exhaustion (no later slot this week): $GATE_OUTPUT"

# 4c) MANUAL run on Monday 16:00 → DEFERS but NEVER claims scheduled-budget
#     exhaustion (a manual run is not part of the scheduled cycle).
run_gate "$CODEX_PLAIN" 16 "" 1
deferred || fail "manual Monday-16:00 world did not defer: $GATE_OUTPUT"
alerted && fail "a manual Monday-16:00 run claimed scheduled-budget exhaustion: $(cat "$ALERTER_LOG")"

# 4d) MANUAL non-Monday run → DEFERS, no alert (unchanged).
run_gate "$CODEX_PLAIN" 16 "" 3
deferred || fail "manual non-Monday world did not defer: $GATE_OUTPUT"
alerted && fail "a manual non-Monday deferral alerted: $(cat "$ALERTER_LOG")"

# 5) The plist declares EXACTLY four Monday retry slots, each a full
#    {Weekday=1, Hour in 4/8/12/16, Minute=0} tuple, AND passes --scheduled in
#    ProgramArguments. Parse the rendered plist as real plist data (plutil ->
#    json -> jq) so dropping Weekday or Minute (which launchd then treats as a
#    wildcard, firing far more often) fails this test.
PLIST="$REPO_ROOT/Library/LaunchAgents/com.webdavis.update-skills.plist.tmpl"
rendered_plist="$(CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$PLIST")" ||
  fail "chezmoi execute-template failed on the update-skills plist"
plist_json="$tmp/plist.json"
printf '%s' "$rendered_plist" | plutil -convert json -o "$plist_json" - 2>/dev/null ||
  fail "rendered plist did not parse as a plist"
slot_count="$(jq '.StartCalendarInterval | length' "$plist_json")"
[[ $slot_count -eq 4 ]] ||
  fail "expected exactly 4 StartCalendarInterval tuples, got $slot_count"
non_conforming="$(jq '[.StartCalendarInterval[] | select(.Weekday != 1 or .Minute != 0)] | length' "$plist_json")"
[[ $non_conforming -eq 0 ]] ||
  fail "a slot is missing Weekday=1 or Minute=0 (launchd would treat the missing key as a wildcard)"
slot_hours="$(jq -c '[.StartCalendarInterval[].Hour] | sort' "$plist_json")"
[[ $slot_hours == "[4,8,12,16]" ]] ||
  fail "slot hours are not exactly 4/8/12/16: $slot_hours"
prog_scheduled="$(jq -r '[.ProgramArguments[] | select(. == "--scheduled")] | length' "$plist_json")"
[[ $prog_scheduled == "1" ]] ||
  fail "the plist ProgramArguments does not pass exactly one --scheduled marker (slot-aware exhaustion needs it)"

echo "update-skills-defer: OK"
