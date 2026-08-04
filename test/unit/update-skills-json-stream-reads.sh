#!/usr/bin/env bash
# update-skills-json-stream-reads.sh: jq reads a file as a STREAM of values, so
# `jq -e . file` says YES to `{"a":1}{"b":2}` and every later filter answers from
# whichever document it happens to land on. The roster gate's version of this bug
# is pinned end to end elsewhere (test/integration/update-skills-roster-failclosed.sh
# and update-skills-converge.sh); this file pins the three sibling reads found in
# the same audit, at the function level:
#
#   1. __gen_validate_candidate     the candidate npx lock must be ONE value, or
#                                   the readers that later ask `.skills | has($n)`
#                                   answer from the last document alone.
#   2. __gen_reconcile_candidate_npx_lock
#                                   its two inputs reach jq --argjson, which
#                                   refuses a stream outright, so a stream has to
#                                   reach the '{}' fallback instead of failing the
#                                   whole reconcile (and discarding the candidate).
#   3. __gen_update_failure_streaks the state file must be ONE object, or every
#                                   per-name read yields a line per document, no
#                                   week ever compares equal, and the file grows a
#                                   document per slot.
#
# Sourced with UPDATE_SKILLS_LIB_ONLY=1, so the main flow never runs; everything
# happens inside a sandbox HOME.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"
fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.agents/skills"
printf '{"npxTracked":{"alpha":{"repo":"x/a"}},"clawhubTracked":{},"tiers":{}}\n' \
  >"$HOME/.agents/custom-skill-lock.json"

export UPDATE_SKILLS_LIB_ONLY=1
# shellcheck disable=SC1090
source "$SCRIPT"

# ── 1. A candidate whose npx lock is two documents is INVALID ────────────────
# The candidate is ADDITIVE on purpose: a full candidate's key-set comparison
# already trips over a stream (two lines where one is expected), so only the
# additive path (what --install-only publishes) exposes the read itself.
candidate="$tmp/candidate/.agents"
mkdir -p "$candidate/skills/alpha"
printf -- '---\nname: alpha\n---\n' >"$candidate/skills/alpha/SKILL.md"
printf '{"skills":{"alpha":{}}}\n{"skills":{"alpha":{}}}\n' >"$candidate/.skill-lock.json"
__gen_write_meta "$candidate" "cand-1" additive
if __gen_validate_candidate "$candidate" >/dev/null 2>&1; then
  fail "a candidate whose .skill-lock.json holds two JSON documents validated"
fi
# ...and the same candidate with ONE document still validates, so the check
# rejects the stream rather than the lock.
printf '{"skills":{"alpha":{}}}\n' >"$candidate/.skill-lock.json"
__gen_validate_candidate "$candidate" >/dev/null 2>&1 ||
  fail "a candidate with a single-document npx lock no longer validates"

# ── 2. The npx lock reconcile treats a stream as unreadable, not as fatal ────
export XDG_STATE_HOME="$tmp/xdg-state"
mkdir -p "$XDG_STATE_HOME/skills"
printf '{"version":3,"skills":{"alpha":{"source":"github:x/a"}}}\n' \
  >"$XDG_STATE_HOME/skills/.skill-lock.json"
printf '{"skills":{"alpha":{}}}\n{"skills":{"beta":{}}}\n' >"$AGENTS/.skill-lock.json"
__gen_reconcile_candidate_npx_lock additive ||
  fail "the npx lock reconcile failed the candidate over a two-document seed instead of falling back to {}"
[[ "$(jq -s 'length' "$AGENTS/.skill-lock.json")" == "1" ]] ||
  fail "the reconciled npx lock is not a single JSON document: $(cat "$AGENTS/.skill-lock.json")"
jq -e '.skills | has("alpha")' "$AGENTS/.skill-lock.json" >/dev/null ||
  fail "the reconciled lock dropped the CLI-recorded entry: $(cat "$AGENTS/.skill-lock.json")"

# ── 3. A two-document streak file is replaced, not appended to ───────────────
mkdir -p "$STATE_DIR"
printf '{"alpha":{"last_failed_week":"1970-01","consecutive_failed_weeks":1}}\n{}\n' \
  >"$STREAK_FILE"
# shellcheck disable=SC2034 # read by __gen_update_failure_streaks in the sourced script
GEN_FAILED_SKILLS=(alpha)
__gen_update_failure_streaks
[[ "$(jq -s 'length' "$STREAK_FILE")" == "1" ]] ||
  fail "the failure-streak file is not a single JSON document after a run: $(cat "$STREAK_FILE")"
streak_count="$(jq -r '.alpha.consecutive_failed_weeks' "$STREAK_FILE")"
[[ $streak_count == "1" ]] ||
  fail "a fresh streak from an unreadable state file counted $streak_count instead of 1"

echo "update-skills-json-stream-reads: OK"
