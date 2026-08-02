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
# Cases A-D pin that refusal. Cases E-G pin its BOUNDARY: `forks` is advisory
# data no mutating step reads, so a typo there must be reported, never escalated
# into a refusal of the whole weekly update.
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

# --- Cases E and F: a malformed forks table must NOT refuse the weekly run ----
# The opposite of every case above, and deliberately so. `forks` is the fork
# drift-watch's input and NOTHING in the mutating path reads it, so a typo
# there can only degrade an ADVISORY report. Holding it to the fail-closed rule
# meant a hand-edit typo in the one field an operator edits every time they
# clear a drift (an unquoted lastComparedTreeHash) refused the entire weekly
# update: no build, no publish, no prune, no stamp, on every remaining Monday
# slot, under an alert naming the DEPLOYED lock when the committed source is
# what is wrong.
#
# So the wanted behaviour is: the run completes, publishes and stamps, and the
# drift-watch reports the corruption by name. Asserting only "rc 0" would pass
# against a gate that silently ignored the table, so each case also demands the
# named advisory, and case F demands the FIELD be named.
assert_advisory_not_refusal() { # $1 label, $2 rc, $3 out, $4 expected substring
  local label="$1" rc="$2" out="$3" expected="$4"
  [[ $rc -eq 0 ]] ||
    fail "$label: a malformed ADVISORY table refused the whole run (rc=$rc): $out"
  if grep -qi 'REQUIRED-FAILURE' <<<"$out"; then
    fail "$label: a malformed ADVISORY table recorded a required failure: $out"
  fi
  [[ -f $STAMP ]] ||
    fail "$label: the run withheld the weekly success stamp over advisory data: $out"
  grep -qF "$expected" <<<"$out" ||
    fail "$label: the corruption was tolerated silently instead of reported ('$expected' absent): $out"
  # alpha (an npx skill) must still be live: tolerating the forks table must not
  # mean tolerating a degraded generation.
  [[ -L "$AGENTS/skills/alpha" && -f "$AGENTS/skills/alpha/SKILL.md" ]] ||
    fail "$label: npx skill alpha was dropped"
}

reset_stamp
cat >"$LOCK" <<'EOF'
{
  "version": 2,
  "tiers": {"alpha": "core", "gamma": "core"},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": {"alpha": {"repo": "fixture/pack"}},
  "clawhubTracked": {"gamma": {"slug": "@o/gamma", "registry": "https://c.example"}},
  "forks": false
}
EOF
set +e
outE="$(run_full)"
rcE=$?
set -e
assert_advisory_not_refusal "case E (forks false)" "$rcE" "$outE" \
  'the forks table in'

# --- Case F: a forks ENTRY the walk cannot use --------------------------------
# The hand-edit typo itself: lastComparedTreeHash written without quotes. Under
# the old gate this exact lock refused the run; now the entry is reported with
# the offending FIELD named, and everything else still updates.
reset_stamp
cat >"$LOCK" <<'EOF'
{
  "version": 2,
  "tiers": {"alpha": "core", "gamma": "core"},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": {"alpha": {"repo": "fixture/pack"}},
  "clawhubTracked": {"gamma": {"slug": "@o/gamma", "registry": "https://c.example"}},
  "forks": {"delta": {"sourceUrl": "https://example.invalid/d.git", "skillPath": "skills/d", "lastComparedTreeHash": 12345}}
}
EOF
set +e
outF="$(run_full)"
rcF=$?
set -e
assert_advisory_not_refusal "case F (mis-typed forks field)" "$rcF" "$outF" \
  'lastComparedTreeHash'
grep -qF 'delta' <<<"$outF" ||
  fail "case F: the advisory does not name the broken fork: $outF"

# --- Case G: a VALID non-empty forks table sails through and is WALKED --------
# Every happy-path fixture in this suite carries `"forks": {}`, so without this
# nothing proved a real forks table reaches the drift-watch at all: cases E and
# F would pass just as well against a run that ignored the table outright. The
# entry's upstream deliberately does not exist: an unreachable upstream is
# advisory, so the run must sail past it, and it names the fork in the log,
# which proves the table was WALKED rather than merely tolerated.
reset_stamp
cat >"$LOCK" <<EOF
{
  "version": 2,
  "tiers": {"alpha": "core", "gamma": "core"},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": {"alpha": {"repo": "fixture/pack"}},
  "clawhubTracked": {"gamma": {"slug": "@o/gamma", "registry": "https://c.example"}},
  "forks": {
    "omega": {
      "source": "fixture/omega",
      "sourceUrl": "$tmp/no-such-upstream",
      "skillPath": "skills/omega",
      "lastComparedTreeHash": "0000000000000000000000000000000000000000"
    }
  }
}
EOF
set +e
outG="$(run_full)"
rcG=$?
set -e
[[ $rcG -eq 0 ]] ||
  fail "case G (valid forks table): the run refused a well-formed roster (rc=$rcG): $outG"
# An explicit refute branch: `grep ... && fail` is a positional lottery under
# `set -e`, so the negative assertion is written as a real if.
if grep -qi 'REQUIRED-FAILURE' <<<"$outG"; then
  fail "case G (valid forks table): a well-formed roster recorded a required failure: $outG"
fi
[[ -f $STAMP ]] ||
  fail "case G (valid forks table): a well-formed roster did not stamp success: $outG"
grep -q 'omega' <<<"$outG" ||
  fail "case G (valid forks table): the forks table was accepted but never walked, so the drift-watch reported nothing about omega: $outG"

echo "update-skills-roster-table-types: OK"
