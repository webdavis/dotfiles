#!/usr/bin/env bash
# update-skills-fork-drift.sh, the weekly run must notice when a skip-listed
# fork's UPSTREAM changes, and must only ever say so, never touch the fork.
#
# The real script runs unmodified in a sandbox: a scratch HOME, a local git
# repo standing in for the fork's upstream, and a fake relay.sh planted in the
# scratch HOME that records its arguments instead of sending a push. The lock
# file's forks table lists two entries: "forkskill" pointing at the fixture
# repo with the true current hash recorded (no drift yet), and "ghostfork"
# pointing at a path that does not exist (an unreachable upstream). Four
# assertions:
#   1. No drift -> no alert: while the recorded hash still matches upstream,
#      the run reports nothing for forkskill and never calls relay.
#   2. The unreachable upstream is a logged warning, not a failure: the run
#      still exits 0 (a dead network must never kill the weekly run).
#   3. After the fixture upstream commits a change to the skill folder, the
#      run prints a drift alert naming the fork and its upstream, and the
#      relay notification carries the fork's name.
#   4. The fork's store content is byte-identical before and after both runs,
#      the check observes upstream, it never writes to the store.
#
# Cases 5-10 pin the three ways this watch used to lie, plus the side effect a
# dry preview must not have. Each runs AFTER the four above so those keep their
# meaning as an unpolluted control:
#   5. The clone resolves the recorded URL as recorded, immune to a
#      url.<base>.insteadOf rewrite in the caller's git config. This repo ships
#      exactly such a rewrite (https://github.com/ -> git@github.com:), which
#      silently turned every anonymous public HTTPS fetch into an SSH fetch
#      whose every failure degraded to "upstream unreachable; skipping".
#   6. A REACHABLE upstream that no longer holds the recorded skillPath is
#      reported as a path problem with its own relay state, not as content
#      drift. The old code told the operator to bump lastComparedTreeHash, a
#      remedy that cannot work when the path itself is gone.
#   7. A forks table that is present but not an object watches NO forks, and
#      says so out loud instead of reporting a silent all-clear.
#   8. The same for an ARRAY forks table, which used to abort the run outright
#      (raw jq error, exit 5), taking the whole weekly update with it.
#   9. A forks ENTRY missing its sourceUrl is named as a malformed lock entry,
#      not mislabelled as an unreachable network.
#  10. --dry-run still LOGS its findings and notifies nobody: a relay push
#      reaches the operator's phone, which is not something a preview does.
set -euo pipefail

# When git runs a hook such as pre-commit (this test runs under one via
# `just test`), it exports GIT_DIR/GIT_INDEX_FILE, which point every later git
# command at the OUTER repository. Unset them so nothing here can reach it.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

scratch_dir="$(mktemp -d)"
trap 'rm -rf "$scratch_dir"' EXIT

# Scratch HOME: the script derives every path from $HOME.
HOME="$scratch_dir/home"
export HOME
mkdir -p "$HOME/.agents/skills"

# The fake upstream: a local git repo carrying the skill folder the fork was
# cut from. Its current tree hash is what the lock records as "last compared".
fixture_repo="$scratch_dir/fixture_repo"
mkdir -p "$fixture_repo/skills/forkskill"
printf -- '---\nname: forkskill\ndescription: upstream fixture\n---\n# Upstream\n' >"$fixture_repo/skills/forkskill/SKILL.md"
git -C "$fixture_repo" init -q
git -C "$fixture_repo" -c user.email=test@test -c user.name=test add -A
git -C "$fixture_repo" -c user.email=test@test -c user.name=test commit -qm upstream
compared_tree_hash="$(git -C "$fixture_repo" rev-parse "HEAD:skills/forkskill")"

# The fork in the store: deliberately different content from upstream (that
# is what makes it a fork), plus a marker file a rewrite would destroy.
fork_store_dir="$HOME/.agents/skills/forkskill"
mkdir -p "$fork_store_dir"
printf -- '---\nname: forkskill\ndescription: local fork\n---\n# Local edits\n' >"$fork_store_dir/SKILL.md"
touch "$fork_store_dir/local-edit.marker"

# A fake relay.sh: the script must call it exactly like the real one (which
# arrives in a later slice), this shim just records the arguments it got.
relay_call_log="$scratch_dir/relay-calls.log"
mkdir -p "$HOME/.local/bin"
cat >"$HOME/.local/bin/relay.sh" <<EOF
#!/usr/bin/env bash
printf '%s\n' "\$*" >>"$relay_call_log"
EOF
chmod +x "$HOME/.local/bin/relay.sh"

# The lock's forks table: forkskill has the true current hash (no drift);
# ghostfork's upstream path does not exist (unreachable network stand-in).
cat >"$HOME/.agents/custom-skill-lock.json" <<EOF
{
  "version": 1,
  "skills": {},
  "forks": {
    "forkskill": {
      "source": "fixture/forkskill",
      "sourceUrl": "$fixture_repo",
      "skillPath": "skills/forkskill",
      "lastComparedTreeHash": "$compared_tree_hash"
    },
    "ghostfork": {
      "source": "fixture/ghostfork",
      "sourceUrl": "$scratch_dir/no-such-repo",
      "skillPath": ".",
      "lastComparedTreeHash": "0000000000000000000000000000000000000000"
    }
  }
}
EOF

# Byte-level snapshot of the fork before any run (assertion 4 compares later).
fork_snapshot_before="$(cd "$fork_store_dir" && find . -type f -print0 | sort -z | xargs -0 shasum -a 256)"

# Run 1: upstream unchanged. FORCE bypasses the idle-gate (a harness is by
# definition running this test).
output="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --check-forks-only 2>&1)" ||
  fail "--check-forks-only exited non-zero with an unreachable upstream in the lock: $output"

# 1) No drift -> no alert, no relay call.
printf '%s\n' "$output" | grep -qi 'drift.*forkskill\|forkskill.*drift' &&
  fail "run alerted drift for forkskill although upstream is unchanged"
[[ ! -s $relay_call_log ]] || fail "relay was called although no fork drifted"

# 2) The unreachable upstream is reported as a warning, by name.
printf '%s\n' "$output" | grep -q 'ghostfork' ||
  fail "unreachable upstream produced no logged warning naming ghostfork: $output"

# The upstream moves: a commit changes the skill folder.
printf -- '\n## New upstream feature\n' >>"$fixture_repo/skills/forkskill/SKILL.md"
git -C "$fixture_repo" -c user.email=test@test -c user.name=test add -A
git -C "$fixture_repo" -c user.email=test@test -c user.name=test commit -qm 'upstream feature'

# Run 2: drift must be alerted, run must still exit 0.
output="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --check-forks-only 2>&1)" ||
  fail "--check-forks-only exited non-zero on drift: $output"

# 3) The alert names the fork and its upstream, and relay got the fork's name.
printf '%s\n' "$output" | grep -q 'FORK DRIFT.*forkskill' ||
  fail "no drift alert naming forkskill: $output"
printf '%s\n' "$output" | grep -qF "$fixture_repo" ||
  fail "drift alert does not name the upstream: $output"
grep -q 'forkskill' "$relay_call_log" 2>/dev/null ||
  fail "relay notification does not carry the fork's name"

# 4) The fork's store content is byte-identical to the pre-run snapshot.
fork_snapshot_after="$(cd "$fork_store_dir" && find . -type f -print0 | sort -z | xargs -0 shasum -a 256)"
[[ $fork_snapshot_before == "$fork_snapshot_after" ]] ||
  fail "the drift check modified the fork's store content"
[[ -f "$fork_store_dir/local-edit.marker" ]] || fail "the fork's marker file is gone"

# ---------------------------------------------------------------------------
# Cases 5-10. Shared helpers, so each case states only what it is pinning.
# ---------------------------------------------------------------------------

# refute_match <haystack> <extended-regex> <message> -- an explicit refute, so
# the negative assertion is a real branch. A bare `! grep` under `set -e`
# decides the test only in final position, which makes whether it guards
# anything a position lottery.
refute_match() {
  local haystack="$1" pattern="$2" message="$3"
  # `--` so a pattern that starts with a dash (a relay flag) is a pattern.
  if printf '%s\n' "$haystack" | grep -qE -- "$pattern"; then
    fail "$message"
  fi
}

# write_forks_lock <forks-json> -- replace the lock, keeping every other table
# valid so a case can only fail because of its forks table.
write_forks_lock() {
  local forks_json="$1"
  jq -n --argjson forks "$forks_json" \
    '{version: 1, skills: {}, forks: $forks}' >"$HOME/.agents/custom-skill-lock.json"
}

# run_fork_check -- run the standalone drift-watch with a fresh relay log,
# leaving its output in $fork_check_output and its status in $fork_check_rc.
# Deliberately NOT called through command substitution: that subshells the
# assignment, and the exit status would silently stay at its initial value, so
# every rc assertion would pass no matter what the script did.
fork_check_output=""
fork_check_rc=0
run_fork_check() {
  : >"$relay_call_log"
  set +e
  fork_check_output="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --check-forks-only 2>&1)"
  fork_check_rc=$?
  set -e
}

# --- Case 5: the clone resolves the recorded URL as recorded ------------------
# A url.<base>.insteadOf rewrite in the caller's git config must not reroute
# the drift clone. insteadOf matches a plain filesystem path exactly as it
# matches an https:// prefix, so the fixture repo's own path is a faithful,
# network-free stand-in for the https://github.com/ -> git@github.com: rewrite
# this repo ships. The upstream already drifted in case 3, so a clone that
# resolves correctly must still report that drift.
#
# XDG_CONFIG_HOME is pinned into the sandbox as well: git reads a global config
# from $XDG_CONFIG_HOME/git/config too, and an inherited one would let the
# host's real config leak into this case either way it is broken.
export XDG_CONFIG_HOME="$HOME/.config"
mkdir -p "$XDG_CONFIG_HOME"
cat >"$HOME/.gitconfig" <<EOF
[url "https://bogus.invalid/nope/"]
	insteadOf = $fixture_repo
EOF

run_fork_check
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 5: --check-forks-only exited $fork_check_rc under a url-rewriting git config: $fork_check_output"
printf '%s\n' "$fork_check_output" | grep -q 'FORK DRIFT.*forkskill' ||
  fail "case 5: a url.<base>.insteadOf rewrite in the caller's git config rerouted the drift clone, so the real drift went unreported: $fork_check_output"
refute_match "$fork_check_output" 'unreachable.*forkskill|forkskill.*unreachable' \
  "case 5: the rewritten clone failed and degraded to a silent 'unreachable' skip: $fork_check_output"
grep -q 'forkskill' "$relay_call_log" 2>/dev/null ||
  fail "case 5: no relay notification for the drift the rewrite had been hiding"

# --- Case 6: a reachable upstream missing the recorded skillPath --------------
# Distinct from drift: the upstream is fine, the lock's path is stale. The old
# code compared `git rev-parse`'s output against a "missing-path" sentinel,
# which never matched (rev-parse ECHOES the unresolvable argument to stdout
# before failing), so this reported content drift and told the operator to bump
# a hash under a path that no longer exists.
missing_skill_path="skills/moved-away"
write_forks_lock "$(jq -n --arg url "$fixture_repo" --arg path "$missing_skill_path" \
  '{pathfork: {source: "fixture/pathfork", sourceUrl: $url, skillPath: $path,
    lastComparedTreeHash: "0000000000000000000000000000000000000000"}}')"

run_fork_check
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 6: --check-forks-only exited $fork_check_rc on a stale skillPath: $fork_check_output"
printf '%s\n' "$fork_check_output" | grep -qF "$missing_skill_path" ||
  fail "case 6: the report never names the skillPath that no longer exists upstream: $fork_check_output"
refute_match "$fork_check_output" 'FORK DRIFT' \
  "case 6: a stale skillPath is still reported as content drift, whose remedy (bump lastComparedTreeHash) cannot be executed: $fork_check_output"
grep -qF "$missing_skill_path" "$relay_call_log" 2>/dev/null ||
  fail "case 6: the relay notification does not name the missing skillPath: $(cat "$relay_call_log")"
refute_match "$(cat "$relay_call_log")" '--state fork-drift( |$)' \
  "case 6: a stale skillPath relays the fork-drift state, so it is indistinguishable from real drift downstream: $(cat "$relay_call_log")"

# --- Cases 7 and 8: a forks table that is present but not an object -----------
# Absent is legal (nothing to watch). Present-but-not-an-object is corruption,
# and both of its old outcomes were wrong: false/null/string/[] walked zero
# entries and reported a silent all-clear, and an ARRAY made the per-entry jq
# index error out, aborting the run (exit 5, raw jq error) and, in the weekly
# flow, taking the success stamp with it.
for malformed_forks in 'false' '[]' '["forkskill"]' '"forkskill"'; do
  printf '{"version":1,"skills":{},"forks":%s}\n' "$malformed_forks" \
    >"$HOME/.agents/custom-skill-lock.json"
  run_fork_check
  [[ $fork_check_rc -eq 0 ]] ||
    fail "case 7/8 (forks=$malformed_forks): the drift-watch exited $fork_check_rc; a corrupt advisory table must never kill the run: $fork_check_output"
  printf '%s\n' "$fork_check_output" | grep -q 'forks table' ||
    fail "case 7/8 (forks=$malformed_forks): a malformed forks table watched nothing and said nothing: $fork_check_output"
  refute_match "$fork_check_output" 'jq: error' \
    "case 7/8 (forks=$malformed_forks): a raw jq error reached the operator instead of a named warning: $fork_check_output"
done

# --- Case 9: a forks entry missing its sourceUrl -----------------------------
# "upstream unreachable (null)" reads as a network problem and sends the
# operator to check their connection. The lock is what is broken.
write_forks_lock '{"brokenfork": {"skillPath": "skills/brokenfork", "lastComparedTreeHash": "0000000000000000000000000000000000000000"}}'
run_fork_check
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 9: the drift-watch exited $fork_check_rc on a malformed forks entry: $fork_check_output"
printf '%s\n' "$fork_check_output" | grep -q 'brokenfork' ||
  fail "case 9: a forks entry with no sourceUrl produced no warning naming it: $fork_check_output"
refute_match "$fork_check_output" 'unreachable' \
  "case 9: a lock entry with no sourceUrl is mislabelled as an unreachable upstream: $fork_check_output"

# --- Case 10: --dry-run reports findings but notifies nobody ------------------
# A relay push reaches the operator's phone. The dry preview is documented as
# making no writes and is the mode you run to see what WOULD happen, so it must
# not page anyone on the way. Driven through the malformed-table finding, the
# one this phase can reach without cloning anything.
printf '{"version":1,"skills":{},"forks":false}\n' >"$HOME/.agents/custom-skill-lock.json"
: >"$relay_call_log"
set +e
dryrun_output="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --check-forks-only --dry-run 2>&1)"
dryrun_rc=$?
set -e
[[ $dryrun_rc -eq 0 ]] ||
  fail "case 10: the dry-run drift-watch exited $dryrun_rc: $dryrun_output"
printf '%s\n' "$dryrun_output" | grep -q 'forks table' ||
  fail "case 10: the dry run hid a finding the real run reports: $dryrun_output"
[[ ! -s $relay_call_log ]] ||
  fail "case 10: the dry run sent a relay notification: $(cat "$relay_call_log")"

echo "update-skills-fork-drift: OK (4 baseline assertions + rewrite immunity, stale skillPath, 4 malformed tables, malformed entry, dry-run notifies nobody)"
