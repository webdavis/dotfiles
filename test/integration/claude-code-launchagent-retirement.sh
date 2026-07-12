#!/usr/bin/env bash
# claude-code-launchagent-retirement.sh: the com.claude.code retirement
# chezmoiscript (run_after_56) must convergently retire the old tmux-coupled
# supervisor. Removing the plist SOURCE (done in S4) does NOT unload an
# already-running LaunchAgent; this is the live-side complement.
#
# It is a run_after, NOT a run_once: run_once records success permanently even
# when a probe or action transiently failed. Instead it gates on a quiescence
# marker at ~/.local/state/claude-code-launchagent/retired. Steady state is one
# file check; the marker is written ONLY after a re-probe confirms the label
# absent (launchctl print not-found = 113) AND the leftover plist is gone.
#
# Convergence + honesty contract (the marker never lies):
#   - the marker + "retirement complete" print ONLY when the label is confirmed
#     absent (113) and the plist is gone
#   - a FAILED bootout leaves the service loaded: the plist is RETAINED and the
#     marker is NOT written (retryable next apply)
#   - the load probe is TRI-STATE (0 = loaded, 113 = confirmed absent, anything
#     else = operational error); an operational error on the first probe makes
#     no change and no bootout, and on the re-probe holds the plist + marker --
#     a nonzero-but-not-113 is never misread as "not loaded"
#   - the plist is removed ONLY when the label is confirmed absent (never while
#     loaded or unknown -- that could orphan a running job)
#   - failing id / launchctl / rm never abort the apply (always exit 0)
#   - the bootout targets the EXACT domain target gui/<uid>/com.claude.code
#
# It renders the REAL template and runs it against a stub `launchctl` on PATH
# that models the GUI domain with a state file: `print` reports loaded while the
# state file exists (else the not-found status 113), `bootout` records its argv
# and clears the state. Failure modes are toggled via env vars. No live launchd
# state is touched.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/.chezmoiscripts/run_after_56-retire-claude-code-launchagent.sh.tmpl"

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

marker_of() { printf '%s/.local/state/claude-code-launchagent/retired' "$1"; }

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
  MARKER="$(marker_of "$CASE_HOME")"
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
    state-unwritable)
      # Pre-create the marker's state dir path as a FILE, so `mkdir -p` (and the
      # marker write) fail even after a full convergence -- an unwritable state
      # dir (e.g. a root-owned leftover) must not abort the whole apply.
      mkdir -p "$CASE_HOME/.local/state"
      : >"$CASE_HOME/.local/state/claude-code-launchagent"
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

# Loaded + plist present: unloaded (exact target), re-probed absent, plist
# removed, marker written.
run_case loaded-plist loaded plist
[[ $RC -eq 0 ]] || fail "loaded-plist: expected exit 0, got $RC ($(cat "$ERR_FILE"))"
grep -qx "bootout $TARGET" "$RECORD" ||
  fail "loaded-plist: bootout argv is not exactly 'bootout $TARGET' ($(cat "$RECORD"))"
grep -qx "print $TARGET" "$RECORD" ||
  fail "loaded-plist: the load probe argv is not exactly 'print $TARGET' ($(cat "$RECORD"))"
grep -q 'retirement complete' "$OUT_FILE" ||
  fail "loaded-plist: no completion message after a confirmed bootout ($(cat "$OUT_FILE"))"
[[ ! -e $PLIST ]] || fail "loaded-plist: leftover plist was not removed"
[[ -f $MARKER ]] || fail "loaded-plist: quiescence marker not written after full convergence"

# Not loaded + no plist: no bootout, marker written (convergent no-op).
run_case clean not-loaded no-plist
[[ $RC -eq 0 ]] || fail "clean: expected exit 0, got $RC"
[[ "$(bootout_count)" -eq 0 ]] || fail "clean: bootout called though the service is not loaded"
[[ -f $MARKER ]] || fail "clean: marker not written on an already-clean host"

# Loaded + no plist: unloaded, re-probed absent, marker written.
run_case loaded-only loaded no-plist
[[ $RC -eq 0 ]] || fail "loaded-only: expected exit 0, got $RC"
[[ "$(bootout_count)" -eq 1 ]] || fail "loaded-only: bootout not called exactly once for a loaded service"
[[ -f $MARKER ]] || fail "loaded-only: marker not written after a confirmed unload"

# Not loaded + leftover plist: no bootout, plist removed, marker written.
run_case plist-only not-loaded plist
[[ $RC -eq 0 ]] || fail "plist-only: expected exit 0, got $RC"
[[ "$(bootout_count)" -eq 0 ]] || fail "plist-only: bootout called though the service is not loaded"
[[ ! -e $PLIST ]] || fail "plist-only: leftover plist was not removed"
[[ -f $MARKER ]] || fail "plist-only: marker not written after removing the orphan plist"

# Bootout FAILS and the service stays loaded: the plist is RETAINED (removing it
# would orphan the running job) and the marker is NOT written (retryable next
# apply); a loud warning names the exact manual command, and still exit 0.
run_case bootout-fails loaded plist bootout-fails
[[ $RC -eq 0 ]] || fail "bootout-fails: a failed bootout aborted the apply (rc=$RC; stderr: $(cat "$ERR_FILE"))"
grep -q 'retirement complete' "$OUT_FILE" &&
  fail "bootout-fails: completion message printed though the service is STILL loaded ($(cat "$OUT_FILE"))"
[[ -e $PLIST ]] ||
  fail "bootout-fails: the plist was removed though the service is STILL loaded (orphans the running job)"
[[ ! -e $MARKER ]] ||
  fail "bootout-fails: marker written though the service is STILL loaded (a run_once-style lie)"
grep -q "launchctl bootout $TARGET" "$ERR_FILE" ||
  fail "bootout-fails: the warning does not name the exact manual command 'launchctl bootout $TARGET' (stderr: $(cat "$ERR_FILE"))"

# TRI-STATE, first probe is an OPERATIONAL ERROR (not 0=loaded, not
# 113=confirmed-absent): the state is unknown, so the script must NOT bootout,
# must NOT remove the plist, and must NOT write the marker; it warns and exits 0.
# The argv trace is exactly one `print` (no bootout, no re-probe).
run_case first-probe-error not-loaded plist none "112"
[[ $RC -eq 0 ]] || fail "first-probe-error: an operational probe error aborted the apply (rc=$RC)"
[[ "$(bootout_count)" -eq 0 ]] ||
  fail "first-probe-error: bootout attempted though the load state is unknown ($(cat "$RECORD"))"
[[ -e $PLIST ]] || fail "first-probe-error: plist removed though the load state was never determined"
[[ ! -e $MARKER ]] || fail "first-probe-error: marker written though the load state was never determined"
[[ -s $ERR_FILE ]] || fail "first-probe-error: no warning printed about the unexpected probe status"
[[ "$(grep -c '^print ' "$RECORD")" -eq 1 ]] ||
  fail "first-probe-error: expected exactly one print probe and no re-probe ($(cat "$RECORD"))"

# TRI-STATE, second probe is an OPERATIONAL ERROR after a bootout: first probe 0
# (loaded) -> bootout -> re-probe returns an unexpected status (not
# 113=confirmed-absent, not 0=still-loaded). Absence is NOT confirmed, so the
# plist is retained and the marker is not written; it warns and exits 0. The
# argv trace is print, bootout, print.
run_case second-probe-error loaded plist none "0 112"
[[ $RC -eq 0 ]] || fail "second-probe-error: an operational re-probe error aborted the apply (rc=$RC)"
[[ "$(bootout_count)" -eq 1 ]] ||
  fail "second-probe-error: bootout was not attempted exactly once ($(cat "$RECORD"))"
[[ -e $PLIST ]] || fail "second-probe-error: plist removed though the re-probe never confirmed absence"
[[ ! -e $MARKER ]] || fail "second-probe-error: marker written though the re-probe never confirmed absence"
printf 'print %s\nbootout %s\nprint %s\n' "$TARGET" "$TARGET" "$TARGET" | cmp -s - "$RECORD" ||
  fail "second-probe-error: argv trace is not exactly print/bootout/print ($(cat "$RECORD"))"

# `rm` FAILS (read-only LaunchAgents dir): the plist survives, so convergence is
# incomplete -- warn, no marker, exit 0, never abort.
run_case rm-fails not-loaded plist rm-fails
[[ $RC -eq 0 ]] || fail "rm-fails: a failed plist removal aborted the apply (rc=$RC; stderr: $(cat "$ERR_FILE"))"
[[ ! -e $MARKER ]] || fail "rm-fails: marker written though the plist could not be removed"
[[ -s $ERR_FILE ]] || fail "rm-fails: no warning printed about the unremovable plist"

# `id -u` FAILS: cannot address the gui domain -> no bootout, the plist is
# retained (state unknown), no marker, warn, exit 0.
run_case id-fails loaded plist id-fails
[[ $RC -eq 0 ]] || fail "id-fails: a failing id aborted the apply (rc=$RC; stderr: $(cat "$ERR_FILE"))"
[[ "$(bootout_count)" -eq 0 ]] || fail "id-fails: bootout attempted without a valid uid ($(cat "$RECORD"))"
[[ -e $PLIST ]] || fail "id-fails: plist removed though the uid (and thus the load state) is unknown"
[[ ! -e $MARKER ]] || fail "id-fails: marker written though the load state is unknown"
[[ -s $ERR_FILE ]] || fail "id-fails: no warning printed about the failed uid lookup"

# State dir UNWRITABLE at the marker write (F4): a full convergence (bootout +
# plist removed) reaches the marker write, but `mkdir -p`/`: >marker` are under
# `set -euo pipefail` -- an unwritable state dir (a root-owned leftover) would
# abort the ENTIRE chezmoi apply. The guarded write must warn, NOT claim
# completion, write no marker, and still exit 0 so the next apply retries.
run_case state-unwritable loaded plist state-unwritable
[[ $RC -eq 0 ]] ||
  fail "state-unwritable: an unwritable state dir aborted the apply (rc=$RC; stderr: $(cat "$ERR_FILE"))"
[[ ! -e $MARKER ]] || fail "state-unwritable: marker written though the state dir is unwritable"
grep -q 'retirement complete' "$OUT_FILE" &&
  fail "state-unwritable: claimed completion though the marker could not be written ($(cat "$OUT_FILE"))"
grep -q 'could not be written' "$ERR_FILE" ||
  fail "state-unwritable: no warning that the marker could not be written (stderr: $(cat "$ERR_FILE"))"

# Idempotence via the marker: a loaded service, run twice. The first run boots it
# out and writes the marker; the second sees the marker and short-circuits with
# ZERO launchctl invocations.
run_case idempotent loaded plist
[[ $RC -eq 0 ]] || fail "idempotent(1): expected exit 0, got $RC"
[[ -f $MARKER ]] || fail "idempotent(1): marker not written on the first converging run"
RC=0
: >"$RECORD"
HOME="$CASE_HOME" PATH="$work/idempotent/bin:$PATH" \
  LAUNCHCTL_STATE="$STATE" LAUNCHCTL_RECORD="$RECORD" \
  LAUNCHCTL_BOOTOUT_FAIL=0 LAUNCHCTL_PRINT_SEQ_FILE="" \
  bash "$rendered" >"$OUT_FILE" 2>"$ERR_FILE" || RC=$?
[[ $RC -eq 0 ]] || fail "idempotent(2): second run must still exit 0, got $RC"
[[ ! -s $RECORD ]] ||
  fail "idempotent(2): the marker fast path did not short-circuit (launchctl was invoked: $(cat "$RECORD"))"
[[ ! -s $OUT_FILE ]] || fail "idempotent(2): second run should be silent ($(cat "$OUT_FILE"))"

printf 'PASS: the convergent retirement script boots out exactly gui/<uid>/com.claude.code, treats the load probe as tri-state, removes the plist and writes the quiescence marker ONLY after a re-probe confirms absence (113), retains the plist and withholds the marker on a failed bootout / still-loaded / operational-error probe / unremovable plist / failed uid, survives id/rm/bootout failures with exit 0, and fast-paths on the marker\n'
