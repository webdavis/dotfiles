#!/usr/bin/env bash
# update-skills-install-only-health.sh (R2-5): the install-only no-op must be
# gated on real live HEALTH, not mere path existence. Pre-fix, a store entry
# that merely EXISTS (-e/-L) counted a skill "present", so the early return
# skipped a genuinely broken skill: a store link that resolves but whose
# SKILL.md is gone, or a core<->on-demand TIER change (whose overlay drift has
# no absent path at all) both false-no-op'd forever. Now the no-op is allowed
# only after validating, for every roster skill: the link resolves into the
# current generation, SKILL.md is present, the npx lock entry is present, and
# the required tier overlay is present. Any drift builds an idle-gated repair
# candidate.
# Cases:
#   1. fully healthy roster -> true no-op (no exchange, no CLI calls);
#   2. a resolving link with a MISSING SKILL.md -> repair (SKILL.md restored,
#      generation exchanged);
#   3. a core->on-demand TIER change -> repair (the overlay appears in the new
#      generation), even though no store path was absent.
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

# alpha is core, beta is on-demand from the start (so overlays are asserted).
write_lock() { # $1 = alpha tier, $2 = beta tier
  cat >"$LOCK" <<EOF
{
  "version": 2,
  "tiers": {"alpha": "$1", "beta": "$2"},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": {"alpha": {"repo": "fixture/pack"}, "beta": {"repo": "fixture/pack"}},
  "clawhubTracked": {},
  "forks": {}
}
EOF
}

stub="$tmp/stub"
mkdir -p "$stub"
NPX_LOG="$tmp/npx.log"
: >"$NPX_LOG"
cat >"$stub/npx" <<EOF
#!/usr/bin/env bash
set -euo pipefail
printf 'npx %s\n' "\$*" >>"$NPX_LOG"
prev=""; skills=()
for a in "\$@"; do
  [[ \$prev == --skill ]] && skills+=("\$a")
  prev="\$a"
done
cli_lock="\${XDG_STATE_HOME:-\$HOME/.local/state}/skills/.skill-lock.json"
mkdir -p "\$(dirname "\$cli_lock")"
[[ -f \$cli_lock ]] || printf '{"version":3,"skills":{}}\n' >"\$cli_lock"
for s in "\${skills[@]}"; do
  mkdir -p "\$HOME/.agents/skills/\$s"
  printf -- '---\nname: %s\n---\n# lane\n' "\$s" >"\$HOME/.agents/skills/\$s/SKILL.md"
  jq --arg s "\$s" '.skills[\$s] = {source: "github:fixture"}' \
    "\$cli_lock" >"\$cli_lock.tmp" && mv "\$cli_lock.tmp" "\$cli_lock"
done
EOF
chmod +x "$stub/npx"
export PATH="$stub:$PATH"

gen_id() { jq -r '.id' "$CURRENT/generation.json" 2>/dev/null || echo NONE; }

# --- Setup: full run publishes a healthy {alpha(core), beta(on-demand)} -------
write_lock core on-demand
for n in alpha beta; do
  mkdir -p "$AGENTS/skills/$n"
  printf -- '---\nname: %s\n---\n# seed\n' "$n" >"$AGENTS/skills/$n/SKILL.md"
done
printf '{"skills":{"alpha":{},"beta":{}}}\n' >"$AGENTS/.skill-lock.json"
UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" >/dev/null 2>&1 || fail "setup full run failed"
[[ -f "$CURRENT/generation.json" ]] || fail "setup produced no live generation"
grep -q 'allow_implicit_invocation: false' "$CURRENT/skills/beta/agents/openai.yaml" ||
  fail "setup: beta did not get its on-demand overlay"
id_setup="$(gen_id)"

# --- Case 1: fully healthy -> true no-op --------------------------------------
: >"$NPX_LOG"
out1="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --install-only 2>&1)" ||
  fail "case 1 install-only exited non-zero: $out1"
[[ "$(gen_id)" == "$id_setup" ]] ||
  fail "case 1: a healthy roster still exchanged the generation"
[[ ! -s $NPX_LOG ]] ||
  fail "case 1: a healthy roster still invoked the npx lane: $(cat "$NPX_LOG")"

# --- Case 2: resolving link, MISSING SKILL.md -> repair -----------------------
# The store link still resolves into the generation, but the generation's
# SKILL.md is gone: -e/-L would call this "present" and skip it.
rm -f "$CURRENT/skills/alpha/SKILL.md"
[[ -L "$AGENTS/skills/alpha" ]] || fail "case 2 precondition: alpha store link is not a symlink"
[[ ! -f "$AGENTS/skills/alpha/SKILL.md" ]] || fail "case 2 precondition: alpha SKILL.md still resolves"
: >"$NPX_LOG"
out2="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --install-only 2>&1)" ||
  fail "case 2 install-only exited non-zero: $out2"
[[ "$(gen_id)" != "$id_setup" ]] ||
  fail "case 2: a broken SKILL.md was treated as healthy (false no-op, no repair): $out2"
[[ -f "$AGENTS/skills/alpha/SKILL.md" ]] ||
  fail "case 2: the repair did not restore alpha's SKILL.md"
id_after2="$(gen_id)"

# --- Case 3: core->on-demand tier change -> repair (overlay appears) ----------
# alpha becomes on-demand; nothing is absent and the content is intact, but the
# required overlay is missing, so a health-gated no-op must refuse and repair.
write_lock on-demand on-demand
: >"$NPX_LOG"
out3="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --install-only 2>&1)" ||
  fail "case 3 install-only exited non-zero: $out3"
[[ "$(gen_id)" != "$id_after2" ]] ||
  fail "case 3: a tier change was a false no-op (no repair candidate built): $out3"
grep -q 'allow_implicit_invocation: false' "$CURRENT/skills/alpha/agents/openai.yaml" ||
  fail "case 3: alpha's new on-demand overlay is missing after the repair"

echo "update-skills-install-only-health: OK"
