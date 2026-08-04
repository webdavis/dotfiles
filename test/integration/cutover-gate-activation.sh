#!/usr/bin/env bash
# cutover-gate-activation.sh: gate 2 of the D1 cutover runner.
#
# Gate 2 is staged activation plus execution of the approved retirement. It runs
# in two stages because the operator's interactive `chezmoi apply` sits between
# them: the pre-apply stage re-verifies both pins fail-closed and lands the live
# checkout ATTACHED to main at the pinned SHA; --post-apply then boots out
# exactly the manifest approved at gate 1 and verifies remote reachability.
#
# Everything runs against a sandbox $HOME (see helpers/cutover-sandbox.sh) with
# stubbed launchctl/tailscale boundaries. The runner never performs the apply
# itself, so no chezmoi apply is invoked anywhere here.
#
# Cases:
#   A. the pre-apply stage refuses without a second remote session
#   B. gate 2 refuses while gate 1 has not passed
#   C. a moved pin refuses and lands nothing
#   D. happy path: attached landing at the pin, operator checkpoint
#   E. a dirty tree refuses immediately before the apply
#   F. --post-apply refuses before the landing stage ran
#   G. happy path: exactly the approved labels booted out, domain-qualified,
#      then Tailscale and sshd reachability
#   H. a failed bootout refuses and does not mark the gate passed
#   I. a label loaded AFTER approval is never retired ad hoc
#   J. unreachable Tailscale refuses
#   K. sshd absent refuses
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR
unset XDG_CONFIG_HOME
export GIT_CONFIG_NOSYSTEM=1

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUNNER="$REPO_ROOT/scripts/cutover-gate.sh"
source "$REPO_ROOT/test/integration/helpers/cutover-sandbox.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failures=0
report() {
  local status="$1" msg="$2"
  if [[ $status == ok ]]; then
    printf '  ok   %s\n' "$msg"
  else
    printf '  FAIL %s\n' "$msg"
    failures=$((failures + 1))
  fi
}

[[ -x $RUNNER ]] || {
  printf 'FAIL: missing or non-executable runner: %s\n' "$RUNNER" >&2
  exit 1
}

uid="$(id -u)"
stub_dir="$work/stubs"
export LOADED_GUI="$work/loaded-gui"
export LOADED_SYSTEM="$work/loaded-system"
export LAUNCHCTL_LOG="$work/launchctl.log"
export CMD_LOG="$work/cmd.log"
export PRINT_DETAIL_DIR="$work/print-detail"
export CHEZMOI_DATA_FILE="$work/render.json"
mkdir -p "$stub_dir" "$PRINT_DETAIL_DIR"
: >"$LAUNCHCTL_LOG"
: >"$CMD_LOG"
cutover_make_launchctl_stub "$stub_dir"
cutover_make_command_stubs "$stub_dir"

out_file="$work/stdout"
err_file="$work/stderr"
neutral="$work/neutral"
mkdir -p "$neutral"
# Checklist item 6: the runner is always invoked from a cwd outside any git
# repository, so it can only be operating through its absolute repo handle.
run_gate() {
  local home="$1"
  shift
  RC=0
  : >"$LAUNCHCTL_LOG"
  : >"$CMD_LOG"
  (
    cd "$neutral" || exit 1
    HOME="$home" PATH="$stub_dir:$PATH" CUTOVER_PHASE_A_BASE="$SANDBOX_BASE" \
      env "$@"
  ) >"$out_file" 2>"$err_file" || RC=$?
}

# gate <home> <args...> : run_gate wrapper for the common no-extra-env case.
gate() {
  local home="$1"
  shift
  run_gate "$home" "$RUNNER" "$@"
}

ledger_of() { printf '%s/.local/state/cutover' "$1"; }

seed_loaded() {
  : >"$LOADED_GUI"
  : >"$LOADED_SYSTEM"
  local arg
  for arg in "$@"; do
    case "$arg" in
      system:*) printf '%s\n' "${arg#system:}" >>"$LOADED_SYSTEM" ;;
      *) printf '%s\n' "$arg" >>"$LOADED_GUI" ;;
    esac
  done
}

# build <home> : sandbox with the openclaw orphan loaded, so the retirement
# manifest gate 1 proposes is non-empty.
build() {
  local home="$1"
  cutover_build_sandbox "$home"
  cutover_write_classification "$home"
  cutover_make_home_scripts "$home"
  seed_loaded com.github.openclaw-setup.watchdog com.webdavis.atuin-daemon \
    system:com.openssh.sshd
}

# approve_gate1 <home> : run gate 1 and approve its manifest.
approve_gate1() {
  local home="$1"
  gate "$home" 1
  gate "$home" 1 --approve-retirement
  [[ -f "$(ledger_of "$home")/gate1.done" ]] ||
    printf 'FAIL: gate 1 setup did not pass for %s (err: %s)\n' "$home" "$(cat "$err_file")" >&2
}

# advance_origin <home> <message> : push a commit to origin/main that the local
# checkout does not have. Echoes the new origin/main SHA.
advance_origin() {
  local home="$1" message="$2" side
  side="$(mktemp -d "$work/side.XXXXXX")"
  cutover_git clone --quiet "$home/origin.git" "$side"
  printf '%s\n' "$message" >"$side/advance.txt"
  cutover_git -C "$side" add -A
  cutover_git -C "$side" commit --quiet -m "$message"
  cutover_git -C "$side" push --quiet origin main
  git -C "$side" rev-parse HEAD
}

prepare() {
  build "$1"
  approve_gate1 "$1"
}

printf 'cutover-gate-activation cases:\n'

# ── Case A: no second remote session ───────────────────────────────────────
hA="$work/homeA"
prepare "$hA"
head_before="$(git -C "$hA/workspaces/Ivy/webdavis/dotfiles" rev-parse HEAD)"
gate "$hA" 2
if [[ $RC -eq 1 ]] && grep -qi 'second' "$err_file"; then
  report ok "A: refuses without a second remote session"
else
  report bad "A: expected a second-session refusal (rc=$RC, err: $(cat "$err_file"))"
fi
if [[ "$(git -C "$hA/workspaces/Ivy/webdavis/dotfiles" rev-parse HEAD)" == "$head_before" ]]; then
  report ok "A: nothing landed"
else
  report bad "A: the checkout moved despite the refusal"
fi

# ── Case B: gate 1 has not passed ──────────────────────────────────────────
hB="$work/homeB"
prepare "$hB"
rm -f "$(ledger_of "$hB")/gate1.done"
gate "$hB" 2 --second-session-open
if [[ $RC -eq 1 ]] && grep -qi 'gate 1' "$err_file"; then
  report ok "B: refuses while gate 1 has not passed"
else
  report bad "B: ran without a passed gate 1 (rc=$RC, err: $(cat "$err_file"))"
fi

# ── Case B2: pins that cannot be trusted ───────────────────────────────────
# Checklist item 7: pins are reloaded from the ledger and each is validated as a
# full 40-hex SHA before use; a short, empty, or missing pin aborts.
hB2="$work/homeB2"
prepare "$hB2"
head_before="$(git -C "$hB2/workspaces/Ivy/webdavis/dotfiles" rev-parse HEAD)"
printf 'MAIN_SHA=abc123\nINT_SHA=\n' >"$(ledger_of "$hB2")/pins.env"
gate "$hB2" 2 --second-session-open
if [[ $RC -eq 1 ]] && grep -q '40-hex' "$err_file"; then
  report ok "B2: a truncated pin aborts before anything is compared"
else
  report bad "B2: a truncated pin was accepted (rc=$RC, err: $(cat "$err_file"))"
fi
if [[ "$(git -C "$hB2/workspaces/Ivy/webdavis/dotfiles" rev-parse HEAD)" == "$head_before" ]]; then
  report ok "B2: nothing landed on a bad pin"
else
  report bad "B2: the checkout moved on a bad pin"
fi
rm -f "$(ledger_of "$hB2")/pins.env"
gate "$hB2" 2 --second-session-open
if [[ $RC -eq 1 ]] && grep -qi 'no pins recorded' "$err_file"; then
  report ok "B2: a missing pins file aborts"
else
  report bad "B2: a missing pins file was tolerated (rc=$RC, err: $(cat "$err_file"))"
fi

# ── Case C: a moved pin refuses ────────────────────────────────────────────
hC="$work/homeC"
prepare "$hC"
advance_origin "$hC" 'a commit that moved the pin' >/dev/null
head_before="$(git -C "$hC/workspaces/Ivy/webdavis/dotfiles" rev-parse HEAD)"
gate "$hC" 2 --second-session-open
if [[ $RC -eq 1 ]] && grep -qi 'restart' "$err_file"; then
  report ok "C: a moved pin refuses and says to restart at gate 1"
else
  report bad "C: a moved pin did not refuse (rc=$RC, err: $(cat "$err_file"))"
fi
if [[ "$(git -C "$hC/workspaces/Ivy/webdavis/dotfiles" rev-parse HEAD)" == "$head_before" ]]; then
  report ok "C: nothing landed after the pin mismatch"
else
  report bad "C: the checkout moved despite a pin mismatch"
fi
if [[ ! -f "$(ledger_of "$hC")/gate2.landed" ]]; then
  report ok "C: the landing stage is not recorded"
else
  report bad "C: gate2.landed written despite a pin mismatch"
fi

# ── Case D: attached landing at the pin ────────────────────────────────────
hD="$work/homeD"
build "$hD"
# origin/main is AHEAD of the local checkout when the pins are taken, so the
# landing has real work to do (a fast-forward), not a no-op.
origin_tip="$(advance_origin "$hD" 'the commit main is pinned at')"
approve_gate1 "$hD"
repoD="$hD/workspaces/Ivy/webdavis/dotfiles"
stale_head="$(git -C "$repoD" rev-parse HEAD)"
# start from a DETACHED head to prove the runner re-attaches rather than
# leaving the live source floating off-branch
cutover_git -C "$repoD" checkout --quiet --detach HEAD
gate "$hD" 2 --second-session-open
if [[ $RC -eq 10 ]]; then
  report ok "D: stops at the staged-apply checkpoint (exit 10)"
else
  report bad "D: expected the checkpoint exit 10, got $RC (err: $(cat "$err_file"))"
fi
main_pin="$(sed -n 's/^MAIN_SHA=//p' "$(ledger_of "$hD")/pins.env")"
if [[ "$(git -C "$repoD" symbolic-ref --quiet --short HEAD || true)" == "main" ]]; then
  report ok "D: the checkout is attached to main"
else
  report bad "D: the checkout is not attached to main"
fi
if [[ $main_pin == "$origin_tip" ]] && [[ $stale_head != "$origin_tip" ]]; then
  report ok "D: the pin is origin/main, ahead of where the checkout started"
else
  report bad "D: the sandbox did not exercise a real fast-forward"
fi
if [[ "$(git -C "$repoD" rev-parse HEAD)" == "$main_pin" ]]; then
  report ok "D: HEAD is exactly the pinned MAIN_SHA"
else
  report bad "D: HEAD is not the pinned MAIN_SHA"
fi
if [[ -f "$(ledger_of "$hD")/gate2.landed" ]] && [[ ! -f "$(ledger_of "$hD")/gate2.done" ]]; then
  report ok "D: the landing is recorded, the gate is not yet passed"
else
  report bad "D: wrong gate-2 markers after the landing stage"
fi

# ── Case E: a dirty tree refuses before the apply ──────────────────────────
hE="$work/homeE"
prepare "$hE"
printf 'late stray\n' >"$hE/workspaces/Ivy/webdavis/dotfiles/stray.txt"
gate "$hE" 2 --second-session-open
if [[ $RC -eq 1 ]] && grep -q 'stray.txt' "$err_file"; then
  report ok "E: a tree that went dirty after gate 1 refuses"
else
  report bad "E: a dirty tree did not refuse (rc=$RC, err: $(cat "$err_file"))"
fi

# ── Case F: --post-apply before the landing stage ──────────────────────────
hF="$work/homeF"
prepare "$hF"
gate "$hF" 2 --post-apply
if [[ $RC -eq 1 ]]; then
  report ok "F: --post-apply refuses before the landing stage ran"
else
  report bad "F: --post-apply ran out of order (rc=$RC)"
fi

# ── Case G: the approved retirement executes ───────────────────────────────
hG="$work/homeG"
prepare "$hG"
gate "$hG" 2 --second-session-open
gate "$hG" 2 --post-apply
if [[ $RC -eq 0 ]]; then
  report ok "G: --post-apply passes"
else
  report bad "G: --post-apply failed (rc=$RC, err: $(cat "$err_file"))"
fi
if grep -qx "bootout gui/$uid/com.github.openclaw-setup.watchdog" "$LAUNCHCTL_LOG"; then
  report ok "G: the approved label is booted out, domain-qualified"
else
  report bad "G: no domain-qualified bootout (log: $(tr '\n' '|' <"$LAUNCHCTL_LOG"))"
fi
if grep -q 'bootout' "$LAUNCHCTL_LOG" &&
  [[ "$(grep -c 'bootout' "$LAUNCHCTL_LOG")" -eq 1 ]]; then
  report ok "G: exactly one bootout, the approved one"
else
  report bad "G: unexpected bootout count ($(grep -c 'bootout' "$LAUNCHCTL_LOG"))"
fi
if grep -q '^tailscale status' "$CMD_LOG"; then
  report ok "G: Tailscale reachability verified"
else
  report bad "G: no Tailscale reachability check (log: $(tr '\n' '|' <"$CMD_LOG"))"
fi
if grep -qx 'print system/com.openssh.sshd' "$LAUNCHCTL_LOG"; then
  report ok "G: sshd reachability verified"
else
  report bad "G: no sshd check"
fi
if [[ -f "$(ledger_of "$hG")/gate2.done" ]]; then
  report ok "G: gate 2 recorded as passed"
else
  report bad "G: gate2.done not written"
fi

# ── Case H: a failed bootout refuses ───────────────────────────────────────
hH="$work/homeH"
prepare "$hH"
gate "$hH" 2 --second-session-open
run_gate "$hH" FAIL_BOOTOUT=com.github.openclaw-setup.watchdog "$RUNNER" 2 --post-apply
if [[ $RC -eq 1 ]] && grep -q 'com.github.openclaw-setup.watchdog' "$err_file"; then
  report ok "H: a failed bootout refuses and names the label"
else
  report bad "H: a failed bootout did not refuse (rc=$RC, err: $(cat "$err_file"))"
fi
if [[ ! -f "$(ledger_of "$hH")/gate2.done" ]]; then
  report ok "H: gate 2 not marked passed after a failed retirement"
else
  report bad "H: gate2.done written despite a failed retirement"
fi

# ── Case I: nothing is retired ad hoc ──────────────────────────────────────
hI="$work/homeI"
prepare "$hI"
gate "$hI" 2 --second-session-open
# a second historical orphan appears AFTER the operator approved the manifest
printf 'com.webdavis.gha-watcher\n' >>"$LOADED_GUI"
gate "$hI" 2 --post-apply
if grep -q 'com.webdavis.gha-watcher' "$LAUNCHCTL_LOG"; then
  report bad "I: retired a label discovered after approval"
else
  report ok "I: only the approved manifest is retired, nothing ad hoc"
fi

# ── Case J: Tailscale unreachable ──────────────────────────────────────────
hJ="$work/homeJ"
prepare "$hJ"
gate "$hJ" 2 --second-session-open
run_gate "$hJ" FAIL_TAILSCALE=1 "$RUNNER" 2 --post-apply
if [[ $RC -eq 1 ]] && grep -qi 'tailscale' "$err_file"; then
  report ok "J: unreachable Tailscale refuses"
else
  report bad "J: unreachable Tailscale did not refuse (rc=$RC, err: $(cat "$err_file"))"
fi

# ── Case K: sshd absent ────────────────────────────────────────────────────
hK="$work/homeK"
prepare "$hK"
gate "$hK" 2 --second-session-open
: >"$LOADED_SYSTEM"
gate "$hK" 2 --post-apply
if [[ $RC -eq 1 ]] && grep -qi 'sshd' "$err_file"; then
  report ok "K: an unloaded sshd refuses"
else
  report bad "K: an unloaded sshd did not refuse (rc=$RC, err: $(cat "$err_file"))"
fi

if [[ $failures -gt 0 ]]; then
  printf 'cutover-gate-activation: %d assertion(s) FAILED\n' "$failures" >&2
  exit 1
fi
printf 'cutover-gate-activation: OK (fail-closed pins, attached landing, approved-only retirement, reachability)\n'
