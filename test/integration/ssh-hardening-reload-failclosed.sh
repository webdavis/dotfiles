#!/usr/bin/env bash
# ssh-hardening-reload-failclosed.sh -- --reload fails closed at EVERY step
# (slice 8, acceptance criteria 1-10, 12, 13). The reload is the first
# genuinely disruptive mode in this program: it restarts the daemon that
# serves remote access, so every "cannot determine" below must resolve to
# FAILURE before the disruptive step, and every failure after it must warn
# and name the way back in. Everything drives the stub seams from
# ssh-reload-lib.bash; no case can reach the live daemon or /etc/ssh.
#
# Each case asserts three dimensions: the exit status, whether a kickstart
# was ATTEMPTED (judged by the launchctl stub's spy log, never by the
# script's output), and the message content.
set -euo pipefail

# Scrubbed at SCRIPT scope. Git exports GIT_DIR to every hook it runs and this
# suite can still inherit one from its caller.
unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

# shellcheck source=../fixtures/ssh-reload-lib.bash
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../fixtures" && pwd)/ssh-reload-lib.bash"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUNBOOK="$REPO_ROOT/docs/runbooks/macos-fresh-machine-quickstart.md"

# fail and refute_contains come from ssh-hardening-lib.bash (via the reload
# lib).

# Fragments of the recovery instruction, pinned as LITERAL text that must
# appear in the script's lockout failure AND somewhere in the runbook
# (criterion 13). Honest scope: this keeps each fragment from being dropped
# or reworded on either side; it does not prove the two render one identical
# complete sentence.
RECOVERY_PHRASES=(
  'ssh-hardening.sh --rollback'
  'Screen Sharing over the tailnet'
  'Remote Login off and back on'
  'keep any SSH session you still have OPEN until a new login succeeds'
  'sudo rm'
)

# The reload's retry loop is attempt-bounded through these knobs; every case
# that expects failure keeps the suite fast by using two attempts with no
# sleep between them.
export SSH_HARDENING_READY_ATTEMPTS=2
export SSH_HARDENING_READY_INTERVAL=0

reload_sandbox_setup
trap 'ssh_sandbox_teardown' EXIT

dropin="$SSHD_CONFIG_D/$SSH_DROPIN_NAME"
write_hardened_dropin
baseline_fingerprint="$(config_tree_fingerprint)"

# --- 1: sudo failure: named as sudo, never mistaken for a stopped daemon -----

SSH_HARDENING_SUDO="$SSH_SUDO_DENY_STUB" run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '1: a sudo failure must fail the reload'
assert_no_kickstart '1'
[[ ! -s $LAUNCHCTL_SPY_LOG ]] ||
  fail "1: with sudo unavailable, the service must not even be probed (spy: $(cat "$LAUNCHCTL_SPY_LOG"))"
grep -q '^-v$' "$SUDO_DENY_SPY_LOG" ||
  fail '1: privilege must be primed visibly via sudo -v before anything else'
[[ "$(grep -c . "$SUDO_DENY_SPY_LOG")" -eq 1 ]] ||
  fail "1: after the failed priming NOTHING else may run through the wrapper (deny spy: $(cat "$SUDO_DENY_SPY_LOG"))"
grep -qi 'sudo' <<<"$SSH_RUN_ERR" ||
  fail "1: the failure must name sudo (stderr: $SSH_RUN_ERR)"
grep -qi 'privilege escalation' <<<"$SSH_RUN_ERR" ||
  fail "1: the failure must be ATTRIBUTED to privilege escalation, not to a downstream step that happened to mention sudo (stderr: $SSH_RUN_ERR)"
refute_contains "$SSH_RUN_ERR" 'not loaded' \
  '1: a sudo failure must never be reported as the service not loaded'
refute_contains "$SSH_RUN_ERR" 'not running' \
  '1: a sudo failure must never be reported as sshd not running'

# --- 2: sshd -t failure: nonzero, no kickstart, names syntax ------------------

SSHD_STUB_SYNTAX_STATUS=78 run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '2: a failed syntax check must fail the reload'
assert_no_kickstart '2'
[[ ! -s $LAUNCHCTL_SPY_LOG ]] ||
  fail '2: validation must fail BEFORE the service is probed'
grep -qi 'syntax' <<<"$SSH_RUN_ERR" ||
  fail "2: the failure must name syntax (stderr: $SSH_RUN_ERR)"

# --- 3: hardening lost: nonzero, no kickstart, not fully hardened -------------

rm -f "$dropin"
run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '3: a tree that lost the hardening must fail the reload'
assert_no_kickstart '3'
[[ ! -s $LAUNCHCTL_SPY_LOG ]] ||
  fail '3: verification must fail BEFORE the service is probed'
grep -qi 'not fully hardened' <<<"$SSH_RUN_ERR" ||
  fail "3: the failure must say the configuration is not fully hardened (stderr: $SSH_RUN_ERR)"
[[ ! -e $dropin ]] ||
  fail '3: --reload must never write the drop-in (criterion 12)'
write_hardened_dropin

# --- 4: service confirmed absent (113): exit 0, no kickstart, explained -------

LAUNCHCTL_STUB_PRINT_STATUSES=113 run_ssh_reload --reload
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "4: a confirmed-absent service is a clean no-op, got exit $SSH_RUN_STATUS (stderr: $SSH_RUN_ERR)"
assert_no_kickstart '4'
grep -qi 'Remote Login' <<<"$SSH_RUN_OUT" ||
  fail "4: stdout must explain the service follows Remote Login (stdout: $SSH_RUN_OUT)"
grep -qi 'next enabled' <<<"$SSH_RUN_OUT" ||
  fail "4: stdout must explain the drop-in applies when Remote Login is next enabled (stdout: $SSH_RUN_OUT)"

# --- 5: probe error (neither 0 nor 113): nonzero, no kickstart ----------------

LAUNCHCTL_STUB_PRINT_STATUSES=150 run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '5: an errored probe must fail the reload, never pass as a stopped service'
assert_no_kickstart '5'
grep -qi 'could not determine' <<<"$SSH_RUN_ERR" ||
  fail "5: the failure must say the state could not be determined (stderr: $SSH_RUN_ERR)"
refute_contains "$SSH_RUN_ERR" 'not loaded' \
  '5: an errored probe must never be reported as not loaded'
refute_contains "$SSH_RUN_ERR" 'not running' \
  '5: an errored probe must never be reported as not running'

# --- 5b: a wrapper failure must never read as "Remote Login is off" -----------
# A privilege wrapper that exits 113 for launchctl WITHOUT running it used to
# be read as the confirmed-absent status: the reload printed its clean no-op
# and exited 0 having restarted nothing. Only a status proven to come from
# launchctl may mean absent.

SSH_HARDENING_SUDO="$SSH_SUDO_LAUNCHCTL_113_STUB" run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '5b: a wrapper exiting 113 without running launchctl must not produce the clean no-op'
refute_contains "$SSH_RUN_OUT" 'no daemon to restart' \
  '5b: a wrapper failure must never be reported as Remote Login being off'

# --- 6: kickstart fails: nonzero, kickstart WAS attempted, named --------------

LAUNCHCTL_STUB_KICKSTART_STATUS=9 run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '6: a failed kickstart must fail the reload'
assert_kickstart_attempted '6'
grep -qi 'kickstart' <<<"$SSH_RUN_ERR" ||
  fail "6: the failure must name kickstart (stderr: $SSH_RUN_ERR)"
grep -qF 'ssh-hardening.sh --rollback' <<<"$SSH_RUN_ERR" ||
  fail "6: a failure AFTER the disruptive step began must name the recovery path (stderr: $SSH_RUN_ERR)"

# --- 7: job does not reload after kickstart: nonzero, says so -----------------

LAUNCHCTL_STUB_PRINT_STATUSES='0 113' run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '7: a job that does not come back loaded must fail the reload'
assert_kickstart_attempted '7'
grep -qi 'did not reload' <<<"$SSH_RUN_ERR" ||
  fail "7: the failure must say the job did not reload (stderr: $SSH_RUN_ERR)"
grep -qF 'ssh-hardening.sh --rollback' <<<"$SSH_RUN_ERR" ||
  fail "7: a failure AFTER the disruptive step must name the recovery path (stderr: $SSH_RUN_ERR)"

# --- 8: loaded but no banner: nonzero, lockout warning, recovery path ---------

KEYSCAN_STUB_MODE=refuse run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '8: a listener that never answers must fail the reload'
assert_kickstart_attempted '8'
grep -qi 'possible lockout' <<<"$SSH_RUN_ERR" ||
  fail "8: the failure must warn about a possible lockout (stderr: $SSH_RUN_ERR)"
for phrase in "${RECOVERY_PHRASES[@]}"; do
  grep -qF -- "$phrase" <<<"$SSH_RUN_ERR" ||
    fail "8: the lockout failure must carry the recovery instruction '$phrase' (stderr: $SSH_RUN_ERR)"
done
keyscan_attempts="$(grep -c . "$KEYSCAN_SPY_LOG")" || true
[[ $keyscan_attempts -eq $SSH_HARDENING_READY_ATTEMPTS ]] ||
  fail "8: the probe must retry to the attempt bound and then STOP, got $keyscan_attempts attempts (want $SSH_HARDENING_READY_ATTEMPTS)"
# The probe port comes from `sshd -G`, but on macOS launchd owns Remote
# Login's listening socket and the Port directive does not move it, so a
# nonstandard Port makes this exact alarm fire against a HEALTHY daemon. The
# lockout text must hand the operator that diagnosis instead of only an
# emergency.
grep -qi 'launchd owns Remote Login' <<<"$SSH_RUN_ERR" ||
  fail "8: the lockout failure must name the launchd-socket possibility so a port mismatch reads as a diagnosis, not a false emergency (stderr: $SSH_RUN_ERR)"
grep -qF -- '-p 22 127.0.0.1' <<<"$SSH_RUN_ERR" ||
  fail "8: the lockout failure must hand the operator the exact port-22 check command (stderr: $SSH_RUN_ERR)"
# The sudo rm fallback must name the CONCRETE file, so it works when the
# script itself is broken.
grep -qF "sudo rm $SSHD_CONFIG_D/$SSH_DROPIN_NAME" <<<"$SSH_RUN_ERR" ||
  fail "8: the sudo rm fallback must name the exact drop-in path (stderr: $SSH_RUN_ERR)"

# A probe that exits 0 with NO banner output proved nothing: the exit status
# is a proxy, the banner is the artifact, and trusting the proxy would report
# green over a crashed sshd.
KEYSCAN_STUB_MODE=silent-zero run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '8: a keyscan that exits 0 with no banner output must still fail the reload'
grep -qi 'possible lockout' <<<"$SSH_RUN_ERR" ||
  fail "8: the silent-success probe must also warn about a possible lockout (stderr: $SSH_RUN_ERR)"

# A probe that exits 0 printing text that is NOT a host-key record proved
# nothing either: the real tool prints host/type/key lines for a completed
# exchange, so arbitrary output through an overridden seam must not satisfy
# readiness.
KEYSCAN_STUB_MODE=garbage-zero run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '8: a keyscan that exits 0 with non-record output must still fail the reload'
grep -qi 'possible lockout' <<<"$SSH_RUN_ERR" ||
  fail "8: the garbage-output probe must also warn about a possible lockout (stderr: $SSH_RUN_ERR)"

# The STATUS half of the readiness conjunction: a valid-looking record with a
# nonzero exit must not count. This shape is HYPOTHETICAL for the real tool
# (measured: a reachable host prints 8 lines and exits 0, an unreachable one
# prints 0 lines and exits 1; no output-plus-nonzero combination could be
# produced), so this pins the seam contract, not a live defect.
KEYSCAN_STUB_MODE=banner-nonzero run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '8: a keyscan that prints a record but exits nonzero must still fail the reload'
grep -qi 'possible lockout' <<<"$SSH_RUN_ERR" ||
  fail "8: the nonzero-status probe must also warn about a possible lockout (stderr: $SSH_RUN_ERR)"

# --- 9: readiness prover unavailable: refuse BEFORE anything disruptive -------

KEYSCAN_BIN="$SSH_SANDBOX/no-such-keyscan" run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '9: with no way to prove the daemon came back, --reload must refuse to run'
assert_no_kickstart '9'
[[ ! -s $LAUNCHCTL_SPY_LOG ]] ||
  fail '9: the refusal must come before the service is even probed'
grep -qi 'refusing' <<<"$SSH_RUN_ERR" ||
  fail "9: the refusal must be explicit (stderr: $SSH_RUN_ERR)"
grep -qF "$SSH_SANDBOX/no-such-keyscan" <<<"$SSH_RUN_ERR" ||
  fail "9: the refusal must name the missing prover (stderr: $SSH_RUN_ERR)"

# --- 10: happy path: kickstart attempted, banner proven, port named -----------

run_ssh_reload --reload
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "10: the happy path must succeed (stderr: $SSH_RUN_ERR)"
assert_kickstart_attempted '10'
grep -qi 'accepting connections' <<<"$SSH_RUN_OUT" ||
  fail "10: stdout must confirm sshd is accepting connections (stdout: $SSH_RUN_OUT)"
grep -qF 'port 2222' <<<"$SSH_RUN_OUT" ||
  fail "10: stdout must name the RESOLVED port, 2222 from the stub's sshd -G (stdout: $SSH_RUN_OUT)"
grep -qF -- '-p 2222' "$KEYSCAN_SPY_LOG" ||
  fail "10: the readiness probe must target the resolved port (keyscan spy: $(cat "$KEYSCAN_SPY_LOG"))"
[[ "$(config_tree_fingerprint)" == "$baseline_fingerprint" ]] ||
  fail '10: a reload must leave every regular file under the drop-in directory unchanged in path and content (criterion 12; see config_tree_fingerprint for what this does not cover)'

# The complete ordered argv of every seam call in the happy path, diffed as
# one list: the syntax check names the REAL main config (sshd -G -f
# /dev/null exits 0, measured, so a wrong -f target passes the gate), the
# child verify resolves both connection samples, port resolution happens
# BEFORE the kickstart, the kickstart carries -k and the exact service
# label, and the readiness probe targets loopback on the resolved port with
# the configured timeout.
invoking_user="$(id -un)"
assert_seam_calls '10' \
  "sshd -t -f $SSHD_MAIN_CONFIG" \
  "sshd -G -f $SSHD_MAIN_CONFIG" \
  "sshd -G -T -C user=root,host=localhost,addr=127.0.0.1 -f $SSHD_MAIN_CONFIG" \
  "sshd -G -T -C user=$invoking_user,host=localhost,addr=127.0.0.1 -f $SSHD_MAIN_CONFIG" \
  "sshd -G -f $SSHD_MAIN_CONFIG" \
  'launchctl print system/com.openssh.sshd' \
  'launchctl kickstart -k system/com.openssh.sshd' \
  'launchctl print system/com.openssh.sshd' \
  'ssh-keyscan -T 5 -p 2222 127.0.0.1'

# What ran through the privilege wrapper, exactly and in order: the priming
# -v, the syntax check, and the kickstart. The service probes are absent
# deliberately (they run unprivileged so their status is launchctl's own).
expected_sudo_calls="$SSH_SANDBOX/expected-sudo-calls"
printf '%s\n' '-v' \
  "$SSHD_BIN -t -f $SSHD_MAIN_CONFIG" \
  "$LAUNCHCTL_BIN kickstart -k system/com.openssh.sshd" >"$expected_sudo_calls"
diff -u "$expected_sudo_calls" "$SUDO_OK_SPY_LOG" >&2 ||
  fail '10: the privileged calls (or their order/argv) differ from expected (diff above)'

# --- 11: the keep-open warning is printed BEFORE the kickstart ----------------
# The kickstart is the step that can kill the SSH session carrying the
# output, so the warning and the COMPLETE recovery command must already be
# out before it runs. Asserted against the launchctl stub's snapshot of the
# script's output taken AT the kickstart call, so moving the warning back
# after the kickstart fails here even though the final output still carries
# it.
pre_kickstart_output="$( (cat "$SSH_STUB_STATE/stdout-at-kickstart" && cat "$SSH_STUB_STATE/stderr-at-kickstart") 2>/dev/null || :)"
grep -qi 'about to restart sshd' <<<"$pre_kickstart_output" ||
  fail "11: the warning must be printed before the kickstart (snapshot: $pre_kickstart_output)"
for phrase in "${RECOVERY_PHRASES[@]}"; do
  grep -qF -- "$phrase" <<<"$pre_kickstart_output" ||
    fail "11: the recovery instruction '$phrase' must be printed before the kickstart (snapshot: $pre_kickstart_output)"
done

# --- 12: the default install mode never reloads -------------------------------

rm -f "$dropin"
run_ssh_reload
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "12: install must succeed in the sandbox (stderr: $SSH_RUN_ERR)"
[[ -f $dropin ]] ||
  fail '12: install must write the drop-in'
[[ ! -s $LAUNCHCTL_SPY_LOG ]] ||
  fail "12: install must never touch launchctl (spy: $(cat "$LAUNCHCTL_SPY_LOG"))"
[[ ! -s $KEYSCAN_SPY_LOG ]] ||
  fail "12: install must never probe the listener (spy: $(cat "$KEYSCAN_SPY_LOG"))"

# --- 17: port resolution fails closed BEFORE the kickstart --------------------
# The port branch had no test at all; every shape that cannot be probed must
# refuse while nothing has been disturbed, and every DECLARED port must be
# probed rather than silently the first (two Port directives produce two
# `port` lines from the real binary, measured).

# 17a: `sshd -G` itself fails at the port-resolution call (the fourth -G of
# the run; the three before it belong to the child verify and must succeed).
SSHD_STUB_RESOLVE_STATUSES='0 0 0 3' run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '17a: a failed port resolution must fail the reload'
assert_no_kickstart '17a'
grep -qi 'could not resolve the effective sshd port' <<<"$SSH_RUN_ERR" ||
  fail "17a: the failure must name the port resolution (stderr: $SSH_RUN_ERR)"

# 17b: -G output with NO port line at all.
SSHD_STUB_PORT='' run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '17b: portless -G output must fail the reload'
assert_no_kickstart '17b'
grep -qi 'no port' <<<"$SSH_RUN_ERR" ||
  fail "17b: the failure must say no port resolved (stderr: $SSH_RUN_ERR)"

# 17c: ports outside the valid range, both sides, plus a non-number.
for bad_port in 0 70000 007 abc; do
  SSHD_STUB_PORT="$bad_port" run_ssh_reload --reload
  [[ $SSH_RUN_STATUS -ne 0 ]] ||
    fail "17c: port '$bad_port' must be refused"
  assert_no_kickstart "17c: port '$bad_port'"
  grep -qF "'$bad_port'" <<<"$SSH_RUN_ERR" ||
    fail "17c: the refusal must name the bad port '$bad_port' (stderr: $SSH_RUN_ERR)"
done

# 17d: TWO declared ports, only the second answering: the probe must walk
# both and succeed, naming the port that answered.
SSHD_STUB_PORT='2222 2223' KEYSCAN_STUB_ANSWER_PORT=2223 run_ssh_reload --reload
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "17d: with the second declared port answering, the reload must succeed (stderr: $SSH_RUN_ERR)"
grep -qF -- '-p 2222' "$KEYSCAN_SPY_LOG" ||
  fail "17d: the first declared port must have been probed (spy: $(cat "$KEYSCAN_SPY_LOG"))"
grep -qF -- '-p 2223' "$KEYSCAN_SPY_LOG" ||
  fail "17d: the second declared port must have been probed (spy: $(cat "$KEYSCAN_SPY_LOG"))"
grep -qF 'port 2223' <<<"$SSH_RUN_OUT" ||
  fail "17d: success must name the port that actually answered (stdout: $SSH_RUN_OUT)"

# --- 16: the retry delay is a validated seam, never a bare sleep --------------
# sleep is /bin/sleep on this platform, an external binary and not a builtin,
# so under a stripped PATH a bare `sleep` is exit 127 INSIDE the readiness
# loop: an abort AFTER the disruptive step, under set -e, printing nothing.
# The delay tool must be resolved and validated BEFORE the kickstart, and a
# nonzero interval must pace through the seam, observably.

# 16a: a nonzero interval paces through SLEEP_BIN: three refused probes make
# exactly two inter-probe delays of the requested length.
SSH_HARDENING_READY_ATTEMPTS=3 SSH_HARDENING_READY_INTERVAL=1 \
  KEYSCAN_STUB_MODE=refuse run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '16a: three refused probes must still be a lockout failure'
keyscan_attempts="$(grep -c . "$KEYSCAN_SPY_LOG")" || true
[[ $keyscan_attempts -eq 3 ]] ||
  fail "16a: expected 3 probes, got $keyscan_attempts"
sleep_calls="$(grep -c . "$SLEEP_SPY_LOG")" || true
[[ $sleep_calls -eq 2 ]] ||
  fail "16a: 3 attempts must sleep exactly twice through the seam, got $sleep_calls (spy: $(cat "$SLEEP_SPY_LOG"))"
grep -qx '1' "$SLEEP_SPY_LOG" ||
  fail "16a: the delay must be the requested interval (spy: $(cat "$SLEEP_SPY_LOG"))"

# 16b: an unavailable delay tool must refuse BEFORE the kickstart, not abort
# silently after it.
SSH_HARDENING_READY_INTERVAL=1 SLEEP_BIN="$SSH_SANDBOX/no-such-sleep" \
  run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '16b: a missing delay tool must fail the reload'
assert_no_kickstart '16b'
grep -qF "$SSH_SANDBOX/no-such-sleep" <<<"$SSH_RUN_ERR" ||
  fail "16b: the refusal must name the missing delay tool (stderr: $SSH_RUN_ERR)"

# 16c: a delay tool that fails MID-loop (after the kickstart) must die
# NAMING the delay failure and carrying the recovery text, never abort bare.
failing_sleep="$SSH_SANDBOX/failing-sleep"
printf '#!/bin/bash\nexit 9\n' >"$failing_sleep"
chmod +x "$failing_sleep"
SSH_HARDENING_READY_ATTEMPTS=2 SSH_HARDENING_READY_INTERVAL=1 \
  SLEEP_BIN="$failing_sleep" KEYSCAN_STUB_MODE=refuse run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '16c: a mid-loop delay failure must fail the reload'
assert_kickstart_attempted '16c'
grep -qi 'retry delay' <<<"$SSH_RUN_ERR" ||
  fail "16c: the failure must be attributed to the retry delay (stderr: $SSH_RUN_ERR)"
grep -qF 'ssh-hardening.sh --rollback' <<<"$SSH_RUN_ERR" ||
  fail "16c: a failure after the disruptive step must carry the recovery path (stderr: $SSH_RUN_ERR)"

# --- 15: readiness knobs are validated BEFORE anything runs -------------------
# The property: attempts and the probe timeout are one canonical base-10
# positive integer (bash arithmetic reads a leading zero as base-8, so 010
# meant 8 probes and 08 died mid-loop; 0 and 00 bounded the loop at zero
# probes and reported POSSIBLE LOCKOUT on a healthy machine); the interval is
# a canonical non-negative decimal. Set-but-EMPTY is an operator statement
# and is refused, never silently rewritten to the default.

for bad_attempts in '' '0' '00' '08' '010' '+1' '1x' '1 0'; do
  SSH_HARDENING_READY_ATTEMPTS="$bad_attempts" run_ssh_reload --reload
  [[ $SSH_RUN_STATUS -ne 0 ]] ||
    fail "15: SSH_HARDENING_READY_ATTEMPTS='$bad_attempts' must be refused"
  assert_no_kickstart "15: attempts '$bad_attempts'"
  [[ ! -s $LAUNCHCTL_SPY_LOG ]] ||
    fail "15: attempts '$bad_attempts' must be refused before the service is probed"
  grep -q 'SSH_HARDENING_READY_ATTEMPTS' <<<"$SSH_RUN_ERR" ||
    fail "15: the refusal of attempts '$bad_attempts' must name the knob (stderr: $SSH_RUN_ERR)"
done

for bad_interval in '' '.' '1.2.3' 'abc' '00' '01'; do
  SSH_HARDENING_READY_INTERVAL="$bad_interval" run_ssh_reload --reload
  [[ $SSH_RUN_STATUS -ne 0 ]] ||
    fail "15: SSH_HARDENING_READY_INTERVAL='$bad_interval' must be refused"
  assert_no_kickstart "15: interval '$bad_interval'"
  grep -q 'SSH_HARDENING_READY_INTERVAL' <<<"$SSH_RUN_ERR" ||
    fail "15: the refusal of interval '$bad_interval' must name the knob (stderr: $SSH_RUN_ERR)"
done

for bad_timeout in '' '0' '05' '5x'; do
  SSH_HARDENING_PROBE_TIMEOUT="$bad_timeout" run_ssh_reload --reload
  [[ $SSH_RUN_STATUS -ne 0 ]] ||
    fail "15: SSH_HARDENING_PROBE_TIMEOUT='$bad_timeout' must be refused"
  assert_no_kickstart "15: timeout '$bad_timeout'"
  grep -q 'SSH_HARDENING_PROBE_TIMEOUT' <<<"$SSH_RUN_ERR" ||
    fail "15: the refusal of timeout '$bad_timeout' must name the knob (stderr: $SSH_RUN_ERR)"
done

# Accepted shapes. attempts=10 must mean TEN probes (base-10, not base-8);
# the probe timeout must reach the keyscan argv; a fractional interval is
# legal (the global interval stays 0 elsewhere so the suite never sleeps).
SSH_HARDENING_READY_ATTEMPTS=10 KEYSCAN_STUB_MODE=refuse run_ssh_reload --reload
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail '15: ten refused probes must still be a lockout failure'
keyscan_attempts="$(grep -c . "$KEYSCAN_SPY_LOG")" || true
[[ $keyscan_attempts -eq 10 ]] ||
  fail "15: SSH_HARDENING_READY_ATTEMPTS=10 must probe exactly 10 times, got $keyscan_attempts"

SSH_HARDENING_PROBE_TIMEOUT=7 run_ssh_reload --reload
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "15: a legal probe timeout must not break the happy path (stderr: $SSH_RUN_ERR)"
grep -qF -- '-T 7' "$KEYSCAN_SPY_LOG" ||
  fail "15: the probe timeout knob must reach the keyscan argv (spy: $(cat "$KEYSCAN_SPY_LOG"))"

SSH_HARDENING_READY_INTERVAL=0.5 run_ssh_reload --reload
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "15: a fractional interval is legal and must not be refused (stderr: $SSH_RUN_ERR)"

# --- 14: mode dispatch is case-sensitive --------------------------------------
# nocasematch is on at file scope for sshd keyword matching; if it reaches
# main's case, a mistyped `--RELOAD` invokes the ONE disruptive mode in the
# script. Both disruptive spellings must be refused as unknown flags while
# the exact lowercase spelling keeps working (the happy-path case above pins
# that side).

run_ssh_reload --RELOAD
[[ $SSH_RUN_STATUS -eq 2 ]] ||
  fail "14: --RELOAD must be an unknown-flag usage error (exit 2), got $SSH_RUN_STATUS"
assert_no_kickstart '14'
[[ ! -s $LAUNCHCTL_SPY_LOG ]] ||
  fail "14: --RELOAD must not touch the service at all (spy: $(cat "$LAUNCHCTL_SPY_LOG"))"
grep -qi 'usage' <<<"$SSH_RUN_ERR" ||
  fail "14: --RELOAD must print usage (stderr: $SSH_RUN_ERR)"

run_ssh_reload --Rollback
[[ $SSH_RUN_STATUS -eq 2 ]] ||
  fail "14: --Rollback must be an unknown-flag usage error (exit 2), got $SSH_RUN_STATUS"
grep -qi 'usage' <<<"$SSH_RUN_ERR" ||
  fail "14: --Rollback must print usage (stderr: $SSH_RUN_ERR)"

# --- 13: the recovery path lives in the runbook, as the same literal text -----

[[ -f $RUNBOOK ]] ||
  fail "13: the runbook is missing at $RUNBOOK"
runbook_content="$(cat "$RUNBOOK")"
for phrase in "${RECOVERY_PHRASES[@]}"; do
  grep -qF -- "$phrase" <<<"$runbook_content" ||
    fail "13: the runbook must carry the recovery instruction '$phrase'"
done
grep -qF 'sudo rm /etc/ssh/sshd_config.d/000-ssh-hardening.conf' <<<"$runbook_content" ||
  fail '13: the runbook sudo rm fallback must name the LIVE drop-in path'

printf 'ssh-hardening-reload-failclosed: OK (every unprovable step refuses before the kickstart, every failure after it names the way back in)\n'
