#!/usr/bin/env bash
# update-skills-install-only-gate.sh (integration-fix F3): --install-only must
# not publish/exchange when nothing is absent, and must gate the exchange behind
# the idle gate when a live generation exists. It bypassed the idle gate on the
# premise it never swaps existing folders, but it always called publish; with
# zero absent skills it still exchanged the live generation (displacing a
# concurrent write, switching readers mid-session). The fix computes the absent
# set FIRST: empty -> no build, no exchange; non-empty with a live generation ->
# the exchange is idle-gated. This test asserts:
#   1. zero absent -> no exchange (generation id unchanged), no CLI calls;
#   2. one absent + an active harness -> deferred (no exchange, skill not added);
#   3. one absent + idle -> added, and the exchange happened (id changed).
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"
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

HOME="$tmp/home"
export HOME
export UPDATE_SKILLS_GMV="$GMV_BIN"
mkdir -p "$HOME/.agents/skills"
AGENTS="$HOME/.agents"
CURRENT="$AGENTS/.skills-current"
LOCK="$AGENTS/custom-skill-lock.json"

write_lock() { # $@ = tracked skill names
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

# Stubs: ps honors FAKE_PS (the simulated process world); npx logs argv and
# writes a SKILL.md per --skill.
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
for a in "\$@"; do
  [[ \$prev == --skill ]] && skills+=("\$a")
  prev="\$a"
done
for s in "\${skills[@]}"; do
  mkdir -p "\$HOME/.agents/skills/\$s"
  printf -- '---\nname: %s\n---\n# lane\n' "\$s" >"\$HOME/.agents/skills/\$s/SKILL.md"
done
EOF
chmod +x "$stub/ps" "$stub/npx"
export PATH="$stub:$PATH"

# Activity evidence surface pointed at the tmp HOME.
ACT_CLAUDE="$HOME/act/claude"
export UPDATE_SKILLS_CLAUDE_ACTIVITY_DIR="$ACT_CLAUDE"
export UPDATE_SKILLS_CODEX_ACTIVITY_DIR="$HOME/act/codex"
export UPDATE_SKILLS_HERMES_ACTIVITY_DIR="$HOME/act/hermes"
export UPDATE_SKILLS_IDLE_THRESHOLD=900
harness_active() {
  rm -rf "$HOME/act"
  mkdir -p "$1"
  : >"$1/live.jsonl"
}
gen_id() { jq -r '.id' "$CURRENT/generation.json" 2>/dev/null || echo NONE; }

HARNESS='/opt/homebrew/bin/claude --remote-control'
NO_HARNESS='/usr/bin/python3 /usr/local/bin/some-tool.py --flag'

# --- Setup: establish a live generation with alpha (FORCE full run) -----------
write_lock alpha
mkdir -p "$AGENTS/skills/alpha"
printf -- '---\nname: alpha\n---\n# seed\n' >"$AGENTS/skills/alpha/SKILL.md"
printf '{"skills":{"alpha":{}}}\n' >"$AGENTS/.skill-lock.json"
UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" >/dev/null 2>&1 || fail "setup full run failed"
[[ -f "$CURRENT/generation.json" ]] || fail "setup did not produce a live generation"
id_setup="$(gen_id)"

# --- Case 1: zero absent -> no exchange, no CLI calls -------------------------
: >"$NPX_LOG"
out1="$(FAKE_PS="$NO_HARNESS" bash "$SCRIPT" --install-only 2>&1)" ||
  fail "install-only (zero absent) exited non-zero: $out1"
printf '%s\n' "$out1" | grep -qF 'present and healthy; no changes' ||
  fail "case 1: install-only with a healthy roster did not report the no-op: $out1"
[[ "$(gen_id)" == "$id_setup" ]] ||
  fail "case 1: the live generation was exchanged though nothing was absent"
[[ ! -s $NPX_LOG ]] ||
  fail "case 1: a package CLI was invoked though nothing was absent: $(cat "$NPX_LOG")"

# --- Case 2: one absent + active harness -> deferred exchange -----------------
write_lock alpha beta
harness_active "$ACT_CLAUDE"
: >"$NPX_LOG"
out2="$(FAKE_PS="$HARNESS" bash "$SCRIPT" --install-only 2>&1)" ||
  fail "install-only (deferred) exited non-zero: $out2"
printf '%s\n' "$out2" | grep -qiF 'deferring the generation exchange' ||
  fail "case 2: install-only with an active harness did not defer the exchange: $out2"
[[ "$(gen_id)" == "$id_setup" ]] ||
  fail "case 2: the generation was exchanged despite an active harness"
[[ ! -e "$AGENTS/skills/beta" && ! -L "$AGENTS/skills/beta" ]] ||
  fail "case 2: absent beta was installed despite the deferral"
[[ ! -s $NPX_LOG ]] ||
  fail "case 2: the lane ran though the exchange was deferred: $(cat "$NPX_LOG")"

# --- Case 3: one absent + idle -> added, exchange happened --------------------
rm -rf "$HOME/act" # no fresh activity
: >"$NPX_LOG"
out3="$(FAKE_PS="$NO_HARNESS" bash "$SCRIPT" --install-only 2>&1)" ||
  fail "install-only (idle add) exited non-zero: $out3"
[[ -L "$AGENTS/skills/beta" && -f "$AGENTS/skills/beta/SKILL.md" ]] ||
  fail "case 3: absent beta was not installed while idle"
[[ "$(gen_id)" != "$id_setup" ]] ||
  fail "case 3: the generation was not exchanged when an absent skill was added"
grep -q -- '--skill beta' "$NPX_LOG" ||
  fail "case 3: the npx lane did not install beta: $(cat "$NPX_LOG")"
# alpha (already present) is untouched and still resolves.
[[ -L "$AGENTS/skills/alpha" && -f "$AGENTS/skills/alpha/SKILL.md" ]] ||
  fail "case 3: alpha stopped resolving after the additive install"

echo "update-skills-install-only-gate: OK"
