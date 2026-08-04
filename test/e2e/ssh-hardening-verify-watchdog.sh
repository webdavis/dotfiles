#!/usr/bin/env bash
# ssh-hardening-verify-watchdog.sh -- install is a transaction against a verify
# that HANGS, not only against one that fails.
#
# THE DEFECT THIS PINS. The install path stages the drop-in, publishes it with
# one rename, moves the legacy file aside, and only then verifies; a verify that
# RETURNS a failure takes the rollback and the tree goes back exactly as it was
# found. A verify that never returns took nothing: the run parked with the new
# drop-in published and the legacy file gone, rollback_install unreached. That is
# not hypothetical, it is measured (2026-08-01): one named pipe in the drop-in
# directory blocks `sshd -G` forever, because sshd resolves its own Include globs
# with no type filter, and the child verify runs sshd.
#
# HOW THE HANG IS BUILT. The reload library's SSH_TREE_MUTATION_HOOK seam runs an
# executable INSIDE each controlled stub, before that stub does its own work, so
# a hook that never returns is an `sshd -G` that never answers. That is the same
# shape as the real defect (sshd blocked in an open) without needing a named pipe
# or the real binary, and it wedges the FIRST sshd call the child verify makes
# (check_global), which is after the drop-in is already published.
#
# WHY e2e. The case waits out a deliberate wall clock, which is what this camp is
# for. The bound is shortened through SSH_HARDENING_VERIFY_DEADLINE; the harness
# clock in run_ssh_reload is the outer net, so a regressed script that hangs
# again fails this suite instead of parking it.
#
# Properties pinned:
#   1. a wedged verify is stopped at the deadline, reported as a failed verify,
#      and the tree it found is restored as it was, with no working files left
#   2. nothing the wedged verify started outlives the run: the sshd stub and the
#      sleeper under it are both gone (a group kill, not a kill of the child
#      shell alone, is what makes that true)
#   3. a healthy verify is untouched by the bound: install still succeeds, and
#      the seam calls are exactly the three resolutions the verify makes, so a
#      watchdog built out of the SLEEP_BIN seam would be caught here
#   4. a verify that genuinely FAILS still rolls back, and is reported as a
#      failure rather than as a timeout
#   5. a wedge that IGNORES SIGTERM is still reaped, so the KILL after the grace
#      is exercised rather than assumed
#   6. an INTERRUPT mid-install rolls back and takes the verify group with it,
#      instead of leaving a published drop-in behind a shell that is gone
#   7. what comes back is the file that was there, not just its bytes: a
#      symlinked drop-in is restored as a symlink to the same target
#   8. --reload's syntax check and port resolution are bounded too, so the two
#      sshd calls that bracket the verify cannot park the reload
#   9. a deadline so large that its tick arithmetic overflows is refused rather
#      than believed
#  10. a healthy verify survives `stty tostop`, which stops a background process
#      group on its first write to the terminal
set -euo pipefail

# Scrubbed at SCRIPT scope. Git exports GIT_DIR to every hook it runs and this
# suite can still inherit one from its caller.
unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

# shellcheck source=../fixtures/ssh-reload-lib.bash
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../fixtures" && pwd)/ssh-reload-lib.bash"

# Three seconds instead of the shipped 120: the case has to wait the deadline
# out, and the property under test is that the bound FIRES, not what number it
# holds. The shipped default is judged where it is defined, against measured
# verify times.
export SSH_HARDENING_VERIFY_DEADLINE=3
# The window a wedged run must land in: not before the deadline, and not much
# after the deadline plus the script's stop grace (VERIFY_STOP_GRACE_SECONDS,
# 2s). Measured on this machine, a wedged run takes 5s to 6s, and $SECONDS
# counts in whole seconds so a 5.4s run reads as either.
#
# The ceiling is what makes the POLL INTERVAL a pinned quantity rather than a
# comment. The wait is counted in ticks, four per second, so the deadline is
# only three seconds if a tick really is a quarter of one: change
# VERIFY_POLL_INTERVAL to 1 and the same twelve ticks take 12s, which lands at
# 14s with the grace. The 20s ceiling this replaces accepted that silently, so a
# whole-second poll passed every case in this file.
#
# The floor pins the other direction, where a bound that fires EARLY looks
# exactly like a bound that works: a healthy install rolled back at once, the
# operator told it waited the whole deadline. That is what an overflowed tick
# budget does (see case 9), and a ceiling alone cannot see it.
WEDGED_RUN_LIMIT_SECONDS=9
WEDGED_RUN_FLOOR_SECONDS="$SSH_HARDENING_VERIFY_DEADLINE"

reload_sandbox_setup
trap 'ssh_sandbox_teardown' EXIT

dropin="$SSHD_CONFIG_D/$SSH_DROPIN_NAME"
legacy="$SSHD_CONFIG_D/50-no-password-auth.conf"

# Every process the wedge starts records its pid here: the sshd stub (as the
# hook's parent) and the sleeper the hook becomes. Read back after the run to
# prove the stop reached the whole group, and read by the cleanup below so a
# FAILING case cannot leave a 15-minute sleeper behind.
SSH_VERIFY_WEDGE_PID_FILE="$SSH_SANDBOX/wedge-pids"
export SSH_VERIFY_WEDGE_PID_FILE
: >"$SSH_VERIFY_WEDGE_PID_FILE"

wedge_cleanup() {
  local pid
  [[ -s ${SSH_VERIFY_WEDGE_PID_FILE:-/dev/null} ]] || return 0
  while IFS= read -r pid; do
    kill -KILL "$pid" 2>/dev/null || :
  done <"$SSH_VERIFY_WEDGE_PID_FILE"
}

# Extended only now that wedge_cleanup exists: a trap naming a function that has
# not been defined yet would fire as a command-not-found on any early exit.
trap 'wedge_cleanup; ssh_sandbox_teardown' EXIT

WEDGE_HOOK="$SSH_SANDBOX/wedge-hook"
cat >"$WEDGE_HOOK" <<'HOOK'
#!/bin/bash
# Runs inside every controlled stub. Only the sshd stub is wedged: the install
# path drives `sudo` through the same hook, and wedging that would stop the run
# before it ever published anything, which is a different case.
#
# Which sshd call is wedged is the caller's choice, because the calls are not
# interchangeable -- the install path's first one is the child verify, and
# --reload makes two more that bracket it:
#   WEDGE_MATCH        a case pattern against " <argv> " (default *: any call)
#   WEDGE_SKIP_MATCHES how many MATCHING calls to let through first (default 0)
#   WEDGE_IGNORE_TERM  nonempty: the sleeper ignores SIGTERM, so only the KILL
#                      after the stop grace can reap it
set -uo pipefail
[[ ${1:-} == 'sshd' ]] || exit 0
shift
# shellcheck disable=SC2254  # WEDGE_MATCH is a PATTERN by design, not a literal
case " $* " in
  ${WEDGE_MATCH:-*}) ;;
  *) exit 0 ;;
esac
count_file="${SSH_VERIFY_WEDGE_PID_FILE:?}.matches"
count="$(cat "$count_file" 2>/dev/null || printf '0')"
count=$((count + 1))
printf '%s' "$count" >"$count_file"
[[ $count -gt ${WEDGE_SKIP_MATCHES:-0} ]] || exit 0
# $PPID is the stub shell; $$ survives the exec below, so it is the sleeper's
# pid. Both are the descendants a kill of the child shell alone would orphan.
printf '%s\n%s\n' "$PPID" "$$" >>"$SSH_VERIFY_WEDGE_PID_FILE"
if [[ -n ${WEDGE_IGNORE_TERM:-} ]]; then
  # No exec: the trap has to belong to a process that stays alive, and an
  # ignored disposition would survive into /bin/sleep and never come back.
  trap '' TERM
  while :; do
    /bin/sleep 1
  done
fi
exec /bin/sleep 900
HOOK
chmod +x "$WEDGE_HOOK"

# reset_wedge: forget the pids and the match count of the previous case, so one
# case's wedge can never satisfy the next case's assertions.
reset_wedge() {
  : >"$SSH_VERIFY_WEDGE_PID_FILE"
  rm -f "$SSH_VERIFY_WEDGE_PID_FILE.matches"
}

# file_fingerprint <path> -> what the file IS as well as what it holds.
#
# Bytes alone are not the restore this suite claims to check. A managed drop-in
# an operator had symlinked elsewhere, replaced by a regular file holding a copy
# of the same bytes, passed a checksum comparison while the link was gone -- and
# `cp -p` (no -R) does exactly that replacement, so the assertion was blind to a
# live defect. Type first, then the link target for a symlink and the checksum
# for a regular file.
file_fingerprint() {
  if [[ -L $1 ]]; then
    printf 'symlink -> %s\n' "$(readlink "$1")"
  elif [[ -p $1 ]]; then
    printf 'named pipe\n'
  elif [[ -d $1 ]]; then
    printf 'directory\n'
  elif [[ -f $1 ]]; then
    printf 'regular file %s\n' "$(cksum <"$1")"
  elif [[ -e $1 ]]; then
    printf 'other file type\n'
  else
    printf 'absent\n'
  fi
}

# working_file_leftovers: any staging or rollback copy still in the drop-in
# directory. Globs rather than `ls | grep`, so an unusual name still matches.
working_file_leftovers() {
  local candidate found=''
  for candidate in "$SSHD_CONFIG_D"/*.staging "$SSHD_CONFIG_D"/*.saved \
    "$SSHD_CONFIG_D"/.*.staging "$SSHD_CONFIG_D"/.*.saved; do
    if [[ -e $candidate ]]; then
      found="$found $candidate"
    fi
  done
  printf '%s\n' "$found"
}

# build_tree_with_previous_policy: a drop-in that is NOT what install writes,
# plus a legacy file. Both must come back byte for byte when the install fails,
# and a drop-in identical to the one install publishes could not tell a restore
# from a leftover.
build_tree_with_previous_policy() {
  rm -f "$SSHD_CONFIG_D"/* "$SSHD_CONFIG_D"/.[!.]* 2>/dev/null || :
  printf '# a previous drop-in, not the one install writes\nPasswordAuthentication no\n' \
    >"$dropin"
  printf 'PasswordAuthentication no\n' >"$legacy"
}

# assert_pids_reaped <label>: every process the wedge started is gone. Polled,
# not asserted once: a killed process is a zombie until its parent (or init,
# after reparenting) reaps it, and `kill -0` answers 0 for a zombie.
assert_pids_reaped() {
  local label="$1" pid waited=0 alive
  [[ -s $SSH_VERIFY_WEDGE_PID_FILE ]] ||
    fail "$label: the wedge never recorded a pid, so it never fired and this case proves nothing about orphans"
  while [[ $waited -lt 50 ]]; do
    alive=''
    while IFS= read -r pid; do
      if kill -0 "$pid" 2>/dev/null; then
        alive="$alive $pid"
      fi
    done <"$SSH_VERIFY_WEDGE_PID_FILE"
    [[ -n $alive ]] || return 0
    /bin/sleep 0.1
    waited=$((waited + 1))
  done
  fail "$label: the wedged verify left processes behind:$alive (a kill of the child shell alone orphans the sshd under it; the stop must signal the process group)"
}

# --- 3: a healthy verify is untouched by the bound ----------------------------
# First, because a bound that broke the normal path would make every case below
# pass for the wrong reason.

rm -f "$SSHD_CONFIG_D"/* "$SSHD_CONFIG_D"/.[!.]* 2>/dev/null || :
run_ssh_reload
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "3: install on a clean tree must still succeed under the bound (stderr: $SSH_RUN_ERR)"
[[ -f $dropin ]] ||
  fail '3: install must leave the drop-in in place'
refute_contains "$SSH_RUN_ERR" 'TIMED OUT' \
  '3: a verify that answered must never be reported as timed out'
invoking_user="$(id -un)"
# The exact seam calls, in order: the verify's global resolution and its two
# per-connection resolutions, and NOTHING else. A watchdog built out of the
# SLEEP_BIN seam would show up here as extra `sleep` lines, and a bound that
# re-ran the verify would show up as a second set of resolutions.
assert_seam_calls '3' \
  "sshd -G -f $SSHD_MAIN_CONFIG" \
  "sshd -G -T -C user=root,host=localhost,addr=127.0.0.1 -f $SSHD_MAIN_CONFIG" \
  "sshd -G -T -C user=$invoking_user,host=localhost,addr=127.0.0.1 -f $SSHD_MAIN_CONFIG"
leftovers="$(working_file_leftovers)"
[[ -z $leftovers ]] ||
  fail "3: a successful install left working files behind:$leftovers"

# --- 4: a verify that genuinely fails still rolls back ------------------------
# The regression pin for the path that already worked. `sshd -G` exits nonzero
# for every call in this run, which is a verify that ANSWERS with a failure.

build_tree_with_previous_policy
dropin_before="$(file_fingerprint "$dropin")"
legacy_before="$(file_fingerprint "$legacy")"

SSHD_STUB_RESOLVE_STATUSES='1' run_ssh_reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '4: an install whose verify failed must not report success'
refute_contains "$SSH_RUN_OUT" 'install complete' \
  '4: a failed verify must not claim the install completed'
refute_contains "$SSH_RUN_ERR" 'TIMED OUT' \
  '4: a verify that answered with a failure must be reported as a failure, not as a timeout'
[[ "$(file_fingerprint "$dropin")" == "$dropin_before" ]] ||
  fail '4: the drop-in that was in place must come back byte for byte'
[[ "$(file_fingerprint "$legacy")" == "$legacy_before" ]] ||
  fail '4: the legacy drop-in must come back byte for byte'
leftovers="$(working_file_leftovers)"
[[ -z $leftovers ]] ||
  fail "4: a failed install left working files behind:$leftovers"

# --- 1 and 2: a wedged verify is stopped, rolled back, and leaves nothing -----

build_tree_with_previous_policy
dropin_before="$(file_fingerprint "$dropin")"
legacy_before="$(file_fingerprint "$legacy")"
reset_wedge

wedged_started=$SECONDS
SSH_TREE_MUTATION_HOOK="$WEDGE_HOOK" run_ssh_reload
wedged_elapsed=$((SECONDS - wedged_started))

[[ $wedged_elapsed -le $WEDGED_RUN_LIMIT_SECONDS ]] ||
  fail "1: the wedged install took ${wedged_elapsed}s, past the ${WEDGED_RUN_LIMIT_SECONDS}s this bound allows (deadline ${SSH_HARDENING_VERIFY_DEADLINE}s plus the stop grace; a poll interval of a whole second instead of a quarter lands here)"
[[ $wedged_elapsed -ge $WEDGED_RUN_FLOOR_SECONDS ]] ||
  fail "1: the wedged install gave up after ${wedged_elapsed}s, before the ${WEDGED_RUN_FLOOR_SECONDS}s deadline it reports having waited; a bound that fires early rolls back installs that were fine"
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '1: an install whose verify never answered must not report success'
grep -qF 'verify TIMED OUT' <<<"$SSH_RUN_ERR" ||
  fail "1: the run must say which path fired, naming the timeout (stderr: $SSH_RUN_ERR)"
grep -qF 'rolled back' <<<"$SSH_RUN_ERR" ||
  fail "1: the run must report the rollback it performed (stderr: $SSH_RUN_ERR)"
refute_contains "$SSH_RUN_OUT" 'install complete' \
  '1: a wedged verify must not claim the install completed'
[[ "$(file_fingerprint "$dropin")" == "$dropin_before" ]] ||
  fail '1: the drop-in that was in place must come back byte for byte after a wedged verify'
[[ "$(file_fingerprint "$legacy")" == "$legacy_before" ]] ||
  fail '1: the legacy drop-in must come back byte for byte after a wedged verify'
leftovers="$(working_file_leftovers)"
[[ -z $leftovers ]] ||
  fail "1: the wedged install left working files behind:$leftovers"
assert_pids_reaped '2'

# --- 5: the KILL after the grace is what reaps a wedge that ignores TERM ------
# The wedge above is a plain /bin/sleep, which dies of the first TERM, so the
# KILL that follows the grace was never the thing that reaped anything: measured
# before this case existed, deleting the `kill -KILL` line left the whole file
# green. A wedge that IGNORES TERM is what makes the second signal load-bearing,
# and it is not a contrived shape -- a process blocked in an uninterruptible
# state, or one with its own TERM handler, is the ordinary reason a group
# survives the first signal.

build_tree_with_previous_policy
dropin_before="$(file_fingerprint "$dropin")"
legacy_before="$(file_fingerprint "$legacy")"
reset_wedge

WEDGE_IGNORE_TERM=1 SSH_TREE_MUTATION_HOOK="$WEDGE_HOOK" run_ssh_reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '5: an install whose verify never answered must not report success'
grep -qF 'verify TIMED OUT' <<<"$SSH_RUN_ERR" ||
  fail "5: the run must still report the timeout (stderr: $SSH_RUN_ERR)"
[[ "$(file_fingerprint "$dropin")" == "$dropin_before" ]] ||
  fail '5: the drop-in that was in place must come back after a TERM-ignoring wedge'
assert_pids_reaped '5 (TERM-ignoring wedge)'

# --- 6: an interrupt mid-install rolls back, and takes the verify with it -----
# The verify runs in a process group of its own, so Ctrl-C reaches the script
# and NOT the verify. Measured before the handler existed: the script died at
# once, the drop-in it had already published stayed published, the legacy file
# stayed moved aside, and both recorded descendants were still running -- a
# half-applied tree reached in a fifth of a second, where the deadline the
# operator interrupted was the only thing that would have collected it.
#
# The launch runs under MONITOR MODE, and that is a correctness requirement of
# the case rather than a detail. Bash gives an asynchronous command SIGINT and
# SIGQUIT ignored when job control is not in effect, and a signal ignored at
# entry cannot be trapped at all -- so a plain `script &` here would run with
# the handler disabled and prove nothing about the handler (measured: the run
# slept through the signal and hit the deadline instead). Monitor mode restores
# the default disposition and gives the run a process group of its own, so the
# signal below reaches the script and its foreground children exactly as Ctrl-C
# would, and reaches neither the test shell nor the verify (which is in a group
# of its own by construction).

build_tree_with_previous_policy
dropin_before="$(file_fingerprint "$dropin")"
legacy_before="$(file_fingerprint "$legacy")"
reset_wedge

INTERRUPT_STATUS=0
INTERRUPT_ERR=''
run_install_and_interrupt() { # <signal>
  local signal="$1" waited=0 child
  local out_file="$SSH_SANDBOX/interrupt.out" err_file="$SSH_SANDBOX/interrupt.err"
  INTERRUPT_STATUS=0
  set -m
  SSH_TREE_MUTATION_HOOK="$WEDGE_HOOK" \
    /bin/bash "$SSH_HARDENING_SCRIPT" >"$out_file" 2>"$err_file" &
  child=$!
  set +m
  # The wedge fires on the verify's first sshd call, which is strictly after the
  # drop-in has been published and the legacy file moved aside. Waiting for it
  # is what makes this an interrupt of a half-applied tree rather than a race
  # with the staging step.
  while [[ ! -s $SSH_VERIFY_WEDGE_PID_FILE ]]; do
    [[ $waited -lt 100 ]] ||
      fail '6: the wedge never fired, so the install was never interrupted mid-transaction'
    /bin/sleep 0.1
    waited=$((waited + 1))
  done
  # To the GROUP, which is what a terminal sends: the script and whatever it has
  # in the foreground (the poll's own sleep), and nothing else.
  kill -"$signal" -"$child"
  wait "$child" 2>/dev/null || INTERRUPT_STATUS=$?
  INTERRUPT_ERR="$(cat "$err_file")"
  assert_no_escalation "run_install_and_interrupt $signal"
}

run_install_and_interrupt INT
[[ $INTERRUPT_STATUS -ne 0 ]] ||
  fail '6: an interrupted install must not report success'
grep -qF 'INTERRUPTED' <<<"$INTERRUPT_ERR" ||
  fail "6: the run must say it was interrupted, which is what distinguishes the handler from the deadline firing later (stderr: $INTERRUPT_ERR)"
[[ "$(file_fingerprint "$dropin")" == "$dropin_before" ]] ||
  fail "6: the drop-in that was in place must come back after an interrupt (before: $dropin_before, after: $(file_fingerprint "$dropin"))"
[[ "$(file_fingerprint "$legacy")" == "$legacy_before" ]] ||
  fail '6: the legacy drop-in must come back after an interrupt'
leftovers="$(working_file_leftovers)"
[[ -z $leftovers ]] ||
  fail "6: an interrupted install left working files behind:$leftovers"
assert_pids_reaped '6 (interrupt)'

# --- 7: a symlinked drop-in comes back as a symlink ---------------------------
# "Byte for byte" was the whole claim and it was the wrong measure. An operator
# who symlinks the managed drop-in at some other file gets it back as a REGULAR
# file holding a copy of that file's bytes, because `cp -p` without -R follows
# what it is given. Every checksum comparison in this file passed that, so the
# suite could not see the link disappear.

rm -f "$SSHD_CONFIG_D"/* "$SSHD_CONFIG_D"/.[!.]* 2>/dev/null || :
symlink_source="$SSH_SANDBOX/previous-policy.conf"
printf '# a previous drop-in reached through a symlink\nPasswordAuthentication no\n' \
  >"$symlink_source"
ln -s "$symlink_source" "$dropin"
printf 'PasswordAuthentication no\n' >"$legacy"
dropin_before="$(file_fingerprint "$dropin")"
[[ $dropin_before == "symlink -> $symlink_source" ]] ||
  fail "7: the fixture must start as a symlink, got '$dropin_before'"
reset_wedge

SSH_TREE_MUTATION_HOOK="$WEDGE_HOOK" run_ssh_reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '7: an install whose verify never answered must not report success'
[[ "$(file_fingerprint "$dropin")" == "$dropin_before" ]] ||
  fail "7: a symlinked drop-in must come back as the same symlink (before: $dropin_before, after: $(file_fingerprint "$dropin"))"
[[ "$(cat "$symlink_source")" == "$(printf '# a previous drop-in reached through a symlink\nPasswordAuthentication no\n')" ]] ||
  fail '7: the file the symlink pointed at must be untouched'
assert_pids_reaped '7 (symlink)'

# --- 8: --reload's syntax check is bounded too --------------------------------
# The verify is not the only sshd call on a disruptive path. `sshd -t` runs
# BEFORE it, over the same tree, opening the same Include graph with the same
# absence of a type filter, and it was unbounded: a named pipe under the drop-in
# glob parked --reload there, before the verify's deadline could apply, with
# nothing printed and no kickstart to show for it. This script's own walk skips
# the pipe and reports the tree healthy, so nothing else would have caught it.

rm -f "$SSHD_CONFIG_D"/* "$SSHD_CONFIG_D"/.[!.]* 2>/dev/null || :
write_hardened_dropin
reset_wedge

syntax_started=$SECONDS
WEDGE_MATCH='* -t *' SSH_TREE_MUTATION_HOOK="$WEDGE_HOOK" run_ssh_reload --reload
syntax_elapsed=$((SECONDS - syntax_started))
[[ $syntax_elapsed -le $WEDGED_RUN_LIMIT_SECONDS ]] ||
  fail "8: the wedged syntax check took ${syntax_elapsed}s, past the ${WEDGED_RUN_LIMIT_SECONDS}s the bound allows"
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '8: a reload whose syntax check never answered must not report success'
# Both halves, because either alone is satisfied by the wrong thing: `sshd -t`
# also fails this reload when it EXITS nonzero, and every other bounded step
# reports being stopped in the same words.
grep -qi 'syntax check' <<<"$SSH_RUN_ERR" ||
  fail "8: the failure must name the step that was stopped (stderr: $SSH_RUN_ERR)"
grep -qi 'was still running after' <<<"$SSH_RUN_ERR" ||
  fail "8: the syntax check must be reported as STOPPED at its bound, not as a configuration that failed to parse (stderr: $SSH_RUN_ERR)"
grep -qi 'sshd was not touched' <<<"$SSH_RUN_ERR" ||
  fail "8: nothing was restarted, and the refusal must say so (stderr: $SSH_RUN_ERR)"
assert_no_kickstart '8'
assert_pids_reaped '8 (wedged syntax check)'

# --- 9: --reload's port resolution is bounded too -----------------------------
# The third and last sshd call before the restart, and the one a tree that grew
# a named pipe DURING the preflight reaches: the syntax check and the verify are
# already past. It is the same `sshd -G`, and it used to be read through a
# command substitution, which waits for the pipe to close and so waits forever.
#
# The wedge skips the first plain `-G -f` call, which is the child verify's
# global resolution; the two per-connection calls carry `-T` and do not match.

reset_wedge
port_started=$SECONDS
WEDGE_MATCH='* -G -f *' WEDGE_SKIP_MATCHES=1 \
  SSH_TREE_MUTATION_HOOK="$WEDGE_HOOK" run_ssh_reload --reload
port_elapsed=$((SECONDS - port_started))
[[ $port_elapsed -le $WEDGED_RUN_LIMIT_SECONDS ]] ||
  fail "9: the wedged port resolution took ${port_elapsed}s, past the ${WEDGED_RUN_LIMIT_SECONDS}s the bound allows"
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '9: a reload whose port resolution never answered must not report success'
# The full phrase, not the word "port": the readiness failure, the
# no-port-at-all refusal and the out-of-range refusal all carry that word, and
# any of them passing here would let the case go green on the wrong branch.
grep -qi 'resolving the effective sshd port' <<<"$SSH_RUN_ERR" ||
  fail "9: the failure must name the step that was stopped (stderr: $SSH_RUN_ERR)"
grep -qi 'was still running after' <<<"$SSH_RUN_ERR" ||
  fail "9: the port resolution must be reported as STOPPED at its bound (stderr: $SSH_RUN_ERR)"
assert_no_kickstart '9'
assert_pids_reaped '9 (wedged port resolution)'

# --- 10: a deadline whose tick arithmetic overflows is refused ----------------
# The wait is counted in ticks, four per second, and bash arithmetic is signed
# 64-bit. 2305843009213693952 * 4 is exactly -9223372036854775808, so the budget
# goes negative, the poll runs zero passes, and a HEALTHY verify is stopped the
# instant it starts -- a valid install rolled back, reported as a timeout that
# waited 2.3e18 seconds. The value is not a fixture curiosity: it is one
# keystroke off a plausible "make the bound effectively infinite".

rm -f "$SSHD_CONFIG_D"/* "$SSHD_CONFIG_D"/.[!.]* 2>/dev/null || :
SSH_HARDENING_VERIFY_DEADLINE=2305843009213693952 run_ssh_reload
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "10: an absurd deadline must fall back to the default, not stop a healthy verify at once (status $SSH_RUN_STATUS, stderr: $SSH_RUN_ERR)"
refute_contains "$SSH_RUN_ERR" 'TIMED OUT' \
  '10: a verify that answered must never be reported as timed out'
[[ -f $dropin ]] ||
  fail '10: install must leave the drop-in in place'

# --- 11: a healthy verify survives `stty tostop` ------------------------------
# The verify runs in a process group of its own, and under `stty tostop` a
# background group is STOPPED by SIGTTOU the moment it writes to the terminal.
# The verify's first act is to write: PASS or the failure list. Measured through
# a pty: the child stopped on that write, `kill -0` kept answering 0 for it
# (a stopped process is still a process), the poll ran out, and a VALID install
# was reported as TIMED OUT and rolled back -- the watchdog firing on a machine
# whose only unusual property is a terminal setting.
#
# The case needs a real controlling terminal, which is what `script` provides.
# The transcript is the assertion surface, because the script's stdout has to BE
# the terminal here; a redirection into a file is the one thing that would make
# the defect unreachable.

rm -f "$SSHD_CONFIG_D"/* "$SSHD_CONFIG_D"/.[!.]* 2>/dev/null || :
tostop_transcript="$SSH_SANDBOX/tostop-transcript"
TOSTOP_STATUS_FILE="$SSH_SANDBOX/tostop-status"
export SSH_HARDENING_SCRIPT TOSTOP_STATUS_FILE
: >"$TOSTOP_STATUS_FILE"
# shellcheck disable=SC2016  # the inner shell expands these, not this one
script -q "$tostop_transcript" /bin/bash -c '
  stty tostop
  status=0
  /bin/bash "$SSH_HARDENING_SCRIPT" || status=$?
  printf "%s\n" "$status" >"$TOSTOP_STATUS_FILE"
' >/dev/null 2>&1 || :
tostop_status="$(cat "$TOSTOP_STATUS_FILE")"
tostop_output="$(cat "$tostop_transcript")"
[[ -n $tostop_status ]] ||
  fail "11: the pty run never recorded a status; transcript: $tostop_output"
[[ $tostop_status -eq 0 ]] ||
  fail "11: a healthy install under 'stty tostop' must still succeed (status $tostop_status, transcript: $tostop_output)"
refute_contains "$tostop_output" 'TIMED OUT' \
  "11: a verify stopped by SIGTTOU is not a verify that timed out (transcript: $tostop_output)"
grep -qF 'install complete' <<<"$tostop_output" ||
  fail "11: the install must complete under 'stty tostop' (transcript: $tostop_output)"
assert_no_escalation '11 (stty tostop)'

printf 'ssh-hardening-verify-watchdog: OK (a verify wedged inside sshd is stopped at its deadline in %ss and rolled back, TERM-ignoring wedges included; an interrupt rolls back the same way and reaps the verify group; a symlinked drop-in comes back as a symlink; --reload bounds its syntax check and its port resolution; an overflowing deadline falls back to the default; and a healthy verify still installs, under a tostop terminal too)\n' \
  "$wedged_elapsed"
