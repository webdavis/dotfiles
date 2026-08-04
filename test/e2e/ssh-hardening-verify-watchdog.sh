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
#      and the tree it found is restored byte for byte with no working files left
#   2. nothing the wedged verify started outlives the run: the sshd stub and the
#      sleeper under it are both gone (a group kill, not a kill of the child
#      shell alone, is what makes that true)
#   3. a healthy verify is untouched by the bound: install still succeeds, and
#      the seam calls are exactly the three resolutions the verify makes, so a
#      watchdog built out of the SLEEP_BIN seam would be caught here
#   4. a verify that genuinely FAILS still rolls back, and is reported as a
#      failure rather than as a timeout
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
# The stop grace the script allows after the deadline (VERIFY_STOP_GRACE_SECONDS)
# plus room for a loaded CI machine. A run that needs longer than this has not
# bounded the verify at all.
WEDGED_RUN_LIMIT_SECONDS=20

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
set -euo pipefail
[[ ${1:-} == 'sshd' ]] || exit 0
# $PPID is the stub shell; $$ survives the exec below, so it is the sleeper's
# pid. Both are the descendants a kill of the child shell alone would orphan.
printf '%s\n%s\n' "$PPID" "$$" >>"${SSH_VERIFY_WEDGE_PID_FILE:?}"
exec /bin/sleep 900
HOOK
chmod +x "$WEDGE_HOOK"

file_fingerprint() { # <path> -> checksum and size, or the word absent
  if [[ -e $1 ]]; then
    cksum <"$1"
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
: >"$SSH_VERIFY_WEDGE_PID_FILE"

wedged_started=$SECONDS
SSH_TREE_MUTATION_HOOK="$WEDGE_HOOK" run_ssh_reload
wedged_elapsed=$((SECONDS - wedged_started))

[[ $wedged_elapsed -le $WEDGED_RUN_LIMIT_SECONDS ]] ||
  fail "1: the wedged install took ${wedged_elapsed}s, past the ${WEDGED_RUN_LIMIT_SECONDS}s this bound allows (deadline ${SSH_HARDENING_VERIFY_DEADLINE}s plus the stop grace)"
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

printf 'ssh-hardening-verify-watchdog: OK (a verify wedged inside sshd is stopped at its deadline in %ss, reported as a failed verify, rolled back to a byte-identical tree with no working files and no surviving processes; a healthy verify still installs with exactly its three seam calls; a verify that answers with a failure still rolls back and is not reported as a timeout)\n' \
  "$wedged_elapsed"
