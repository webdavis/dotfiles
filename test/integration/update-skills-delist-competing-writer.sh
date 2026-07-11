#!/usr/bin/env bash
# update-skills-delist-competing-writer.sh (fix-A F4): a DELISTED skill whose
# store link was replaced out-of-band by a REAL DIR must not survive the run.
# The delist pruner only removed SYMLINKS (real dirs are skipped as
# foreign/vendored), recovery walked only the NEW tracked set (so it never saw
# the delisted name), the candidate clone-filter dropped it, but the surviving
# real dir was then treated by Claude convergence as a desired store entry: the
# revoked skill stayed installed and fanned out. The fix carries previous-
# generation ownership provenance: a real dir at a generation-owned name that is
# no longer tracked was updater-owned and is quarantined, while a genuinely
# FOREIGN real dir (never a generation skill) is preserved.
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

run_full() { UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1; }

# --- Phase 1: track {alpha, beta}, publish ------------------------------------
write_lock alpha beta
for n in alpha beta; do
  mkdir -p "$AGENTS/skills/$n"
  printf -- '---\nname: %s\n---\n# seed\n' "$n" >"$AGENTS/skills/$n/SKILL.md"
done
printf '{"skills":{"alpha":{},"beta":{}}}\n' >"$AGENTS/.skill-lock.json"
out1="$(run_full)" || fail "phase 1 run exited non-zero: $out1"
for n in alpha beta; do
  [[ -L "$AGENTS/skills/$n" && -d "$CURRENT/skills/$n" ]] ||
    fail "phase 1: store/$n did not become a generation symlink"
  [[ -L "$HOME/.claude/skills/$n" ]] || fail "phase 1: no Claude link for $n"
done

# --- Phase 2: an out-of-band writer replaces beta's link with a REAL DIR;
#             beta is delisted; a genuinely foreign real dir gamma appears -----
rm "$AGENTS/skills/beta" # remove the symlink
mkdir -p "$AGENTS/skills/beta"
printf 'OUT-OF-BAND-BETA\n' >"$AGENTS/skills/beta/SKILL.md"
mkdir -p "$AGENTS/skills/gamma"
printf 'FOREIGN-GAMMA\n' >"$AGENTS/skills/gamma/keep.txt"
[[ -d "$AGENTS/skills/beta" && ! -L "$AGENTS/skills/beta" ]] ||
  fail "setup: beta is not a real dir"

write_lock alpha # delist beta
out2="$(run_full)" || fail "phase 2 run exited non-zero: $out2"

# beta is gone from the store (the updater-owned real dir was quarantined).
[[ ! -e "$AGENTS/skills/beta" && ! -L "$AGENTS/skills/beta" ]] ||
  fail "the delisted competing-writer real dir beta survived in the store: $(ls -ld "$AGENTS/skills/beta" 2>&1)"
# beta is gone from the live generation.
[[ ! -e "$CURRENT/skills/beta" ]] ||
  fail "delisted beta persists in the live generation"
# beta is gone from the Claude fan-out.
[[ ! -e "$HOME/.claude/skills/beta" && ! -L "$HOME/.claude/skills/beta" ]] ||
  fail "delisted beta persists in the Claude fan-out"

# alpha survives and resolves.
[[ -L "$AGENTS/skills/alpha" && -f "$AGENTS/skills/alpha/SKILL.md" ]] ||
  fail "still-tracked alpha did not survive"
[[ -L "$HOME/.claude/skills/alpha" ]] || fail "alpha lost its Claude link"

# the genuinely foreign real dir gamma survives untouched.
[[ -d "$AGENTS/skills/gamma" && ! -L "$AGENTS/skills/gamma" ]] ||
  fail "the foreign real dir gamma was destroyed"
[[ "$(cat "$AGENTS/skills/gamma/keep.txt")" == "FOREIGN-GAMMA" ]] ||
  fail "gamma content changed"

echo "update-skills-delist-competing-writer: OK"
