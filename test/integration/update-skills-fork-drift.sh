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
# Cases 5-13 pin the ways this watch used to lie, plus the side effect a dry
# preview must not have. Each runs AFTER the four above so those keep their
# meaning as an unpolluted control:
#   5. The clone resolves the recorded URL as recorded, immune to a
#      url.<base>.insteadOf rewrite in the caller's git config. This repo ships
#      exactly such a rewrite (https://github.com/ -> git@github.com:), which
#      silently turned every anonymous public HTTPS fetch into an SSH fetch
#      whose every failure degraded to "upstream unreachable; skipping".
#  5b. The same immunity for git's SYSTEM config channel, which the global one
#      does not cover.
#   6. A REACHABLE upstream that no longer holds the recorded skillPath is
#      reported as a path problem with its own relay state, not as content
#      drift. The old code told the operator to bump lastComparedTreeHash, a
#      remedy that cannot work when the path itself is gone.
#   7. A forks table that is present but not an object watches NO forks, and
#      says so out loud instead of reporting a silent all-clear.
#   8. The same for an ARRAY forks table, which used to abort the run outright
#      (raw jq error, exit 5), taking the whole weekly update with it.
#   9. A forks ENTRY missing its sourceUrl is named as a malformed lock entry,
#      not mislabelled as an unreachable network, and the forks after it are
#      still checked.
#  9b. A forks entry that is not an object at all gets the same treatment. It
#      used to abort the run exactly like the array table, one level down.
#  10. --dry-run still LOGS its findings and notifies nobody: a relay push
#      reaches the operator's phone, which is not something a preview does.
#  11. skillPath "." means the whole repository: HEAD's root tree is what is
#      compared, so the one committed entry that uses it (elevenlabs) neither
#      cries drift nor cries missing-path every week.
#  12. A relay push that FAILS is advisory too: it must not decide the run's
#      exit status, or the weekly run dies after publishing a generation and
#      before stamping success.
#  13. A lock that does not parse at all is named as such, not mis-reported as
#      a malformed forks table in an otherwise healthy lock.
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

# A fake relay.sh: the script must call it exactly like the real one, this shim
# just records the arguments it got and exits with the code it was installed
# with (case 12 installs a failing one).
relay_call_log="$scratch_dir/relay-calls.log"
mkdir -p "$HOME/.local/bin"
install_relay_shim() {
  local exit_code="$1"
  cat >"$HOME/.local/bin/relay.sh" <<EOF
#!/usr/bin/env bash
printf '%s\n' "\$*" >>"$relay_call_log"
exit $exit_code
EOF
  chmod +x "$HOME/.local/bin/relay.sh"
}
install_relay_shim 0

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

# run_fork_check [NAME=VALUE ...] -- run the standalone drift-watch with a fresh
# relay log, leaving its output in $fork_check_output and its status in
# $fork_check_rc. Any arguments are env assignments applied to that run alone
# (through `env`, since bash only honours an assignment PREFIX written
# literally, never one arriving from an expansion).
# Deliberately NOT called through command substitution: that subshells the
# assignment, and the exit status would silently stay at its initial value, so
# every rc assertion would pass no matter what the script did.
fork_check_output=""
fork_check_rc=0
run_fork_check() {
  : >"$relay_call_log"
  set +e
  fork_check_output="$(UPDATE_SKILLS_FORCE=1 env "$@" bash "$SCRIPT" --check-forks-only 2>&1)"
  fork_check_rc=$?
  set -e
}

# assert_relay_state <state> <message> -- the relay notification carries this
# exact --state. The states are the operator-facing vocabulary CLAUDE.md
# documents, and each maps to a different remedy, so an arbitrary third string
# is a regression even when the log text still reads correctly.
assert_relay_state() {
  local state="$1" message="$2"
  grep -qF -- "--state $state " "$relay_call_log" 2>/dev/null ||
    fail "$message (relay log: $(cat "$relay_call_log"))"
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
hostile_rewrite_config() { # $1 target file: rewrite the fixture path to nowhere
  cat >"$1" <<EOF
[url "https://bogus.invalid/nope/"]
	insteadOf = $fixture_repo
EOF
}
hostile_rewrite_config "$HOME/.gitconfig"

# Control FIRST: the planted rewrite must actually reroute a plain clone, or
# this case discriminates nothing and would pass against the very bug it exists
# to catch. (insteadOf applying to a bare filesystem path is what makes this
# network-free; a git that stopped doing that would silently gut the case.)
set +e
git clone --quiet --depth 1 "$fixture_repo" "$scratch_dir/case5-control-clone" >/dev/null 2>&1
case5_control_rc=$?
set -e
[[ $case5_control_rc -ne 0 ]] ||
  fail "case 5 control: the planted url.<base>.insteadOf rewrite did not reroute a plain clone, so this case cannot tell a neutralized clone from a broken one"

run_fork_check
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 5: --check-forks-only exited $fork_check_rc under a url-rewriting git config: $fork_check_output"
printf '%s\n' "$fork_check_output" | grep -q 'FORK DRIFT.*forkskill' ||
  fail "case 5: a url.<base>.insteadOf rewrite in the caller's git config rerouted the drift clone, so the real drift went unreported: $fork_check_output"
refute_match "$fork_check_output" 'unreachable.*forkskill|forkskill.*unreachable' \
  "case 5: the rewritten clone failed and degraded to a silent 'unreachable' skip: $fork_check_output"
grep -q 'forkskill' "$relay_call_log" 2>/dev/null ||
  fail "case 5: no relay notification for the drift the rewrite had been hiding"

# --- Case 5b: the same immunity for git's SYSTEM config channel ---------------
# GIT_CONFIG_GLOBAL=/dev/null covers ~/.gitconfig AND $XDG_CONFIG_HOME/git/config
# but NOT the system config, which needs its own GIT_CONFIG_SYSTEM=/dev/null, so
# case 5 alone cannot tell a clone that neutralizes both from one that
# neutralizes only the global channel. A test cannot write /etc/gitconfig, so
# the system channel is exercised the way git itself offers: an INHERITED
# GIT_CONFIG_SYSTEM, which the clone's own assignment has to override.
hostile_system_config="$scratch_dir/hostile-system.gitconfig"
hostile_rewrite_config "$hostile_system_config"

# Control FIRST, with the global channel neutralized so only the system channel
# can be doing the rerouting.
set +e
GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM="$hostile_system_config" \
  git clone --quiet --depth 1 "$fixture_repo" "$scratch_dir/case5b-control-clone" >/dev/null 2>&1
case5b_control_rc=$?
set -e
[[ $case5b_control_rc -ne 0 ]] ||
  fail "case 5b control: a rewrite in the SYSTEM config channel did not reroute a plain clone, so this case cannot discriminate"

run_fork_check GIT_CONFIG_SYSTEM="$hostile_system_config"
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 5b: --check-forks-only exited $fork_check_rc under a url-rewriting system git config: $fork_check_output"
printf '%s\n' "$fork_check_output" | grep -q 'FORK DRIFT.*forkskill' ||
  fail "case 5b: a url.<base>.insteadOf rewrite in the SYSTEM git config rerouted the drift clone, so the real drift went unreported: $fork_check_output"
refute_match "$fork_check_output" 'unreachable.*forkskill|forkskill.*unreachable' \
  "case 5b: the rewritten clone failed and degraded to a silent 'unreachable' skip: $fork_check_output"

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
printf '%s\n' "$fork_check_output" | grep -q 'FORK PATH MISSING' ||
  fail "case 6: the report is not filed under the FORK PATH MISSING heading CLAUDE.md documents, so a log reader cannot tell it from any other warning: $fork_check_output"
refute_match "$fork_check_output" 'FORK DRIFT' \
  "case 6: a stale skillPath is still reported as content drift, whose remedy (bump lastComparedTreeHash) cannot be executed: $fork_check_output"
grep -qF "$missing_skill_path" "$relay_call_log" 2>/dev/null ||
  fail "case 6: the relay notification does not name the missing skillPath: $(cat "$relay_call_log")"
assert_relay_state fork-path-missing \
  "case 6: a stale skillPath does not relay the fork-path-missing state, so downstream cannot route it to its own remedy"
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
  assert_relay_state fork-lock-broken \
    "case 7/8 (forks=$malformed_forks): a corrupt forks table was logged but never relayed, so it reaches nobody who is not reading the run log"
done

# --- Case 9: a forks entry missing its sourceUrl -----------------------------
# "upstream unreachable (null)" reads as a network problem and sends the
# operator to check their connection. The lock is what is broken.
#
# The lock carries a HEALTHY sibling as well (jq sorts keys, so the broken entry
# is walked first): reporting a broken entry must skip that ENTRY, never end the
# walk, or one typo silently unwatches every fork after it.
write_forks_lock "$(jq -n --arg url "$fixture_repo" \
  '{brokenfork: {skillPath: "skills/brokenfork",
      lastComparedTreeHash: "0000000000000000000000000000000000000000"},
    zdriftfork: {source: "fixture/zdriftfork", sourceUrl: $url,
      skillPath: "skills/forkskill",
      lastComparedTreeHash: "0000000000000000000000000000000000000000"}}')"
run_fork_check
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 9: the drift-watch exited $fork_check_rc on a malformed forks entry: $fork_check_output"
printf '%s\n' "$fork_check_output" | grep -q 'brokenfork' ||
  fail "case 9: a forks entry with no sourceUrl produced no warning naming it: $fork_check_output"
refute_match "$fork_check_output" 'unreachable' \
  "case 9: a lock entry with no sourceUrl is mislabelled as an unreachable upstream: $fork_check_output"
assert_relay_state fork-lock-broken \
  "case 9: a malformed forks entry was logged but never relayed, so it reaches nobody who is not reading the run log"
printf '%s\n' "$fork_check_output" | grep -q 'FORK DRIFT.*zdriftfork' ||
  fail "case 9: the fork listed AFTER the malformed entry was never checked, so one broken entry silences every fork behind it: $fork_check_output"

# --- Case 9b: a forks entry that is not an object at all ---------------------
# Same crash as the array TABLE, one level down: `jq '.forks[$f].sourceUrl'`
# fails with "Cannot index string with string", and an assignment from a failing
# command substitution aborts the run under `set -euo pipefail` (measured: exit
# 5 with a raw jq error, no warning, no relay).
for malformed_entry in '"not-an-object"' '["not-an-object"]' '7'; do
  write_forks_lock "$(printf '{"stringfork": %s}' "$malformed_entry")"
  run_fork_check
  [[ $fork_check_rc -eq 0 ]] ||
    fail "case 9b (entry=$malformed_entry): the drift-watch exited $fork_check_rc; a corrupt advisory entry must never kill the run: $fork_check_output"
  printf '%s\n' "$fork_check_output" | grep -q 'stringfork' ||
    fail "case 9b (entry=$malformed_entry): a non-object forks entry produced no warning naming it: $fork_check_output"
  refute_match "$fork_check_output" 'jq: error' \
    "case 9b (entry=$malformed_entry): a raw jq error reached the operator instead of a named warning: $fork_check_output"
  assert_relay_state fork-lock-broken \
    "case 9b (entry=$malformed_entry): a non-object forks entry was logged but never relayed"
done

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

# --- Case 11: skillPath "." means the whole repository ------------------------
# The committed lock uses it for elevenlabs, whose upstream ships its SKILL.md at
# the repo root. The comparison is against HEAD's ROOT TREE: HEAD itself is the
# commit hash (a different value that can never match a recorded tree hash, so
# every week would cry drift), and "HEAD:." does not resolve at all (rc 1, so
# every week would cry a missing path). Both mis-readings are the permanent
# cry-wolf this suite exists to prevent, and neither is visible from the
# unreachable-upstream fixture the other cases use.
whole_repo_tree_hash="$(git -C "$fixture_repo" rev-parse 'HEAD^{tree}')"
write_forks_lock "$(jq -n --arg url "$fixture_repo" --arg hash "$whole_repo_tree_hash" \
  '{wholefork: {source: "fixture/wholefork", sourceUrl: $url, skillPath: ".",
    lastComparedTreeHash: $hash}}')"
run_fork_check
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 11: the drift-watch exited $fork_check_rc on a whole-repo fork: $fork_check_output"
printf '%s\n' "$fork_check_output" | grep -q 'wholefork: upstream unchanged' ||
  fail "case 11: a whole-repo fork recorded at HEAD's root tree hash was not reported as unchanged: $fork_check_output"
refute_match "$fork_check_output" 'FORK DRIFT|FORK PATH MISSING' \
  "case 11: an unchanged whole-repo fork raised an alert, which is the weekly cry-wolf this watch must not produce: $fork_check_output"
[[ ! -s $relay_call_log ]] ||
  fail "case 11: an unchanged whole-repo fork paged the operator: $(cat "$relay_call_log")"

# Same entry, upstream moves: the whole-repo branch must still SEE drift.
printf -- '\n## a later upstream change\n' >>"$fixture_repo/skills/forkskill/SKILL.md"
git -C "$fixture_repo" -c user.email=test@test -c user.name=test add -A
git -C "$fixture_repo" -c user.email=test@test -c user.name=test commit -qm 'later upstream change'
run_fork_check
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 11: the drift-watch exited $fork_check_rc on a drifting whole-repo fork: $fork_check_output"
printf '%s\n' "$fork_check_output" | grep -q 'FORK DRIFT.*wholefork' ||
  fail "case 11: a whole-repo fork whose upstream changed was not reported as drift: $fork_check_output"
assert_relay_state fork-drift \
  "case 11: a drifting whole-repo fork did not relay the fork-drift state"

# --- Case 12: a failing relay must not decide the run's exit status -----------
# relay.sh is an advisory notifier. If its exit status escapes, a push failure
# aborts the weekly run at this phase, which is AFTER the generation exchange
# has published and BEFORE the success stamp is written: every remaining retry
# slot then redoes the whole update and dies at the same line. The lock still
# holds the drifting whole-repo fork from case 11, so this run reaches a relay.
install_relay_shim 3
run_fork_check
install_relay_shim 0
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 12: a relay push that exited non-zero took the drift-watch down with it (rc=$fork_check_rc): $fork_check_output"
grep -q 'wholefork' "$relay_call_log" 2>/dev/null ||
  fail "case 12: the failing relay was never called, so this case proves nothing about its exit status"
printf '%s\n' "$fork_check_output" | grep -q 'FORK DRIFT.*wholefork' ||
  fail "case 12: the drift itself went unreported when the relay failed: $fork_check_output"

# --- Case 13: the lock does not parse at all ---------------------------------
# Distinct from a malformed forks table: there is no table to blame, and telling
# the operator the forks table is broken sends them to the wrong line of a file
# whose real problem is that it is not JSON.
printf 'this is not json\n' >"$HOME/.agents/custom-skill-lock.json"
run_fork_check
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 13: the drift-watch exited $fork_check_rc on an unparseable lock: $fork_check_output"
printf '%s\n' "$fork_check_output" | grep -q 'does not parse as a JSON object' ||
  fail "case 13: an unparseable lock produced no warning saying so: $fork_check_output"
refute_match "$fork_check_output" 'forks table' \
  "case 13: an unparseable lock is blamed on its forks table, which sends the operator to the wrong line: $fork_check_output"
assert_relay_state fork-lock-broken \
  "case 13: an unparseable lock was logged but never relayed"

echo "update-skills-fork-drift: OK (4 baseline assertions + rewrite immunity (global and system), stale skillPath, 4 malformed tables, malformed entries, dry-run notifies nobody, whole-repo skillPath, failing relay, unparseable lock)"
