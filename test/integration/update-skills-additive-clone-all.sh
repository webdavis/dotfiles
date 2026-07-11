#!/usr/bin/env bash
# update-skills-additive-clone-all.sh (R2-7): an install-only (additive)
# candidate must clone EVERY current generation entry unchanged. The exact
# tracked-name filter and the delist pruner belong to the FULL weekly run
# (where fan-out convergence also drops the delisted links); applying the
# filter in additive mode drops a delisted entry from the generation while
# install-only never runs the pruner, so the store link and Claude fan-out
# for it dangle. Scenario: track {alpha, beta}, publish; then move the lock to
# {alpha, gamma} (delist beta, add gamma) and run --install-only.
#   - gamma is installed (it was genuinely absent);
#   - beta SURVIVES untouched (generation dir, store link, Claude fan-out): an
#     additive run removes nothing;
#   - alpha survives.
# The companion delist test asserts the FULL run still drops beta.
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

# npx stub: writes a SKILL.md per --skill and records it in the CLI's global
# lock (matches the real CLI's XDG_STATE_HOME location).
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
  jq --arg s "$s" '.skills[$s] = {source: "github:fixture"}' \
    "$cli_lock" >"$cli_lock.tmp" && mv "$cli_lock.tmp" "$cli_lock"
done
EOF
chmod +x "$stub/npx"
export PATH="$stub:$PATH"

# --- Phase 1: track {alpha, beta}, seed a flat store, publish (full run) -------
write_lock alpha beta
for n in alpha beta; do
  mkdir -p "$AGENTS/skills/$n"
  printf -- '---\nname: %s\n---\n# seed\n' "$n" >"$AGENTS/skills/$n/SKILL.md"
done
printf '{"skills":{"alpha":{},"beta":{}}}\n' >"$AGENTS/.skill-lock.json"
out1="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1)" || fail "phase 1 full run exited non-zero: $out1"
for n in alpha beta; do
  [[ -L "$AGENTS/skills/$n" && -d "$CURRENT/skills/$n" ]] ||
    fail "phase 1: $n did not publish"
  [[ -L "$HOME/.claude/skills/$n" && -f "$HOME/.claude/skills/$n/SKILL.md" ]] ||
    fail "phase 1: Claude fan-out for $n does not resolve"
done

# --- Phase 2: delist beta + add gamma in the lock; run --install-only ----------
write_lock alpha gamma
out2="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --install-only 2>&1)" ||
  fail "phase 2 install-only exited non-zero: $out2"

# gamma (genuinely absent) was installed.
[[ -L "$AGENTS/skills/gamma" && -f "$AGENTS/skills/gamma/SKILL.md" ]] ||
  fail "install-only did not install the newly tracked gamma: $out2"
[[ -d "$CURRENT/skills/gamma" ]] || fail "gamma is not in the published generation"

# beta (delisted, but install-only removes NOTHING) survives fully.
[[ -d "$CURRENT/skills/beta" ]] ||
  fail "additive install-only dropped delisted beta from the generation (must clone every current entry)"
[[ -L "$AGENTS/skills/beta" && -f "$AGENTS/skills/beta/SKILL.md" ]] ||
  fail "additive install-only broke beta's store link (it must survive untouched)"
[[ -L "$HOME/.claude/skills/beta" && -f "$HOME/.claude/skills/beta/SKILL.md" ]] ||
  fail "additive install-only left beta's Claude fan-out dangling"

# alpha survives.
[[ -d "$CURRENT/skills/alpha" && -L "$AGENTS/skills/alpha" && -f "$AGENTS/skills/alpha/SKILL.md" ]] ||
  fail "alpha did not survive the additive install"

echo "update-skills-additive-clone-all: OK"
