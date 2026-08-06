#!/usr/bin/env bash
# cutover-gate-usage.sh: the cutover gate runner's entry contract.
#
# scripts/cutover-gate.sh is the single entry point for every D1 cutover
# command (SP2 plan, "Cutover tooling PR"). This pins the parts of the binding
# acceptance checklist that need no repository:
#
#   - one entry point per gate, 1..5 only; anything else is usage to stderr and
#     a non-zero exit, never a silent fallthrough (Wooledge house rule)
#   - checklist item 5, the repo handle is validated at the top of EVERY
#     invocation, so all five gates refuse when $repo/.git is missing
#
# The refusal cases run with HOME pointed at an empty sandbox, so the repo
# handle ($HOME/workspaces/Ivy/webdavis/dotfiles) cannot resolve and nothing
# reaches a real checkout.
set -euo pipefail

# git exports GIT_DIR/GIT_INDEX_FILE when this runs under the pre-commit hook;
# unset so nothing here can reach the outer repository.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUNNER="$REPO_ROOT/scripts/cutover-gate.sh"

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

[[ -f $RUNNER ]] || {
  printf 'FAIL: missing runner: %s\n' "$RUNNER" >&2
  exit 1
}
[[ -x $RUNNER ]] || {
  printf 'FAIL: runner is not executable: %s\n' "$RUNNER" >&2
  exit 1
}

out_file="$work/stdout"
err_file="$work/stderr"

# run_runner <args...> : runs the runner with an EMPTY sandbox HOME, capturing
# stdout/stderr to files and the exit status in RC.
run_runner() {
  RC=0
  HOME="$work/home" "$RUNNER" "$@" >"$out_file" 2>"$err_file" || RC=$?
}

mkdir -p "$work/home"

printf 'cutover-gate-usage cases:\n'

# ── Rejected invocations: usage to stderr, non-zero exit ────────────────────
for bad in "" 0 6 -1 1.5 abc "1 2" --dry-run; do
  if [[ -z $bad ]]; then
    run_runner
    label='no argument'
  else
    # shellcheck disable=SC2086  # deliberate: some cases are multi-word argv
    run_runner $bad
    label="argument '$bad'"
  fi
  if [[ $RC -ne 0 ]]; then
    report ok "$label: exits non-zero"
  else
    report bad "$label: exited 0 (must refuse)"
  fi
  if grep -qi 'usage' "$err_file"; then
    report ok "$label: usage on stderr"
  else
    report bad "$label: no usage on stderr (err: $(cat "$err_file"))"
  fi
  if [[ -s $out_file ]]; then
    report bad "$label: wrote to stdout (usage belongs on stderr)"
  else
    report ok "$label: stdout stays empty"
  fi
done

# ── Checklist item 5: repo handle validated on EVERY gate invocation ────────
for gate in 1 2 3 4 5; do
  run_runner "$gate"
  if [[ $RC -ne 0 ]]; then
    report ok "gate $gate: refuses when the repo handle is not a git checkout"
  else
    report bad "gate $gate: proceeded without a repo handle"
  fi
  if grep -q 'workspaces/Ivy/webdavis/dotfiles' "$err_file"; then
    report ok "gate $gate: names the missing repo path"
  else
    report bad "gate $gate: refusal does not name the repo path (err: $(cat "$err_file"))"
  fi
done

if [[ $failures -gt 0 ]]; then
  printf 'cutover-gate-usage: %d assertion(s) FAILED\n' "$failures" >&2
  exit 1
fi
printf 'cutover-gate-usage: OK (gate 1-5 only, repo handle enforced per invocation)\n'
