#!/usr/bin/env bash
#
# Fix 5 (plist wiring): the com.webdavis.homebrew-weekly-upgrade LaunchAgent must
# fire Monday at 12:00 (launchd Weekday 1 == Monday) and must NOT run at load
# time (RunAtLoad false), so loading the agent never triggers an unattended
# upgrade -- the whole point is that upgrades happen only when the operator is
# present at Monday noon.
#
# Unit test: render the plist template with the host chezmoi and assert the
# StartCalendarInterval fields and RunAtLoad. No launchctl, no side effects.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PLIST="$REPO_ROOT/Library/LaunchAgents/com.webdavis.homebrew-weekly-upgrade.plist.tmpl"

fail() {
  printf 'homebrew-weekly-plist: FAIL -- %s\n' "$*" >&2
  exit 1
}

command -v chezmoi >/dev/null 2>&1 || {
  printf 'SKIP: chezmoi not on PATH; cannot render the plist\n'
  exit 0
}
[[ -f $PLIST ]] || fail "missing plist template: $PLIST"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
rendered="$work/plist.xml"
render_home="$(mktemp -d)"
HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$PLIST" >"$rendered" || fail "chezmoi failed to render the plist"
rm -rf "$render_home"
[[ -s $rendered ]] || fail "empty plist render"

# assert_kv <key> <expected-value-line-fragment> -- the plist pairs a <key> with
# the value element on the FOLLOWING line, so match the key then its next line.
assert_kv() {
  local key="$1" want="$2"
  if ! grep -A1 -F "<key>$key</key>" "$rendered" | grep -qF "$want"; then
    printf 'rendered plist:\n' >&2
    cat "$rendered" >&2
    fail "expected <key>$key</key> followed by '$want'"
  fi
}

# Must actually be a calendar-scheduled agent, not an interval one.
grep -qF '<key>StartCalendarInterval</key>' "$rendered" ||
  fail "no StartCalendarInterval (not a calendar-scheduled agent)"

assert_kv Weekday '<integer>1</integer>' # Monday
assert_kv Hour '<integer>12</integer>'   # 12:00
assert_kv Minute '<integer>0</integer>'
assert_kv RunAtLoad '<false/>' # loading must never trigger an upgrade

printf 'homebrew-weekly-plist: OK (Monday 12:00, Weekday 1; RunAtLoad false)\n'
