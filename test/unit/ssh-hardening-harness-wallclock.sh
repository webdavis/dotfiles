#!/usr/bin/env bash
# ssh-hardening-harness-wallclock.sh -- run_ssh_hardening (the runner every
# ssh-hardening suite drives the script through) must FAIL the suite when the
# script under test spins, never hang it. A hang is strictly worse than a
# failure: it blocks the pre-push gate with no diagnosis, and on CI it burns
# the job's whole time budget before being killed. run_ssh_reload already
# carries a wall clock; this suite pins the same property onto the base
# runner, because every tokenizer and verify test reaches the script through
# run_ssh_hardening and a spinning --verify child was observed to hang the
# whole suite under mutation.
#
# Properties pinned:
#   1. a script that spins forever makes run_ssh_hardening ABORT nonzero
#      within its wall clock (seconds), naming the wall clock in the failure
#   2. a well-behaved script still runs to completion through the SAME bound,
#      with SSH_RUN_STATUS / SSH_RUN_OUT captured as before
set -euo pipefail

# Scrubbed at SCRIPT scope. Git exports GIT_DIR to every hook it runs and this
# suite runs from the pre-push hook.
unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

# shellcheck source=../fixtures/ssh-hardening-lib.bash
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../fixtures" && pwd)/ssh-hardening-lib.bash"

ssh_sandbox_setup
trap 'ssh_sandbox_teardown' EXIT

# --- 1: a spinning script fails the runner in seconds, not never --------------
# The runner aborts with `exit 1` on timeout, so it runs inside a subshell
# here and the abort is read out of the subshell's status. The elapsed-time
# cap is deliberately loose (the limit is 2s, the cap 15s): it separates
# "failed within the wall clock" from "hung until something external killed
# it", not one second from two.

spin_script="$SSH_SANDBOX/spin.sh"
printf '#!/bin/bash\nwhile :; do :; done\n' >"$spin_script"
chmod +x "$spin_script"

started_at=$SECONDS
spin_status=0
spin_output="$(
  SSH_HARDENING_SCRIPT="$spin_script" SSH_HARDENING_TIME_LIMIT=2 \
    run_ssh_hardening --verify 2>&1
)" || spin_status=$?
elapsed=$((SECONDS - started_at))

[[ $spin_status -ne 0 ]] ||
  fail "1: a spinning script under test must abort run_ssh_hardening nonzero (output: $spin_output)"
[[ $elapsed -le 15 ]] ||
  fail "1: the abort took ${elapsed}s; the wall clock was 2s, so this is a hang, not a bound"
grep -qi 'wall clock' <<<"$spin_output" ||
  fail "1: the failure must name the wall clock so the hang is diagnosable (output: $spin_output)"

# The spinning child must not outlive the runner: a leaked spinner burns CPU
# for the rest of the suite and on CI for the rest of the job.
if pgrep -f "$spin_script" >/dev/null 2>&1; then
  pkill -9 -f "$spin_script" 2>/dev/null || true
  fail '1: the spinning child survived the abort; the runner must kill what it timed out'
fi

# --- 2: a well-behaved script still completes through the same bound ----------

well_behaved_script="$SSH_SANDBOX/well-behaved.sh"
printf '#!/bin/bash\nprintf "ran to completion\\n"\nexit 0\n' >"$well_behaved_script"
chmod +x "$well_behaved_script"

SSH_HARDENING_SCRIPT="$well_behaved_script" run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "2: a well-behaved script must still succeed through the bounded runner (status $SSH_RUN_STATUS, stderr: $SSH_RUN_ERR)"
grep -qF 'ran to completion' <<<"$SSH_RUN_OUT" ||
  fail "2: SSH_RUN_OUT must still capture the script's stdout (got: $SSH_RUN_OUT)"

printf 'ssh-hardening-harness-wallclock: OK (a spinning script fails the runner inside its wall clock; a healthy one still completes)\n'
