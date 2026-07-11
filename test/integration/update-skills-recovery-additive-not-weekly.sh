#!/usr/bin/env bash
# update-skills-recovery-additive-not-weekly.sh (integration-fix F4): weekly
# recovery must never publish an ADDITIVE (install-only) candidate as a full
# weekly refresh. generation.json records identity + source hashes; an
# install-only run killed after the ready marker but before publish leaves a
# COMPLETE candidate whose hashes match desired state, yet whose existing skills
# are stale byte-clones (nothing was refreshed). Reusing it would ship
# unrefreshed content and stamp the week a success. The fix records a validated
# buildMode ("full" | "additive"); weekly recovery reuses only a "full"
# candidate. This test fabricates a ready ADDITIVE candidate and drives the
# weekly path, asserting it is NOT published (a fresh full build runs instead)
# and the stamp is not written from the additive clone.
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

cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{
  "version": 2,
  "tiers": {"alpha": "core"},
  "hermesProfiles": {"alpha": []},
  "hermesRegistry": {},
  "npxTracked": {"alpha": {"repo": "fixture/alpha"}},
  "clawhubTracked": {},
  "forks": {}
}
EOF

# npx stub: the lane writes a distinctive REFRESHED marker into alpha's SKILL.md
# and logs its argv, so a fresh full build is observable and distinguishable from
# a reused (never-relaned) candidate.
stub="$tmp/stub"
mkdir -p "$stub"
NPX_LOG="$tmp/npx.log"
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
  printf -- '---\nname: %s\n---\n# REFRESHED-BY-LANE\n' "\$s" >"\$HOME/.agents/skills/\$s/SKILL.md"
done
EOF
chmod +x "$stub/npx"
export PATH="$stub:$PATH"

AGENTS="$HOME/.agents"
CURRENT="$AGENTS/.skills-current"
GENERATIONS="$AGENTS/.skills-generations"
STAMP="$HOME/.local/state/update-skills/last-success"

run_full() { UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1; }

# --- Establish a live FULL generation (seed a flat store so migration builds it) ---
mkdir -p "$AGENTS/skills/alpha"
printf -- '---\nname: alpha\n---\n# seed\n' >"$AGENTS/skills/alpha/SKILL.md"
printf '{"skills":{"alpha":{}}}\n' >"$AGENTS/.skill-lock.json"
out1="$(run_full)" || fail "seed full run exited non-zero: $out1"
[[ -f "$CURRENT/generation.json" ]] || fail "no live generation after the seed run"
[[ "$(jq -r '.buildMode' "$CURRENT/generation.json")" == "full" ]] ||
  fail "the seed generation was not recorded as buildMode=full"

# --- Fabricate a ready ADDITIVE candidate that MATCHES desired hashes ----------
# Its existing skill is a STALE byte-clone (distinct marker), and its meta copies
# the live hashes but flips buildMode to "additive": the exact crash-window state
# an install-only run killed after the ready marker leaves behind.
cand="$GENERATIONS/additivecand-5-5/home/.agents"
mkdir -p "$cand/skills/alpha"
printf -- '---\nname: alpha\n---\n# ADDITIVE-STALE-CLONE\n' >"$cand/skills/alpha/SKILL.md"
printf '{"skills":{"alpha":{}}}\n' >"$cand/.skill-lock.json"
jq '.id = "additivecand-5-5" | .buildMode = "additive"' \
  "$CURRENT/generation.json" >"$cand/generation.json"
[[ "$(jq -r '.customLockHash' "$cand/generation.json")" == "$(jq -r '.customLockHash' "$CURRENT/generation.json")" ]] ||
  fail "the fabricated additive candidate does not carry matching desired hashes"

# --- Run the weekly path with the additive candidate present ------------------
: >"$NPX_LOG"
out2="$(run_full)" || fail "weekly run exited non-zero: $out2"

# 1) The additive clone was NOT published: live content is the fresh lane's
#    REFRESHED marker, never the ADDITIVE-STALE-CLONE.
live_alpha="$CURRENT/skills/alpha/SKILL.md"
[[ -f $live_alpha ]] || fail "no live alpha after the weekly run"
grep -q 'ADDITIVE-STALE-CLONE' "$live_alpha" &&
  fail "the additive candidate was published as the weekly result (stale clone is live)"
grep -q 'REFRESHED-BY-LANE' "$live_alpha" ||
  fail "the weekly run did not publish a freshly relaned full generation"

# 2) A fresh FULL build actually ran the npx lane (the reuse path would skip it).
[[ -s $NPX_LOG ]] ||
  fail "the weekly run reused a candidate instead of building fresh (npx lane never ran)"

# 3) The published generation is recorded as full, and the additive candidate dir
#    was cleaned up (recovery destroyed the non-reusable staging).
[[ "$(jq -r '.buildMode' "$CURRENT/generation.json")" == "full" ]] ||
  fail "the published weekly generation is not buildMode=full"
[[ ! -d "$GENERATIONS/additivecand-5-5" ]] ||
  fail "the non-reusable additive candidate was not cleaned up by recovery"

# 4) The stamp reflects a real full success, not the additive clone.
[[ -f $STAMP ]] || fail "a successful weekly run did not write the stamp"

echo "update-skills-recovery-additive-not-weekly: OK"
