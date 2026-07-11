#!/usr/bin/env bash
# update-skills-tier-symmetric.sh (fix-A F6): tier reconciliation must be
# symmetric. Health only checked that an ON-DEMAND skill HAS its Codex policy
# overlay, so an on-demand -> core change left the stale
# `allow_implicit_invocation: false` block in place: health reported the skill
# healthy, install-only no-op'd, and the candidate overlay logic (add-only)
# never removed it, so Codex kept treating a now-core skill as
# never-auto-invokable. The fix: when the desired tier is core, REMOVE the
# updater-owned policy block (preserving any upstream metadata) and treat a
# lingering block as unhealthy so it drives a repair.
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
LOCK="$AGENTS/custom-skill-lock.json"
OVERLAY="$AGENTS/skills/beta/agents/openai.yaml"

# $1 = alpha's tier, $2 = beta's tier
write_lock() {
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
cat >"$stub/npx" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
prev=""; skills=()
for a in "$@"; do [[ $prev == --skill ]] && skills+=("$a"); prev="$a"; done
cli_lock="${XDG_STATE_HOME:-$HOME/.local/state}/skills/.skill-lock.json"
mkdir -p "$(dirname "$cli_lock")"
[[ -f $cli_lock ]] || printf '{"version":3,"skills":{}}\n' >"$cli_lock"
for s in "${skills[@]}"; do
  mkdir -p "$HOME/.agents/skills/$s"
  printf -- '---\nname: %s\n---\n# lane\n' "$s" >"$HOME/.agents/skills/$s/SKILL.md"
  jq --arg s "$s" '.skills[$s] = {source: "github:fixture/pack", agents: ["claude-code","codex"]}' \
    "$cli_lock" >"$cli_lock.tmp" && mv "$cli_lock.tmp" "$cli_lock"
done
EOF
printf '#!/usr/bin/env bash\nexit 0\n' >"$stub/alerter"
chmod +x "$stub/npx" "$stub/alerter"
export PATH="$stub:$PATH"

has_policy() { grep -q 'allow_implicit_invocation: false' "$OVERLAY" 2>/dev/null; }

# ── Phase 1: publish {alpha core, beta on-demand}, beta gets the policy ──────
write_lock core on-demand
for n in alpha beta; do
  mkdir -p "$AGENTS/skills/$n"
  printf -- '---\nname: %s\n---\n# seed\n' "$n" >"$AGENTS/skills/$n/SKILL.md"
done
printf '{"skills":{"alpha":{},"beta":{}}}\n' >"$AGENTS/.skill-lock.json"
UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" >/dev/null 2>&1 || fail "phase 1 full run failed"
has_policy || fail "phase 1: on-demand beta did not get the Codex policy overlay"

# ── Phase 2: beta -> core; an install-only run must REMOVE the stale policy ────
write_lock core core
out="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --install-only 2>&1)" ||
  fail "phase 2 install-only run exited non-zero: $out"

! has_policy ||
  fail "beta went on-demand -> core but its stale Codex policy overlay was not removed (false no-op)"
# alpha (still core) and beta both resolve.
for n in alpha beta; do
  [[ -f "$AGENTS/skills/$n/SKILL.md" ]] || fail "phase 2: $n no longer resolves"
done

echo "update-skills-tier-symmetric: OK"
