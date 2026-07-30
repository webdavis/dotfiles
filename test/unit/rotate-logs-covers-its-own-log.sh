#!/usr/bin/env bash
#
# The trap this mechanism could most easily fall into: a log rotator whose OWN
# LaunchAgent log grows without bound. That would reintroduce, in the fix
# itself, the exact condition being fixed.
#
# Two independent pins, because either alone is weak:
#
#   structural  the plist's StandardOutPath must resolve INSIDE the root the
#               script scans by default. Derived from both files rather than
#               asserted against a hardcoded string, so moving either one
#               without the other fails here.
#   behavioural a file at the rotator's own log path, seeded over the
#               threshold, must actually rotate. The structural check alone
#               would still pass if the scan excluded the file for some other
#               reason (a name filter, a depth limit).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROTATE_LOGS="$REPO_ROOT/dot_local/bin/executable_rotate-logs.sh"
PLIST="$REPO_ROOT/Library/LaunchAgents/com.webdavis.rotate-logs.plist.tmpl"

failures=0
fail() {
  printf 'rotate-logs-covers-its-own-log: FAIL -- %s\n' "$*" >&2
  failures=$((failures + 1))
}

[[ -f $ROTATE_LOGS ]] || {
  printf 'rotate-logs-covers-its-own-log: FAIL -- missing script: %s\n' "$ROTATE_LOGS" >&2
  exit 1
}
[[ -f $PLIST ]] || {
  printf 'rotate-logs-covers-its-own-log: FAIL -- missing plist: %s\n' "$PLIST" >&2
  exit 1
}

sandbox="$(mktemp -d)"
trap 'rm -rf "$sandbox"' EXIT

# stat(1) differs between GNU and BSD, and the nix dev shell can put GNU
# coreutils first on PATH even on macOS, where the BSD `-f` flag then means
# "filesystem status" and succeeds with the wrong output. GNU form first,
# BSD form as the fallback -- the order the repo guard enforces.
file_size() { stat -c '%s' "$1" 2>/dev/null || stat -f '%z' "$1"; }

# --- structural: the agent logs inside the scanned root --------------------
if command -v chezmoi >/dev/null 2>&1; then
  fake_home="$sandbox/home"
  mkdir -p "$fake_home"
  rendered="$sandbox/plist.xml"
  HOME="$fake_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
    <"$PLIST" >"$rendered" || fail "chezmoi failed to render the plist"

  # The script's default root, resolved against the same HOME the plist saw.
  # LOG_ROOT is defined by the sourced script, which shellcheck cannot follow
  # through a variable path, hence the SC2153 exemption rather than a rename.
  default_root="$(
    HOME="$fake_home"
    # shellcheck source=/dev/null
    source "$ROTATE_LOGS"
    # shellcheck disable=SC2153
    printf '%s' "$LOG_ROOT"
  )"
  [[ -n $default_root ]] || fail "could not read LOG_ROOT from the script"

  agent_log="$(/usr/bin/sed -n 's|.*<string>\(.*rotate-logs\.log\)</string>.*|\1|p' "$rendered" | /usr/bin/head -1)"
  [[ -n $agent_log ]] ||
    fail "plist declares no StandardOutPath ending in rotate-logs.log"

  if [[ -n $agent_log && -n $default_root ]]; then
    case "$agent_log" in
      "$default_root"/*) : ;;
      *) fail "the agent logs to '$agent_log', which is OUTSIDE the scanned root '$default_root': its own log would grow unbounded" ;;
    esac
  fi
else
  printf 'rotate-logs-covers-its-own-log: NOTE chezmoi absent, structural check skipped\n'
fi

# --- behavioural: that path really does rotate -----------------------------
log_root="$sandbox/log"
mkdir -p "$log_root"
own_log="$log_root/rotate-logs.log"
/usr/bin/head -c 4096 /dev/zero | /usr/bin/tr '\0' 'r' >"$own_log"

ROTATE_LOGS_ROOT="$log_root" \
  ROTATE_LOGS_AT_BYTES=1024 \
  ROTATE_LOGS_ARCHIVES_KEPT=3 \
  bash "$ROTATE_LOGS" >"$sandbox/report.txt" 2>&1 ||
  fail "rotation pass exited non-zero: $(cat "$sandbox/report.txt")"

own_size="$(file_size "$own_log")"
[[ $own_size -eq 0 ]] ||
  fail "the rotator's own log was NOT rotated (size $own_size): the fix admits the condition it fixes"
[[ -f $own_log.1.gz ]] || fail "no archive for the rotator's own log"

if [[ $failures -gt 0 ]]; then
  printf 'rotate-logs-covers-its-own-log: %d assertion(s) failed\n' "$failures" >&2
  exit 1
fi

printf "rotate-logs-covers-its-own-log: OK (agent log is inside the scanned root and rotates)\n"
