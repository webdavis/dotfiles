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

# The agent must pass --scheduled, and it is the ONLY caller that does. Only a
# scheduled run posts the weekly record, so without the marker the agent could
# run every Monday and post nothing, which reads exactly like a dead agent. With
# it, `just brew-upgrade` on a Wednesday stays silent instead of making a dead
# agent look alive. Parsed as real plist data (plutil -> json -> jq) so a marker
# lost inside a malformed array cannot pass a text grep.
plist_json="$work/plist.json"
plutil -convert json -o "$plist_json" - <"$rendered" 2>/dev/null ||
  fail "the rendered plist did not parse as a plist"
prog_scheduled="$(jq -r '[.ProgramArguments[] | select(. == "--scheduled")] | length' "$plist_json")"
[[ $prog_scheduled == "1" ]] ||
  fail "ProgramArguments does not pass exactly one --scheduled marker (got $prog_scheduled); the weekly record would never be posted"
helper_arg="$(jq -r '.ProgramArguments[1]' "$plist_json")"
[[ $helper_arg == */.local/libexec/unattended-upgrades/homebrew-weekly-upgrade.sh ]] ||
  fail "the agent does not run the deployed helper (got $helper_arg)"

printf 'homebrew-weekly-plist: OK (Monday 12:00, Weekday 1; RunAtLoad false; --scheduled passed once)\n'
