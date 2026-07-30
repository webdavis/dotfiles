#!/usr/bin/env bash
# ssh-hardening-rollback.sh -- --rollback is the way back in, expressed as
# code (slice 8, acceptance criterion 11). It removes the managed drop-in,
# re-verifies that the hardening is GONE from the effective configuration, and
# fails closed on every step it cannot prove. Everything here drives the stub
# seams from ssh-reload-lib.bash; no case can reach the live daemon or
# /etc/ssh.
#
# Properties pinned:
#   1. rollback removes the drop-in, PROVES an interactive password channel
#      resolves ON for the sampled connections, and says which restart route
#      actually works (--reload refuses an unhardened tree, so it is never
#      advertised)
#   2. a second rollback re-runs the same proof on the already-absent path:
#      "nothing to remove" is never "access is back" by itself
#   3. rollback never restarts anything: no launchctl call, no keyscan call
#   4. a removal that fails leaves a loud nonzero failure, not a success claim
#   5. a tree that STILL verifies hardened after the removal is a loud
#      failure: the drop-in was not the (only) thing enforcing the policy, so
#      password access is NOT back
#   6. an unrunnable verifier after the removal fails closed: the file is
#      gone, but rollback refuses to CLAIM the hardening is gone unchecked
#   7. a PARTIAL policy that still closes both password channels fails loudly
#      even though the tree no longer verifies fully hardened (negating
#      --verify is not proof of access)
#   8. a verifier that runs but ERRORS is the error outcome, distinct from
#      "still blocked" and never a success
#   9. the already-absent path with the policy still in force fails loudly
#   10. the OFF-LOOPBACK recovery sample is load-bearing: a tree that blocks
#       passwords only for the off-loopback sample fails the gate, so a
#       rollback verified over loopback alone can never claim success (the
#       locked-out operator this gate exists for connects from OFF the
#       machine, where loopback proves nothing)
#   11. the LOOPBACK sample is load-bearing the same way, so neither sample
#       can be deleted with the suite still green
set -euo pipefail

# Scrubbed at SCRIPT scope. Git exports GIT_DIR to every hook it runs and this
# suite runs from the pre-push hook.
unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

# shellcheck source=../fixtures/ssh-reload-lib.bash
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../fixtures" && pwd)/ssh-reload-lib.bash"

# fail and refute_contains come from ssh-hardening-lib.bash (via the reload
# lib).

assert_no_reload_side_effects() { # <label>
  [[ ! -s $LAUNCHCTL_SPY_LOG ]] ||
    fail "$1: rollback must never touch launchctl (spy: $(cat "$LAUNCHCTL_SPY_LOG"))"
  [[ ! -s $KEYSCAN_SPY_LOG ]] ||
    fail "$1: rollback must never probe the listener (spy: $(cat "$KEYSCAN_SPY_LOG"))"
}

reload_sandbox_setup
trap 'ssh_sandbox_teardown' EXIT

dropin="$SSHD_CONFIG_D/$SSH_DROPIN_NAME"

# --- 1: remove, and the hardening really is gone ------------------------------

write_hardened_dropin
run_ssh_reload --verify
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "setup: the hardened tree must verify before the rollback (stderr: $SSH_RUN_ERR)"

run_ssh_reload --rollback
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "1: rollback must succeed on a hardened tree (stderr: $SSH_RUN_ERR)"
[[ ! -e $dropin ]] ||
  fail '1: rollback must remove the drop-in'
grep -qF "$dropin" <<<"$SSH_RUN_OUT" ||
  fail "1: rollback must name the file it removed (stdout: $SSH_RUN_OUT)"
grep -qi 'password access is restored' <<<"$SSH_RUN_OUT" ||
  fail "1: rollback must claim success as PROVEN password access, not as policy drift (stdout: $SSH_RUN_OUT)"
grep -qi 'resolves ON' <<<"$SSH_RUN_OUT" ||
  fail "1: the success claim must name what was proven, a password channel resolving ON (stdout: $SSH_RUN_OUT)"
# The restart guidance must name a route that can actually run: --reload
# refuses a tree that is not fully hardened, which is exactly the state a
# successful rollback leaves, so it must only ever appear as the thing that
# CANNOT perform this restart.
grep -qi 'toggle Remote Login off and back on' <<<"$SSH_RUN_OUT" ||
  fail "1: the success message must name the reachable restart route (stdout: $SSH_RUN_OUT)"
grep -qF -- '--reload cannot perform this restart' <<<"$SSH_RUN_OUT" ||
  fail "1: the success message must explain that --reload refuses an unhardened tree (stdout: $SSH_RUN_OUT)"
assert_no_reload_side_effects '1'

run_ssh_reload --verify
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '1: after rollback, --verify must report the hardening ABSENT (it exited 0)'

# --- 2: a second rollback re-proves access on the already-absent path ---------

run_ssh_reload --rollback
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "2: a second rollback must be a clean no-op (stderr: $SSH_RUN_ERR)"
grep -qi 'already absent' <<<"$SSH_RUN_OUT" ||
  fail "2: the no-op must say there was nothing to remove (stdout: $SSH_RUN_OUT)"
grep -qi 'password access is restored' <<<"$SSH_RUN_OUT" ||
  fail "2: even with nothing to remove, success must rest on the SAME access proof (stdout: $SSH_RUN_OUT)"
[[ ! -e $dropin ]] ||
  fail '2: the second rollback must leave the drop-in absent'
assert_no_reload_side_effects '2'

# --- 3: a failed removal is a loud failure, never a success claim -------------

write_hardened_dropin
SSH_HARDENING_SUDO="$SSH_SUDO_DENY_STUB" run_ssh_reload --rollback
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '3: rollback must fail when the removal command fails'
[[ -e $dropin ]] ||
  fail '3: with the removal refused, the drop-in must still be in place'
grep -qi 'could not remove' <<<"$SSH_RUN_ERR" ||
  fail "3: the failure must name the removal (stderr: $SSH_RUN_ERR)"
refute_contains "$SSH_RUN_OUT" 'rollback complete' \
  '3: a failed removal must not claim completion'
assert_no_reload_side_effects '3'

# --- 4: still hardened after removal -> loud failure --------------------------
# SSHD_STUB_FORCE_HARDENED models a sibling file still enforcing the policy:
# the drop-in is gone, yet the effective configuration still verifies
# hardened, so the way back in is NOT restored and rollback must say so.

SSHD_STUB_FORCE_HARDENED=1 run_ssh_reload --rollback
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '4: rollback must fail when the tree still verifies hardened after the removal'
[[ ! -e $dropin ]] ||
  fail '4: the drop-in itself must still have been removed'
grep -qi 'NOT restored' <<<"$SSH_RUN_ERR" ||
  fail "4: the failure must say password access is NOT restored (stderr: $SSH_RUN_ERR)"
refute_contains "$SSH_RUN_OUT" 'rollback complete' \
  '4: a still-hardened tree must not produce a completion claim'
assert_no_reload_side_effects '4'

# --- 5: unrunnable verifier after removal -> fail closed ----------------------

write_hardened_dropin
SSHD_BIN="$SSH_SANDBOX/no-such-sshd" run_ssh_reload --rollback
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '5: rollback must fail closed when the removal cannot be verified'
[[ ! -e $dropin ]] ||
  fail '5: the removal itself must still happen (it is the way back in)'
grep -qi 'failing closed' <<<"$SSH_RUN_ERR" ||
  fail "5: the failure must say it is failing closed (stderr: $SSH_RUN_ERR)"
refute_contains "$SSH_RUN_OUT" 'rollback complete' \
  '5: an unverified removal must not claim completion'
assert_no_reload_side_effects '5'

# --- 6: partial policy still closes both channels -> loud failure -------------
# The drop-in is gone and the tree no longer verifies fully hardened
# (permitrootlogin drifted back to its default), yet PasswordAuthentication
# and KbdInteractiveAuthentication both still resolve no. The previous
# rollback treated ANY failed child verify as proof of restored access and
# claimed success here; the exact fail-open this case exists to keep dead.

write_hardened_dropin
SSHD_STUB_PARTIAL_HARDENED=1 run_ssh_reload --rollback
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '6: with both password channels still OFF, rollback must fail even though the tree no longer verifies fully hardened'
[[ ! -e $dropin ]] ||
  fail '6: the removal itself must still happen'
grep -qi 'NOT restored' <<<"$SSH_RUN_ERR" ||
  fail "6: the failure must say password access is NOT restored (stderr: $SSH_RUN_ERR)"
refute_contains "$SSH_RUN_OUT" 'rollback complete' \
  '6: a blocked password path must not produce a completion claim'
assert_no_reload_side_effects '6'

# --- 7: verifier runs but ERRORS -> the error outcome, never success ----------
# Distinct from case 5: the binary IS executable and its failure is an exit
# status, not a missing file. The previous rollback read this nonzero child
# status as "the hardening is gone" and claimed success.

write_hardened_dropin
SSHD_STUB_RESOLVE_STATUSES=1 run_ssh_reload --rollback
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '7: an ERRORING verifier must fail the rollback, not pass as proof of access'
[[ ! -e $dropin ]] ||
  fail '7: the removal itself must still happen'
grep -qi 'errored' <<<"$SSH_RUN_ERR" ||
  fail "7: the failure must name the ERROR outcome, distinct from a blocked channel (stderr: $SSH_RUN_ERR)"
refute_contains "$SSH_RUN_ERR" 'NOT restored' \
  '7: an errored check must not claim to know the channels are blocked'
refute_contains "$SSH_RUN_OUT" 'rollback complete' \
  '7: an errored check must not produce a completion claim'
assert_no_reload_side_effects '7'

# --- 8: already absent, policy still in force -> loud failure -----------------
# The exact scenario the old already-absent branch fail-opened on: nothing to
# remove, but a sibling (modeled by SSHD_STUB_FORCE_HARDENED) still enforces
# the full policy, so the operator is told "nothing to remove" while password
# access is NOT back.

rm -f "$dropin"
SSHD_STUB_FORCE_HARDENED=1 run_ssh_reload --rollback
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '8: an already-absent drop-in with the policy still in force must fail the rollback'
grep -qi 'already absent' <<<"$SSH_RUN_OUT" ||
  fail "8: the run must still report there was nothing to remove (stdout: $SSH_RUN_OUT)"
grep -qi 'NOT restored' <<<"$SSH_RUN_ERR" ||
  fail "8: the failure must say password access is NOT restored (stderr: $SSH_RUN_ERR)"
assert_no_reload_side_effects '8'

# --- 9: a removal command that LIES produces no success claim -----------------
# The wrapper reports rm succeeded without running it; the post-removal
# existence re-check must convict the lie by looking at the file itself,
# BEFORE any recovery proof runs against a tree that still carries the
# drop-in.

write_hardened_dropin
SSH_HARDENING_SUDO="$SSH_SUDO_SWALLOW_RM_STUB" run_ssh_reload --rollback
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '9: a swallowed rm must fail the rollback'
[[ -e $dropin ]] ||
  fail '9: the drop-in must still be in place (the stub never removed it)'
grep -qi 'still exists after the removal command reported success' <<<"$SSH_RUN_ERR" ||
  fail "9: the failure must convict the lying removal specifically (stderr: $SSH_RUN_ERR)"
refute_contains "$SSH_RUN_OUT" 'rollback complete' \
  '9: a swallowed removal must not produce a completion claim'
assert_no_reload_side_effects '9'
rm -f "$dropin"

# --- 10: the off-loopback sample is load-bearing ------------------------------
# The stub resolves the two password channels CLOSED for the off-loopback
# recovery sample (SSHD_STUB_BLOCKED_ADDRESSES) while loopback resolves them
# open -- the shape of a Match block scoped away from loopback still enforcing
# the password block for the address the locked-out operator actually
# connects from. The gate must report BLOCKED. A gate sampling only loopback
# claims success here, so deleting the off-loopback sample fails this case.
# The address is pinned to the script's documented RFC 5737 recovery sample.

write_hardened_dropin
SSHD_STUB_BLOCKED_ADDRESSES='198.51.100.23' run_ssh_reload --rollback
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '10: with passwords still blocked for the OFF-LOOPBACK sample, rollback must fail even though loopback resolves open'
[[ ! -e $dropin ]] ||
  fail '10: the removal itself must still happen'
grep -qi 'NOT restored' <<<"$SSH_RUN_ERR" ||
  fail "10: the failure must say password access is NOT restored (stderr: $SSH_RUN_ERR)"
refute_contains "$SSH_RUN_OUT" 'rollback complete' \
  '10: a loopback-only proof must not produce a completion claim'
assert_no_reload_side_effects '10'

# --- 11: the loopback sample is load-bearing the same way ---------------------
# The mirror image: loopback still blocks passwords while the off-loopback
# sample resolves open. Together with case 10 this pins BOTH samples: neither
# can be deleted from the gate with the suite still green.

write_hardened_dropin
SSHD_STUB_BLOCKED_ADDRESSES='127.0.0.1' run_ssh_reload --rollback
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '11: with passwords still blocked for the LOOPBACK sample, rollback must fail even though off-loopback resolves open'
[[ ! -e $dropin ]] ||
  fail '11: the removal itself must still happen'
grep -qi 'NOT restored' <<<"$SSH_RUN_ERR" ||
  fail "11: the failure must say password access is NOT restored (stderr: $SSH_RUN_ERR)"
refute_contains "$SSH_RUN_OUT" 'rollback complete' \
  '11: an off-loopback-only proof must not produce a completion claim'
assert_no_reload_side_effects '11'

printf 'ssh-hardening-rollback: OK (success means a PROVEN password channel, on both the removal and the already-absent path; blocked, errored, and unverifiable are three distinct loud failures; both recovery samples are load-bearing)\n'
