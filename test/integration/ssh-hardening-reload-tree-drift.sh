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
#     020-inert.conf                                    a removable sibling
#
# extra.conf is what proves the fingerprint follows the graph sshd follows
# rather than listing the two directories the script was pointed at.
OUTSIDE_DIR="$SSH_SANDBOX/outside"
mkdir -p "$OUTSIDE_DIR"
OUTSIDE_INCLUDE="$OUTSIDE_DIR/extra.conf"
INERT_DROPIN="$SSHD_CONFIG_D/020-inert.conf"
printf '# inert file pulled in from outside the drop-in directory\n' >"$OUTSIDE_INCLUDE"
printf 'Include %s\n' "$OUTSIDE_INCLUDE" >"$SSHD_CONFIG_D/010-include-outside.conf"
printf '# inert sibling drop-in\n' >"$INERT_DROPIN"

# --- the mutation hook -------------------------------------------------------
# Fires once, at the SSH_TREE_MUTATION_OCCURRENCE-th call whose "<tool> <argv>"
# matches SSH_TREE_MUTATION_MATCH, and applies SSH_TREE_MUTATION_ACTION to
# SSH_TREE_MUTATION_TARGET. Records every mutation it performs, so a case can
# assert the injection really happened.
SSH_TREE_MUTATION_MATCH=''
SSH_TREE_MUTATION_OCCURRENCE=1
SSH_TREE_MUTATION_ACTION=''
SSH_TREE_MUTATION_TARGET=''
SSH_TREE_MUTATION_LOG="$SSH_SANDBOX/tree-mutation.log"
export SSH_TREE_MUTATION_MATCH SSH_TREE_MUTATION_OCCURRENCE \
  SSH_TREE_MUTATION_ACTION SSH_TREE_MUTATION_TARGET SSH_TREE_MUTATION_LOG
: >"$SSH_TREE_MUTATION_LOG"

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
case "${SSH_TREE_MUTATION_ACTION:?}" in
  append-comment)
    printf '# drift injected at: %s\n' "$description" >>"$target"
    ;;
  create-file)
    printf '# file created by drift injection at: %s\n' "$description" >"$target"
    ;;
  remove-file)
    rm -f -- "$target"
    ;;
  make-world-writable)
    # `--` BEFORE the mode: BSD chmod reads a `--` after the mode as a file
    # operand and fails.
    chmod -- 0666 "$target"
    ;;
  *)
    printf 'tree-mutation-hook: unknown action %s\n' "$SSH_TREE_MUTATION_ACTION" >&2
    exit 70
    ;;
esac
printf '%s %s %s\n' "${SSH_TREE_MUTATION_ACTION}" "$target" "$description" \
  >>"${SSH_TREE_MUTATION_LOG:?}"
HOOK
chmod +x "$MUTATION_HOOK"

# run_reload_with_mutation <match> <occurrence> <action> <target>: one --reload
# run with the hook armed at exactly one seam call.
run_reload_with_mutation() {
  : >"$SSH_TREE_MUTATION_LOG"
  SSH_TREE_MUTATION_HOOK="$MUTATION_HOOK" \
    SSH_TREE_MUTATION_MATCH="$1" \
    SSH_TREE_MUTATION_OCCURRENCE="$2" \
    SSH_TREE_MUTATION_ACTION="$3" \
    SSH_TREE_MUTATION_TARGET="$4" \
    run_ssh_reload --reload
}

assert_mutation_fired() {
  [[ -s $SSH_TREE_MUTATION_LOG ]] ||
    fail "$1: the injected mutation never fired, so this case proves nothing about drift detection"
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
baseline_fingerprint="$(config_tree_fingerprint)"

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
write_hardened_dropin

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

new_dropin="$SSHD_CONFIG_D/030-appeared.conf"
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

# --- tooling: an unusable checksum tool refuses BEFORE the kickstart ----------
# The fingerprint is the last gate before the disruptive step. If it cannot be
# taken at all, the answer is a refusal with nothing disturbed, never a silent
# skip of the check.

: >"$LAUNCHCTL_SPY_LOG"
run_ssh_hardening_without cksum --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail 'tooling: with no checksum tool the reload must refuse, not proceed unchecked'
assert_no_kickstart 'tooling'
grep -qi 'sshd was not touched' <<<"$SSH_RUN_ERR" ||
  fail "tooling: the refusal must say sshd was not touched (stderr: $SSH_RUN_ERR)"

# --- the mode still writes nothing -------------------------------------------

run_ssh_reload --reload
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "final control: the tree must be back to a reloadable state (stderr: $SSH_RUN_ERR)"
[[ "$(config_tree_fingerprint)" == "$baseline_fingerprint" ]] ||
  fail 'final control: --reload must still write nothing under the drop-in directory'

printf 'ssh-hardening-reload-tree-drift: OK (every inter-step window refuses before the kickstart, the irreducible window refuses the success claim, and an unchanged tree still reloads)\n'
