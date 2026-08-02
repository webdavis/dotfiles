#!/usr/bin/env bash
# ssh-hardening-reload-tree-drift.sh -- --reload refuses to restart onto, and
# refuses to claim success over, a configuration tree that moved under it.
#
# THE DEFECT THIS PINS. --reload reads the configuration many times before it
# restarts anything: the syntax check, then the child verify (which itself
# runs a global resolution, a text scan of the include graph, and two
# per-connection resolutions), then the port resolution. Every one of those is
# a separate open of the tree, so a writer landing between any two of them
# makes the preflight judge a tree that is no longer on disk, and the daemon
# that comes back may serve yet another one.
#
# HOW EACH CASE IS BUILT. The reload library's SSH_TREE_MUTATION_HOOK seam
# runs an executable inside each controlled stub, at the exact moment that
# stub is called, so a case can change the sandbox tree at a named point
# INSIDE one run of the script. Every mutation below is deliberately
# HARDENING-NEUTRAL (a comment line, an inert file, a mode change): the stub's
# `sshd -G` keys its hardened output off the PRESENCE of the drop-in, so a
# mutation that unhardened the tree would be caught by the existing verify and
# the case would go red for the wrong reason, pinning nothing. Every case also
# asserts the mutation actually FIRED, so a hook that silently never matched
# cannot pass as a refusal.
#
# WHAT IS DELIBERATELY NOT CLAIMED. The last window, between the final
# pre-restart observation and the kickstart itself, cannot be closed from
# inside this process. The `window` case below drives a mutation into exactly
# that window and requires the kickstart to HAPPEN, then requires the reload
# to refuse to call it a success. That is the honest guarantee, and asserting
# it here keeps a future change from quietly claiming more.
set -euo pipefail

# Scrubbed at SCRIPT scope. Git exports GIT_DIR to every hook it runs and this
# suite can still inherit one from its caller.
unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

# shellcheck source=../fixtures/ssh-reload-lib.bash
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../fixtures" && pwd)/ssh-reload-lib.bash"

# The reload's retry loop is attempt-bounded through these knobs; two attempts
# with no sleep keeps the failure cases fast.
export SSH_HARDENING_READY_ATTEMPTS=2
export SSH_HARDENING_READY_INTERVAL=0

reload_sandbox_setup
trap 'ssh_sandbox_teardown' EXIT

dropin="$SSHD_CONFIG_D/$SSH_DROPIN_NAME"
write_hardened_dropin

# A tree with real shape, so the drift check is exercised against an include
# graph rather than one directory:
#
#   sshd_config            Include <drop-in dir>/*      (written by the lib)
#     000-ssh-hardening.conf                            the policy
#     010-include-outside.conf   Include <outside>/extra.conf
#       extra.conf                                      OUTSIDE the drop-in dir
#     011-include-glob.conf      Include <globbed>/*.conf
#       first.conf                                      first glob match
#       second.conf                                     SECOND glob match
#     020-inert.conf                                    a removable sibling
#
# extra.conf is what proves the fingerprint follows the graph sshd follows
# rather than listing the two directories the script was pointed at.
#
# The globbed directory is separate from the outside one on purpose. One
# Include pattern must resolve to MORE THAN ONE file, so an implementation that
# records only the first match of a pattern is caught (drift 12); and control C
# needs a directory the graph does NOT reach, which a `*.conf` pattern over the
# outside directory would have destroyed.
OUTSIDE_DIR="$SSH_SANDBOX/outside"
GLOBBED_DIR="$SSH_SANDBOX/globbed"
mkdir -p "$OUTSIDE_DIR" "$GLOBBED_DIR"
OUTSIDE_INCLUDE="$OUTSIDE_DIR/extra.conf"
GLOBBED_FIRST="$GLOBBED_DIR/first.conf"
GLOBBED_SECOND="$GLOBBED_DIR/second.conf"
INERT_DROPIN="$SSHD_CONFIG_D/020-inert.conf"
printf '# inert file pulled in from outside the drop-in directory\n' >"$OUTSIDE_INCLUDE"
printf 'Include %s\n' "$OUTSIDE_INCLUDE" >"$SSHD_CONFIG_D/010-include-outside.conf"
printf '# first file matched by the globbed Include\n' >"$GLOBBED_FIRST"
printf '# second file matched by the globbed Include\n' >"$GLOBBED_SECOND"
printf 'Include %s/*.conf\n' "$GLOBBED_DIR" >"$SSHD_CONFIG_D/011-include-glob.conf"
printf '# inert sibling drop-in\n' >"$INERT_DROPIN"

# observed_tree_fingerprint: every regular file the reload's own walk can
# reach, with its path, mode, owner and checksum. The shared
# config_tree_fingerprint covers the drop-in directory only, so a reload that
# appended to the main config or to an out-of-tree Include target passed its
# "this mode writes nothing" assertion untouched.
observed_tree_fingerprint() {
  local file
  find "$SSHD_CONFIG_D" "$OUTSIDE_DIR" "$GLOBBED_DIR" "$SSHD_MAIN_CONFIG" \
    -type f -print0 | LC_ALL=C sort -z |
    while IFS= read -r -d '' file; do
      printf '%s %s ' "$file" "$(/usr/bin/stat -Lf '%Lp %u %g' -- "$file")"
      /usr/bin/cksum <"$file"
    done
}

# --- the mutation hook -------------------------------------------------------
# Fires once, at the SSH_TREE_MUTATION_OCCURRENCE-th call whose "<tool> <argv>"
# matches SSH_TREE_MUTATION_MATCH, and applies SSH_TREE_MUTATION_ACTION to
# SSH_TREE_MUTATION_TARGET. Records every mutation it COMPLETED to
# SSH_TREE_MUTATION_LOG, and every one it attempted and could not complete to
# SSH_TREE_MUTATION_FAILURE_LOG, so a case can tell "the tree really moved"
# apart from "the injection itself errored", which are opposite verdicts about
# the guard under test and used to be the same empty log.
#
# WHY A CASE-ONLY RENAME IS STAGED THROUGH A THIRD NAME, here and in the
# restore. A single `mv <name> <NAME>` is not portable, measured 2026-08-01 on
# this machine: the flake's `run` shell (which is what CI runs the suite in)
# leads PATH with nix coreutils, so a bare `mv` there is GNU mv 9.7, and GNU mv
# refuses a rename whose source and destination differ only in case on a
# case-insensitive volume. It exits 1 saying the two paths "are the same file"
# and renames nothing, while the host's BSD /bin/mv issues the rename(2). The
# same command therefore moved the tree or did not depending on which `mv` the
# PATH led to, which is a test whose verdict is decided by its environment.
#
# Neither leg of a staged rename is a case-only rename, so every `mv` performs
# both, on a case-sensitive volume and on a case-insensitive one. The inode and
# the bytes survive the pair (measured), so the spelling of the path is still
# the only thing that moves. The staging name is transient and exists only
# while the seam call that fired the hook is blocked in its stub, so no
# observation of the tree can fall between the two legs.
#
# APFS is case-INSENSITIVE but case-PRESERVING, so after the rename the
# directory entry carries the new spelling and the walk's re-glob returns it.
# That is what makes the drift-14 case a real, observable tree change on macOS
# rather than a no-op, and it is why the case is not gated on the filesystem.
CASE_ONLY_RENAME_STAGING_SUFFIX='.case-only-rename-staging'
export CASE_ONLY_RENAME_STAGING_SUFFIX

# rename_case_only <from> <to>: a rename whose paths differ only in case,
# staged so that no `mv` implementation can refuse it. See the measurement
# above. The hook is a separate program and carries its own copy of this.
rename_case_only() {
  local staged="$1$CASE_ONLY_RENAME_STAGING_SUFFIX"
  mv -- "$1" "$staged" && mv -- "$staged" "$2"
}

SSH_TREE_MUTATION_MATCH=''
SSH_TREE_MUTATION_OCCURRENCE=1
SSH_TREE_MUTATION_ACTION=''
SSH_TREE_MUTATION_TARGET=''
SSH_TREE_MUTATION_LOG="$SSH_SANDBOX/tree-mutation.log"
SSH_TREE_MUTATION_FAILURE_LOG="$SSH_SANDBOX/tree-mutation-failure.log"
export SSH_TREE_MUTATION_MATCH SSH_TREE_MUTATION_OCCURRENCE \
  SSH_TREE_MUTATION_ACTION SSH_TREE_MUTATION_TARGET SSH_TREE_MUTATION_LOG \
  SSH_TREE_MUTATION_FAILURE_LOG
: >"$SSH_TREE_MUTATION_LOG"
: >"$SSH_TREE_MUTATION_FAILURE_LOG"

MUTATION_HOOK="$SSH_SANDBOX/tree-mutation-hook"
cat >"$MUTATION_HOOK" <<'HOOK'
#!/bin/bash
set -uo pipefail
description="$*"
# shellcheck disable=SC2053  # the right-hand side is a PATTERN on purpose:
# each case names the seam call it wants by glob, not by literal argv.
if [[ $description != ${SSH_TREE_MUTATION_MATCH:-} ]]; then
  exit 0
fi
count_file="${SSH_STUB_STATE:?}/tree-mutation-count"
count="$(cat "$count_file" 2>/dev/null || printf '0')"
count=$((count + 1))
printf '%s' "$count" >"$count_file"
if [[ $count -ne ${SSH_TREE_MUTATION_OCCURRENCE:-1} ]]; then
  exit 0
fi
target="${SSH_TREE_MUTATION_TARGET:?}"
# Every branch below records its own exit status instead of letting it fall on
# the floor. `set -e` is deliberately NOT on here (a hook that aborted mid-run
# would leave the sandbox half-mutated), so without this an action that FAILED
# ran straight into the success log and every case asserting "the mutation
# fired" passed over a tree that had not moved. That is the exact shape of a
# test that cannot fail, and it is how a rename that GNU mv refused outright
# read as "the drift guard missed a case-only rename".
#
# action_failure_detail carries what a bare exit status cannot say, for the one
# way an action can report success and still not have moved the tree.
action_status=0
action_failure_detail=''
case "${SSH_TREE_MUTATION_ACTION:?}" in
  append-comment)
    printf '# drift injected at: %s\n' "$description" >>"$target" || action_status=$?
    ;;
  create-file)
    printf '# file created by drift injection at: %s\n' "$description" >"$target" ||
      action_status=$?
    ;;
  remove-file)
    rm -f -- "$target" || action_status=$?
    ;;
  make-world-writable)
    # `--` BEFORE the mode: BSD chmod reads a `--` after the mode as a file
    # operand and fails.
    chmod -- 0666 "$target" || action_status=$?
    ;;
  make-unreadable)
    chmod -- 0000 "$target" || action_status=$?
    ;;
  rewrite-same-length)
    # Different bytes, IDENTICAL byte count. An observation that kept only the
    # byte-count half of `cksum`'s output would call this unchanged, and every
    # other content mutation here appends, so nothing else catches it.
    # The `printf X`/strip pair preserves trailing newlines that command
    # substitution would otherwise eat, and the write is IN PLACE so the mode
    # and the owner stay exactly what they were.
    {
      rewritten="$(
        LC_ALL=C tr 'ie' 'oa' <"$target"
        printf 'X'
      )" && printf '%s' "${rewritten%X}" >"$target"
    } || action_status=$?
    ;;
  rename-to-uppercase)
    # A rename that differs from the original ONLY in case. The comparison
    # turns nocasematch off for exactly this: left on (it is on at the script's
    # file scope), the two paths compare equal and the rename reads as no
    # change at all.
    #
    # Staged through a third name, never a single `mv`: this hook runs with
    # whatever PATH the suite was launched under, and a single case-only `mv`
    # succeeds or refuses depending on which implementation that PATH leads to.
    # The full measurement is recorded beside the staging-suffix constant in
    # the test that generates this hook.
    uppercased="${target%/*}/$(printf '%s' "${target##*/}" | LC_ALL=C tr '[:lower:]' '[:upper:]')"
    staged="$target${CASE_ONLY_RENAME_STAGING_SUFFIX:?}"
    { mv -- "$target" "$staged" && mv -- "$staged" "$uppercased"; } || action_status=$?
    # Two successful renames are not the same as a re-spelled directory entry.
    # A volume that FOLDS case rather than preserving it hands back the old
    # spelling with both renames reporting success, and this action would then
    # log a mutation over a tree no observer can tell apart from the one before
    # it. That reads as "the drift guard missed a case-only rename", which is
    # the same wrong verdict this hook used to give for a refused rename, so
    # the entry is re-read and disagreement is reported as a failed injection.
    if [[ $action_status -eq 0 ]] &&
      [[ -z $(/usr/bin/find "${uppercased%/*}" -maxdepth 1 -name "${uppercased##*/}" -print -quit) ]]; then
      action_status=1
      action_failure_detail=', because both renames reported success and the directory still does not hold that spelling: this volume folds case instead of preserving it, so a case-only rename is not observable here'
    fi
    ;;
  *)
    printf 'tree-mutation-hook: unknown action %s\n' "$SSH_TREE_MUTATION_ACTION" >&2
    exit 70
    ;;
esac
if [[ $action_status -ne 0 ]]; then
  printf 'tree-mutation-hook: %s on %s exited %s, so the tree was NOT mutated%s\n' \
    "$SSH_TREE_MUTATION_ACTION" "$target" "$action_status" "$action_failure_detail" >&2
  printf '%s on %s exited %s%s\n' \
    "$SSH_TREE_MUTATION_ACTION" "$target" "$action_status" "$action_failure_detail" \
    >>"${SSH_TREE_MUTATION_FAILURE_LOG:?}"
  exit "$action_status"
fi
printf '%s %s %s\n' "${SSH_TREE_MUTATION_ACTION}" "$target" "$description" \
  >>"${SSH_TREE_MUTATION_LOG:?}"
HOOK
chmod +x "$MUTATION_HOOK"

# run_reload_with_mutation <match> <occurrence> <action> <target>: one --reload
# run with the hook armed at exactly one seam call.
run_reload_with_mutation() {
  : >"$SSH_TREE_MUTATION_LOG"
  : >"$SSH_TREE_MUTATION_FAILURE_LOG"
  SSH_TREE_MUTATION_HOOK="$MUTATION_HOOK" \
    SSH_TREE_MUTATION_MATCH="$1" \
    SSH_TREE_MUTATION_OCCURRENCE="$2" \
    SSH_TREE_MUTATION_ACTION="$3" \
    SSH_TREE_MUTATION_TARGET="$4" \
    run_ssh_reload --reload
}

# assert_mutation_fired <label>: the injection COMPLETED, so the case that
# follows is judging a tree that really moved. The two ways it can not have
# completed are reported apart, because they accuse opposite things: a hook
# that never matched its seam call is a broken case, while a hook that matched
# and then errored is a broken injection tool, and reporting either as "never
# fired" sent the last round hunting a drift-detection bug that did not exist.
assert_mutation_fired() {
  local reason
  if [[ -s $SSH_TREE_MUTATION_LOG ]]; then
    return 0
  fi
  reason='the hook never matched the seam call it was armed for'
  if [[ -s $SSH_TREE_MUTATION_FAILURE_LOG ]]; then
    reason="the hook matched its seam call but the injection failed: $(cat "$SSH_TREE_MUTATION_FAILURE_LOG")"
  fi
  fail "$1: the injected mutation never landed, so this case proves nothing about drift detection ($reason)"
}

# assert_drift_refused_before_kickstart <label> <path the mutation touched>:
# the reload refused, nothing was restarted, the refusal names the tree change
# AND the file that moved, and it states that sshd was left alone.
assert_drift_refused_before_kickstart() {
  local label="$1" moved="$2"
  assert_mutation_fired "$label"
  [[ $SSH_RUN_STATUS -ne 0 ]] ||
    fail "$label: a tree that changed during the preflight must fail the reload (stdout: $SSH_RUN_OUT)"
  assert_no_kickstart "$label"
  grep -qi 'configuration tree CHANGED' <<<"$SSH_RUN_ERR" ||
    fail "$label: the refusal must say the configuration tree changed (stderr: $SSH_RUN_ERR)"
  grep -qF -- "$moved" <<<"$SSH_RUN_ERR" ||
    fail "$label: the refusal must name the file that moved, '$moved' (stderr: $SSH_RUN_ERR)"
  grep -qi 'sshd was not touched' <<<"$SSH_RUN_ERR" ||
    fail "$label: a refusal before the disruptive step must say sshd was not touched (stderr: $SSH_RUN_ERR)"
  refute_contains "$SSH_RUN_OUT" 'reload complete' \
    "$label: a refused reload must never print the success line"
}

# assert_drift_refuses_the_success_claim <label> <path the mutation touched>:
# the kickstart DID happen, and the reload still refuses to call it a success,
# names what moved, hands over the recovery path, and says nothing was rolled
# back.
assert_drift_refuses_the_success_claim() {
  local label="$1" moved="$2" phrase
  assert_mutation_fired "$label"
  [[ $SSH_RUN_STATUS -ne 0 ]] ||
    fail "$label: a tree that changed under the restart must not produce a success (stdout: $SSH_RUN_OUT)"
  assert_kickstart_attempted "$label"
  grep -qi 'configuration tree CHANGED' <<<"$SSH_RUN_ERR" ||
    fail "$label: the failure must say the configuration tree changed (stderr: $SSH_RUN_ERR)"
  grep -qF -- "$moved" <<<"$SSH_RUN_ERR" ||
    fail "$label: the failure must name the file that moved, '$moved' (stderr: $SSH_RUN_ERR)"
  grep -qi 'nothing was rolled back' <<<"$SSH_RUN_ERR" ||
    fail "$label: the failure must state that nothing was rolled back (stderr: $SSH_RUN_ERR)"
  for phrase in 'ssh-hardening.sh --rollback' 'Screen Sharing over the tailnet'; do
    grep -qF -- "$phrase" <<<"$SSH_RUN_ERR" ||
      fail "$label: a failure after the disruptive step must carry the recovery instruction '$phrase' (stderr: $SSH_RUN_ERR)"
  done
  refute_contains "$SSH_RUN_OUT" 'reload complete' \
    "$label: a reload whose tree moved must never print the success line"
  refute_contains "$SSH_RUN_ERR" 'sshd was not touched' \
    "$label: the failure must not claim sshd was untouched; the kickstart already ran"
}

# --- control A: the unmutated happy path still reloads ------------------------
# Run FIRST and asserted in full. A drift guard that refuses everything would
# make every case below pass while breaking the mode outright.

run_ssh_reload --reload
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "control A: an unchanged tree must still reload cleanly (stderr: $SSH_RUN_ERR)"
assert_kickstart_attempted 'control A'
grep -qi 'accepting connections' <<<"$SSH_RUN_OUT" ||
  fail "control A: the success line must survive (stdout: $SSH_RUN_OUT)"
# The success line states what was MEASURED, and says so. Two observations that
# compare equal establish that each file read identically at the two moments it
# was read; they do not establish that the tree was ever, at one instant, the
# thing the preflight judged, and they cannot see the instant the daemon read
# it. A sentence that claims the tree IS what was validated claims an
# unobservable, which is the difference between a check and a guarantee.
grep -qi 'each time this run read it' <<<"$SSH_RUN_OUT" ||
  fail "control A: the success line must say what was measured, not assert the tree IS what was validated (stdout: $SSH_RUN_OUT)"
grep -qi 'not observable from here' <<<"$SSH_RUN_OUT" ||
  fail "control A: the success line must state that what the daemon read is unobservable (stdout: $SSH_RUN_OUT)"
baseline_fingerprint="$(config_tree_fingerprint)"
baseline_observed_fingerprint="$(observed_tree_fingerprint)"

# --- control B: the confirmed-absent service is still a clean no-op -----------
# The drift recheck must sit AFTER the service probe's early return: put it
# before, and Remote Login being off turns from a documented exit-0 no-op into
# a refusal, which is a false positive arriving through placement.

LAUNCHCTL_STUB_PRINT_STATUSES=113 run_ssh_reload --reload
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "control B: a confirmed-absent service must stay a clean no-op, got exit $SSH_RUN_STATUS (stderr: $SSH_RUN_ERR)"
assert_no_kickstart 'control B'
grep -qi 'Remote Login' <<<"$SSH_RUN_OUT" ||
  fail "control B: the no-op must still explain the service follows Remote Login (stdout: $SSH_RUN_OUT)"

# --- control D: drift on the confirmed-absent path is still a clean no-op -----
# The recheck must sit AFTER the service probe's early return, and only a
# DRIFTING tree can tell the two placements apart: with Remote Login off
# nothing is restarted and nothing is claimed about a running daemon, so a
# moved tree is not a reason to fail. Put the recheck before the probe and this
# case turns a spec-mandated exit 0 into a refusal. The mutation lands at the
# port resolution, which is strictly before the probe, so it is already on disk
# whichever side of the probe the recheck sits on.

LAUNCHCTL_STUB_PRINT_STATUSES=113 \
  run_reload_with_mutation 'sshd -G -f *' 2 append-comment "$dropin"
assert_mutation_fired 'control D'
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "control D: a confirmed-absent service must stay a clean no-op even when the tree moved, got exit $SSH_RUN_STATUS (stderr: $SSH_RUN_ERR)"
assert_no_kickstart 'control D'
grep -qi 'Remote Login' <<<"$SSH_RUN_OUT" ||
  fail "control D: the no-op must still explain the service follows Remote Login (stdout: $SSH_RUN_OUT)"
# The exit status and the silence toward launchd are the spec. The SENTENCE is
# not: "the installed drop-in applies when Remote Login is next enabled" is a
# claim about the tree on disk, and the checks that judged that tree ran before
# the mutation landed. Claiming it anyway is a success claim over validation
# that is known to be stale, which is the same defect the pre-kickstart guard
# exists to prevent, arriving on the one path that does not kickstart.
refute_contains "$SSH_RUN_OUT$SSH_RUN_ERR" 'applies when Remote Login is next enabled' \
  'control D: with the tree moved, nothing may be claimed about what the drop-in on disk will do later'
grep -qF -- "$dropin" <<<"$SSH_RUN_ERR" ||
  fail "control D: the operator must still be told WHICH file moved (stderr: $SSH_RUN_ERR)"
write_hardened_dropin

# --- control E: the confirmed-absent path still makes its claim when nothing --
# --- moved --------------------------------------------------------------------
# The other half of control D. Withholding the sentence must be caused by the
# DRIFT, not by the code path: an implementation that simply deleted the claim
# would pass control D and lose the one thing this exit tells the operator.

LAUNCHCTL_STUB_PRINT_STATUSES=113 run_ssh_reload --reload
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "control E: a confirmed-absent service must stay a clean no-op (stderr: $SSH_RUN_ERR)"
assert_no_kickstart 'control E'
grep -qF -- 'applies when Remote Login is next enabled' <<<"$SSH_RUN_OUT" ||
  fail "control E: with an unchanged tree the no-op must still say the installed drop-in applies later (stdout: $SSH_RUN_OUT)"

# --- control C: a write OUTSIDE the resolved graph is not drift ---------------
# The guard's subject is the tree sshd reads, not the filesystem. A new file in
# a directory no Include names must not refuse a reload; if it did, any
# unrelated tool touching a neighbouring path could block hardening.

run_reload_with_mutation 'launchctl print *' 1 create-file "$OUTSIDE_DIR/unreferenced.conf"
assert_mutation_fired 'control C'
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "control C: a file created outside the resolved include graph is not drift (stderr: $SSH_RUN_ERR)"
assert_kickstart_attempted 'control C'
rm -f "$OUTSIDE_DIR/unreferenced.conf"

# --- drift 1: between the fingerprint and the syntax check --------------------

run_reload_with_mutation 'sshd -t *' 1 append-comment "$dropin"
assert_drift_refused_before_kickstart 'drift 1 (at the syntax check)' "$dropin"
write_hardened_dropin

# --- drift 2: between the syntax check and the verify -------------------------

run_reload_with_mutation 'sshd -G -f *' 1 append-comment "$dropin"
assert_drift_refused_before_kickstart 'drift 2 (at the verify global check)' "$dropin"
write_hardened_dropin

# --- drift 3: INSIDE the verify, between its global check and its specs -------
# The verify is not one read of the tree: it resolves globally, scans the
# include graph, and resolves two connection specs, each a separate open. A
# mutation landing between them makes the verify itself internally
# inconsistent, and only a fingerprint spanning the whole preflight sees it.

run_reload_with_mutation 'sshd -G -T -C user=root*' 1 append-comment "$dropin"
assert_drift_refused_before_kickstart 'drift 3 (inside the verify)' "$dropin"
write_hardened_dropin

# --- drift 4: between the verify and the port resolution ----------------------

run_reload_with_mutation 'sshd -G -f *' 2 append-comment "$dropin"
assert_drift_refused_before_kickstart 'drift 4 (at the port resolution)' "$dropin"
write_hardened_dropin

# --- drift 5: between the port resolution and the service probe ---------------

run_reload_with_mutation 'launchctl print *' 1 append-comment "$dropin"
assert_drift_refused_before_kickstart 'drift 5 (at the service probe)' "$dropin"
write_hardened_dropin

# --- drift 6: the main config itself moves ------------------------------------

run_reload_with_mutation 'launchctl print *' 1 append-comment "$SSHD_MAIN_CONFIG"
assert_drift_refused_before_kickstart 'drift 6 (the main config)' "$SSHD_MAIN_CONFIG"
printf 'Include %s/*\n' "$SSHD_CONFIG_D" >"$SSHD_MAIN_CONFIG"

# --- drift 7: a file the graph reaches only through an out-of-tree Include ----
# The old scan globbed the main config plus the drop-in directory. sshd does
# not stop there and neither may the fingerprint: extra.conf lives in another
# directory entirely and is reached only by following an Include.

run_reload_with_mutation 'launchctl print *' 1 append-comment "$OUTSIDE_INCLUDE"
assert_drift_refused_before_kickstart 'drift 7 (an out-of-tree Include target)' "$OUTSIDE_INCLUDE"
printf '# inert file pulled in from outside the drop-in directory\n' >"$OUTSIDE_INCLUDE"

# --- drift 8: the SET grows while every surviving file is byte-identical ------

# The file name deliberately carries NO form of the word this case greps for.
# Named 030-appeared.conf, the refusal's own interpolation of the path satisfied
# `grep -i appeared` whatever verb the message used, and an implementation that
# reported a new file as merely "moved" passed.
new_dropin="$SSHD_CONFIG_D/030-new.conf"
run_reload_with_mutation 'launchctl print *' 1 create-file "$new_dropin"
assert_drift_refused_before_kickstart 'drift 8 (a file appeared)' "$new_dropin"
grep -qi 'appeared' <<<"$SSH_RUN_ERR" ||
  fail "drift 8: the refusal must say the file APPEARED, not merely that something changed (stderr: $SSH_RUN_ERR)"
rm -f "$new_dropin"

# --- drift 9: the SET shrinks while every surviving file is byte-identical ----

run_reload_with_mutation 'launchctl print *' 1 remove-file "$INERT_DROPIN"
assert_drift_refused_before_kickstart 'drift 9 (a file disappeared)' "$INERT_DROPIN"
grep -qi 'disappeared' <<<"$SSH_RUN_ERR" ||
  fail "drift 9: the refusal must say the file DISAPPEARED, not merely that something changed (stderr: $SSH_RUN_ERR)"
printf '# inert sibling drop-in\n' >"$INERT_DROPIN"

# --- drift 10: identical bytes at a different mode ----------------------------
# Same content is not the same proposition: the install path already reasons
# about mode (a root-owned 0600 drop-in makes unprivileged `sshd -G` fail), and
# a world-writable include is a file anyone can rewrite a moment later.

run_reload_with_mutation 'launchctl print *' 1 make-world-writable "$INERT_DROPIN"
assert_drift_refused_before_kickstart 'drift 10 (mode changed, content identical)' "$INERT_DROPIN"
grep -qi 'mode or owner' <<<"$SSH_RUN_ERR" ||
  fail "drift 10: the refusal must name the dimension that moved, so an operator diffing the bytes is not left confused (stderr: $SSH_RUN_ERR)"
chmod -- 0644 "$INERT_DROPIN"

# --- drift 11: the content changes without the LENGTH changing ----------------
# Every other content case here appends, so an observation that recorded only
# the byte-count half of `cksum`'s output would pass all of them. This one
# rewrites the same number of bytes.

run_reload_with_mutation 'launchctl print *' 1 rewrite-same-length "$INERT_DROPIN"
assert_drift_refused_before_kickstart 'drift 11 (same length, different bytes)' "$INERT_DROPIN"
grep -qi 'content' <<<"$SSH_RUN_ERR" ||
  fail "drift 11: the refusal must name content as the dimension that moved (stderr: $SSH_RUN_ERR)"
printf '# inert sibling drop-in\n' >"$INERT_DROPIN"

# --- drift 12: the SECOND file one Include pattern resolves to moves ----------
# One Include line can name a pattern that matches several files, and sshd
# reads all of them. An observation that recorded only the first match of each
# pattern would watch first.conf and never see second.conf move.

run_reload_with_mutation 'launchctl print *' 1 append-comment "$GLOBBED_SECOND"
assert_drift_refused_before_kickstart 'drift 12 (the second match of a globbed Include)' "$GLOBBED_SECOND"
printf '# second file matched by the globbed Include\n' >"$GLOBBED_SECOND"

# --- drift 13: a SYMLINKED include changes mode at its target -----------------
# `[[ -f ]]` and the content read both follow a symlink, so the file sshd opens
# is the target. An observation that stat()s the link instead of the target
# records the link's own mode (a constant 0755 on macOS) and the whole
# mode-and-owner dimension silently stops meaning anything.

LINKED_TARGET="$OUTSIDE_DIR/linked-target.conf"
LINKED_DROPIN="$SSHD_CONFIG_D/012-link.conf"
printf '# a drop-in reached through a symlink\n' >"$LINKED_TARGET"
chmod -- 0644 "$LINKED_TARGET"
ln -s "$LINKED_TARGET" "$LINKED_DROPIN"
# False-positive direction FIRST: a symlinked include that has not moved must
# still reload. Reading the LINK's own metadata instead of the target's is one
# way to get drift 13 wrong; refusing every symlink is the other, and it would
# make the guard unusable on a tree that uses one.
run_ssh_reload --reload
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "drift 13: an unchanged tree containing a symlinked include must still reload (stderr: $SSH_RUN_ERR)"
assert_kickstart_attempted 'drift 13 (unchanged symlink)'
run_reload_with_mutation 'launchctl print *' 1 make-world-writable "$LINKED_TARGET"
assert_drift_refused_before_kickstart 'drift 13 (mode of a symlinked include target)' "$LINKED_DROPIN"
grep -qi 'mode or owner' <<<"$SSH_RUN_ERR" ||
  fail "drift 13: the refusal must name the mode dimension, which means the observation followed the link (stderr: $SSH_RUN_ERR)"
rm -f "$LINKED_DROPIN" "$LINKED_TARGET"

# --- drift 14: a rename that differs only in CASE -----------------------------
# The comparison turns nocasematch off deliberately (it is on at the script's
# file scope so keyword matching mirrors sshd). Left on, the old and new paths
# compare equal and a rename reads as no change at all. The rename keeps the
# same inode and the same bytes, so the case of the path is the ONLY thing that
# moved.
#
# This case is meaningful on a case-INSENSITIVE volume, which is what macOS
# ships, and not only on a case-sensitive one: APFS is case-preserving, so the
# directory entry carries the new spelling and the walk's re-glob of the
# drop-in directory returns it (measured 2026-08-01). The old spelling still
# resolves through lookup, which is exactly why the case-sensitive PATH
# comparison, not the ability to open the file, is what catches this.

# The name the hook's rename-to-uppercase action produces, derived the way that
# action derives it, so the restore cannot drift from the injection.
INERT_DROPIN_UPPERCASED="${INERT_DROPIN%/*}/$(printf '%s' "${INERT_DROPIN##*/}" | LC_ALL=C tr '[:lower:]' '[:upper:]')"

# directory_holds_entry_spelled_exactly <path>: does <path>'s directory hold an
# entry spelled EXACTLY like <path>'s last component? `[[ -e ]]` cannot answer
# that on a case-insensitive volume, where both spellings resolve to the same
# file whichever one the directory actually holds, and `[[ == ]]` cannot either
# while nocasematch is on at this script's file scope. `find -name` matches
# against the entry the directory returns, case-sensitively, and answers the
# question that is actually being asked.
#
# The last component is handed to `find` as a PATTERN, which is exact for the
# two fixed drop-in names below and would need escaping for any name carrying
# glob metacharacters. Nothing here builds such a name.
directory_holds_entry_spelled_exactly() {
  [[ -n $(/usr/bin/find "${1%/*}" -maxdepth 1 -name "${1##*/}" -print -quit) ]]
}

# restore_inert_dropin_spelling: put the inert drop-in back under its original
# spelling. Every case after this one addresses that file by its lowercase
# path, and the closing "this mode writes nothing" comparison is against a
# fingerprint that RECORDS each path, so an uppercase entry left behind fails a
# later case instead of this one.
#
# Idempotent, because the EXIT trap below runs it on paths where the injection
# never landed. Nothing about it assumes the assertions passed.
restore_inert_dropin_spelling() {
  directory_holds_entry_spelled_exactly "$INERT_DROPIN_UPPERCASED" || return 0
  rename_case_only "$INERT_DROPIN_UPPERCASED" "$INERT_DROPIN"
}

# The restore is TRAPPED for the length of this case, not merely sequenced
# after its assertions: `fail` exits, so a restore that only runs on the
# success path is one that does not run on any path that needs it. The trap
# keeps the sandbox teardown reachable by reporting a failed restore instead of
# aborting on it.
trap 'restore_inert_dropin_spelling || printf "WARN: drift 14: the drop-in spelling could not be restored\n" >&2; ssh_sandbox_teardown' EXIT
run_reload_with_mutation 'launchctl print *' 1 rename-to-uppercase "$INERT_DROPIN"
assert_mutation_fired 'drift 14'
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail "drift 14: a path that changed only in case must still count as drift (stdout: $SSH_RUN_OUT)"
assert_no_kickstart 'drift 14'
grep -qi 'configuration tree CHANGED' <<<"$SSH_RUN_ERR" ||
  fail "drift 14: the refusal must say the configuration tree changed (stderr: $SSH_RUN_ERR)"
# Case-SENSITIVE on purpose (no -i): naming the new spelling is what proves the
# second observation re-read the directory and saw the entry the rename wrote,
# rather than reusing the listing the first one took.
grep -q -- "${INERT_DROPIN_UPPERCASED##*/}" <<<"$SSH_RUN_ERR" ||
  fail "drift 14: the refusal must name the new spelling '${INERT_DROPIN_UPPERCASED##*/}' (stderr: $SSH_RUN_ERR)"
restore_inert_dropin_spelling ||
  fail "drift 14: the drop-in spelling must be restorable; every later case addresses it by its lowercase path"
trap 'ssh_sandbox_teardown' EXIT
directory_holds_entry_spelled_exactly "$INERT_DROPIN" ||
  fail "drift 14: the restore must leave the drop-in directory holding '${INERT_DROPIN##*/}', so the cases below judge the tree they were written against"

# --- window: the one gap that cannot be closed from inside this process -------
# A mutation handed to the privilege wrapper along with the kickstart lands
# after the last possible pre-restart observation. The restart therefore
# HAPPENS. What must not happen is a success claim over a tree nobody checked.

run_reload_with_mutation 'sudo *launchctl kickstart -k *' 1 append-comment "$dropin"
assert_drift_refuses_the_success_claim 'window (between the last check and the kickstart)' "$dropin"
write_hardened_dropin

# --- post: the tree moves after the daemon is already back --------------------

run_reload_with_mutation 'ssh-keyscan *' 1 append-comment "$dropin"
assert_drift_refuses_the_success_claim 'post (after the kickstart)' "$dropin"
write_hardened_dropin

# --- post ordering: a silent listener outranks a moved tree -------------------
# Drift makes a success claim wrong. A listener that never answers is a
# possible LOCKOUT. The operator needs the lockout first, so the readiness
# proof must be judged BEFORE the post-restart comparison.
#
# The mutation rides the kickstart, so the tree has already moved by the time
# the readiness loop starts: both failures are available at once and the
# reported one is decided purely by the order of the two checks.

KEYSCAN_STUB_MODE=refuse
run_reload_with_mutation 'sudo *launchctl kickstart -k *' 1 append-comment "$dropin"
# shellcheck disable=SC2034  # exported by reload_sandbox_setup and read by the
# ssh-keyscan stub in a child process, so the restore has no in-file reader
KEYSCAN_STUB_MODE=banner
assert_mutation_fired 'post ordering'
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail 'post ordering: a listener that never answers must still fail the reload'
grep -qi 'possible lockout' <<<"$SSH_RUN_ERR" ||
  fail "post ordering: the lockout must be reported, not displaced by the drift message (stderr: $SSH_RUN_ERR)"
write_hardened_dropin

# --- fifo: a named pipe in the drop-in directory must never be read -----------
# `cksum < fifo` never returns, and a fingerprint that hangs AFTER the kickstart
# is strictly worse than the race it was added to close. Termination is
# asserted by run_ssh_reload's wall clock, which aborts the suite on a hang.

fifo_path="$SSHD_CONFIG_D/040-fifo.conf"
mkfifo "$fifo_path"
run_ssh_reload --reload
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "fifo: a named pipe is not a regular file and must simply be skipped, got exit $SSH_RUN_STATUS (stderr: $SSH_RUN_ERR)"
assert_kickstart_attempted 'fifo'
rm -f "$fifo_path"

# --- unreadable mid-run, before the kickstart ---------------------------------
# The drift comparison is only ever reached when an observation succeeded. The
# branch that fires when one FAILS is a separate refusal, and it is the one
# that decides whether an unobservable tree reads as a refusal or as "no
# change". This drives it at the last pre-restart observation.

run_reload_with_mutation 'launchctl print *' 1 make-unreadable "$INERT_DROPIN"
assert_mutation_fired 'observe fails before the kickstart'
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail "observe fails before the kickstart: an unobservable tree must refuse, not pass as unchanged (stdout: $SSH_RUN_OUT)"
assert_no_kickstart 'observe fails before the kickstart'
grep -qi 'could not be re-read before the restart' <<<"$SSH_RUN_ERR" ||
  fail "observe fails before the kickstart: the refusal must name the failed re-read (stderr: $SSH_RUN_ERR)"
grep -qi 'sshd was not touched' <<<"$SSH_RUN_ERR" ||
  fail "observe fails before the kickstart: the refusal must say sshd was not touched (stderr: $SSH_RUN_ERR)"
chmod -- 0644 "$INERT_DROPIN"

# --- unreadable mid-run, after the kickstart ----------------------------------
# Same branch at step 12, where the restart has already happened. The refusal
# must therefore refuse the SUCCESS CLAIM, keep the recovery path, and never
# claim sshd was left alone.

run_reload_with_mutation 'ssh-keyscan *' 1 make-unreadable "$INERT_DROPIN"
assert_mutation_fired 'observe fails after the kickstart'
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail "observe fails after the kickstart: an unobservable tree must not produce a success (stdout: $SSH_RUN_OUT)"
assert_kickstart_attempted 'observe fails after the kickstart'
grep -qi 'could not be re-read afterwards' <<<"$SSH_RUN_ERR" ||
  fail "observe fails after the kickstart: the failure must name the failed re-read (stderr: $SSH_RUN_ERR)"
grep -qi 'nothing was rolled back' <<<"$SSH_RUN_ERR" ||
  fail "observe fails after the kickstart: the failure must state that nothing was rolled back (stderr: $SSH_RUN_ERR)"
refute_contains "$SSH_RUN_OUT" 'reload complete' \
  'observe fails after the kickstart: no success line may be printed'
refute_contains "$SSH_RUN_ERR" 'sshd was not touched' \
  'observe fails after the kickstart: the kickstart already ran, so sshd was touched'
chmod -- 0644 "$INERT_DROPIN"

# --- an Include cycle refuses BEFORE anything is judged -----------------------
# The walk's cycle guard is what stops a self-including tree from spinning. It
# is reached at the FIRST observation, before the syntax check, so it must
# refuse there with nothing disturbed.

# The file name carries NO form of the word this case greps for: the refusal
# interpolates the full path, so a fixture called 050-cycle.conf would satisfy
# `grep -i cycle` whatever the message said.
cycle_dropin="$SSHD_CONFIG_D/050-self.conf"
printf 'Include %s\n' "$cycle_dropin" >"$cycle_dropin"
run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail 'cycle: a self-including tree must refuse, not be walked forever or judged partially'
assert_no_kickstart 'cycle'
grep -qi 'cycle' <<<"$SSH_RUN_ERR" ||
  fail "cycle: the refusal must name the cycle (stderr: $SSH_RUN_ERR)"
rm -f "$cycle_dropin"

# --- a tree wider than the bound refuses, and says so -------------------------
# The width bound is the only observation failure with no second gate behind
# it: an unreadable file and a cycle both fail the verify independently, a tree
# of 100000 files does not. Raise the constant and nothing else in the suite
# notices.

bulk_dir_marker="$SSHD_CONFIG_D/900-bulk"
bulk_index=0
while [[ $bulk_index -lt 300 ]]; do
  printf '# bulk %s\n' "$bulk_index" >"$bulk_dir_marker-$bulk_index.conf"
  bulk_index=$((bulk_index + 1))
done
run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail 'width bound: a tree naming more files than the bound must refuse, not be walked in full'
assert_no_kickstart 'width bound'
grep -qi 'more than 512 files' <<<"$SSH_RUN_ERR" ||
  fail "width bound: the refusal must name the bound it hit (stderr: $SSH_RUN_ERR)"
grep -qi 'sshd was not touched' <<<"$SSH_RUN_ERR" ||
  fail "width bound: the refusal must say sshd was not touched (stderr: $SSH_RUN_ERR)"
bulk_index=0
while [[ $bulk_index -lt 300 ]]; do
  rm -f "$bulk_dir_marker-$bulk_index.conf"
  bulk_index=$((bulk_index + 1))
done

# --- a tree bigger than the byte bound refuses, and says so -------------------
# The file COUNT is the wrong axis on its own: the walk's cost is a per-
# character bash tokenizer over every line, and one oversized file matched by
# the stock `Include <dir>/*` is enough. Review measured 6.0 s per observation
# at 1.1 MB and 45.8 s at 9.1 MB, three observations per reload, one of them
# after the restart -- a silent stall exactly where the operator is watching for
# a lockout. This file is a little over the bound, so the walk refuses partway
# through it rather than reading it all.

oversized_dropin="$SSHD_CONFIG_D/910-oversized.conf"
: >"$oversized_dropin"
oversized_line=0
while [[ $oversized_line -lt 4200 ]]; do
  printf '# %064d padding to take the tree past the byte bound\n' "$oversized_line" \
    >>"$oversized_dropin"
  oversized_line=$((oversized_line + 1))
done
run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail 'byte bound: a tree holding more bytes than the bound must refuse, not be read in full'
assert_no_kickstart 'byte bound'
grep -qi 'more than 262144 bytes' <<<"$SSH_RUN_ERR" ||
  fail "byte bound: the refusal must name the bound it hit (stderr: $SSH_RUN_ERR)"
grep -qi 'sshd was not touched' <<<"$SSH_RUN_ERR" ||
  fail "byte bound: the refusal must say sshd was not touched (stderr: $SSH_RUN_ERR)"
rm -f "$oversized_dropin"

# --- a path the record format cannot carry refuses, by name -------------------
# The record packs three fields into one line with a separator between them. A
# file name containing that separator is legal on macOS, and it does not make
# the record unreadable in a way anyone notices: it makes two DIFFERENT files
# parse to the SAME path, so an untouched tree compares unequal to itself and
# the reload refuses claiming the tree CHANGED. A false accusation is not a
# fail-closed outcome, it is a wrong one; the honest answer names the file and
# the reason.

separator_a="$(printf '%s/013-a\037b.conf' "$SSHD_CONFIG_D")"
separator_b="$(printf '%s/013-a\037c.conf' "$SSHD_CONFIG_D")"
printf '# a path the record format cannot carry\n' >"$separator_a"
printf '# a path the record format cannot carry\n' >"$separator_b"
run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail 'separator: a path the record format cannot carry must refuse, not be silently mis-parsed'
assert_no_kickstart 'separator'
grep -qi 'cannot be recorded' <<<"$SSH_RUN_ERR" ||
  fail "separator: the refusal must name the record format as the reason (stderr: $SSH_RUN_ERR)"
refute_contains "$SSH_RUN_ERR" 'configuration tree CHANGED' \
  'separator: an untouched tree must never be reported as changed'
rm -f "$separator_a" "$separator_b"

# The refusal above must be NARROW. A space is the character a naive record
# format would have split on, it is legal and ordinary in a path, and refusing
# it would block a tree nothing is wrong with.
spaced_dropin="$SSHD_CONFIG_D/015 spaced name.conf"
printf '# a drop-in whose name contains spaces\n' >"$spaced_dropin"
run_ssh_reload --reload
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "spaced path: a path containing spaces is recordable and must still reload (stderr: $SSH_RUN_ERR)"
assert_kickstart_attempted 'spaced path'
rm -f "$spaced_dropin"

# --- an Include pattern bash and sshd resolve differently refuses -------------
# Bash negates a bracket introduced by '!' OR '^'; glob(3), which is what sshd
# uses, negates on '!' only and reads a leading '^' as an ordinary member.
# Measured on macOS 26.2 with OpenSSH 10.0p2: with a.conf and b.conf present,
# `Include <dir>/[^a].conf` makes sshd read a.conf while bash matches b.conf.
# Disjoint. A guard that watched b.conf while the daemon read a.conf would
# report byte-for-byte stability over files nobody reads, so the pattern is
# refused instead. If a future change makes the walk resolve this faithfully,
# this case is the one that will say so.

divergent_dir="$SSH_SANDBOX/divergent"
mkdir -p "$divergent_dir"
printf '# the file sshd reads under [^a]\n' >"$divergent_dir/a.conf"
printf '# the file bash matches under [^a]\n' >"$divergent_dir/b.conf"
divergent_dropin="$SSHD_CONFIG_D/014-divergent.conf"
printf 'Include %s/[^a].conf\n' "$divergent_dir" >"$divergent_dropin"
run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail 'divergent glob: an Include pattern bash and glob(3) resolve differently must refuse, not be walked at the wrong files'
assert_no_kickstart 'divergent glob'
grep -qi "bracket begins with" <<<"$SSH_RUN_ERR" ||
  fail "divergent glob: the refusal must name the construct it cannot model (stderr: $SSH_RUN_ERR)"
# --verify shares the one resolver, so it must refuse the same tree for the same
# reason. A gate on the reload path alone would leave the Match scan silently
# scanning a different set of files than sshd reads.
run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail 'divergent glob: --verify shares the resolver and must fail closed on the same pattern'
grep -qi "bracket begins with" <<<"$SSH_RUN_ERR" ||
  fail "divergent glob: --verify must name the same construct (stderr: $SSH_RUN_ERR)"
rm -f "$divergent_dropin"
rm -rf "$divergent_dir"

# The neighbouring bracket forms must keep resolving. `[!^a]` negates the same
# three members for bash and for glob(3), and `[\^a]` makes '^' a literal member
# for both, so refusing either would be a false alarm that blocks a legitimate
# tree.
for divergent_probe in '[!^a].conf' '[\^a].conf' '[ab].conf'; do
  printf 'Include %s/%s\n' "$SSH_SANDBOX/absent-directory" "$divergent_probe" \
    >"$divergent_dropin"
  run_ssh_reload --reload
  [[ $SSH_RUN_STATUS -eq 0 ]] ||
    fail "divergent glob: '$divergent_probe' resolves the same for bash and glob(3) and must not be refused (stderr: $SSH_RUN_ERR)"
done
rm -f "$divergent_dropin"

# --- tooling: the observation's tools are SEAMS, not PATH lookups -------------
# `stat -f '<format>'` is BSD syntax; GNU stat reads -f as "file system", exits
# 1 and names the format string as a missing file. A Homebrew coreutils gnubin
# ahead of /usr/bin therefore turns every reload on the machine into a refusal.
# The two tools are resolved through named seams for exactly that reason, and
# the pair of cases below is what says so: a broken seam refuses, a broken PATH
# entry of the same name changes nothing.

: >"$LAUNCHCTL_SPY_LOG"
broken_tool="$SSH_SANDBOX/broken-tool"
printf '#!/bin/bash\nprintf %%s "broken tool stub" >&2\nexit 91\n' >"$broken_tool"
chmod +x "$broken_tool"

CKSUM_BIN="$broken_tool" run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail 'tooling: with no usable checksum tool the reload must refuse, not proceed unchecked'
assert_no_kickstart 'tooling (cksum)'
grep -qi 'sshd was not touched' <<<"$SSH_RUN_ERR" ||
  fail "tooling (cksum): the refusal must say sshd was not touched (stderr: $SSH_RUN_ERR)"

STAT_BIN="$broken_tool" run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail 'tooling: with no usable stat tool the reload must refuse, not proceed unchecked'
assert_no_kickstart 'tooling (stat)'
grep -qi 'sshd was not touched' <<<"$SSH_RUN_ERR" ||
  fail "tooling (stat): the refusal must say sshd was not touched (stderr: $SSH_RUN_ERR)"

# A file that passed the walk's regular-file test and is something else by the
# time it is read cannot be staged from here: the swap has to land between one
# stat and the open that follows it, inside a single observation, and the seams
# only reach the gaps BETWEEN observations. What this case does pin is the
# branch itself -- that a non-regular type coming back from stat is a refusal
# and not a file to open -- because reading a named pipe never returns and a
# hang after the kickstart is worse than the drift the guard exists to catch.
type_stub="$SSH_SANDBOX/stat-says-fifo"
printf '#!/bin/bash\nprintf "Fifo File\\037644 501 20\\n"\n' >"$type_stub"
chmod +x "$type_stub"
STAT_BIN="$type_stub" run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail 'tooling: a tree file that is no longer a regular file must refuse, never be opened'
assert_no_kickstart 'tooling (non-regular type)'
grep -qi 'Fifo File' <<<"$SSH_RUN_ERR" ||
  fail "tooling (non-regular type): the refusal must name the type it found (stderr: $SSH_RUN_ERR)"
grep -qi 'sshd was not touched' <<<"$SSH_RUN_ERR" ||
  fail "tooling (non-regular type): the refusal must say sshd was not touched (stderr: $SSH_RUN_ERR)"

run_ssh_hardening_without cksum --reload
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "tooling: a broken PATH entry named cksum must not reach the reload, which resolves its own tools (stderr: $SSH_RUN_ERR)"
run_ssh_hardening_without stat --reload
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "tooling: a broken PATH entry named stat must not reach the reload, which resolves its own tools (stderr: $SSH_RUN_ERR)"

# --- the mode still writes nothing -------------------------------------------
# Judged over the WHOLE observed tree, not the drop-in directory alone: a
# reload that appended a byte to the main config or to an out-of-tree Include
# target left the drop-in directory untouched and passed.

run_ssh_reload --reload
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "final control: the tree must be back to a reloadable state (stderr: $SSH_RUN_ERR)"
[[ "$(config_tree_fingerprint)" == "$baseline_fingerprint" ]] ||
  fail 'final control: --reload must still write nothing under the drop-in directory'
[[ "$(observed_tree_fingerprint)" == "$baseline_observed_fingerprint" ]] ||
  fail 'final control: --reload must write nothing anywhere in the tree it observes, including the main config and out-of-tree Include targets'

printf 'ssh-hardening-reload-tree-drift: OK (every inter-step window refuses before the kickstart, the irreducible window refuses the success claim, and an unchanged tree still reloads)\n'
