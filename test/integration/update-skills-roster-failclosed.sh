#!/usr/bin/env bash
# update-skills-roster-failclosed.sh (R2-2): a missing, truncated, or
# schema-broken roster lock must FAIL the run CLOSED, never publish an empty
# generation. Pre-fix, __gen_tracked_names silently returned an empty set on a
# parse error, so the candidate builder dropped every skill, validation passed
# on zero names, the delist pruner removed every store link, and the EMPTY
# publication got a success stamp. Assertions, per broken-roster shape
# (removed lock, truncated JSON, wrong-typed schema):
#   - the run exits non-zero with a loud required failure;
#   - the live store links, the live generation, and the Claude fan-out are
#     byte-for-byte UNCHANGED;
#   - no success stamp is written.
# Plus the empty-tracked-set guard: a VALID lock whose tracked set is empty
# while the live generation is non-empty must refuse to clone-filter/prune
# (required failure), because "delist everything at once" is indistinguishable
# from a corrupted roster; and a mid-run roster edit is caught by the
# snapshot-hash re-check before publish.
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

# npx stub: writes a SKILL.md for each --skill.
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
for s in "${skills[@]}"; do
  mkdir -p "$HOME/.agents/skills/$s"
  printf -- '---\nname: %s\n---\n# lane\n' "$s" >"$HOME/.agents/skills/$s/SKILL.md"
done
EOF
chmod +x "$stub/npx"
export PATH="$stub:$PATH"

run_full() { UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1; }
gen_id() { jq -r '.id' "$CURRENT/generation.json" 2>/dev/null || echo NONE; }
live_state() {
  # A stable fingerprint of everything the run must not touch: store entries
  # (with link targets), the live generation tree, and the Claude fan-out.
  {
    find "$AGENTS/skills" -mindepth 1 2>/dev/null | sort
    find "$CURRENT" 2>/dev/null | sort
    find "$HOME/.claude/skills" -mindepth 1 2>/dev/null | sort
    for l in "$AGENTS/skills"/*; do
      [[ -L $l ]] && printf '%s -> %s\n' "$l" "$(readlink "$l")"
    done
    gen_id
  } 2>/dev/null
}

# --- Setup: healthy publish of {alpha, beta} ----------------------------------
write_lock alpha beta
for n in alpha beta; do
  mkdir -p "$AGENTS/skills/$n"
  printf -- '---\nname: %s\n---\n# seed\n' "$n" >"$AGENTS/skills/$n/SKILL.md"
done
printf '{"skills":{"alpha":{},"beta":{}}}\n' >"$AGENTS/.skill-lock.json"
out0="$(run_full)" || fail "setup full run exited non-zero: $out0"
[[ -f "$CURRENT/generation.json" ]] || fail "setup did not produce a live generation"
[[ -L "$AGENTS/skills/alpha" && -L "$AGENTS/skills/beta" ]] ||
  fail "setup did not produce store links"
id0="$(gen_id)"
baseline="$(live_state)"
good_lock_bytes="$(cat "$LOCK")"

assert_unchanged_and_failed() { # $1 = case label, $2 = rc, $3 = output
  local label="$1" rc="$2" out="$3"
  [[ $rc -ne 0 ]] ||
    fail "$label: the run exited 0 on a broken roster (must fail closed): $out"
  grep -qi 'REQUIRED-FAILURE' <<<"$out" ||
    fail "$label: no required failure was recorded: $out"
  [[ "$(gen_id)" == "$id0" ]] ||
    fail "$label: the live generation was exchanged under a broken roster"
  [[ "$(live_state)" == "$baseline" ]] ||
    fail "$label: live state changed under a broken roster:
--- expected ---
$baseline
--- got ---
$(live_state)"
  [[ ! -f $STAMP ]] ||
    fail "$label: a success stamp was written for a broken-roster run: $(cat "$STAMP")"
}

# --- Case 1: roster lock REMOVED ----------------------------------------------
rm -f "$LOCK"
rm -f "$STAMP"
set +e
out1="$(run_full)"
rc1=$?
set -e
assert_unchanged_and_failed "case 1 (removed lock)" "$rc1" "$out1"

# --- Case 2: roster lock TRUNCATED (invalid JSON) ------------------------------
printf '%s' "${good_lock_bytes:0:37}" >"$LOCK" # torn write: unparseable prefix
set +e
out2="$(run_full)"
rc2=$?
set -e
assert_unchanged_and_failed "case 2 (truncated lock)" "$rc2" "$out2"

# --- Case 3: valid JSON, broken SCHEMA (npxTracked is an array) ----------------
printf '{"version":2,"tiers":{},"npxTracked":["alpha","beta"],"clawhubTracked":{}}\n' >"$LOCK"
set +e
out3="$(run_full)"
rc3=$?
set -e
assert_unchanged_and_failed "case 3 (schema-broken lock)" "$rc3" "$out3"

# --- Case 4: VALID empty tracked set, non-empty live generation ----------------
# "Operator delisted everything at once" is indistinguishable from corruption;
# the clone-filter/prune path must refuse rather than empty the store.
write_lock # zero tracked names, schema otherwise valid
set +e
out4="$(run_full)"
rc4=$?
set -e
assert_unchanged_and_failed "case 4 (empty tracked set, live generation non-empty)" "$rc4" "$out4"

# --- Case 5: --install-only under a broken roster also fails closed ------------
printf 'not json at all' >"$LOCK"
set +e
out5="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --install-only 2>&1)"
rc5=$?
set -e
assert_unchanged_and_failed "case 5 (install-only, broken roster)" "$rc5" "$out5"

# --- Case 6: a mid-run roster edit is caught before publish --------------------
# The lanes stub rewrites the ROSTER lock mid-transaction (simulating a chezmoi
# apply landing between snapshot and publish). The publish-time hash re-check
# must refuse to publish and record a required failure.
write_lock alpha beta
cat >"$stub/npx" <<EOF
#!/usr/bin/env bash
set -euo pipefail
# mutate the REAL roster mid-run (the lane runs in the candidate fake HOME, so
# the real lock path is baked in absolutely)
printf '%s\n' '{"version":2,"tiers":{"alpha":"core"},"hermesProfiles":{},"hermesRegistry":{},"npxTracked":{"alpha":{"repo":"fixture/pack"}},"clawhubTracked":{},"forks":{}}' >"$LOCK"
prev=""
skills=()
for a in "\$@"; do
  [[ \$prev == --skill ]] && skills+=("\$a")
  prev="\$a"
done
for s in "\${skills[@]}"; do
  mkdir -p "\$HOME/.agents/skills/\$s"
  printf -- '---\nname: %s\n---\n# lane\n' "\$s" >"\$HOME/.agents/skills/\$s/SKILL.md"
done
EOF
chmod +x "$stub/npx"
set +e
out6="$(run_full)"
rc6=$?
set -e
[[ "$(gen_id)" == "$id0" ]] ||
  fail "case 6: a mid-run roster edit still published (snapshot hash re-check missing): $out6"
grep -qi 'REQUIRED-FAILURE' <<<"$out6" ||
  fail "case 6: the refused publish did not record a required failure: $out6"
[[ ! -f $STAMP ]] ||
  fail "case 6: a success stamp was written though the roster changed mid-run"
: "$rc6" # rc may be 0 or 1 depending on withhold semantics; state assertions above are the contract

echo "update-skills-roster-failclosed: OK"
