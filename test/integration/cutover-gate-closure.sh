#!/usr/bin/env bash
# cutover-gate-closure.sh: gates 3, 4 and 5 of the D1 cutover runner.
#
# Gate 3 runs the already-merged reconcile tool (dry-run, then live), verifies
# every service against the retirement manifest rather than a before/after diff,
# and runs the test suite plus the live smoke checks. Gate 4 soaks the final,
# retired topology. Gate 5 re-verifies both pins in a fresh shell and closes the
# reference PRs with the GitHub repository named explicitly.
#
# All boundaries are stubbed (see helpers/cutover-sandbox.sh): launchctl, gh,
# just, chezmoi, tailscale, hermes, plus the relay and osquery-heartbeat
# executables under the sandbox $HOME.
#
# Cases:
#   A. gate 3 runs the reconcile tool by absolute path, --dry-run BEFORE live
#   B. a failed dry-run refuses and never reaches the live run
#   C. an approved-retired label still loaded refuses
#   D. steady-state predicates: persistent needs a running pid, scheduled needs
#      a registered trigger, conditional needs a clean last exit
#   E. a red test suite refuses; a chezmoi drift report refuses
#   F. gate 3 happy path records the pass
#   G. gate 4 holds the soak window open, then passes once it has elapsed
#   H. gate 5 refuses before the soak, on a moved pin, and closes each
#      reference PR with an explicit --repo when everything holds
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

stub_dir="$work/stubs"
export LOADED_GUI="$work/loaded-gui"
export LOADED_SYSTEM="$work/loaded-system"
export LAUNCHCTL_LOG="$work/launchctl.log"
export CMD_LOG="$work/cmd.log"
export RECONCILE_LOG="$work/reconcile.log"
export PRINT_DETAIL_DIR="$work/print-detail"
export CHEZMOI_DATA_FILE="$work/render.json"
mkdir -p "$stub_dir" "$PRINT_DETAIL_DIR"
: >"$LAUNCHCTL_LOG"
: >"$CMD_LOG"
: >"$RECONCILE_LOG"
cutover_make_launchctl_stub "$stub_dir"
cutover_make_command_stubs "$stub_dir"

out_file="$work/stdout"
err_file="$work/stderr"
neutral="$work/neutral"
mkdir -p "$neutral"
# Checklist item 6: the runner is always invoked from a cwd outside any git
# repository, so it can only be operating through its absolute repo handle.
PATH_PREFIX=""
run_gate() {
  local home="$1"
  shift
  RC=0
  : >"$LAUNCHCTL_LOG"
  : >"$CMD_LOG"
  : >"$RECONCILE_LOG"
  (
    cd "$neutral" || exit 1
    HOME="$home" PATH="${PATH_PREFIX}$stub_dir:$PATH" CUTOVER_PHASE_A_BASE="$SANDBOX_BASE" \
      env "$@"
  ) >"$out_file" 2>"$err_file" || RC=$?
}
gate() {
  local home="$1"
  shift
  run_gate "$home" "$RUNNER" "$@"
}

ledger_of() { printf '%s/.local/state/cutover' "$1"; }

# seed_healthy : the topology gate 3 expects after a good activation. Every
# desired label is loaded with a print body that satisfies its predicate; the
# retired orphan is gone.
seed_healthy() {
  {
    printf 'com.webdavis.atuin-daemon\n'
    printf 'com.webdavis.osquery-uptime-watchdog\n'
    printf 'com.webdavis.osquery-results-alerter\n'
  } >"$LOADED_GUI"
  {
    printf 'com.openssh.sshd\n'
    printf 'systems.nixos.nix-installer.nix-hook\n'
  } >"$LOADED_SYSTEM"
  printf '\tstate = running\n\tpid = 4242\n\tlast exit code = 0\n' \
    >"$PRINT_DETAIL_DIR/com.webdavis.atuin-daemon"
  printf '\tstate = not running\n\truns = 818\n\tlast exit code = 0\n\trun interval = 900 seconds\n' \
    >"$PRINT_DETAIL_DIR/com.webdavis.osquery-uptime-watchdog"
  printf '\tstate = not running\n\tlast exit code = 0\n' \
    >"$PRINT_DETAIL_DIR/com.webdavis.osquery-results-alerter"
  printf '\tstate = not running\n\truns = 1\n\tlast exit code = 0\n' \
    >"$PRINT_DETAIL_DIR/systems.nixos.nix-installer.nix-hook"
}

# prepare <home> : sandbox carried through an approved gate 1 and a full gate 2.
prepare() {
  local home="$1"
  cutover_build_sandbox "$home"
  cutover_write_classification "$home"
  cutover_make_home_scripts "$home"
  : >"$LOADED_GUI"
  : >"$LOADED_SYSTEM"
  {
    printf 'com.github.openclaw-setup.watchdog\n'
    printf 'com.webdavis.atuin-daemon\n'
  } >"$LOADED_GUI"
  printf 'com.openssh.sshd\n' >"$LOADED_SYSTEM"
  gate "$home" 1
  gate "$home" 1 --approve-retirement
  gate "$home" 2 --second-session-open
  gate "$home" 2 --post-apply
  [[ -f "$(ledger_of "$home")/gate2.done" ]] ||
    printf 'FAIL: gate 2 setup did not pass for %s (err: %s)\n' "$home" "$(cat "$err_file")" >&2
  seed_healthy
}

printf 'cutover-gate-closure cases:\n'

# ── Case A: the reconcile tool, dry-run before live ────────────────────────
hA="$work/homeA"
prepare "$hA"
gate "$hA" 3
if [[ $RC -eq 0 ]]; then
  report ok "A: gate 3 passes on a converged machine"
else
  report bad "A: gate 3 failed (rc=$RC, err: $(cat "$err_file"))"
fi
if [[ "$(sed -n '1p' "$RECONCILE_LOG")" == "--dry-run" ]] &&
  [[ "$(sed -n '2p' "$RECONCILE_LOG")" == "" ]] &&
  [[ "$(wc -l <"$RECONCILE_LOG" | tr -d ' ')" -eq 2 ]]; then
  report ok "A: live-reconcile.sh ran --dry-run first, then live, exactly once each"
else
  report bad "A: wrong reconcile invocation order (log: $(tr '\n' '|' <"$RECONCILE_LOG"))"
fi
if grep -q "^just test$" "$CMD_LOG"; then
  report ok "A: the repository test suite runs"
else
  report bad "A: no test-suite run (log: $(tr '\n' '|' <"$CMD_LOG"))"
fi
for probe in 'relay ' 'hermes gateway status' 'heartbeat' 'chezmoi status'; do
  if grep -q "$probe" "$CMD_LOG"; then
    report ok "A: smoke check ran: $probe"
  else
    report bad "A: missing smoke check: $probe"
  fi
done

# ── Case B: a failed dry-run never reaches the live run ────────────────────
hB="$work/homeB"
prepare "$hB"
run_gate "$hB" RECONCILE_FAIL=dry-run "$RUNNER" 3
if [[ $RC -eq 1 ]]; then
  report ok "B: a failed reconcile dry-run refuses"
else
  report bad "B: a failed dry-run did not refuse (rc=$RC)"
fi
if [[ "$(wc -l <"$RECONCILE_LOG" | tr -d ' ')" -eq 1 ]]; then
  report ok "B: the live reconcile never ran"
else
  report bad "B: the live reconcile ran after a failed dry-run"
fi

# ── Case C: an approved-retired label still loaded ─────────────────────────
hC="$work/homeC"
prepare "$hC"
printf 'com.github.openclaw-setup.watchdog\n' >>"$LOADED_GUI"
gate "$hC" 3
if [[ $RC -eq 1 ]] && grep -q 'com.github.openclaw-setup.watchdog' "$err_file"; then
  report ok "C: a retired label found loaded refuses"
else
  report bad "C: a resurrected retired label passed (rc=$RC, err: $(cat "$err_file"))"
fi

# ── Case D: steady-state predicates ────────────────────────────────────────
hD="$work/homeD"
prepare "$hD"
printf '\tstate = not running\n\tlast exit code = 0\n' \
  >"$PRINT_DETAIL_DIR/com.webdavis.atuin-daemon"
gate "$hD" 3
if [[ $RC -eq 1 ]] && grep -q 'com.webdavis.atuin-daemon' "$err_file"; then
  report ok "D: a persistent job that is not running refuses"
else
  report bad "D: a dead KeepAlive job passed (rc=$RC, err: $(cat "$err_file"))"
fi
seed_healthy
printf '\tstate = not running\n\tlast exit code = 0\n' \
  >"$PRINT_DETAIL_DIR/com.webdavis.osquery-uptime-watchdog"
gate "$hD" 3
if [[ $RC -eq 1 ]] && grep -q 'com.webdavis.osquery-uptime-watchdog' "$err_file"; then
  report ok "D: a scheduled job with no registered trigger refuses"
else
  report bad "D: an untriggered scheduled job passed (rc=$RC, err: $(cat "$err_file"))"
fi
seed_healthy
printf '\tstate = not running\n\tlast exit code = 78\n' \
  >"$PRINT_DETAIL_DIR/systems.nixos.nix-installer.nix-hook"
gate "$hD" 3
if [[ $RC -eq 1 ]] && grep -q 'nix-hook' "$err_file"; then
  report ok "D: a conditional-KeepAlive job with a failed last exit refuses"
else
  report bad "D: an unhealthy conditional job passed (rc=$RC, err: $(cat "$err_file"))"
fi
seed_healthy
: >"$LOADED_SYSTEM"
printf 'com.openssh.sshd\n' >"$LOADED_SYSTEM"
gate "$hD" 3
if [[ $RC -eq 1 ]] && grep -q 'nix-hook' "$err_file"; then
  report ok "D: a desired label that is not loaded at all refuses"
else
  report bad "D: a missing desired service passed (rc=$RC, err: $(cat "$err_file"))"
fi
seed_healthy

# ── Case E: red suite and chezmoi drift both refuse ────────────────────────
hE="$work/homeE"
prepare "$hE"
run_gate "$hE" FAIL_JUST=1 "$RUNNER" 3
if [[ $RC -eq 1 ]] && grep -qi 'test suite' "$err_file"; then
  report ok "E: a red test suite refuses"
else
  report bad "E: a red test suite passed (rc=$RC, err: $(cat "$err_file"))"
fi
run_gate "$hE" FAIL_CHEZMOI=dot_bashrc "$RUNNER" 3
if [[ $RC -eq 1 ]] && grep -q 'dot_bashrc' "$err_file"; then
  report ok "E: source-to-target drift refuses and names the entry"
else
  report bad "E: chezmoi drift passed (rc=$RC, err: $(cat "$err_file"))"
fi
run_gate "$hE" FAIL_RELAY=1 "$RUNNER" 3
if [[ $RC -eq 1 ]] && grep -qi 'relay' "$err_file"; then
  report ok "E: a relay that cannot fire refuses"
else
  report bad "E: a broken relay passed (rc=$RC, err: $(cat "$err_file"))"
fi

# ── Case F/G: the soak window ──────────────────────────────────────────────
hG="$work/homeG"
prepare "$hG"
ledG="$(ledger_of "$hG")"
gate "$hG" 4
if [[ $RC -eq 1 ]] && grep -qi 'gate 3' "$err_file"; then
  report ok "G: gate 4 refuses before gate 3 passed"
else
  report bad "G: gate 4 ran without gate 3 (rc=$RC, err: $(cat "$err_file"))"
fi
gate "$hG" 3
gate "$hG" 4
if [[ $RC -eq 10 ]] && [[ ! -f "$ledG/gate4.done" ]]; then
  report ok "G: the soak window holds the gate open"
else
  report bad "G: the soak window did not hold (rc=$RC)"
fi
if grep -qi 'remaining' "$out_file"; then
  report ok "G: reports the remaining soak time"
else
  report bad "G: no remaining-time report (out: $(cat "$out_file"))"
fi
gate "$hG" 4 --window-hours 1
if [[ $RC -eq 10 ]]; then
  report ok "G: a shorter window is still measured from the gate 3 pass"
else
  report bad "G: --window-hours was not honoured (rc=$RC)"
fi
# backdate the gate 3 pass so the window has elapsed
touch -t 202601010000 "$ledG/gate3.done"
gate "$hG" 4
if [[ $RC -eq 0 ]] && [[ -f "$ledG/gate4.done" ]]; then
  report ok "G: an elapsed soak window passes gate 4"
else
  report bad "G: an elapsed soak window did not pass (rc=$RC, err: $(cat "$err_file"))"
fi
if grep -q 'hermes gateway status' "$CMD_LOG"; then
  report ok "G: the daily-critical paths are re-probed at the end of the soak"
else
  report bad "G: no end-of-soak probes (log: $(tr '\n' '|' <"$CMD_LOG"))"
fi
rm -f "$ledG/gate4.done"
run_gate "$hG" FAIL_HERMES=1 "$RUNNER" 4
if [[ $RC -eq 1 ]] && [[ ! -f "$ledG/gate4.done" ]]; then
  report ok "G: a regression during the soak refuses"
else
  report bad "G: a failing probe still passed the soak (rc=$RC)"
fi

# ── Case G2: the soak clock reads under either stat flavor ─────────────────
# The window is measured from a file mtime. macOS ships BSD stat and the nix
# `run` shell CI uses ships GNU coreutils, and their flags are mutually
# invalid: GNU's -f is --file-system, so `stat -f %m FILE` there fails on the
# format operand while still printing a human block for FILE. Reading that
# block as an epoch is how gate 4 died in CI while passing on a BSD host.
for flavor in gnu bsd; do
  mkdir -p "$work/stat-$flavor"
done
# shellcheck disable=SC2016  # literal stub bodies; $vars resolve when they run
printf '%s\n' '#!/usr/bin/env bash
if [[ "${1:-}" == "-c" && "${2:-}" == "%Y" ]]; then
  exec /usr/bin/stat -f %m "$3"
fi
if [[ "${1:-}" == "-f" ]]; then
  printf "  File: \"%s\"\n" "${3:-}"
  printf "    ID: 0 Namelen: 255 Type: apfs\n"
  exit 1
fi
exit 1' >"$work/stat-gnu/stat"
# shellcheck disable=SC2016  # literal stub bodies; $vars resolve when they run
printf '%s\n' '#!/usr/bin/env bash
if [[ "${1:-}" == "-f" && "${2:-}" == "%m" ]]; then
  exec /usr/bin/stat -f %m "$3"
fi
printf "stat: illegal option -- %s\n" "${1#-}" >&2
exit 1' >"$work/stat-bsd/stat"
chmod +x "$work/stat-gnu/stat" "$work/stat-bsd/stat"
for flavor in gnu bsd; do
  rm -f "$ledG/gate4.done"
  PATH_PREFIX="$work/stat-$flavor:"
  gate "$hG" 4
  PATH_PREFIX=""
  if [[ $RC -eq 0 ]] && [[ -f "$ledG/gate4.done" ]]; then
    report ok "G2: the soak clock reads correctly under $flavor stat"
  else
    report bad "G2: $flavor stat broke the soak clock (rc=$RC, err: $(cat "$err_file"))"
  fi
done

# ── Case H: closure ────────────────────────────────────────────────────────
hH="$work/homeH"
prepare "$hH"
ledH="$(ledger_of "$hH")"
gate "$hH" 5
if [[ $RC -eq 1 ]] && grep -qi 'gate 4' "$err_file"; then
  report ok "H: gate 5 refuses before the soak passed"
else
  report bad "H: gate 5 ran before the soak (rc=$RC, err: $(cat "$err_file"))"
fi
if ! grep -q '^gh ' "$CMD_LOG"; then
  report ok "H: no pull request is touched before the soak passes"
else
  report bad "H: gh ran before the soak passed"
fi
gate "$hH" 3
touch -t 202601010000 "$ledH/gate3.done"
gate "$hH" 4
# a pin moves during the soak: the soaked state is not the closing state
side="$work/sideH"
cutover_git clone --quiet "$hH/origin.git" "$side"
printf 'dependabot\n' >"$side/bump.txt"
cutover_git -C "$side" add -A
cutover_git -C "$side" commit --quiet -m 'auto-merge during the soak'
cutover_git -C "$side" push --quiet origin main
gate "$hH" 5
if [[ $RC -eq 1 ]] && grep -qi 'restart' "$err_file"; then
  report ok "H: a pin that moved during the soak refuses closure"
else
  report bad "H: closure proceeded on a moved pin (rc=$RC, err: $(cat "$err_file"))"
fi
if ! grep -q '^gh ' "$CMD_LOG"; then
  report ok "H: no pull request is closed on a moved pin"
else
  report bad "H: a pull request was closed despite a moved pin"
fi
# restore the pin, then close for real, with a hostile inherited GH_REPO
cutover_git -C "$side" reset --hard --quiet HEAD~1
cutover_git -C "$side" push --quiet --force origin main
run_gate "$hH" GH_REPO=someone-else/wrong-repo "$RUNNER" 5
if [[ $RC -eq 0 ]]; then
  report ok "H: gate 5 passes once both pins hold"
else
  report bad "H: gate 5 failed (rc=$RC, err: $(cat "$err_file"))"
fi
for pr in 25 31 32; do
  if grep -q "^gh pr close $pr .*--repo=github.com/webdavis/dotfiles" "$CMD_LOG"; then
    report ok "H: PR #$pr closed against the explicitly named repository"
  else
    report bad "H: PR #$pr not closed with an explicit --repo (log: $(tr '\n' '|' <"$CMD_LOG"))"
  fi
done
if grep -q 'gh-env GH_REPO= ' "$CMD_LOG"; then
  report ok "H: an inherited GH_REPO cannot reach the gh invocation"
else
  report bad "H: GH_REPO leaked into the gh environment (log: $(grep 'gh-env' "$CMD_LOG" || true))"
fi
if grep -q '^gh pr close 25 .*--comment' "$CMD_LOG"; then
  report ok "H: each closure carries a comment"
else
  report bad "H: no closing comment"
fi
if [[ -f "$ledH/gate5.done" ]]; then
  report ok "H: gate 5 recorded as passed"
else
  report bad "H: gate5.done not written"
fi

if [[ $failures -gt 0 ]]; then
  printf 'cutover-gate-closure: %d assertion(s) FAILED\n' "$failures" >&2
  exit 1
fi
printf 'cutover-gate-closure: OK (reconcile order, manifest verification, predicates, soak window, explicit closure targeting)\n'
