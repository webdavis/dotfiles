#!/usr/bin/env bash
# update-skills-install-defer-retry.sh (R2-6): an install-only run deferred by
# harness activity must stay RETRYABLE across applies. Pre-fix,
# __gen_install_only_attempt returned 0 on an activity-defer, so the run
# exited 0, the run_onchange_after_64 wrapper treated the uninstalled roster
# addition as success and cleared/never set its retry marker, and chezmoi had
# already consumed the run_onchange trigger, so the next apply never retried.
# Now:
#   - the updater exits a DISTINCT deferred code (75, EX_TEMPFAIL) when
#     install-only defers on activity (not 0, not a hard failure);
#   - the first-install wrapper reads that code, PRESERVES/CREATES its retry
#     marker (so the rendered content changes and run_onchange re-fires next
#     apply), and NEVER exits non-zero (which would abort the apply);
#   - a real success still clears the marker; a hard failure still bumps it.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"
TMPL="$REPO_ROOT/.chezmoiscripts/run_onchange_after_64-update-skills-first-install.sh.tmpl"
fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# shellcheck source=test/fixtures/exchange-tool.lib.sh
source "$REPO_ROOT/test/fixtures/exchange-tool.lib.sh"
GMV_BIN="$(resolve_exchange_tool)" ||
  fail "no GNU coreutils mv with a working --exchange on PATH (need gmv or mv)"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# ── Part A: the updater exits 75 when install-only defers on activity ────────
HOME="$tmp/uhome"
export HOME
export UPDATE_SKILLS_GMV="$GMV_BIN"
mkdir -p "$HOME/.agents/skills"
AGENTS="$HOME/.agents"
CURRENT="$AGENTS/.skills-current"
LOCK="$AGENTS/custom-skill-lock.json"

write_lock() {
  local tiers="" npx="" n
  for n in "$@"; do
    tiers+="\"$n\": \"core\", "
    npx+="\"$n\": {\"repo\": \"fixture/pack\"}, "
  done
  cat >"$LOCK" <<EOF
{
  "version": 2,
  "tiers": {${tiers%, }},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": {${npx%, }},
  "clawhubTracked": {},
  "forks": {}
}
EOF
}

stub="$tmp/stub"
mkdir -p "$stub"
NPX_LOG="$tmp/npx.log"
: >"$NPX_LOG"
cat >"$stub/ps" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "${FAKE_PS:-}"
EOF
cat >"$stub/npx" <<EOF
#!/usr/bin/env bash
set -euo pipefail
printf 'npx %s\n' "\$*" >>"$NPX_LOG"
prev=""; skills=()
for a in "\$@"; do [[ \$prev == --skill ]] && skills+=("\$a"); prev="\$a"; done
for s in "\${skills[@]}"; do
  mkdir -p "\$HOME/.agents/skills/\$s"
  printf -- '---\nname: %s\n---\n# lane\n' "\$s" >"\$HOME/.agents/skills/\$s/SKILL.md"
done
EOF
chmod +x "$stub/ps" "$stub/npx"
export PATH="$stub:$PATH"

ACT_CLAUDE="$HOME/act/claude"
export UPDATE_SKILLS_CLAUDE_ACTIVITY_DIR="$ACT_CLAUDE"
export UPDATE_SKILLS_CODEX_ACTIVITY_DIR="$HOME/act/codex"
export UPDATE_SKILLS_HERMES_ACTIVITY_DIR="$HOME/act/hermes"
export UPDATE_SKILLS_IDLE_THRESHOLD=900

# Establish a live generation with alpha.
write_lock alpha
mkdir -p "$AGENTS/skills/alpha"
printf -- '---\nname: alpha\n---\n# seed\n' >"$AGENTS/skills/alpha/SKILL.md"
printf '{"skills":{"alpha":{}}}\n' >"$AGENTS/.skill-lock.json"
UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" >/dev/null 2>&1 || fail "part A setup full run failed"
id_setup="$(jq -r '.id' "$CURRENT/generation.json")"

# beta absent + an active harness -> the updater must DEFER with exit 75.
write_lock alpha beta
mkdir -p "$ACT_CLAUDE"
: >"$ACT_CLAUDE/live.jsonl"
set +e
out_defer="$(FAKE_PS='/opt/homebrew/bin/claude --remote-control' bash "$SCRIPT" --install-only 2>&1)"
rc_defer=$?
set -e
[[ $rc_defer -eq 75 ]] ||
  fail "install-only deferred on activity did not exit the distinct code 75 (got $rc_defer): $out_defer"
grep -qi 'deferring the generation exchange' <<<"$out_defer" ||
  fail "the deferred run did not log the deferral: $out_defer"
[[ "$(jq -r '.id' "$CURRENT/generation.json")" == "$id_setup" ]] ||
  fail "the deferred run still exchanged the generation"
[[ ! -e "$AGENTS/skills/beta" && ! -L "$AGENTS/skills/beta" ]] ||
  fail "the deferred run still installed beta"

# A hard required failure must NOT masquerade as a defer: exit 1, not 75.
# (npx add fails -> a required-phase failure -> exit 1.)
rm -rf "$HOME/act" # idle now, so the exchange is attempted and the lane runs
cat >"$stub/npx" <<'EOF'
#!/usr/bin/env bash
echo "npx boom" >&2
exit 1
EOF
chmod +x "$stub/npx"
set +e
out_fail="$(FAKE_PS='/usr/bin/python3 idle.py' bash "$SCRIPT" --install-only 2>&1)"
rc_fail=$?
set -e
[[ $rc_fail -eq 1 ]] ||
  fail "a hard install-only failure did not exit 1 (got $rc_fail): $out_fail"

# ── Part B: the wrapper preserves the retry marker on the deferred code ──────
sbox="$tmp/home"
mkdir -p "$sbox/.local/bin"
MARKER="$sbox/.local/state/skills/first-install-pending"
cat >"$sbox/.local/bin/update-skills.sh" <<'EOF'
#!/usr/bin/env bash
exit "${FAKE_UPDATER_RC:-0}"
EOF
chmod +x "$sbox/.local/bin/update-skills.sh"
render() { HOME="$sbox" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$TMPL" >"$1"; }
runner="$tmp/runner.sh"
render "$runner" || fail "rendering the first-install wrapper failed"

# deferred (75): marker created, wrapper exits 0 (apply not aborted).
rm -rf "$sbox/.local/state"
FAKE_UPDATER_RC=75 HOME="$sbox" bash "$runner" ||
  fail "the wrapper aborted the apply on a deferred (75) install-only (must exit 0)"
[[ -f $MARKER ]] ||
  fail "a deferred install-only left no retry marker (next apply will not re-fire)"

# a second deferral keeps the marker present (still retryable).
FAKE_UPDATER_RC=75 HOME="$sbox" bash "$runner" ||
  fail "the wrapper aborted the apply on the second deferral"
[[ -f $MARKER ]] || fail "the retry marker vanished after a second deferral"

# the install finally completes: marker removed.
FAKE_UPDATER_RC=0 HOME="$sbox" bash "$runner" ||
  fail "the wrapper exited non-zero on the completing run"
[[ ! -e $MARKER ]] || fail "the retry marker was not cleared once the install completed"

# a hard failure (1) still bumps a marker and exits 0 (unchanged contract).
rm -rf "$sbox/.local/state"
FAKE_UPDATER_RC=1 HOME="$sbox" bash "$runner" ||
  fail "the wrapper aborted the apply on a hard install failure (must exit 0)"
[[ -f $MARKER ]] || fail "a hard failure left no retry marker"

echo "update-skills-install-defer-retry: OK"
