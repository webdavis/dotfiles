#!/usr/bin/env bash
# update-skills-exchange-reprobe.sh (fix-A F5): the activity idle-gate must be
# re-checked IMMEDIATELY before every generation exchange. The only install-only
# probe ran BEFORE the long npx/clawhub lanes, so a harness that became active
# mid-build still got its generation swapped underneath it. The fix re-probes
# fail-closed right before the exchange (weekly and install-only), gated on the
# current path EXISTING; on deferral it preserves retryability (exit 75) and
# never publishes.
#
# Regression: a run that is idle at the first probe but whose lane turns a
# harness ACTIVE (a fresh activity file) before the exchange must DEFER (75) and
# leave the live generation untouched.
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

ACT_CLAUDE="$HOME/act/claude"
export UPDATE_SKILLS_CLAUDE_ACTIVITY_DIR="$ACT_CLAUDE"
export UPDATE_SKILLS_CODEX_ACTIVITY_DIR="$HOME/act/codex"
export UPDATE_SKILLS_HERMES_ACTIVITY_DIR="$HOME/act/hermes"
export UPDATE_SKILLS_IDLE_THRESHOLD=900

stub="$tmp/stub"
mkdir -p "$stub"
cat >"$stub/ps" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "${FAKE_PS:-}"
EOF
# npx stub: installs each --skill AND, on first invocation, turns the Claude
# harness ACTIVE by creating a fresh activity file (absolute path baked in, since
# the lane runs under env -i with no activity-dir env). This simulates a harness
# becoming busy DURING the build, after the first idle probe already passed.
cat >"$stub/npx" <<EOF
#!/usr/bin/env bash
set -euo pipefail
mkdir -p "$ACT_CLAUDE"
: >"$ACT_CLAUDE/live.jsonl"
prev=""; skills=()
for a in "\$@"; do [[ \$prev == --skill ]] && skills+=("\$a"); prev="\$a"; done
cli_lock="\${XDG_STATE_HOME:-\$HOME/.local/state}/skills/.skill-lock.json"
mkdir -p "\$(dirname "\$cli_lock")"
[[ -f \$cli_lock ]] || printf '{"version":3,"skills":{}}\n' >"\$cli_lock"
for s in "\${skills[@]}"; do
  mkdir -p "\$HOME/.agents/skills/\$s"
  printf -- '---\nname: %s\n---\n# lane\n' "\$s" >"\$HOME/.agents/skills/\$s/SKILL.md"
  jq --arg s "\$s" '.skills[\$s] = {source: "github:fixture/pack", agents: ["claude-code","codex"]}' \
    "\$cli_lock" >"\$cli_lock.tmp" && mv "\$cli_lock.tmp" "\$cli_lock"
done
EOF
printf '#!/usr/bin/env bash\nexit 0\n' >"$stub/alerter"
chmod +x "$stub/ps" "$stub/npx" "$stub/alerter"
export PATH="$stub:$PATH"

# ── Establish a live generation with alpha (FORCE bypasses the gate) ──────────
write_lock alpha
mkdir -p "$AGENTS/skills/alpha"
printf -- '---\nname: alpha\n---\n# seed\n' >"$AGENTS/skills/alpha/SKILL.md"
printf '{"skills":{"alpha":{}}}\n' >"$AGENTS/.skill-lock.json"
UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" >/dev/null 2>&1 || fail "setup full run failed"
id_before="$(jq -r '.id' "$CURRENT/generation.json")"
rm -rf "$HOME/act" # idle at the first probe

# ── install-only: beta is work; the lane turns the harness active mid-build ───
# A harness PROCESS exists throughout, but no fresh activity file until the lane
# creates one, so the FIRST probe sees idle and the run proceeds into the lanes.
write_lock alpha beta
set +e
out="$(FAKE_PS='/opt/homebrew/bin/claude --remote-control' bash "$SCRIPT" --install-only 2>&1)"
rc=$?
set -e

[[ $rc -eq 75 ]] ||
  fail "a mid-build activity onset did not defer the exchange with exit 75 (got $rc): $out"
[[ "$(jq -r '.id' "$CURRENT/generation.json")" == "$id_before" ]] ||
  fail "the exchange happened despite mid-build harness activity (generation swapped)"
[[ ! -e "$AGENTS/skills/beta" && ! -L "$AGENTS/skills/beta" ]] ||
  fail "beta was installed despite the deferral"

echo "update-skills-exchange-reprobe: OK"
