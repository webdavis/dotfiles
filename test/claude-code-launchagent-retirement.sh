#!/usr/bin/env bash
# claude-code-launchagent-retirement.sh — the one-time retirement chezmoiscript
# must unload the old com.claude.code LaunchAgent and delete a leftover deployed
# plist, be a silent no-op when both are already gone, be idempotent on a second
# run, and never abort the apply. Removing the plist SOURCE (done in S4) does NOT
# unload an already-running LaunchAgent; this is the live-side complement.
#
# It renders the REAL template and runs it against a stub `launchctl` on PATH
# that models the GUI domain with a state file: `print` reports loaded while the
# state file exists, `bootout` records its argv and clears the state (so the
# service is unloaded afterward). No live launchd state is touched.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_ROOT/.chezmoiscripts/run_once_after_59-retire-claude-code-launchagent.sh.tmpl"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

if ! command -v chezmoi >/dev/null 2>&1; then
  printf 'SKIP: chezmoi not on PATH; cannot render the retirement script\n'
  exit 0
fi
[[ -f $SCRIPT ]] || fail "missing template: $SCRIPT"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

rendered="$work/rendered.sh"
render_home="$(mktemp -d)"
HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$SCRIPT" >"$rendered" || fail "chezmoi failed to render $SCRIPT"
rm -rf "$render_home"
if [[ ! -s $rendered ]]; then
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi

# Stub launchctl: STATE present => service loaded. `print` exits per load state;
# `bootout` records argv and clears STATE (service becomes unloaded).
make_launchctl_stub() {
  local dir="$1"
  cat >"$dir/launchctl" <<'STUB'
#!/bin/bash
case "$1" in
  print)
    [[ -f $LAUNCHCTL_STATE ]] && exit 0
    exit 113 ;;
  bootout)
    printf '%s\n' "$*" >>"$LAUNCHCTL_BOOTOUT_RECORD"
    rm -f "$LAUNCHCTL_STATE"
    exit 0 ;;
esac
exit 0
STUB
  chmod +x "$dir/launchctl"
}

# run_case <name> <loaded?> <plist?>
run_case() {
  local name="$1" loaded="$2" plist="$3"
  CASE_HOME="$work/$name/home"
  local bin="$work/$name/bin"
  STATE="$work/$name/state"
  BOOTOUT_RECORD="$work/$name/bootout-argv"
  PLIST="$CASE_HOME/Library/LaunchAgents/com.claude.code.plist"
  mkdir -p "$bin" "$CASE_HOME/Library/LaunchAgents"
  make_launchctl_stub "$bin"
  [[ $loaded == loaded ]] && : >"$STATE"
  [[ $plist == plist ]] && : >"$PLIST"
  RC=0
  HOME="$CASE_HOME" PATH="$bin:$PATH" \
    LAUNCHCTL_STATE="$STATE" LAUNCHCTL_BOOTOUT_RECORD="$BOOTOUT_RECORD" \
    bash "$rendered" >/dev/null 2>&1 || RC=$?
}

booted_out() { [[ -s $BOOTOUT_RECORD ]]; }

# Loaded + plist present: unloaded and plist removed.
run_case loaded-plist loaded plist
[[ $RC -eq 0 ]] || fail "loaded-plist: expected exit 0, got $RC"
booted_out || fail "loaded-plist: launchctl bootout was not called for a loaded service"
grep -q 'com.claude.code' "$BOOTOUT_RECORD" || fail "loaded-plist: bootout did not target com.claude.code ($(cat "$BOOTOUT_RECORD"))"
[[ ! -e $PLIST ]] || fail "loaded-plist: leftover plist was not removed"

# Not loaded + no plist: silent no-op, no bootout.
run_case clean not-loaded no-plist
[[ $RC -eq 0 ]] || fail "clean: expected exit 0, got $RC"
booted_out && fail "clean: bootout called though the service is not loaded"

# Loaded + no plist: unloaded, exit 0.
run_case loaded-only loaded no-plist
[[ $RC -eq 0 ]] || fail "loaded-only: expected exit 0, got $RC"
booted_out || fail "loaded-only: bootout not called for a loaded service"

# Not loaded + leftover plist: no bootout, plist still removed.
run_case plist-only not-loaded plist
[[ $RC -eq 0 ]] || fail "plist-only: expected exit 0, got $RC"
booted_out && fail "plist-only: bootout called though the service is not loaded"
[[ ! -e $PLIST ]] || fail "plist-only: leftover plist was not removed"

# Idempotence: a loaded service, run twice. The first run boots it out; the
# second sees it already gone and must NOT bootout again (still exit 0).
run_case idempotent loaded plist
[[ $RC -eq 0 ]] || fail "idempotent(1): expected exit 0, got $RC"
RC=0
HOME="$CASE_HOME" PATH="$work/idempotent/bin:$PATH" \
  LAUNCHCTL_STATE="$STATE" LAUNCHCTL_BOOTOUT_RECORD="$BOOTOUT_RECORD" \
  bash "$rendered" >/dev/null 2>&1 || RC=$?
[[ $RC -eq 0 ]] || fail "idempotent(2): second run must still exit 0, got $RC"
[[ $(wc -l <"$BOOTOUT_RECORD") -eq 1 ]] ||
  fail "idempotent(2): bootout ran again on a second pass ($(cat "$BOOTOUT_RECORD"))"

printf 'PASS: the retirement script boots out a loaded com.claude.code, removes a leftover plist, no-ops when clean, is idempotent on a second run, and never aborts the apply\n'
