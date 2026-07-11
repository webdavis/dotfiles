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
# means the service is loaded. The load probe is TRI-STATE: `print` exits 0
# (loaded), 113 (the known not-found status on this host, i.e. confirmed
# absent), or another code (an operational error). By default `print` derives
# 0/113 from the STATE file; when LAUNCHCTL_PRINT_SEQ_FILE holds a
# space-separated list of codes, each `print` pops the next one instead (so a
# specific probe -- first or second -- can be forced to an operational error).
# `bootout` clears STATE on success, or exits 77 leaving STATE intact under
# LAUNCHCTL_BOOTOUT_FAIL=1.
make_launchctl_stub() {
  local dir="$1"
  cat >"$dir/launchctl" <<'STUB'
#!/bin/bash
printf '%s\n' "$*" >>"$LAUNCHCTL_RECORD"
case "$1" in
  print)
    if [[ -n ${LAUNCHCTL_PRINT_SEQ_FILE:-} && -s $LAUNCHCTL_PRINT_SEQ_FILE ]]; then
      read -r code rest <"$LAUNCHCTL_PRINT_SEQ_FILE"
      printf '%s\n' "$rest" >"$LAUNCHCTL_PRINT_SEQ_FILE"
      exit "$code"
    fi
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

# run_case <name> <loaded?> <plist?> [failure-mode] [print-seq]
#   failure-mode: bootout-fails | rm-fails | id-fails | none
#   print-seq: optional space-separated `launchctl print` exit codes, one
#              popped per invocation (overrides the STATE-derived 0/113)
run_case() {
  local name="$1" loaded="$2" plist="$3" failure="${4:-none}" print_seq="${5:-}"
  CASE_HOME="$work/$name/home"
  local bin="$work/$name/bin"
  STATE="$work/$name/state"
  RECORD="$work/$name/launchctl-argv"
  OUT_FILE="$work/$name/stdout"
  ERR_FILE="$work/$name/stderr"
  PRINT_SEQ_FILE="$work/$name/print-seq"
  PLIST="$CASE_HOME/Library/LaunchAgents/com.claude.code.plist"
  mkdir -p "$bin" "$CASE_HOME/Library/LaunchAgents"
  make_launchctl_stub "$bin"
  : >"$RECORD"
  [[ $loaded == loaded ]] && : >"$STATE"
  [[ $plist == plist ]] && : >"$PLIST"
  local seq_file=""
  if [[ -n $print_seq ]]; then
    printf '%s\n' "$print_seq" >"$PRINT_SEQ_FILE"
    seq_file="$PRINT_SEQ_FILE"
  fi
  local bootout_fail=0
  case "$failure" in
    bootout-fails) bootout_fail=1 ;;
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
    LAUNCHCTL_BOOTOUT_FAIL="$bootout_fail" LAUNCHCTL_PRINT_SEQ_FILE="$seq_file" \
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

# TRI-STATE, first probe is an OPERATIONAL ERROR (not 0=loaded, not
# 113=confirmed-absent): the state is unknown, so the script must NOT bootout
# and must NOT claim retirement; it warns and exits 0. The argv trace is
# exactly one `print` (no bootout, no second probe).
run_case first-probe-error not-loaded no-plist none "2"
[[ $RC -eq 0 ]] || fail "first-probe-error: an operational probe error aborted the apply (rc=$RC)"
[[ "$(bootout_count)" -eq 0 ]] ||
  fail "first-probe-error: bootout attempted though the load state is unknown ($(cat "$RECORD"))"
grep -q 'retired' "$OUT_FILE" &&
  fail "first-probe-error: retired message printed though the load state was never determined ($(cat "$OUT_FILE"))"
[[ -s $ERR_FILE ]] ||
  fail "first-probe-error: no warning printed about the unexpected probe status"
[[ "$(printf '%s\n' "$(grep -c '^print ' "$RECORD")")" -eq 1 ]] ||
  fail "first-probe-error: expected exactly one print probe and no re-probe ($(cat "$RECORD"))"

# TRI-STATE, second probe is an OPERATIONAL ERROR after a bootout: first probe
# 0 (loaded) -> bootout -> re-probe returns an unexpected status (not
# 113=confirmed-absent, not 0=still-loaded). Absence is NOT confirmed, so the
# script must NOT claim retirement; it warns and exits 0. The argv trace is
# print, bootout, print (in that order).
run_case second-probe-error loaded no-plist none "0 2"
[[ $RC -eq 0 ]] || fail "second-probe-error: an operational re-probe error aborted the apply (rc=$RC)"
[[ "$(bootout_count)" -eq 1 ]] ||
  fail "second-probe-error: bootout was not attempted exactly once ($(cat "$RECORD"))"
grep -q 'retired' "$OUT_FILE" &&
  fail "second-probe-error: retired message printed though the re-probe never confirmed absence ($(cat "$OUT_FILE"))"
[[ -s $ERR_FILE ]] ||
  fail "second-probe-error: no warning printed about the unconfirmed unload"
printf 'print %s\nbootout %s\nprint %s\n' "$TARGET" "$TARGET" "$TARGET" | cmp -s - "$RECORD" ||
  fail "second-probe-error: argv trace is not exactly print/bootout/print ($(cat "$RECORD"))"

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
  LAUNCHCTL_BOOTOUT_FAIL=0 LAUNCHCTL_PRINT_SEQ_FILE="" \
  bash "$rendered" >"$OUT_FILE" 2>"$ERR_FILE" || RC=$?
[[ $RC -eq 0 ]] || fail "idempotent(2): second run must still exit 0, got $RC"
[[ "$(bootout_count)" -eq 1 ]] ||
  fail "idempotent(2): bootout ran again on a second pass ($(cat "$RECORD"))"
[[ ! -s $OUT_FILE ]] || fail "idempotent(2): second run should be silent ($(cat "$OUT_FILE"))"

printf 'PASS: the retirement script boots out exactly gui/<uid>/com.claude.code, treats the load probe as tri-state (0=loaded, 113=confirmed-absent, else=operational-error), claims retirement ONLY after a re-probe confirms absence (113), warns without any success claim on a still-loaded or operational-error re-probe, warns and makes no change on an operational-error first probe, survives id/rm/bootout failures with exit 0, removes the leftover plist, and is idempotent\n'
