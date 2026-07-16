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

echo "update-skills-fork-drift: OK"
