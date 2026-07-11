#!/usr/bin/env bash
# update-skills-delist.sh (integration-fix F2): a skill DELISTED from the lock
# must not persist live. The candidate cloned every dir from the current
# generation and convergence trusted every surviving store entry, so removing a
# skill from the lock (e.g. a revoked or compromised one) left it live in the
# store and in Claude's desired set. The fix carries only TRACKED names forward
# into the candidate and, after publish, removes obsolete updater-owned store
# links no longer tracked, while preserving foreign real dirs. Track {alpha,
# beta}, publish; then delist beta and run; assert beta is gone from the store,
# the generation, and the Claude desired set, while a foreign real dir `gamma`
# survives.
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
  local -a entries=()
  local n
  for n in "$@"; do entries+=("$n"); done
  local tiers="" npx=""
  for n in "${entries[@]}"; do
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

# npx stub: writes a SKILL.md for each --skill in the group, and maintains the
# CLI's global lock at $XDG_STATE_HOME/skills/.skill-lock.json exactly like the
# real skills CLI (verified empirically against skills 1.5.16: with
# XDG_STATE_HOME set, getSkillLockPath() reads AND writes there, never
# ~/.agents/.skill-lock.json).
stub="$tmp/stub"
mkdir -p "$stub"
cat >"$stub/npx" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
prev=""
skills=()
for a in "$@"; do
  [[ $prev == --skill ]] && skills+=("$a")
  prev="$a"
done
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
chmod +x "$stub/npx"
export PATH="$stub:$PATH"

published_lock_keys() {
  jq -r '.skills // {} | keys | sort | join(",")' "$AGENTS/.skill-lock.json" 2>/dev/null || echo READ-ERROR
}

run_full() { UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1; }

# --- Phase 1: track {alpha, beta}, seed a flat store, publish -----------------
write_lock alpha beta
for n in alpha beta; do
  mkdir -p "$AGENTS/skills/$n"
  printf -- '---\nname: %s\n---\n# seed\n' "$n" >"$AGENTS/skills/$n/SKILL.md"
done
printf '{"skills":{"alpha":{},"beta":{}}}\n' >"$AGENTS/.skill-lock.json"
out1="$(run_full)" || fail "phase 1 run exited non-zero: $out1"

for n in alpha beta; do
  [[ -L "$AGENTS/skills/$n" ]] || fail "phase 1: store/$n is not a symlink"
  [[ -d "$CURRENT/skills/$n" ]] || fail "phase 1: generation is missing $n"
  [[ -L "$HOME/.claude/skills/$n" && -f "$HOME/.claude/skills/$n/SKILL.md" ]] ||
    fail "phase 1: Claude fan-out for $n does not resolve"
done

# --- Phase 2: delist beta; add a foreign real dir gamma; run ------------------
write_lock alpha
mkdir -p "$AGENTS/skills/gamma"
printf 'FOREIGN-GAMMA\n' >"$AGENTS/skills/gamma/keep.txt"
out2="$(run_full)" || fail "phase 2 run exited non-zero: $out2"

# beta is gone from the store (no symlink, no dir).
[[ ! -e "$AGENTS/skills/beta" && ! -L "$AGENTS/skills/beta" ]] ||
  fail "delisted beta persists in the store: $(ls -ld "$AGENTS/skills/beta" 2>&1)"
# beta is gone from the live generation.
[[ ! -e "$CURRENT/skills/beta" ]] ||
  fail "delisted beta persists in the live generation"
# beta is gone from the Claude desired set (its owned link was reaped).
[[ ! -e "$HOME/.claude/skills/beta" && ! -L "$HOME/.claude/skills/beta" ]] ||
  fail "delisted beta persists in the Claude fan-out"

# alpha survives and still resolves.
[[ -L "$AGENTS/skills/alpha" && -f "$AGENTS/skills/alpha/SKILL.md" ]] ||
  fail "still-tracked alpha did not survive the delist run"
[[ -d "$CURRENT/skills/alpha" ]] || fail "alpha left the generation"

# the foreign real dir gamma survives untouched (never an updater-owned link).
[[ -d "$AGENTS/skills/gamma" && ! -L "$AGENTS/skills/gamma" ]] ||
  fail "the foreign real dir gamma was destroyed"
[[ "$(cat "$AGENTS/skills/gamma/keep.txt")" == "FOREIGN-GAMMA" ]] ||
  fail "gamma content changed"

# R2-4: the PUBLISHED npx lock must hold EXACTLY the npxTracked set. A
# delisted key surviving in the published lock would let a later
# `npx skills update -g` reinstall the revoked skill as a real store dir
# (which the pruner spares and the Claude fan-out then accepts).
[[ "$(published_lock_keys)" == "alpha" ]] ||
  fail "delisted beta survives in the PUBLISHED npx lock (keys: $(published_lock_keys))"

# --- Phase 3: a newly tracked skill's lock entry is CAPTURED on publish -------
# The child CLI writes its lock under the pinned XDG_STATE_HOME (verified
# empirically); the candidate must capture that write, or the published lock
# silently loses every skill the lanes just installed.
write_lock alpha delta
out3="$(run_full)" || fail "phase 3 run exited non-zero: $out3"
[[ -L "$AGENTS/skills/delta" && -f "$AGENTS/skills/delta/SKILL.md" ]] ||
  fail "phase 3: newly tracked delta was not installed"
[[ "$(published_lock_keys)" == "alpha,delta" ]] ||
  fail "phase 3: the published npx lock did not capture the lane's lock write for delta (keys: $(published_lock_keys))"

echo "update-skills-delist: OK"
