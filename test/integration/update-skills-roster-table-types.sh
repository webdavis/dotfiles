#!/usr/bin/env bash
# update-skills-roster-table-types.sh (fix-A F2): the roster gate coerced a
# malformed tracked table to empty. `.npxTracked // {}` substitutes on null AND
# false (jq: `false // {}` -> `{}`), so `npxTracked: false` with a valid clawhub
# table passed the zero-count guard, and a full build silently dropped every npx
# skill, pruned its links, emptied the npx lock, and STAMPED success. The fix
# validates each tracked table is PRESENT and an OBJECT (reject false, null,
# string, array) and validates entry schemas (an npx entry has a non-empty
# `repo`; a clawhub entry has non-empty `slug` + `registry`) BEFORE any mutation.
# A malformed table is a required failure, never an empty table.
#
# Each case keeps the tracked UNION non-empty (a valid clawhub skill `gamma`) so
# the F3 empty-union guard cannot mask the table-type bug: the run must refuse
# specifically because a table/entry is malformed.
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
STAMP="$HOME/.local/state/update-skills/last-success"

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
cat >"$stub/clawhub" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
wd=""; dir="skills"; mode=""; prev=""
for a in "$@"; do
  case "$prev" in --workdir) wd="$a" ;; --dir) dir="$a" ;; esac
  case "$a" in install) mode=install ;; update) mode=update ;; esac
  prev="$a"
done
args=("$@"); slug="${args[${#args[@]} - 1]}"
if [[ $mode == install ]]; then
  dest="$wd/$dir/$slug"; mkdir -p "$dest/.clawhub"
  printf -- '---\nname: %s\n---\n' "$(basename "$slug")" >"$dest/SKILL.md"
  printf '{"slug":"%s"}\n' "$(basename "$slug")" >"$dest/.clawhub/origin.json"
fi
EOF
# no-op alerter: the real one blocks for its --timeout waiting for interaction.
printf '#!/usr/bin/env bash\nexit 0\n' >"$stub/alerter"
chmod +x "$stub/npx" "$stub/clawhub" "$stub/alerter"
export PATH="$stub:$PATH"

run_full() { UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1; }
gen_id() { jq -r '.id' "$CURRENT/generation.json" 2>/dev/null || echo NONE; }
live_state() {
  {
    find "$AGENTS/skills" -mindepth 1 2>/dev/null | sort
    find "$CURRENT" 2>/dev/null | sort
    for l in "$AGENTS/skills"/*; do
      [[ -L $l ]] && printf '%s -> %s\n' "$l" "$(readlink "$l")"
    done
    gen_id
  } 2>/dev/null
}

# --- Setup: a healthy publish of npx {alpha} + clawhub {gamma} ----------------
cat >"$LOCK" <<'EOF'
{
  "version": 2,
  "tiers": {"alpha": "core", "gamma": "core"},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": {"alpha": {"repo": "fixture/pack"}},
  "clawhubTracked": {"gamma": {"slug": "@o/gamma", "registry": "https://c.example"}},
  "forks": {}
}
EOF
mkdir -p "$AGENTS/skills/alpha"
printf -- '---\nname: alpha\n---\n# seed\n' >"$AGENTS/skills/alpha/SKILL.md"
mkdir -p "$AGENTS/skills/gamma/.clawhub"
printf -- '---\nname: gamma\n---\n# seed\n' >"$AGENTS/skills/gamma/SKILL.md"
printf '{"slug":"gamma"}\n' >"$AGENTS/skills/gamma/.clawhub/origin.json"
printf '{"skills":{"alpha":{}}}\n' >"$AGENTS/.skill-lock.json"
out0="$(run_full)" || fail "setup full run exited non-zero: $out0"
[[ -L "$AGENTS/skills/alpha" && -L "$AGENTS/skills/gamma" ]] ||
  fail "setup did not produce store links"
[[ -f $STAMP ]] || fail "setup did not stamp success"
id0="$(gen_id)"
baseline="$(live_state)"

assert_fail_closed() { # $1 label, $2 rc, $3 out
  local label="$1" rc="$2" out="$3"
  [[ $rc -ne 0 ]] ||
    fail "$label: run exited 0 on a malformed roster (must fail closed): $out"
  grep -qi 'REQUIRED-FAILURE' <<<"$out" ||
    fail "$label: no required failure recorded: $out"
  [[ "$(gen_id)" == "$id0" ]] ||
    fail "$label: the live generation was exchanged under a malformed roster"
  [[ "$(live_state)" == "$baseline" ]] ||
    fail "$label: live state changed under a malformed roster"
  # alpha (an npx skill) must never be silently dropped.
  [[ -L "$AGENTS/skills/alpha" && -f "$AGENTS/skills/alpha/SKILL.md" ]] ||
    fail "$label: npx skill alpha was dropped by a malformed roster"
}

reset_stamp() { rm -f "$STAMP"; }

# --- Case A: npxTracked is false (coerces to {} via // pre-fix) ----------------
reset_stamp
cat >"$LOCK" <<'EOF'
{
  "version": 2,
  "tiers": {"alpha": "core", "gamma": "core"},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": false,
  "clawhubTracked": {"gamma": {"slug": "@o/gamma", "registry": "https://c.example"}},
  "forks": {}
}
EOF
set +e
outA="$(run_full)"
rcA=$?
set -e
assert_fail_closed "case A (npxTracked false)" "$rcA" "$outA"

# --- Case B: npxTracked is null (explicit) ------------------------------------
reset_stamp
cat >"$LOCK" <<'EOF'
{
  "version": 2,
  "tiers": {"alpha": "core", "gamma": "core"},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": null,
  "clawhubTracked": {"gamma": {"slug": "@o/gamma", "registry": "https://c.example"}},
  "forks": {}
}
EOF
set +e
outB="$(run_full)"
rcB=$?
set -e
assert_fail_closed "case B (npxTracked null)" "$rcB" "$outB"

# --- Case C: an npx entry is missing its repo ---------------------------------
reset_stamp
cat >"$LOCK" <<'EOF'
{
  "version": 2,
  "tiers": {"alpha": "core", "beta": "core", "gamma": "core"},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": {"alpha": {"repo": "fixture/pack"}, "beta": {}},
  "clawhubTracked": {"gamma": {"slug": "@o/gamma", "registry": "https://c.example"}},
  "forks": {}
}
EOF
set +e
outC="$(run_full)"
rcC=$?
set -e
assert_fail_closed "case C (npx entry missing repo)" "$rcC" "$outC"

# --- Case D: a clawhub entry is malformed (slug + registry stripped) ----------
# gamma is already a healthy store clawhub skill, so a full run would otherwise
# clone it forward and validate clean, silently accepting the broken entry.
reset_stamp
cat >"$LOCK" <<'EOF'
{
  "version": 2,
  "tiers": {"alpha": "core", "gamma": "core"},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": {"alpha": {"repo": "fixture/pack"}},
  "clawhubTracked": {"gamma": {}},
  "forks": {}
}
EOF
set +e
outD="$(run_full)"
rcD=$?
set -e
assert_fail_closed "case D (malformed clawhub entry)" "$rcD" "$outD"

echo "update-skills-roster-table-types: OK"
