#!/usr/bin/env bash
# update-skills-install-only-gate.sh (integration-fix F3): --install-only must
# not publish/exchange when nothing is absent. It always called publish, so with
# zero absent skills it still exchanged the live generation, displacing a
# concurrent out-of-band write for no gain. The fix computes the needs-work set
# FIRST: empty -> no build, no exchange. This test asserts:
#   1. zero absent -> no exchange (generation id unchanged), no CLI calls;
#   2. one absent -> added, and the exchange happened (id changed);
#   3. a live agent world does not hold the install back (the activity gate that
#      used to defer this exchange is gone; see test/e2e/update-skills-unattended.sh
#      for why).
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
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

# Stubs: ps honors FAKE_PS (the simulated process world, kept so a reintroduced
# activity gate observes the world this test stages rather than the real
# machine's); npx logs argv and writes a SKILL.md per --skill.
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

# A per-turn file whose mtime is now, in the location the removed activity gate
# probed by default, so case 3 stages the world that used to defer.
stage_live_agent_activity() {
  mkdir -p "$HOME/.claude/projects"
  : >"$HOME/.claude/projects/live.jsonl"
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

# --- Case 2: one absent -> added, exchange happened ---------------------------
write_lock alpha beta
: >"$NPX_LOG"
out2="$(FAKE_PS="$NO_HARNESS" bash "$SCRIPT" --install-only 2>&1)" ||
  fail "install-only (add) exited non-zero: $out2"
[[ -L "$AGENTS/skills/beta" && -f "$AGENTS/skills/beta/SKILL.md" ]] ||
  fail "case 2: absent beta was not installed"
[[ "$(gen_id)" != "$id_setup" ]] ||
  fail "case 2: the generation was not exchanged when an absent skill was added"
grep -q -- '--skill beta' "$NPX_LOG" ||
  fail "case 2: the npx lane did not install beta: $(cat "$NPX_LOG")"
# alpha (already present) is untouched and still resolves.
[[ -L "$AGENTS/skills/alpha" && -f "$AGENTS/skills/alpha/SKILL.md" ]] ||
  fail "case 2: alpha stopped resolving after the additive install"
id_case2="$(gen_id)"

# --- Case 3: one absent + a live agent world -> still added -------------------
# The exchange under an agent session is safe (one atomic swap, one retained
# generation, content read at invocation time), so activity no longer defers it.
write_lock alpha beta gamma
stage_live_agent_activity
: >"$NPX_LOG"
out3="$(FAKE_PS="$HARNESS" bash "$SCRIPT" --install-only 2>&1)" ||
  fail "install-only with a live agent world exited non-zero: $out3"
grep -qi 'deferring' <<<"$out3" &&
  fail "case 3: install-only deferred under a live agent world; the activity gate is back: $out3"
[[ -L "$AGENTS/skills/gamma" && -f "$AGENTS/skills/gamma/SKILL.md" ]] ||
  fail "case 3: absent gamma was not installed under a live agent world"
[[ "$(gen_id)" != "$id_case2" ]] ||
  fail "case 3: the generation was not exchanged under a live agent world"

echo "update-skills-install-only-gate: OK"
