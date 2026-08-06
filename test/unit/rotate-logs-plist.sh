#!/usr/bin/env bash
#
# The com.webdavis.rotate-logs LaunchAgent must run on a schedule (rotation that
# only happens when someone remembers to run it is not rotation), and must NOT
# run at load time.
#
# RunAtLoad false is deliberate: `chezmoi apply` loads the agent, and the first
# pass will archive-and-truncate whatever is already oversized. Deferring that
# to the next scheduled tick keeps `chezmoi apply` from silently reshaping
# ~/.local/log the instant it is run.
#
# Unit test: render the plist template with the host chezmoi and assert the
# schedule fields. No launchctl, no side effects.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PLIST="$REPO_ROOT/Library/LaunchAgents/com.webdavis.rotate-logs.plist.tmpl"

fail() {
  printf 'rotate-logs-plist: FAIL -- %s\n' "$*" >&2
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

grep -qF '<key>StartCalendarInterval</key>' "$rendered" ||
  fail "no StartCalendarInterval: rotation must be scheduled, not manual"

# Hourly: a Minute key with no Hour key means "every hour at that minute".
assert_kv Minute '<integer>15</integer>'
if grep -qF '<key>Hour</key>' "$rendered"; then
  fail "an Hour key pins the agent to once a day; rotation must run hourly"
fi

assert_kv RunAtLoad '<false/>'
assert_kv Label '<string>com.webdavis.rotate-logs</string>'

grep -qF '/.local/libexec/compress-and-truncate-local-logs.sh' "$rendered" ||
  fail "the agent does not invoke ~/.local/libexec/compress-and-truncate-local-logs.sh"

printf 'rotate-logs-plist: OK (hourly at :15, RunAtLoad false)\n'
