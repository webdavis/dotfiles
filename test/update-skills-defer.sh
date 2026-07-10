#!/usr/bin/env bash
# update-skills-defer.sh — the weekly idle-gate must distinguish an ACTIVE agent
# harness session (defer — never swap skills under a live session) from a
# persistent BACKGROUND daemon (proceed — a daemon never loads a skill from the
# store on its own, so blocking on it defers the weekly run forever).
#
# Ground truth (dresden, read-only ps, 2026-07-10):
#   DAEMONS — must NOT block the run:
#     python -m hermes_cli.main gateway run --replace        (hermes gateway)
#     .../claude --bg-spare | daemon run | --bg-pty-host     (Claude bg helpers)
#     codex app-server [--analytics-default-enabled]         (Codex app server)
#   INTERACTIVE SESSIONS — must block the run:
#     /opt/homebrew/bin/claude --remote-control              (a Claude Code TUI)
#     codex resume <id>                                      (a Codex CLI session)
#     .../hermes -c <prompt> | hermes -p <profile>           (an interactive hermes run)
#
# NOTE on the brief's premise: on this machine `pgrep -x hermes` matches NOTHING
# (the gateway's comm is `python`, not `hermes`); the real persistent matcher of
# the OLD gate is `claude --remote-control` bridges that linger for days. The
# defect (defer-forever) is real regardless of which harness triggers it, and
# the stub world below models the pre-fix gate's view (`pgrep -x hermes` matching
# the gateway) so the RED is meaningful against the shipped script.
#
# The gate is exercised through the REAL script (no UPDATE_SKILLS_FORCE — that
# bypasses the gate). PATH shims present a controllable process world (ps),
# replicate the pre-fix gate (pgrep), record alerter invocations, and pin the
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

# ps stub: prints the simulated process world ($FAKE_PS), whatever flags it gets.
cat >"$stub_dir/ps" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "${FAKE_PS:-}"
EOF

# pgrep stub: models the PRE-FIX gate. `-x hermes` matches the persistent gateway
# (the defer-forever bug the audit found); `-x claude`/`-x codex` match only when
# an interactive line for that harness is present in $FAKE_PS.
cat >"$stub_dir/pgrep" <<'EOF'
#!/usr/bin/env bash
pat=""
for a in "$@"; do case "$a" in -*) ;; *) pat="$a" ;; esac; done
case "$pat" in
  hermes) printf '%s\n' "${FAKE_PS:-}" | grep -q 'hermes' && exit 0 ;;
  claude) printf '%s\n' "${FAKE_PS:-}" | grep -Eq '(^|/| )claude( |$)' && exit 0 ;;
  codex) printf '%s\n' "${FAKE_PS:-}" | grep -Eq '(^|/| )codex( |$)' && exit 0 ;;
esac
exit 1
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

# run_gate <fake_ps> <fake_hour> — reset per-run state, run the real script with
# the given world, capture combined output. No FORCE: the gate is under test.
GATE_OUTPUT=""
run_gate() {
  local fake_ps="$1" fake_hour="$2"
  rm -rf "$HOME/.local/state"
  : >"$ALERTER_LOG"
  GATE_OUTPUT="$(FAKE_PS="$fake_ps" FAKE_HOUR="$fake_hour" bash "$SCRIPT" 2>&1)" ||
    fail "script exited non-zero (gate should always exit 0): $GATE_OUTPUT"
}

proceeded() { printf '%s\n' "$GATE_OUTPUT" | grep -qF '[update-skills] done'; }
deferred() { printf '%s\n' "$GATE_OUTPUT" | grep -qiF 'deferring'; }
early_exited() { printf '%s\n' "$GATE_OUTPUT" | grep -qiF 'already succeeded'; }
alerted() { [[ -s $ALERTER_LOG ]]; }

GATEWAY='/Users/x/.hermes/hermes-agent/venv/bin/python -m hermes_cli.main gateway run --replace'
CLAUDE_BG='/opt/homebrew/Caskroom/claude-code@latest/2.1.200/claude --bg-spare /tmp/x.sock'
CLAUDE_DAEMON='/opt/homebrew/Caskroom/claude-code@latest/2.1.200/claude daemon run --origin transient'
CODEX_SERVER='/Applications/Codex.app/Contents/Resources/codex app-server --analytics-default-enabled'
CLAUDE_TUI='/opt/homebrew/bin/claude --remote-control'
CODEX_SESSION='codex resume 019f4a8f-c990-7441-b02f-086e4bd16e87'
HERMES_RUN='/Users/x/.hermes/hermes-agent/venv/bin/hermes -c Do a thing'

# 1) Gateway-only world → PROCEEDS. The persistent hermes gateway is a daemon,
#    not a session; blocking on it is the defer-forever bug. (RED against the
#    pre-fix script, whose pgrep -x hermes matches the gateway and defers.)
run_gate "$GATEWAY" 04
proceeded || fail "gateway-only world did not proceed (defer-forever bug): $GATE_OUTPUT"
deferred && fail "gateway-only world deferred — a persistent daemon must not block the run: $GATE_OUTPUT"

# 1b) Daemons-only world (gateway + Claude bg helpers + Codex app-server) →
#     PROCEEDS. None of these is an interactive session.
run_gate "$(printf '%s\n%s\n%s\n%s' "$GATEWAY" "$CLAUDE_BG" "$CLAUDE_DAEMON" "$CODEX_SERVER")" 04
proceeded || fail "daemons-only world did not proceed: $GATE_OUTPUT"
deferred && fail "daemons-only world deferred — background daemons must not block the run: $GATE_OUTPUT"

# 2) Interactive Claude TUI → DEFERS (never swap skills under a live session).
run_gate "$CLAUDE_TUI" 04
deferred || fail "interactive claude session did not defer: $GATE_OUTPUT"
proceeded && fail "interactive claude session proceeded — a live session must defer: $GATE_OUTPUT"
alerted && fail "a non-last slot (04:00) fired the LOUD alert — only the last slot should: $(cat "$ALERTER_LOG")"

# 2b) Interactive Codex session → DEFERS (proves the discriminator excludes the
#     Codex app-server daemon but catches an interactive `codex resume`).
run_gate "$(printf '%s\n%s' "$CODEX_SERVER" "$CODEX_SESSION")" 04
deferred || fail "interactive codex session did not defer despite an app-server-only exclusion: $GATE_OUTPUT"

# 2c) Interactive hermes run → DEFERS (the gateway is excluded, but an
#     interactive `hermes -c` counts).
run_gate "$(printf '%s\n%s' "$GATEWAY" "$HERMES_RUN")" 04
deferred || fail "interactive hermes run did not defer despite a gateway-only exclusion: $GATE_OUTPUT"

# 3) Weekly success stamp present → EARLY-EXIT before any work (extra Monday
#    slots are no-ops once one slot succeeded this week). (RED against the
#    pre-fix script, which has no stamp and would proceed.)
mkdir -p "$HOME/.local/state/update-skills"
printf '%s' "$FAKE_WEEK" >"$HOME/.local/state/update-skills/last-success"
: >"$ALERTER_LOG"
GATE_OUTPUT="$(FAKE_PS="$GATEWAY" FAKE_HOUR=08 bash "$SCRIPT" 2>&1)" ||
  fail "stamped run exited non-zero: $GATE_OUTPUT"
early_exited || fail "a run whose week already succeeded did not early-exit: $GATE_OUTPUT"
proceeded && fail "a stamped week re-ran the full pass instead of early-exiting: $GATE_OUTPUT"
rm -rf "$HOME/.local/state"

# 3b) A proceeding full run WRITES the week stamp (so the next slot early-exits).
run_gate "$GATEWAY" 04
[[ -f "$HOME/.local/state/update-skills/last-success" ]] ||
  fail "a successful full run did not write the weekly success stamp"
[[ "$(<"$HOME/.local/state/update-skills/last-success")" == "$FAKE_WEEK" ]] ||
  fail "the success stamp is not the current ISO week: $(<"$HOME/.local/state/update-skills/last-success")"

# 4) Last retry slot (16:00) still deferring → LOUD alerter notification + log
#    line (the weekly budget is exhausted). (RED against the pre-fix script,
#    which never calls alerter.)
run_gate "$CLAUDE_TUI" 16
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
