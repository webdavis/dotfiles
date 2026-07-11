#!/usr/bin/env bash
# update-skills-defer.sh — the weekly idle-gate must distinguish a LIVE
# interactive agent-harness session (defer, never swap skills under a live
# session) from a persistent BACKGROUND daemon (proceed, a daemon never loads a
# skill from the store on its own, so blocking on it defers the weekly run
# forever). The discriminator keys strictly on the argv EFFECTIVE program and
# its flags-position token, never on free prompt text.
#
# Ground truth (dresden, read-only ps, 2026-07-10):
#   DAEMONS — must NOT block the run (proceed):
#     python -m hermes_cli.main gateway run --replace        (hermes gateway)
#     .../claude --remote-control                            (Remote Control bridge)
#     .../claude --bg-spare | daemon run | --bg-pty-host     (Claude bg helpers)
#     codex app-server [--analytics-default-enabled]         (Codex app server)
#   LIVE SESSIONS — must block the run (defer):
#     .../python .../bin/hermes -c <prompt>                  (hermes console-script session)
#     /opt/homebrew/bin/claude -p <prompt>                   (a Claude one-shot)
#     codex resume <id> | plain codex                        (a Codex CLI session)
#
# Two holes the pre-fix gate had, both covered below: (a) a hermes session's
# argv starts with `python` (console script), so the old first-word gate never
# saw it as an agent and swapped skills under it; (b) prompt free text that
# merely CONTAINS a daemon phrase (`claude -p "restart the hermes gateway"`) was
# substring-matched as a daemon and let a live session proceed. The new gate
# also fails CLOSED when ps cannot be read.
#
# The gate is exercised through the REAL script (no UPDATE_SKILLS_FORCE — that
# bypasses the gate). PATH shims present a controllable process world (ps, which
# can also simulate a read failure), record alerter invocations, and pin the
# clock (date) so the "last retry slot" branch is deterministic. A minimal lock
# with empty tables makes every network pass a no-op, so a proceeding full run
# runs clean to its final `[update-skills] done`.
set -euo pipefail

# git hooks (this runs under pre-commit) leak GIT_DIR/GIT_INDEX_FILE — unset so
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
# failure (exit non-zero) when FAKE_PS_FAIL is set — the gate must fail closed.
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

# date stub: pin the ISO week and the hour; everything else falls through to the
# real date.
cat >"$stub_dir/date" <<'EOF'
#!/usr/bin/env bash
case "$1" in
  +%H) printf '%s\n' "${FAKE_HOUR:-04}" ;;
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

# run_gate <fake_ps> [fake_hour] [fake_ps_fail] — reset per-run state, run the
# real script with the given world, capture combined output. No FORCE: the gate
# is under test.
GATE_OUTPUT=""
run_gate() {
  local fake_ps="$1" fake_hour="${2:-04}" fake_ps_fail="${3:-}"
  rm -rf "$HOME/.local/state"
  : >"$ALERTER_LOG"
  GATE_OUTPUT="$(FAKE_PS="$fake_ps" FAKE_HOUR="$fake_hour" FAKE_PS_FAIL="$fake_ps_fail" bash "$SCRIPT" 2>&1)" ||
    fail "script exited non-zero (gate should always exit 0): $GATE_OUTPUT"
}

proceeded() { printf '%s\n' "$GATE_OUTPUT" | grep -qF '[update-skills] done'; }
deferred() { printf '%s\n' "$GATE_OUTPUT" | grep -qiF 'deferring'; }
early_exited() { printf '%s\n' "$GATE_OUTPUT" | grep -qiF 'already succeeded'; }
alerted() { [[ -s $ALERTER_LOG ]]; }

# argv fixtures — real dresden shapes.
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

# ── Hostile-argv discriminator table ───────────────────────────────────────
# 1) python console-script hermes session → DEFER (argv starts with python, but
#    the effective program is the hermes console script, and the flags token is
#    -c, not the gateway daemon shape). This is hole (a).
run_gate "$HERMES_SESSION"
deferred || fail "python-console hermes session did not defer (hole a): $GATE_OUTPUT"
proceeded && fail "python-console hermes session proceeded — a live session must defer: $GATE_OUTPUT"

# 2) hermes gateway (python -m hermes_cli.main gateway) → PROCEED (module maps
#    to hermes, flags token is exactly `gateway`).
run_gate "$HERMES_GATEWAY"
proceeded || fail "hermes gateway daemon did not proceed: $GATE_OUTPUT"
deferred && fail "hermes gateway daemon deferred — a daemon must not block: $GATE_OUTPUT"

# 3) claude -p with the daemon phrase in the PROMPT free text → DEFER (the
#    flags-position token is -p; free text is never matched). This is hole (b).
run_gate "$CLAUDE_PROMPT_TRAP"
deferred || fail "claude -p with 'hermes gateway' in the prompt did not defer (hole b): $GATE_OUTPUT"

# 4) claude --remote-control (Remote Control bridge, daemon by shape) → PROCEED.
run_gate "$CLAUDE_REMOTE"
proceeded || fail "claude --remote-control bridge did not proceed (daemon by shape): $GATE_OUTPUT"

# 5) codex app-server → PROCEED.
run_gate "$CODEX_SERVER"
proceeded || fail "codex app-server daemon did not proceed: $GATE_OUTPUT"

# 6) plain codex (interactive) → DEFER.
run_gate "$CODEX_PLAIN"
deferred || fail "plain codex session did not defer: $GATE_OUTPUT"

# 6b) codex resume <id> → DEFER (flags token is `resume`, not `app-server`).
run_gate "$CODEX_SESSION"
deferred || fail "codex resume session did not defer: $GATE_OUTPUT"

# 7) ps read failure → DEFER (fail closed).
run_gate "$HERMES_GATEWAY" 04 1
deferred || fail "ps read failure did not fail closed (must defer): $GATE_OUTPUT"

# 8) unrelated python process → PROCEED (effective program is the script name,
#    not an agent binary).
run_gate "$UNRELATED_PYTHON"
proceeded || fail "unrelated python process did not proceed: $GATE_OUTPUT"

# 8b) Daemons-only world (gateway + Claude bg helpers + Claude bridge + Codex
#     app-server) → PROCEEDS. None is a live session.
run_gate "$(printf '%s\n%s\n%s\n%s\n%s' "$HERMES_GATEWAY" "$CLAUDE_REMOTE" "$CLAUDE_BG" "$CLAUDE_DAEMON" "$CODEX_SERVER")"
proceeded || fail "daemons-only world did not proceed: $GATE_OUTPUT"
deferred && fail "daemons-only world deferred — background daemons must not block the run: $GATE_OUTPUT"

# 8c) A live session hiding among daemons → DEFER.
run_gate "$(printf '%s\n%s' "$CODEX_SERVER" "$CODEX_SESSION")" 04
deferred || fail "a live codex session among daemons did not defer: $GATE_OUTPUT"

# ── Weekly success stamp + alert (last-slot) ────────────────────────────────
# 2) A non-last slot deferral must NOT fire the LOUD alert.
run_gate "$CODEX_PLAIN" 04
deferred || fail "interactive session did not defer: $GATE_OUTPUT"
alerted && fail "a non-last slot (04:00) fired the LOUD alert — only the last slot should: $(cat "$ALERTER_LOG")"

# 3) Weekly success stamp present → EARLY-EXIT before any work.
mkdir -p "$HOME/.local/state/update-skills"
printf '%s' "$FAKE_WEEK" >"$HOME/.local/state/update-skills/last-success"
: >"$ALERTER_LOG"
GATE_OUTPUT="$(FAKE_PS="$HERMES_GATEWAY" FAKE_HOUR=08 bash "$SCRIPT" 2>&1)" ||
  fail "stamped run exited non-zero: $GATE_OUTPUT"
early_exited || fail "a run whose week already succeeded did not early-exit: $GATE_OUTPUT"
proceeded && fail "a stamped week re-ran the full pass instead of early-exiting: $GATE_OUTPUT"
rm -rf "$HOME/.local/state"

# 3b) A proceeding full run WRITES the week stamp (so the next slot early-exits).
run_gate "$HERMES_GATEWAY" 04
[[ -f "$HOME/.local/state/update-skills/last-success" ]] ||
  fail "a successful full run did not write the weekly success stamp"
[[ "$(<"$HOME/.local/state/update-skills/last-success")" == "$FAKE_WEEK" ]] ||
  fail "the success stamp is not the current ISO week: $(<"$HOME/.local/state/update-skills/last-success")"

# 4) Last retry slot (16:00) still deferring → LOUD alerter notification + log
#    line (the weekly budget is exhausted).
run_gate "$CODEX_PLAIN" 16
deferred || fail "last-slot world did not defer: $GATE_OUTPUT"
alerted || fail "last slot (16:00) deferral did not fire the LOUD alerter notification: (empty log)"
grep -qiE 'exhaust|budget|last' <<<"$GATE_OUTPUT" ||
  fail "last-slot deferral did not emit an exhausted-budget log line: $GATE_OUTPUT"

# 5) The plist declares FOUR Monday retry slots (04:00/08:00/12:00/16:00) instead
#    of the single 04:00 that could defer forever. Render it and count them.
PLIST="$REPO_ROOT/Library/LaunchAgents/com.webdavis.update-skills.plist.tmpl"
rendered_plist="$(CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$PLIST")" ||
  fail "chezmoi execute-template failed on the update-skills plist"
hour_lines="$(printf '%s\n' "$rendered_plist" | grep -c '<key>Hour</key>')"
[[ $hour_lines -eq 4 ]] ||
  fail "expected 4 Monday retry slots in the plist, found $hour_lines"
for slot_hour in 4 8 12 16; do
  printf '%s\n' "$rendered_plist" | grep -A1 '<key>Hour</key>' | grep -qF "<integer>${slot_hour}</integer>" ||
    fail "plist is missing the Monday ${slot_hour}:00 retry slot"
done

echo "update-skills-defer: OK"
