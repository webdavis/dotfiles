#!/usr/bin/env bash
# claude-code-launchagent-retirement.sh: the one-time retirement chezmoiscript
# (run_once_after_59) must unload the old com.claude.code LaunchAgent and
# delete a leftover deployed plist, be a silent no-op when both are already
# gone, be idempotent on a second run, and NEVER abort the apply. Removing the
# plist SOURCE (done in S4) does NOT unload an already-running LaunchAgent;
# this is the live-side complement.
#
# Honesty contract (run_once never retries, so failures must be loud):
#   - the retired/success message is printed ONLY when a re-probe confirms the
#     service is actually gone after bootout
#   - a bootout that leaves the service loaded prints a LOUD warning naming
#     the exact manual command to run, and still exits 0
#   - failing `id`, `launchctl print`, and `rm` paths never abort the apply
#   - the bootout targets the EXACT domain target gui/<uid>/com.claude.code
#     (the stub records full argv, so a wrong domain or a similar label fails)
#
# It renders the REAL template and runs it against a stub `launchctl` on PATH
# that models the GUI domain with a state file: `print` reports loaded while
# the state file exists, `bootout` records its argv and clears the state.
# Failure modes are toggled via env vars. No live launchd state is touched.
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

REAL_UID="$(id -u)"
TARGET="gui/$REAL_UID/com.claude.code"

# Stub launchctl: records FULL argv for every invocation; STATE file present
# means the service is loaded. `print` exits per load state (or fails outright
# under LAUNCHCTL_PRINT_FAIL=1); `bootout` clears STATE on success, or exits
# 77 leaving STATE intact under LAUNCHCTL_BOOTOUT_FAIL=1.
make_launchctl_stub() {
  local dir="$1"
  cat >"$dir/launchctl" <<'STUB'
#!/bin/bash
printf '%s\n' "$*" >>"$LAUNCHCTL_RECORD"
case "$1" in
  print)
    [[ ${LAUNCHCTL_PRINT_FAIL:-0} == 1 ]] && exit 2
    [[ -f $LAUNCHCTL_STATE ]] && exit 0
    exit 113 ;;
  bootout)
    if [[ ${LAUNCHCTL_BOOTOUT_FAIL:-0} == 1 ]]; then
      exit 77
    fi
    rm -f "$LAUNCHCTL_STATE"
    exit 0 ;;
esac
exit 0
STUB
  chmod +x "$dir/launchctl"
}

# run_case <name> <loaded?> <plist?> [failure-mode]
#   failure-mode: bootout-fails | print-fails | rm-fails | id-fails | none
run_case() {
  local name="$1" loaded="$2" plist="$3" failure="${4:-none}"
  CASE_HOME="$work/$name/home"
  local bin="$work/$name/bin"
  STATE="$work/$name/state"
  RECORD="$work/$name/launchctl-argv"
  OUT_FILE="$work/$name/stdout"
  ERR_FILE="$work/$name/stderr"
  PLIST="$CASE_HOME/Library/LaunchAgents/com.claude.code.plist"
  mkdir -p "$bin" "$CASE_HOME/Library/LaunchAgents"
  make_launchctl_stub "$bin"
  : >"$RECORD"
  [[ $loaded == loaded ]] && : >"$STATE"
  [[ $plist == plist ]] && : >"$PLIST"
  local bootout_fail=0 print_fail=0
  case "$failure" in
    bootout-fails) bootout_fail=1 ;;
    print-fails) print_fail=1 ;;
    rm-fails) chmod 555 "$CASE_HOME/Library/LaunchAgents" ;;
    id-fails)
      printf '#!/bin/bash\nexit 1\n' >"$bin/id"
      chmod +x "$bin/id"
      ;;
    none) ;;
    *) fail "unknown failure mode: $failure" ;;
  esac
  RC=0
  HOME="$CASE_HOME" PATH="$bin:$PATH" \
    LAUNCHCTL_STATE="$STATE" LAUNCHCTL_RECORD="$RECORD" \
    LAUNCHCTL_BOOTOUT_FAIL="$bootout_fail" LAUNCHCTL_PRINT_FAIL="$print_fail" \
    bash "$rendered" >"$OUT_FILE" 2>"$ERR_FILE" || RC=$?
  if [[ $failure == rm-fails ]]; then
    chmod 755 "$CASE_HOME/Library/LaunchAgents"
  fi
}

bootout_count() { grep -c '^bootout ' "$RECORD" || true; }

# Loaded + plist present: unloaded (exact target), re-probed, plist removed.
run_case loaded-plist loaded plist
[[ $RC -eq 0 ]] || fail "loaded-plist: expected exit 0, got $RC ($(cat "$ERR_FILE"))"
grep -qx "bootout $TARGET" "$RECORD" ||
  fail "loaded-plist: bootout argv is not exactly 'bootout $TARGET' ($(cat "$RECORD"))"
grep -qx "print $TARGET" "$RECORD" ||
  fail "loaded-plist: the load probe argv is not exactly 'print $TARGET' ($(cat "$RECORD"))"
grep -q 'retired' "$OUT_FILE" ||
  fail "loaded-plist: no retired message after a confirmed bootout ($(cat "$OUT_FILE"))"
[[ ! -e $PLIST ]] || fail "loaded-plist: leftover plist was not removed"

# Not loaded + no plist: silent no-op, no bootout.
run_case clean not-loaded no-plist
[[ $RC -eq 0 ]] || fail "clean: expected exit 0, got $RC"
[[ "$(bootout_count)" -eq 0 ]] || fail "clean: bootout called though the service is not loaded"
[[ ! -s $OUT_FILE ]] || fail "clean: expected a silent no-op, got output ($(cat "$OUT_FILE"))"

# Loaded + no plist: unloaded, exit 0.
run_case loaded-only loaded no-plist
[[ $RC -eq 0 ]] || fail "loaded-only: expected exit 0, got $RC"
[[ "$(bootout_count)" -eq 1 ]] || fail "loaded-only: bootout not called exactly once for a loaded service"

# Not loaded + leftover plist: no bootout, plist still removed.
run_case plist-only not-loaded plist
[[ $RC -eq 0 ]] || fail "plist-only: expected exit 0, got $RC"
[[ "$(bootout_count)" -eq 0 ]] || fail "plist-only: bootout called though the service is not loaded"
[[ ! -e $PLIST ]] || fail "plist-only: leftover plist was not removed"

# Bootout FAILS and the service stays loaded: run_once never retries, so the
# script must NOT claim success; it must print a loud warning naming the exact
# manual command, and still exit 0 (never abort the apply).
run_case bootout-fails loaded plist bootout-fails
[[ $RC -eq 0 ]] || fail "bootout-fails: a failed bootout aborted the apply (rc=$RC; stderr: $(cat "$ERR_FILE"))"
grep -q 'retired' "$OUT_FILE" &&
  fail "bootout-fails: retired message printed though the service is STILL loaded ($(cat "$OUT_FILE"))"
grep -q "launchctl bootout $TARGET" "$ERR_FILE" ||
  fail "bootout-fails: the warning does not name the exact manual command 'launchctl bootout $TARGET' (stderr: $(cat "$ERR_FILE"))"

# `launchctl print` itself fails: treated as not-loaded (the probe is the only
# visibility we have), no bootout attempted, exit 0.
run_case print-fails loaded no-plist print-fails
[[ $RC -eq 0 ]] || fail "print-fails: a failing probe aborted the apply (rc=$RC)"
[[ "$(bootout_count)" -eq 0 ]] || fail "print-fails: bootout attempted though the probe never confirmed a loaded service"

# `rm` FAILS (read-only LaunchAgents dir): warn and exit 0, never abort.
run_case rm-fails not-loaded plist rm-fails
[[ $RC -eq 0 ]] || fail "rm-fails: a failed plist removal aborted the apply (rc=$RC; stderr: $(cat "$ERR_FILE"))"
[[ -s $ERR_FILE ]] || fail "rm-fails: no warning printed about the unremovable plist"

# `id -u` FAILS: the launchctl phase is skipped with a warning (no malformed
# domain target), the plist is still removed, exit 0.
run_case id-fails loaded plist id-fails
[[ $RC -eq 0 ]] || fail "id-fails: a failing id aborted the apply (rc=$RC; stderr: $(cat "$ERR_FILE"))"
[[ "$(bootout_count)" -eq 0 ]] || fail "id-fails: bootout attempted without a valid uid ($(cat "$RECORD"))"
[[ -s $ERR_FILE ]] || fail "id-fails: no warning printed about the failed uid lookup"
[[ ! -e $PLIST ]] || fail "id-fails: leftover plist was not removed"

# Idempotence: a loaded service, run twice against the same state. The first
# run boots it out; the second sees it already gone and must NOT bootout again
# (still exit 0, still silent).
run_case idempotent loaded plist
[[ $RC -eq 0 ]] || fail "idempotent(1): expected exit 0, got $RC"
RC=0
HOME="$CASE_HOME" PATH="$work/idempotent/bin:$PATH" \
  LAUNCHCTL_STATE="$STATE" LAUNCHCTL_RECORD="$RECORD" \
  LAUNCHCTL_BOOTOUT_FAIL=0 LAUNCHCTL_PRINT_FAIL=0 \
  bash "$rendered" >"$OUT_FILE" 2>"$ERR_FILE" || RC=$?
[[ $RC -eq 0 ]] || fail "idempotent(2): second run must still exit 0, got $RC"
[[ "$(bootout_count)" -eq 1 ]] ||
  fail "idempotent(2): bootout ran again on a second pass ($(cat "$RECORD"))"
[[ ! -s $OUT_FILE ]] || fail "idempotent(2): second run should be silent ($(cat "$OUT_FILE"))"

printf 'PASS: the retirement script boots out exactly gui/<uid>/com.claude.code, claims success only after a confirming re-probe, warns loudly (with the manual command) when the service survives, survives id/print/rm failures with exit 0, removes the leftover plist, and is idempotent\n'
