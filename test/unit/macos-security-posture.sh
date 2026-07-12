#!/usr/bin/env bash
# macos-security-posture.sh -- the security-posture reminder
# (run_after_67-macos-security-posture) must REPORT FileVault, Gatekeeper, and SIP
# (System Integrity Protection) state and NEVER change any of them. It renders the
# REAL chezmoiscript and runs the rendered body against fake fdesetup/spctl/csrutil
# binaries (FDESETUP_BIN/SPCTL_BIN/CSRUTIL_BIN) per posture. The fakes answer ONLY
# their status query; ANY other invocation (an enable/--master-enable fix attempt)
# is logged to a mutation file and fails, so a single fix attempt is caught.
#
# Asserts, for every case: exit 0 (a reminder must never abort `chezmoi apply`) and
# an EMPTY mutation log (assert-only). Plus: enabled -> a stdout OK line and no
# DISABLED warning; disabled -> a stderr DISABLED warning; unknown -> a
# "could not determine" note. SIP and FileVault are assert-only by policy; a
# disabled stub must produce a warning, never a fix.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/.chezmoiscripts/run_after_67-macos-security-posture.sh.tmpl"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

command -v chezmoi >/dev/null 2>&1 || {
  printf 'SKIP: chezmoi not on PATH; cannot render the posture reminder\n'
  exit 0
}
[[ -f $SCRIPT ]] || fail "missing template: $SCRIPT"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Render the darwin-only script once (scratch HOME, CI=1). Empty render ==
# non-darwin host: skip.
rendered="$work/rendered.sh"
render_home="$(mktemp -d)"
HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$SCRIPT" >"$rendered" || fail "chezmoi failed to render $SCRIPT"
rm -rf "$render_home"
if [[ ! -s $rendered ]]; then
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi

# make_stub <path> <status_output> <mutation_log> [status_rc]: a fake fdesetup/
# spctl/csrutil. It answers `status` / `--status` with the given text and exit code
# (default 0); ANY other argument is a fix attempt -> record it and fail loudly. A
# nonzero status_rc models a DEGRADED query (a probe that prints text yet fails).
make_stub() {
  local path="$1" out="$2" mlog="$3" status_rc="${4:-0}"
  cat >"$path" <<STUB
#!/bin/bash
case "\$1" in
  status|--status) printf '%s\n' "$out"; exit $status_rc ;;
  *) printf '%s %s\n' "\$(basename "\$0")" "\$*" >>"$mlog"; exit 1 ;;
esac
STUB
  chmod +x "$path"
}

# run_case <name> <fv_out> <gk_out> <sip_out> [fv_rc] [gk_rc] [sip_rc] -> populates
# RC, OUT, ERR, MUT. The trailing rc args default to 0 (successful query).
run_case() {
  local name="$1" fv="$2" gk="$3" sip="$4"
  local fv_rc="${5:-0}" gk_rc="${6:-0}" sip_rc="${7:-0}"
  local dir="$work/$name"
  mkdir -p "$dir"
  local mlog="$dir/mutations"
  : >"$mlog"
  make_stub "$dir/fdesetup" "$fv" "$mlog" "$fv_rc"
  make_stub "$dir/spctl" "$gk" "$mlog" "$gk_rc"
  make_stub "$dir/csrutil" "$sip" "$mlog" "$sip_rc"
  RC=0
  OUT="$(FDESETUP_BIN="$dir/fdesetup" SPCTL_BIN="$dir/spctl" CSRUTIL_BIN="$dir/csrutil" \
    bash "$rendered" 2>"$dir/err")" || RC=$?
  ERR="$(cat "$dir/err")"
  MUT="$(cat "$mlog")"
}

failures=0
report() {
  if [[ $1 == ok ]]; then printf '  ok   %s\n' "$2"; else
    printf '  FAIL %s\n' "$2"
    failures=$((failures + 1))
  fi
}
a0() { if [[ $RC -eq 0 ]]; then report ok "$1: exits 0"; else report bad "$1: exits 0 (got rc=$RC; err: $ERR)"; fi; }
# Assert-only: no fix command was ever invoked.
anomut() { if [[ -z $MUT ]]; then report ok "$1: no mutation attempted"; else report bad "$1: attempted a fix: $MUT"; fi; }
outhas() { if grep -qi -- "$2" <<<"$OUT"; then report ok "$1: stdout has '$2'"; else report bad "$1: stdout has '$2' (out: $OUT)"; fi; }
outno() { if grep -qi -- "$2" <<<"$OUT"; then report bad "$1: stdout must NOT contain '$2' (out: $OUT)"; else report ok "$1: no '$2' in stdout"; fi; }
errhas() { if grep -qi -- "$2" <<<"$ERR"; then report ok "$1: stderr warns '$2'"; else report bad "$1: stderr warns '$2' (err: $ERR)"; fi; }
errno() { if grep -qi -- "$2" <<<"$ERR"; then report bad "$1: stderr must NOT contain '$2' (err: $ERR)"; else report ok "$1: no '$2' in stderr"; fi; }

printf 'macos-security-posture cases:\n'

# All enabled -> OK on stdout, no warnings, no mutation.
run_case enabled "FileVault is On." "assessments enabled" "System Integrity Protection status: enabled."
a0 enabled
anomut enabled
outhas enabled "FileVault: enabled"
outhas enabled "Gatekeeper: enabled"
outhas enabled "SIP: enabled"
errno enabled "DISABLED"

# All disabled -> a DISABLED warning per posture, exit 0, and NO fix attempt.
run_case disabled "FileVault is Off." "assessments disabled" "System Integrity Protection status: disabled."
a0 disabled
anomut disabled
errhas disabled "FileVault is DISABLED"
errhas disabled "Gatekeeper is DISABLED"
errhas disabled "SIP is DISABLED"

# Unparseable -> a could-not-determine note, exit 0, no mutation.
run_case unknown "gibberish" "gibberish" "unknown (Custom Configuration)"
a0 unknown
anomut unknown
errhas unknown "could not determine"

# Degraded query (R1-4): a probe that prints ENABLED-looking text but EXITS
# NONZERO must be reported INDETERMINATE, never enabled -- a failed query's stdout
# is untrustworthy. Here fdesetup fails (rc 23) while printing "FileVault is On.";
# Gatekeeper and SIP succeed and stay classifiable. FileVault must read
# could-not-determine (never "FileVault: enabled"); the others still report enabled.
run_case degraded "FileVault is On." "assessments enabled" \
  "System Integrity Protection status: enabled." 23 0 0
a0 degraded
anomut degraded
errhas degraded "FileVault: could not determine"
outno degraded "FileVault: enabled"
outhas degraded "Gatekeeper: enabled"
outhas degraded "SIP: enabled"

if [[ $failures -gt 0 ]]; then
  printf 'macos-security-posture: %d assertion(s) FAILED\n' "$failures" >&2
  exit 1
fi
printf 'macos-security-posture: OK (reports FileVault/Gatekeeper/SIP; warns on disabled; never mutates; always exits 0)\n'
