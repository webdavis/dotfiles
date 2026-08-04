#!/usr/bin/env bash
# update-skills-unattended.sh: the weekly run is UNATTENDED. It does its work
# whatever the machine is doing, and the only thing that holds a slot back is the
# per-week success stamp (another run holding the serialize lock is covered by
# test/e2e/update-skills-lock-contention.sh).
#
# There used to be an activity idle-gate here: a slot deferred while claude,
# codex or hermes had touched a per-turn file within IDLE_THRESHOLD, and failed
# closed to deferring when it could not tell. On a machine that is in use every
# day that gate deferred every one of the 24 Monday slots, so the update never
# ran at all. It bought nothing: the publish is one atomic exchange with one
# retained generation, so a path resolved during or after it yields a complete
# tree from exactly one generation, and a harness reads skill content at
# invocation time, so a swap mid-session costs at most that the next invocation
# reads the new copy.
#
# What this pins:
#   1. A run PROCEEDS with an agent process on the machine AND fresh per-turn
#      activity in all three harness locations the old gate probed. This is the
#      regression test for reintroducing any such gate.
#   2. The weekly success stamp still gates the extra slots, still keys on the
#      roster and updater hashes, and a completed run still writes it.
#   3. The plist still declares exactly 24 hourly Monday slots and passes
#      --scheduled.
#
# Everything runs against a sandbox HOME with stubbed ps/date/npx; the real
# ~/.agents and ~/.local are never read or written.
set -euo pipefail

# git hooks (this runs under pre-commit) leak GIT_DIR/GIT_INDEX_FILE, unset so
# no child git command reaches the outer repo.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# Explicit refutation: `! grep` under set -e never fails a test.
refute() {
  local pattern="$1" haystack="$2" message="$3"
  if grep -qiE "$pattern" <<<"$haystack"; then
    printf '=== output ===\n%s\n' "$haystack" >&2
    fail "$message"
  fi
}

tmp="$(mktemp -d)"
cleanup() {
  chmod -R u+rwx "$tmp" 2>/dev/null || true
  rm -rf "$tmp"
}
trap cleanup EXIT

HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.agents/skills"

# Minimal lock: one npx-tracked `anchor` so the tracked union is non-empty (the
# zero-union guard would otherwise refuse before any of this); the hermes and
# fork passes no-op and a proceeding full run reaches `[update-skills] done`.
cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{
  "version": 2,
  "tiers": {"anchor": "core"},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": {"anchor": {"repo": "fixture/pack"}},
  "clawhubTracked": {},
  "forks": {}
}
EOF
# anchor: an npx-tracked store real dir that migrates into a live generation.
mkdir -p "$HOME/.agents/skills/anchor"
printf -- '---\nname: anchor\ndescription: fixture\n---\n' >"$HOME/.agents/skills/anchor/SKILL.md"
printf '{"skills":{"anchor":{}}}\n' >"$HOME/.agents/.skill-lock.json"

stub_dir="$tmp/stubs"
mkdir -p "$stub_dir"
ALERTER_LOG="$tmp/alerter.log"
NPX_LOG="$tmp/npx.log"
: >"$ALERTER_LOG"

# ps stub: prints the simulated process world ($FAKE_PS). The script reads no
# process table any more; the stub stays so a reintroduced gate observes exactly
# the world this test stages rather than the real machine's.
cat >"$stub_dir/ps" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "${FAKE_PS:-}"
EOF

# alerter stub: record every invocation so a loud alert is observable.
cat >"$stub_dir/alerter" <<EOF
#!/usr/bin/env bash
printf 'alerter %s\n' "\$*" >>"$ALERTER_LOG"
EOF

# date stub: pin the ISO week, the hour and the weekday (1 = Monday) so the
# slot-aware branches are deterministic; everything else falls through to the
# real date.
cat >"$stub_dir/date" <<'EOF'
#!/usr/bin/env bash
case "$1" in
  +%H) printf '%s\n' "${FAKE_HOUR:-04}" ;;
  +%u) printf '%s\n' "${FAKE_DOW:-1}" ;;
  +%G-%V) printf '%s\n' "${FAKE_WEEK:-2026-28}" ;;
  *) exec /bin/date "$@" ;;
esac
EOF

# npx stub: a proceeding full run runs the npx lane; log and succeed.
cat >"$stub_dir/npx" <<EOF
#!/usr/bin/env bash
printf 'npx %s\n' "\$*" >>"$NPX_LOG"
echo "stub npx"
EOF

chmod +x "$stub_dir"/*
export PATH="$stub_dir:$PATH"
export FAKE_WEEK="2026-28"

# argv fixtures: real dresden process shapes. CLAUDE_REMOTE is the always-up
# bridge whose mere existence used to defer the run forever.
CLAUDE_REMOTE='/opt/homebrew/bin/claude --remote-control'
UNRELATED_PYTHON='/usr/bin/python3 /usr/local/bin/some-tool.py --flag'

# The three per-turn activity locations the removed gate probed, at their
# defaults inside the sandbox HOME. A gate reintroduced with those defaults reads
# exactly these files.
stage_live_agent_activity() {
  local dir
  for dir in "$HOME/.claude/projects" "$HOME/.codex/sessions" "$HOME/.hermes/logs"; do
    mkdir -p "$dir"
    : >"$dir/live.jsonl" # mtime = now, i.e. a turn in flight
  done
}
clear_agent_activity() { rm -rf "$HOME/.claude/projects" "$HOME/.codex/sessions" "$HOME/.hermes/logs"; }

RUN_OUTPUT=""
# run_updater <fake_ps> [fake_hour] [fake_dow] [--scheduled], with per-run state
# reset and combined output captured. No UPDATE_SKILLS_FORCE: it bypasses the
# weekly stamp, and the stamp is under test below.
run_updater() {
  local fake_ps="$1" fake_hour="${2:-04}" fake_dow="${3:-1}" sched="${4:-}"
  local -a run_args=()
  [[ $sched == "--scheduled" ]] && run_args=(--scheduled)
  rm -rf "$HOME/.local/state"
  : >"$ALERTER_LOG"
  RUN_OUTPUT="$(FAKE_PS="$fake_ps" FAKE_HOUR="$fake_hour" FAKE_DOW="$fake_dow" bash "$SCRIPT" "${run_args[@]}" 2>&1)" ||
    fail "the run exited non-zero: $RUN_OUTPUT"
}

proceeded() { printf '%s\n' "$RUN_OUTPUT" | grep -qF '[update-skills] done'; }
early_exited() { printf '%s\n' "$RUN_OUTPUT" | grep -qiF 'already succeeded'; }

# ── 1. The headline: a live agent world does not hold the run back. An agent
#      process is up AND all three harnesses have a per-turn file whose mtime is
#      now, which is the exact world the old gate deferred on. ─────────────────
stage_live_agent_activity
: >"$NPX_LOG"
run_updater "$CLAUDE_REMOTE"
proceeded || fail "a run with a live agent world did not complete: $RUN_OUTPUT"
refute 'deferring' "$RUN_OUTPUT" "a run with a live agent world deferred; the activity gate is back"
[[ -s $NPX_LOG ]] ||
  fail "the npx lane never ran, so the run did not reach its work: $RUN_OUTPUT"
[[ -f "$HOME/.local/state/update-skills/last-success" ]] ||
  fail "a completed run under a live agent world wrote no weekly success stamp: $RUN_OUTPUT"

# ...and a SCHEDULED run in the same world behaves the same way and claims no
# exhaustion, even on the last Monday slot, where a deferral used to alert.
stage_live_agent_activity
run_updater "$CLAUDE_REMOTE" 23 1 --scheduled
proceeded || fail "a scheduled last-slot run with a live agent world did not complete: $RUN_OUTPUT"
[[ ! -s $ALERTER_LOG ]] ||
  fail "a completed last-slot run raised an alert: $(cat "$ALERTER_LOG")"
[[ -f "$HOME/.local/state/update-skills/last-scheduled-week" ]] ||
  fail "a scheduled run did not record its ISO week in the scheduled-attempt state file"

# ── 2. The weekly success stamp. It is <week> <roster-hash> <updater-hash>, and
#      only a full match makes a later slot a no-op. ──────────────────────────
clear_agent_activity
rm -rf "$HOME/.local/state"
mkdir -p "$HOME/.local/state/update-skills"
lock_hash="$(shasum -a 256 "$HOME/.agents/custom-skill-lock.json" | awk '{print $1}')"
updater_hash="$(shasum -a 256 "$SCRIPT" | awk '{print $1}')"
printf '%s %s %s' "$FAKE_WEEK" "$lock_hash" "$updater_hash" >"$HOME/.local/state/update-skills/last-success"
RUN_OUTPUT="$(FAKE_PS="$UNRELATED_PYTHON" FAKE_HOUR=08 bash "$SCRIPT" 2>&1)" ||
  fail "the stamped run exited non-zero: $RUN_OUTPUT"
early_exited || fail "a run whose week already succeeded did not early-exit: $RUN_OUTPUT"
proceeded && fail "a stamped week re-ran the full pass instead of early-exiting: $RUN_OUTPUT"

# A stamp for the same week but a DIFFERENT roster hash must NOT early-exit (a
# roster change un-stamps the week).
printf '%s %s %s' "$FAKE_WEEK" "deadbeef" "$updater_hash" >"$HOME/.local/state/update-skills/last-success"
RUN_OUTPUT="$(FAKE_PS="$UNRELATED_PYTHON" FAKE_HOUR=08 bash "$SCRIPT" 2>&1)" ||
  fail "the roster-changed run exited non-zero: $RUN_OUTPUT"
early_exited && fail "a stamp with a stale roster hash early-exited instead of rebuilding: $RUN_OUTPUT"

# A completed run writes a stamp that begins with the current ISO week.
run_updater "$UNRELATED_PYTHON"
[[ "$(<"$HOME/.local/state/update-skills/last-success")" == "$FAKE_WEEK "* ]] ||
  fail "the success stamp does not begin with the current ISO week: $(<"$HOME/.local/state/update-skills/last-success")"

# ── 3. The plist declares EXACTLY 24 hourly Monday retry slots, each a full
#      {Weekday=1, Hour in 0..23, Minute=0} tuple, AND passes --scheduled in
#      ProgramArguments. Parse the rendered plist as real plist data (plutil ->
#      json -> jq) so dropping Weekday or Minute (which launchd then treats as a
#      wildcard, firing far more often) fails this test. The expected hour set is
#      generated programmatically (0..23) rather than hand-listed. ────────────
PLIST="$REPO_ROOT/Library/LaunchAgents/com.webdavis.update-skills.plist.tmpl"
rendered_plist="$(CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$PLIST")" ||
  fail "chezmoi execute-template failed on the update-skills plist"
plist_json="$tmp/plist.json"
printf '%s' "$rendered_plist" | plutil -convert json -o "$plist_json" - 2>/dev/null ||
  fail "the rendered plist did not parse as a plist"
slot_count="$(jq '.StartCalendarInterval | length' "$plist_json")"
[[ $slot_count -eq 24 ]] ||
  fail "expected exactly 24 StartCalendarInterval tuples, got $slot_count"
non_conforming="$(jq '[.StartCalendarInterval[] | select(.Weekday != 1 or .Minute != 0)] | length' "$plist_json")"
[[ $non_conforming -eq 0 ]] ||
  fail "a slot is missing Weekday=1 or Minute=0 (launchd would treat the missing key as a wildcard)"
slot_hours="$(jq -c '[.StartCalendarInterval[].Hour] | sort' "$plist_json")"
expected_hours="$(jq -cn '[range(0;24)]')"
[[ $slot_hours == "$expected_hours" ]] ||
  fail "the slot hours are not exactly 0..23: $slot_hours"
prog_scheduled="$(jq -r '[.ProgramArguments[] | select(. == "--scheduled")] | length' "$plist_json")"
[[ $prog_scheduled == "1" ]] ||
  fail "the plist ProgramArguments does not pass exactly one --scheduled marker (slot-aware exhaustion needs it)"

echo "update-skills-unattended: OK"
