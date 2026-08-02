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
#      remedy that cannot work when the path itself is gone. The same run walks
#      every branch that stages a clone, and none of them may leak it.
#  6b. A temp dir that cannot be created is advisory too, and the run says which
#      directory it could not use. This is also what proves the clone is staged
#      under TMPDIR, without which case 6's cleanup check would be vacuous.
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
#
# Cases 14-20 hold the phase to one rule: an upstream this run did not compare
# has to be REPORTED, not merely logged. Everything that reaches only
# ~/.local/log/skills/ is how a fork stops being watched without anyone finding
# out, which is the failure this phase exists to prevent:
#  14. An unreachable upstream is RELAYED, carrying git's own message, so a
#      renamed, deleted or newly private upstream is not filed under "check
#      your network" forever.
#  15. A field that is not a JSON string is a malformed entry, per field and per
#      type. `jq -r` renders any scalar as text, so an unquoted hash cried FORK
#      DRIFT every week, a numeric sourceUrl was blamed on the network, and a
#      boolean skillPath was reported as a path upstream had deleted.
#  16. A temp dir that cannot be created is relayed too: it means EVERY fork
#      goes unchecked, the widest silent outage this phase has.
#  17. A forks key with an embedded newline is ONE key. Line-delimited, it split
#      into two phantom entries, each relayed as a broken fork, while the real
#      entry was never walked.
#  18. The two lock-level advisories carry a namespaced --project, so a fork
#      literally named `lock` stays distinguishable from the lock file.
#  19. The malformed-entry advisory says WHICH way the entry was broken.
#  20. An absent lock is named. Silence made "no lock" and "no drift" identical.
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

# run_fork_check_bounded <watchdog-seconds> [NAME=VALUE ...] -- the same run
# under an OUTER watchdog, leaving the wall-clock seconds it took in
# $fork_check_elapsed. Only case 21 needs it, and it needs it twice over: a run
# that never returns is exactly what that case pins, so the assertion has to be
# able to observe a run that would otherwise never end, and a regression must
# fail this test rather than wedge the whole suite behind it. Polled in whole
# seconds (no timeout(1): it is GNU coreutils, present in the flake shell and on
# this host but not something a test may assume).
fork_check_elapsed=0
run_fork_check_bounded() {
  local watchdog_seconds="$1"
  shift
  local output_file="$scratch_dir/bounded-run.out" waited=0 runner_pid
  : >"$relay_call_log"
  UPDATE_SKILLS_FORCE=1 env "$@" bash "$SCRIPT" --check-forks-only >"$output_file" 2>&1 &
  runner_pid=$!
  while [[ $waited -lt $watchdog_seconds ]] && kill -0 "$runner_pid" 2>/dev/null; do
    sleep 1
    waited=$((waited + 1))
  done
  if kill -0 "$runner_pid" 2>/dev/null; then
    kill -KILL "$runner_pid" 2>/dev/null || true
  fi
  set +e
  wait "$runner_pid"
  fork_check_rc=$?
  set -e
  fork_check_elapsed="$waited"
  fork_check_output="$(cat "$output_file")"
}

# assert_relay_line <state> <substring> <message> -- ONE relay push carries both
# this exact --state and this substring. Asserted together, on one line, because
# two independent greps over the whole log also pass when the state rides one
# push and the detail rides another, which is exactly what a mislabelled alert
# looks like. The states are the operator-facing vocabulary CLAUDE.md documents
# and each maps to a different remedy, so an arbitrary third string is a
# regression even when the log text still reads correctly.
assert_relay_line() {
  local state="$1" substring="$2" message="$3"
  grep -F -- "--state $state " "$relay_call_log" 2>/dev/null | grep -qF -- "$substring" ||
    fail "$message (relay log: $(cat "$relay_call_log"))"
}

# assert_relay_state <state> <message> -- the state alone, for the advisories
# whose payload is the lock rather than a value worth co-locating.
assert_relay_state() {
  assert_relay_line "$1" "" "$2"
}

# assert_log_line_has <output> <marker> <substring> <message> -- the line
# carrying <marker> also carries <substring>. Same reason as assert_relay_line:
# an operator scanning for the heading has to find the detail ON it, and two
# whole-output greps pass with the two on unrelated lines.
assert_log_line_has() {
  local output="$1" marker="$2" substring="$3" message="$4"
  printf '%s\n' "$output" | grep -F -- "$marker" | grep -qF -- "$substring" ||
    fail "$message ($output)"
}

# assert_clone_dir_removed <dir> <message> -- nothing this phase creates under
# its own private TMPDIR outlives the run. Only meaningful once something has
# proved the script honours TMPDIR at all: case 6b is that proof, and without it
# this would pass against a script that staged its clones somewhere else
# entirely (measured: a bare `mktemp -d` on macOS does exactly that).
assert_clone_dir_removed() {
  local tmpdir="$1" message="$2" residue
  residue="$(find "$tmpdir" -mindepth 1 -maxdepth 1 2>/dev/null)"
  [[ -z $residue ]] || fail "$message (left behind: $residue)"
}

# Byte-level snapshot of the fork before any run (assertion 4 compares later).
fork_snapshot_before="$(cd "$fork_store_dir" && find . -type f -print0 | sort -z | xargs -0 shasum -a 256)"

# Run 1: upstream unchanged. FORCE bypasses the idle-gate (a harness is by
# definition running this test).
output="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --check-forks-only 2>&1)" ||
  fail "--check-forks-only exited non-zero with an unreachable upstream in the lock: $output"

# 1) No drift -> no drift alert and no drift push. Scoped to the DRIFT state
# rather than to an empty relay log: the same fixture carries an unreachable
# upstream on purpose, and an upstream nobody could compare is itself a
# reportable finding (case 14), so "the log is empty" would pin the opposite of
# what this phase is for.
refute_match "$output" 'drift.*forkskill|forkskill.*drift' \
  "run alerted drift for forkskill although upstream is unchanged: $output"
refute_match "$(cat "$relay_call_log")" '--state fork-drift( |$)' \
  "a drift push went out although no fork drifted: $(cat "$relay_call_log")"

# 2) The unreachable upstream is reported as a warning, by name.
printf '%s\n' "$output" | grep -q 'ghostfork' ||
  fail "unreachable upstream produced no logged warning naming ghostfork: $output"

# The upstream moves: a commit changes the skill folder.
printf -- '\n## New upstream feature\n' >>"$fixture_repo/skills/forkskill/SKILL.md"
git -C "$fixture_repo" -c user.email=test@test -c user.name=test add -A
git -C "$fixture_repo" -c user.email=test@test -c user.name=test commit -qm 'upstream feature'

# Run 2: drift must be alerted, run must still exit 0.
: >"$relay_call_log"
output="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --check-forks-only 2>&1)" ||
  fail "--check-forks-only exited non-zero on drift: $output"

# 3) The alert names the fork and its upstream, and the DRIFT push carries the
# fork's name (the same run also relays the unreachable ghostfork, so a bare
# name grep over the whole log no longer says which push it landed on).
printf '%s\n' "$output" | grep -q 'FORK DRIFT.*forkskill' ||
  fail "no drift alert naming forkskill: $output"
printf '%s\n' "$output" | grep -qF "$fixture_repo" ||
  fail "drift alert does not name the upstream: $output"
assert_relay_line fork-drift forkskill \
  "the drift push does not carry the fork's name"

# 4) The fork's store content is byte-identical to the pre-run snapshot.
fork_snapshot_after="$(cd "$fork_store_dir" && find . -type f -print0 | sort -z | xargs -0 shasum -a 256)"
[[ $fork_snapshot_before == "$fork_snapshot_after" ]] ||
  fail "the drift check modified the fork's store content"
[[ -f "$fork_store_dir/local-edit.marker" ]] || fail "the fork's marker file is gone"

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

# --- Case 5c: the same immunity for git's COMMAND-scope config channels -------
# GIT_CONFIG_GLOBAL and GIT_CONFIG_SYSTEM neutralize the two FILE channels and
# nothing else. Config injected through GIT_CONFIG_COUNT/GIT_CONFIG_KEY_n, and
# through GIT_CONFIG_PARAMETERS (what `git -c` exports to every subprocess,
# including hooks), still applied, so a rewrite arriving that way redirected the
# clone to a DIFFERENT repository while every report still named the recorded
# URL: measured as FORK DRIFT against an upstream that had not changed.
#
# The alternate repository holds different content at the same skill path, so a
# redirected clone reports drift and a faithful one reports unchanged. That is
# the whole discrimination, and it needs no network.
case5c_repo="$scratch_dir/case5c-alternate-repo"
mkdir -p "$case5c_repo/skills/forkskill"
printf -- '---\nname: forkskill\ndescription: a DIFFERENT repository\n---\n# elsewhere\n' \
  >"$case5c_repo/skills/forkskill/SKILL.md"
git -C "$case5c_repo" init -q
git -C "$case5c_repo" -c user.email=test@test -c user.name=test add -A
git -C "$case5c_repo" -c user.email=test@test -c user.name=test commit -qm alternate
rm -f "$HOME/.gitconfig" # case 5's file-channel rewrite is not what this pins
write_forks_lock "$(jq -n --arg url "$fixture_repo" \
  --arg hash "$(git -C "$fixture_repo" rev-parse 'HEAD:skills/forkskill')" \
  '{recordedfork: {source: "fixture/recordedfork", sourceUrl: $url,
    skillPath: "skills/forkskill", lastComparedTreeHash: $hash}}')"

# Control FIRST: with no rewrite the recorded upstream really is unchanged, so
# any drift below comes from the redirect and not from a stale fixture hash.
run_fork_check
printf '%s\n' "$fork_check_output" | grep -q 'recordedfork: upstream unchanged' ||
  fail "case 5c control: the recorded upstream is not unchanged without a rewrite, so the redirected runs below prove nothing: $fork_check_output"

# Control SECOND: each channel must really reroute a plain clone, or the case
# discriminates nothing.
for case5c_channel in count parameters; do
  case "$case5c_channel" in
    count) case5c_env=(GIT_CONFIG_COUNT=1 "GIT_CONFIG_KEY_0=url.$case5c_repo.insteadOf" "GIT_CONFIG_VALUE_0=$fixture_repo") ;;
    parameters) case5c_env=("GIT_CONFIG_PARAMETERS='url.$case5c_repo.insteadOf=$fixture_repo'") ;;
  esac
  rm -rf "$scratch_dir/case5c-control-clone"
  env "${case5c_env[@]}" git clone --quiet --depth 1 "$fixture_repo" "$scratch_dir/case5c-control-clone" >/dev/null 2>&1
  grep -q 'elsewhere' "$scratch_dir/case5c-control-clone/skills/forkskill/SKILL.md" 2>/dev/null ||
    fail "case 5c control ($case5c_channel): the planted rewrite did not redirect a plain clone, so this channel cannot tell a neutralized clone from a faithful one"

  run_fork_check "${case5c_env[@]}"
  [[ $fork_check_rc -eq 0 ]] ||
    fail "case 5c ($case5c_channel): --check-forks-only exited $fork_check_rc under a url-rewriting $case5c_channel channel: $fork_check_output"
  printf '%s\n' "$fork_check_output" | grep -q 'recordedfork: upstream unchanged' ||
    fail "case 5c ($case5c_channel): a rewrite arriving through git's $case5c_channel channel redirected the drift clone, so the phase compared a DIFFERENT repository and reported the verdict against the recorded URL: $fork_check_output"
  [[ ! -s $relay_call_log ]] ||
    fail "case 5c ($case5c_channel): an unchanged upstream paged the operator, which is what comparing the wrong repository looks like: $(cat "$relay_call_log")"
done
hostile_rewrite_config "$HOME/.gitconfig" # restore what case 5 planted

# --- Case 6: a reachable upstream missing the recorded skillPath --------------
# Distinct from drift: the upstream is fine, the lock's path is stale. The old
# code compared `git rev-parse`'s output against a "missing-path" sentinel,
# which never matched (rev-parse ECHOES the unresolvable argument to stdout
# before failing), so this reported content drift and told the operator to bump
# a hash under a path that no longer exists.
#
# The lock carries one fork per outcome, so this single run walks ALL THREE
# branches that stage a clone and then have to remove it (path missing,
# unreachable upstream, hash compared): a leak on any of them fills the temp
# dir with upstream copies week after week.
missing_skill_path="skills/moved-away"
current_forkskill_hash="$(git -C "$fixture_repo" rev-parse "HEAD:skills/forkskill")"
write_forks_lock "$(jq -n --arg url "$fixture_repo" --arg path "$missing_skill_path" \
  --arg ghost "$scratch_dir/no-such-repo" --arg hash "$current_forkskill_hash" \
  '{pathfork: {source: "fixture/pathfork", sourceUrl: $url, skillPath: $path,
      lastComparedTreeHash: "0000000000000000000000000000000000000000"},
    reachedfork: {source: "fixture/reachedfork", sourceUrl: $url,
      skillPath: "skills/forkskill", lastComparedTreeHash: $hash},
    unreachedfork: {source: "fixture/unreachedfork", sourceUrl: $ghost,
      skillPath: ".", lastComparedTreeHash: "0000000000000000000000000000000000000000"}}')"

# A private TMPDIR for this run: the drift clones are the only thing this phase
# creates there, so what is left afterwards says whether each branch cleaned up
# after itself. Case 6b is what makes this discriminating rather than vacuous.
case6_tmpdir="$scratch_dir/case6-tmp"
mkdir -p "$case6_tmpdir"
run_fork_check TMPDIR="$case6_tmpdir"
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 6: --check-forks-only exited $fork_check_rc on a stale skillPath: $fork_check_output"
assert_log_line_has "$fork_check_output" 'FORK PATH MISSING' "$missing_skill_path" \
  "case 6: no FORK PATH MISSING line names the skillPath that no longer exists upstream, so the heading CLAUDE.md documents and the detail an operator acts on are not on the same line"
refute_match "$fork_check_output" 'FORK DRIFT' \
  "case 6: a stale skillPath is still reported as content drift, whose remedy (bump lastComparedTreeHash) cannot be executed: $fork_check_output"
assert_relay_line fork-path-missing "$missing_skill_path" \
  "case 6: no single relay push carries both the fork-path-missing state and the missing skillPath, so downstream cannot route it to its own remedy with the detail attached"
refute_match "$(cat "$relay_call_log")" '--state fork-drift( |$)' \
  "case 6: a stale skillPath relays the fork-drift state, so it is indistinguishable from real drift downstream: $(cat "$relay_call_log")"
printf '%s\n' "$fork_check_output" | grep -q 'reachedfork: upstream unchanged' ||
  fail "case 6: the reachable, unchanged fork was not walked, so the hash-compared branch never staged a clone and the cleanup assertion below covers less than it claims: $fork_check_output"
assert_log_line_has "$fork_check_output" 'FORK UNREACHABLE' 'unreachedfork' \
  "case 6: the unreachable fork was not walked, so the unreachable branch never staged a clone and the cleanup assertion below covers less than it claims"
assert_clone_dir_removed "$case6_tmpdir" \
  "case 6: a branch left its clone behind, so a weekly run fills the temp dir with upstream copies"

# --- Case 6b: a temp dir that cannot be created is advisory, not fatal --------
# Two jobs. It pins the branch itself: staging a clone is the one step of this
# phase that can fail before any network call, and a failing `mktemp` assignment
# under `set -euo pipefail` would kill the weekly run after the generation
# exchange has published and before the success stamp is written. And it is what
# makes case 6's residue check mean anything, by proving the script stages its
# clones under TMPDIR at all: a script that ignored TMPDIR would clone happily
# here and report the same findings as case 6 (measured on macOS 26.2: a bare
# `mktemp -d` ignores TMPDIR, with and without -t).
case6b_tmpdir="$scratch_dir/case6b-tmp-absent"
[[ ! -e $case6b_tmpdir ]] ||
  fail "case 6b: the fixture directory must NOT exist for this case to test anything"
run_fork_check TMPDIR="$case6b_tmpdir"
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 6b: an uncreatable temp dir took the drift-watch down with it (rc=$fork_check_rc): $fork_check_output"
assert_log_line_has "$fork_check_output" 'could not create a temp dir' "$case6b_tmpdir" \
  "case 6b: the run does not say which directory it failed to stage a clone under: $fork_check_output"
refute_match "$fork_check_output" 'FORK PATH MISSING|FORK DRIFT|upstream unchanged' \
  "case 6b: the run reported a per-fork finding although no clone could be staged, so it is not reading TMPDIR and case 6's cleanup assertion is vacuous: $fork_check_output"
[[ ! -e $case6b_tmpdir ]] ||
  fail "case 6b: the run created the temp parent directory instead of reporting that it could not"

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

# ---------------------------------------------------------------------------
# Cases 14-20. The rule they share: an upstream this run did not compare has to
# be REPORTED, not merely logged. A line in ~/.local/log/skills/ that nobody
# reads is how a fork stops being watched without anyone finding out, which is
# the failure the whole phase exists to prevent.
#
# Read LIVE, not carried down from the setup block: the cases above commit to
# the fixture repo, so the hash that means "no drift" is whatever upstream holds
# right now. Shared by several cases below, and computed here rather than inside
# one of them so each case can be run on its own.
# ---------------------------------------------------------------------------
current_fixture_hash="$(git -C "$fixture_repo" rev-parse "HEAD:skills/forkskill")"

# --- Case 14: an unreachable upstream is relayed, and says what git said ------
# The url-rewrite defect was one CAUSE of a permanently silent skip. This is the
# REPORTING: every other cause of a durable clone failure (an upstream renamed,
# deleted or made private, a proxy, DNS, a rewrite arriving through
# GIT_CONFIG_COUNT, which this phase documents as still applying) put the fork
# back into "unwatched forever, one log line nobody reads". git's own message
# rides along because "unreachable" alone cannot tell a dead network from a
# dead URL, and those have opposite remedies.
write_forks_lock "$(jq -n --arg ghost "$scratch_dir/no-such-repo" \
  '{goneupstream: {source: "fixture/goneupstream", sourceUrl: $ghost, skillPath: ".",
    lastComparedTreeHash: "0000000000000000000000000000000000000000"}}')"
run_fork_check
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 14: the drift-watch exited $fork_check_rc on an unreachable upstream: $fork_check_output"
assert_relay_line fork-upstream-unreachable goneupstream \
  "case 14: an upstream that could not be fetched was logged and never relayed, so the fork is unwatched and nobody outside the run log is told"
assert_log_line_has "$fork_check_output" 'git said' 'does not exist' \
  "case 14: git's own diagnosis is discarded, so the log cannot tell a dead network from a dead URL"
refute_match "$(cat "$relay_call_log")" '--state fork-drift( |$)' \
  "case 14: an unreachable upstream relays the fork-drift state, whose remedy (compare and bump) cannot be executed: $(cat "$relay_call_log")"

# --- Case 15: a field that is not a STRING is a malformed entry ---------------
# `jq -r` renders any scalar as text, so the entry-level object check let every
# non-string field through. Each one is PERMANENT and points somewhere the
# defect is not: an unquoted hash matched nothing and cried FORK DRIFT every
# week; a numeric sourceUrl was reported as an unreachable NETWORK; a boolean
# skillPath resolved as the literal path "true" and was reported as a path
# upstream had deleted. Per FIELD and per TYPE, because a whole-entry check
# still passes when only one field is wrong.
write_typed_fork_lock() { # $1 field, $2 JSON value
  write_forks_lock "$(jq -n --arg url "$fixture_repo" --arg hash "$current_fixture_hash" \
    --arg field "$1" --argjson value "$2" \
    '{typedfork: ({source: "fixture/typedfork", sourceUrl: $url,
        skillPath: "skills/forkskill", lastComparedTreeHash: $hash}
      | .[$field] = $value)}')"
}
# FALSE-POSITIVE DIRECTION FIRST: the untouched fixture must be walked and
# reported clean, or every assertion below passes for the wrong reason.
write_typed_fork_lock source '"fixture/typedfork"'
run_fork_check
printf '%s\n' "$fork_check_output" | grep -q 'typedfork: upstream unchanged' ||
  fail "case 15 control: the well-formed fixture was not walked clean, so the malformed variants below prove nothing: $fork_check_output"
[[ ! -s $relay_call_log ]] ||
  fail "case 15 control: a well-formed entry paged the operator: $(cat "$relay_call_log")"
for case15_field in sourceUrl skillPath lastComparedTreeHash; do
  for case15_value in '12345' 'true' 'false' 'null' '["x"]' '{"a":1}' '""'; do
    write_typed_fork_lock "$case15_field" "$case15_value"
    run_fork_check
    [[ $fork_check_rc -eq 0 ]] ||
      fail "case 15 ($case15_field=$case15_value): the drift-watch exited $fork_check_rc: $fork_check_output"
    assert_log_line_has "$fork_check_output" 'typedfork' "$case15_field" \
      "case 15 ($case15_field=$case15_value): the advisory does not name the field the operator has to fix"
    refute_match "$fork_check_output" 'FORK DRIFT|FORK PATH MISSING|FORK UNREACHABLE|upstream unchanged' \
      "case 15 ($case15_field=$case15_value): a broken lock entry was reported as an upstream finding, which sends the operator to the upstream when the lock is what is wrong: $fork_check_output"
    assert_relay_line fork-lock-broken "$case15_field" \
      "case 15 ($case15_field=$case15_value): no relay push carries both the fork-lock-broken state and the offending field"
  done
done

# --- Case 16: a temp dir that cannot be created is relayed too ----------------
# Case 6b pins that it does not kill the run. This pins that anyone is told: if
# the temp dir is unusable then EVERY fork goes unchecked, which is the widest
# possible silent outage of this phase.
write_forks_lock "$(jq -n --arg url "$fixture_repo" --arg hash "$current_fixture_hash" \
  '{stagedfork: {source: "fixture/stagedfork", sourceUrl: $url,
    skillPath: "skills/forkskill", lastComparedTreeHash: $hash}}')"
case16_tmpdir="$scratch_dir/case16-not-a-dir"
: >"$case16_tmpdir"
run_fork_check TMPDIR="$case16_tmpdir"
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 16: an unusable temp dir took the drift-watch down with it (rc=$fork_check_rc): $fork_check_output"
assert_relay_line fork-clone-unstageable stagedfork \
  "case 16: no clone could be staged, so nothing was drift-checked, and the only trace is a log line nobody reads"

# --- Case 17: a forks key with an embedded newline is ONE key -----------------
# A forks key is a JSON string and may hold anything a JSON string may. Under a
# line-delimited feed, `bad\nname` split into two phantom entries, each relayed
# as its own broken fork, while the real entry was never walked at all: two
# false alarms and one silent hole, from one key.
case17_key=$'bad\nname'
write_forks_lock "$(jq -n --arg url "$fixture_repo" --arg hash "$current_fixture_hash" \
  --arg key "$case17_key" \
  '{($key): {source: "fixture/newlinefork", sourceUrl: $url,
      skillPath: "skills/forkskill", lastComparedTreeHash: $hash},
    zsiblingfork: {source: "fixture/zsiblingfork", sourceUrl: $url,
      skillPath: "skills/forkskill", lastComparedTreeHash: $hash}}')"
run_fork_check
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 17: the drift-watch exited $fork_check_rc on a forks key with a newline: $fork_check_output"
refute_match "$fork_check_output" 'fork drift-check (bad|name): the lock entry is malformed' \
  "case 17: a key with an embedded newline split into phantom entries, each reported as its own broken fork: $fork_check_output"
[[ "$(printf '%s\n' "$fork_check_output" | grep -c 'upstream unchanged')" -eq 2 ]] ||
  fail "case 17: both entries must be walked and reported clean (the newline key and its sibling): $fork_check_output"
[[ ! -s $relay_call_log ]] ||
  fail "case 17: two healthy forks paged the operator: $(cat "$relay_call_log")"

# --- Case 18: the lock-level advisories cannot collide with a fork name -------
# Every per-fork push carries a fork name as its --project. The two pushes that
# are about the LOCK carry a namespaced label instead, so a fork literally named
# `lock` stays distinguishable from the lock file itself downstream. A colon
# cannot occur in a skill directory name, which is what makes the namespaces
# disjoint rather than merely unlikely to meet.
first_relay_project() {
  grep -o -- '--project [^ ]*' "$relay_call_log" | head -1
}
printf 'this is not json\n' >"$HOME/.agents/custom-skill-lock.json"
run_fork_check
case18_file_project="$(first_relay_project)"
[[ $case18_file_project == "--project lock:file" ]] ||
  fail "case 18: the unparseable-lock advisory is labelled '$case18_file_project', which sits in the fork name space"
write_forks_lock '{"lock": {}}'
run_fork_check
case18_fork_project="$(first_relay_project)"
[[ $case18_fork_project == "--project lock" ]] ||
  fail "case 18: a fork named 'lock' relayed as '$case18_fork_project', so this case cannot show the two are distinguishable"
[[ $case18_file_project != "$case18_fork_project" ]] ||
  fail "case 18: a fork named 'lock' and the lock file itself relay the same --project, so downstream cannot tell them apart"

# --- Case 19: the malformed-entry advisory says WHICH way it was broken -------
# One remedy (fix that lock entry), two causes. Collapsing them into one message
# leaves the operator to re-derive which, on a file they have to hand-edit.
write_forks_lock '{"scalarfork": "not-an-object"}'
run_fork_check
case19_scalar_line="$(printf '%s\n' "$fork_check_output" | grep 'scalarfork')"
printf '%s\n' "$case19_scalar_line" | grep -qF 'must be a JSON object' ||
  fail "case 19: a scalar entry does not say that an object is what was expected: $fork_check_output"
write_forks_lock "$(jq -n --arg url "$fixture_repo" \
  '{fieldfork: {source: "fixture/fieldfork", sourceUrl: $url,
    lastComparedTreeHash: "0000000000000000000000000000000000000000"}}')"
run_fork_check
case19_field_line="$(printf '%s\n' "$fork_check_output" | grep 'fieldfork')"
printf '%s\n' "$case19_field_line" | grep -qF 'skillPath' ||
  fail "case 19: an entry missing one field does not name that field: $fork_check_output"
refute_match "$case19_field_line" 'must be a JSON object' \
  "case 19: an entry that IS an object is told it is not one, which sends the operator to the wrong repair: $case19_field_line"

# --- Case 20: an ABSENT lock is named, not silently treated as nothing to do --
# The weekly flow cannot reach this (it walks a validated snapshot), so this is
# the health probe's own finding, and silence is the one answer a probe must not
# give: "no lock" and "no drift" printed exactly the same thing.
rm -f "$HOME/.agents/custom-skill-lock.json"
run_fork_check
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 20: the drift-watch exited $fork_check_rc with no lock file: $fork_check_output"
assert_log_line_has "$fork_check_output" 'does not exist' 'custom-skill-lock.json' \
  "case 20: an absent lock produced no warning naming it, so no upstream is watched and nothing says so"
assert_relay_state fork-lock-missing \
  "case 20: an absent lock was logged but never relayed"

# --- Case 21: an upstream that never answers is stopped at a deadline ---------
# The worst thing this phase can do is not a wrong report, it is not returning.
# It runs in the weekly flow AFTER the generation exchange has published and
# BEFORE the success stamp is written, so a clone that hangs does not skip one
# fork: it parks the whole weekly update, and every later slot redoes the work
# and stalls at the same line. A transport helper that accepts the connection
# and then says nothing is the network-free stand-in (git runs
# `git-remote-<transport>` for a `<transport>::<address>` URL). It sleeps well
# past the deadline, so the elapsed assertion can tell "stopped at the deadline"
# from "waited for the remote", and it ends on its own so the case leaves
# nothing sleeping for long after the suite.
case21_bin="$scratch_dir/case21-bin"
mkdir -p "$case21_bin"
cat >"$case21_bin/git-remote-stall" <<'EOF'
#!/usr/bin/env bash
sleep 25
EOF
chmod +x "$case21_bin/git-remote-stall"
write_forks_lock "$(jq -n --arg hash "$current_fixture_hash" \
  '{stalledfork: {source: "fixture/stalledfork", sourceUrl: "stall::example.invalid/repo.git",
    skillPath: ".", lastComparedTreeHash: $hash}}')"
run_fork_check_bounded 60 PATH="$case21_bin:$PATH" UPDATE_SKILLS_FORK_CLONE_DEADLINE=2
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 21: a stalled clone took the drift-watch down with it (rc=$fork_check_rc): $fork_check_output"
[[ $fork_check_elapsed -lt 15 ]] ||
  fail "case 21: the run spent ${fork_check_elapsed}s on a clone whose deadline was 2s, so nothing bounds the fetch and a remote that never answers parks the weekly update after publishing and before stamping: $fork_check_output"
assert_log_line_has "$fork_check_output" 'stalledfork' 'NOT drift-checked' \
  "case 21: a clone stopped at its deadline was not reported as an upstream this run did not compare"
assert_relay_line fork-clone-timeout stalledfork \
  "case 21: a clone stopped at its deadline was logged but never relayed, so the fork is unwatched and nobody outside the run log is told"

# --- Case 21b: what a stopped clone leaves behind holds no lock ---------------
# The run's serialize lock is a kernel flock on fd 9, held for the process's
# lifetime and INHERITED by every child. Killing git does not reap a transport
# helper that never reads its stdin, so a leftover helper keeps that fd (and the
# lock with it) open, and the next scheduled slot defers with exit 75 over a
# fork nobody could clone: one stalled upstream, a weekly update that never runs
# again. Case 21's helper is still asleep right now, which is what makes this
# discriminating rather than a second copy of case 22.
write_forks_lock "$(jq -n --arg url "$fixture_repo" --arg hash "$current_fixture_hash" \
  '{afterstallfork: {source: "fixture/afterstallfork", sourceUrl: $url,
    skillPath: "skills/forkskill", lastComparedTreeHash: $hash}}')"
run_fork_check
[[ $fork_check_rc -ne 75 ]] ||
  fail "case 21b: the run after a stopped clone deferred on the serialize lock, so what the stalled clone left behind still holds it and every later slot defers: $fork_check_output"
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 21b: the run after a stopped clone exited $fork_check_rc: $fork_check_output"
printf '%s\n' "$fork_check_output" | grep -q 'afterstallfork: upstream unchanged' ||
  fail "case 21b: the run after a stopped clone did not walk its forks: $fork_check_output"

# --- Case 22: a clone deadline that is not a positive number is the default ---
# The deadline is env-overridable, and an override nobody validates is a guard
# nobody has: a garbage value reaching the comparison as 0 stops EVERY clone at
# once (every fork "timed out", nothing compared again, forever), and one
# reaching it as a shell error takes the phase down. Each variant must leave the
# healthy fixture walked and compared, which is what says the default took over.
for case22_deadline in 'not-a-number' '' '0' '-5' '3.5'; do
  write_forks_lock "$(jq -n --arg url "$fixture_repo" --arg hash "$current_fixture_hash" \
    '{guardfork: {source: "fixture/guardfork", sourceUrl: $url,
      skillPath: "skills/forkskill", lastComparedTreeHash: $hash}}')"
  run_fork_check UPDATE_SKILLS_FORK_CLONE_DEADLINE="$case22_deadline"
  [[ $fork_check_rc -eq 0 ]] ||
    fail "case 22 (deadline='$case22_deadline'): the drift-watch exited $fork_check_rc on an unusable deadline override: $fork_check_output"
  printf '%s\n' "$fork_check_output" | grep -q 'guardfork: upstream unchanged' ||
    fail "case 22 (deadline='$case22_deadline'): an unusable deadline override was taken at face value instead of falling back to the default, so a reachable upstream went uncompared: $fork_check_output"
done

# --- Case 23: a lock with NO forks table is not a clean zero-entry watch ------
# `"forks": {}` is a lock SAYING there is deliberately nothing to watch. An
# absent table says nothing at all, and it is what a typo'd key (`forkss`,
# `Forks`) or a hand-edit that dropped the table leaves behind. Treated as legal
# it printed exactly what a healthy zero-drift run prints, so the weekly run
# published a generation, stamped the week a success, compared no vendored
# upstream, and nothing anywhere said so. The two shapes must be told apart:
# absent is reported, empty stays quiet.
jq -n '{version: 1, skills: {}}' >"$HOME/.agents/custom-skill-lock.json"
run_fork_check
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 23: the drift-watch exited $fork_check_rc on a lock with no forks table: $fork_check_output"
assert_log_line_has "$fork_check_output" 'forks table' 'NO fork upstream is being watched' \
  "case 23: a lock with no forks table watched nothing and reported nothing, which is byte-for-byte what a healthy run with no drift prints: $fork_check_output"
assert_relay_line fork-table-absent 'no forks table' \
  "case 23: a lock with no forks table was not relayed, so a dropped table reaches nobody who is not reading the run log"
refute_match "$fork_check_output" 'does not parse as a JSON object' \
  "case 23: a lock that parses fine is blamed on its JSON, which sends the operator to the wrong repair: $fork_check_output"

# The neighbouring shape, which must stay silent: a table that is PRESENT and
# empty is the deliberate statement, and reporting it would cry wolf every week
# on a machine that watches nothing on purpose.
write_forks_lock '{}'
run_fork_check
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 23: the drift-watch exited $fork_check_rc on an empty forks table: $fork_check_output"
refute_match "$fork_check_output" 'forks table' \
  "case 23: an explicitly EMPTY forks table is reported like a missing one, so the deliberate statement and the mistake are indistinguishable: $fork_check_output"
[[ ! -s $relay_call_log ]] ||
  fail "case 23: an explicitly empty forks table paged the operator: $(cat "$relay_call_log")"

# --- Case 24: a forks table the walk cannot READ is reported ------------------
# The walk sizes itself and feeds itself from the table. When BOTH reads failed
# the two failures cancelled: the size was coerced to 0, the feed yielded no
# keys, and 0 == 0 said the walk owed nothing and finished complete. No fork was
# compared, no incomplete-walk warning fired, and in the weekly flow the run
# went on to stamp the week a success. The reads are simulated by a jq that
# fails the ENUMERATION of the forks table and passes everything else through,
# keyed on the vocabulary any such read has to use rather than on one exact
# program text, so this case keeps its meaning if the read is rewritten.
case24_bin="$scratch_dir/case24-bin"
mkdir -p "$case24_bin"
case24_real_jq="$(command -v jq)"
cat >"$case24_bin/jq" <<EOF
#!/usr/bin/env bash
for case24_arg in "\$@"; do
  case "\$case24_arg" in
    *forks*keys*|*forks*to_entries*|*forks*length*) exit 86 ;;
  esac
done
exec "$case24_real_jq" "\$@"
EOF
chmod +x "$case24_bin/jq"
write_forks_lock "$(jq -n --arg url "$fixture_repo" --arg hash "$current_fixture_hash" \
  '{unreadfork: {source: "fixture/unreadfork", sourceUrl: $url,
    skillPath: "skills/forkskill", lastComparedTreeHash: $hash}}')"

# Control FIRST: the same lock, the same run, without the shim. Without this the
# case cannot tell a reported read failure from a table that was never walkable.
run_fork_check
printf '%s\n' "$fork_check_output" | grep -q 'unreadfork: upstream unchanged' ||
  fail "case 24 control: the fixture was not walked clean without the shim, so the shimmed run below proves nothing: $fork_check_output"

run_fork_check PATH="$case24_bin:$PATH"
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 24: the drift-watch exited $fork_check_rc when the forks table could not be read: $fork_check_output"
assert_log_line_has "$fork_check_output" 'forks table' 'could not be read' \
  "case 24: a forks table the walk could not read produced no warning saying so, so a run that compared nothing is indistinguishable from a run that found no drift: $fork_check_output"
refute_match "$fork_check_output" 'upstream unchanged' \
  "case 24: a fork was reported clean although the table feeding the walk could not be read: $fork_check_output"
assert_relay_line fork-lock-broken 'could not be read' \
  "case 24: a forks table the walk could not read was logged but never relayed, so every upstream went unwatched and only the run log says so"

# --- Case 24b: a feed that stops early is a SHORT walk, and says so -----------
# The other half of the same invariant, and the reason the walk counts at all: a
# feed that truncates (a jq too old for --raw-output0, a read that stops early)
# leaves the walked entries reporting clean while the rest go unwatched, and a
# clean report over a short walk looks exactly like a healthy run. Only the
# count can tell them apart, so the count gets a test.
case24b_bin="$scratch_dir/case24b-bin"
mkdir -p "$case24b_bin"
cat >"$case24b_bin/jq" <<EOF
#!/usr/bin/env bash
for case24b_arg in "\$@"; do
  case "\$case24b_arg" in
    *forks*keys*|*forks*to_entries*)
      # Drop the LAST record of the enumeration, whatever the records are, so
      # the truncation is what a short read looks like without this shim having
      # to know how the read is spelled.
      case24b_records=()
      while IFS= read -r -d '' case24b_record; do
        case24b_records+=("\$case24b_record")
      done < <("$case24_real_jq" "\$@")
      for ((case24b_i = 0; case24b_i < \${#case24b_records[@]} - 1; case24b_i++)); do
        printf '%s\0' "\${case24b_records[case24b_i]}"
      done
      exit 0
      ;;
  esac
done
exec "$case24_real_jq" "\$@"
EOF
chmod +x "$case24b_bin/jq"
write_forks_lock "$(jq -n --arg url "$fixture_repo" --arg hash "$current_fixture_hash" \
  '{afork: {source: "fixture/afork", sourceUrl: $url, skillPath: "skills/forkskill",
      lastComparedTreeHash: $hash},
    zfork: {source: "fixture/zfork", sourceUrl: $url, skillPath: "skills/forkskill",
      lastComparedTreeHash: $hash}}')"
run_fork_check PATH="$case24b_bin:$PATH"
[[ $fork_check_rc -eq 0 ]] ||
  fail "case 24b: the drift-watch exited $fork_check_rc on a truncated feed: $fork_check_output"
assert_log_line_has "$fork_check_output" 'reached the walk' 'of 2' \
  "case 24b: a feed that delivered fewer entries than the table holds reported a complete walk, so the entries it never reached are unwatched and nothing says so: $fork_check_output"
assert_relay_line fork-walk-incomplete 'not drift-checked' \
  "case 24b: a short walk was logged but never relayed"

echo "update-skills-fork-drift: OK (4 baseline assertions + rewrite immunity (global and system), stale skillPath, clone staging and cleanup, 4 malformed tables, malformed entries, dry-run notifies nobody, whole-repo skillPath, failing relay, unparseable lock, unreachable upstream relayed with git's message, 21 mis-typed fields, unstageable clone relayed, newline key, namespaced lock project, distinguishable malformed-entry reasons, absent lock, stalled clone stopped at its deadline, unusable deadline override, absent vs empty forks table, unreadable and truncated table feeds)"
