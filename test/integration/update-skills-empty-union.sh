#!/usr/bin/env bash
# update-skills-empty-union.sh (fix-A F3): a zero tracked UNION must never
# publish, in any mutation mode, regardless of live-generation state. The old
# empty-refusal fired only when the tracked set was zero AND the live generation
# already held skills, so on a FRESH machine (no live generation) or with a
# DAMAGED current generation (present but zero skill dirs), a zero/empty roster
# migrated over zero names, published an EMPTY generation, and stamped success.
# There is no legitimate empty roster (the committed roster always has entries),
# so a zero tracked union is refused unconditionally.
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

# A VALID roster whose tracked tables are present but empty (schema passes, the
# tracked union is zero).
empty_roster() {
  cat >"$1" <<'EOF'
{
  "version": 2,
  "tiers": {},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": {},
  "clawhubTracked": {},
  "forks": {}
}
EOF
}

stub="$tmp/stub"
mkdir -p "$stub"
printf '#!/usr/bin/env bash\nexit 0\n' >"$stub/npx"
printf '#!/usr/bin/env bash\nexit 0\n' >"$stub/clawhub"
printf '#!/usr/bin/env bash\nexit 0\n' >"$stub/alerter"
chmod +x "$stub/npx" "$stub/clawhub" "$stub/alerter"
export PATH="$stub:$PATH"
export UPDATE_SKILLS_GMV="$GMV_BIN"

assert_refused() { # $1 label, $2 rc, $3 out, $4 stamp, $5 current
  local label="$1" rc="$2" out="$3" stamp="$4" current="$5"
  [[ $rc -ne 0 ]] ||
    fail "$label: run exited 0 on a zero-union roster (must refuse): $out"
  grep -qi 'REQUIRED-FAILURE' <<<"$out" ||
    fail "$label: no required failure recorded: $out"
  [[ ! -f $stamp ]] ||
    fail "$label: a success stamp was written for a zero-union run: $(cat "$stamp")"
  [[ ! -f "$current/generation.json" ]] ||
    fail "$label: an EMPTY generation was published (generation.json exists)"
}

# ── Case 1: fresh home (no .skills-current), nothing to migrate over ─────────
HOME="$tmp/fresh"
export HOME
mkdir -p "$HOME/.agents/skills"
empty_roster "$HOME/.agents/custom-skill-lock.json"
set +e
out1="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1)"
rc1=$?
set -e
assert_refused "case 1 (fresh home, weekly)" "$rc1" "$out1" \
  "$HOME/.local/state/update-skills/last-success" "$HOME/.agents/.skills-current"

# ── Case 1b: fresh home under --install-only also refuses ─────────────────────
set +e
out1b="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --install-only 2>&1)"
rc1b=$?
set -e
assert_refused "case 1b (fresh home, install-only)" "$rc1b" "$out1b" \
  "$HOME/.local/state/update-skills/last-success" "$HOME/.agents/.skills-current"

# ── Case 2: current exists but is incomplete (zero skill dirs) ────────────────
HOME="$tmp/incomplete"
export HOME
AGENTS="$HOME/.agents"
CURRENT="$AGENTS/.skills-current"
mkdir -p "$AGENTS/skills" "$CURRENT/skills"
# A "complete" ready marker but an EMPTY skills tree: a damaged current with
# zero skill dirs. The zero-union roster must not migrate/clone-filter over it.
printf '{"id":"cur-1-1","buildMode":"full"}\n' >"$CURRENT/generation.json"
printf '{"skills":{}}\n' >"$CURRENT/.skill-lock.json"
empty_roster "$AGENTS/custom-skill-lock.json"
current_before="$(find "$CURRENT" | sort)"
set +e
out2="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1)"
rc2=$?
set -e
[[ $rc2 -ne 0 ]] ||
  fail "case 2 (incomplete current): run exited 0 on a zero-union roster: $out2"
grep -qi 'REQUIRED-FAILURE' <<<"$out2" ||
  fail "case 2: no required failure recorded: $out2"
[[ ! -f "$HOME/.local/state/update-skills/last-success" ]] ||
  fail "case 2: a success stamp was written for a zero-union run"
[[ "$(find "$CURRENT" | sort)" == "$current_before" ]] ||
  fail "case 2: the incomplete current generation was mutated"

echo "update-skills-empty-union: OK"
