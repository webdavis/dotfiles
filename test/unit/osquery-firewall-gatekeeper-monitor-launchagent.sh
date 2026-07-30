#!/usr/bin/env bash
#
# The scheduled security-posture poller LaunchAgent:
# com.webdavis.osquery-firewall-gatekeeper-monitor must run the deployed poller
# every 60 seconds AND at load time. RunAtLoad is true on purpose: on load the
# poller establishes the baseline and, via the first-observation floor, surfaces
# an already-off protection immediately instead of waiting a full interval. It is
# a gui/<uid> USER agent, not a system daemon: the screenlock osquery table is
# scoped to the logged-in user, so the poller needs the user session to read it.
# Its loader chezmoiscript must be wired exactly like the sibling osquery agents
# (darwin gate, plist-hash onchange trigger, bootout + bootstrap with the retry
# loop).
#
# Unit test: render the plist and loader templates with the host chezmoi and
# assert their content. No launchctl, no side effects.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PLIST="$REPO_ROOT/Library/LaunchAgents/com.webdavis.osquery-firewall-gatekeeper-monitor.plist.tmpl"
LOADER="$REPO_ROOT/.chezmoiscripts/run_onchange_after_60-load-osquery-firewall-gatekeeper-monitor-launchagent.sh.tmpl"

fail() {
  printf 'osquery-firewall-gatekeeper-monitor-launchagent: FAIL -- %s\n' "$*" >&2
  exit 1
}

command -v chezmoi >/dev/null 2>&1 || {
  printf 'SKIP: chezmoi not on PATH; cannot render the templates\n'
  exit 0
}
[[ -f $PLIST ]] || fail "missing plist template: $PLIST"
[[ -f $LOADER ]] || fail "missing loader chezmoiscript: $LOADER"

# Render both templates exactly as at apply time. --source pins the render to
# THIS checkout (hermetic), mirroring the sibling launchagent tests.
rendered_plist="$(CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$PLIST")" ||
  fail "chezmoi failed to render the plist"
[[ -n $rendered_plist ]] || fail "empty plist render"
rendered_loader="$(CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$LOADER")" ||
  fail "chezmoi failed to render the loader"

home_dir="$(chezmoi --source "$REPO_ROOT" execute-template --no-tty <<<'{{ .chezmoi.homeDir }}')"

# assert_plist_kv <key> <expected-value-line-fragment> -- a plist pairs a <key>
# with the value element on the FOLLOWING line, so match the key then its next
# line (same helper shape as the sibling launchagent tests).
assert_plist_kv() {
  local key="$1" want="$2"
  if ! printf '%s\n' "$rendered_plist" | grep -A1 -F "<key>$key</key>" | grep -qF "$want"; then
    printf 'rendered plist:\n%s\n' "$rendered_plist" >&2
    fail "expected <key>$key</key> followed by '$want'"
  fi
}

# --- the plist: label, schedule, program path, logs -------------------------

assert_plist_kv Label '<string>com.webdavis.osquery-firewall-gatekeeper-monitor</string>'
assert_plist_kv StartInterval '<integer>60</integer>'
# RunAtLoad true: on load the poller establishes the baseline and surfaces an
# already-off protection immediately (the first-observation floor), not a full
# interval later. Matches c69baab and the uptime-watchdog sibling.
assert_plist_kv RunAtLoad '<true/>'

# ProgramArguments runs the DEPLOYED poller from the libexec home (the
# executable_ source prefix is dropped in the target).
printf '%s\n' "$rendered_plist" |
  grep -qF "<string>$home_dir/.local/libexec/osquery/firewall-gatekeeper-monitor.sh</string>" ||
  fail "ProgramArguments does not run $home_dir/.local/libexec/osquery/firewall-gatekeeper-monitor.sh"

# Both log streams land in the osquery log dir, like every sibling agent.
assert_plist_kv StandardOutPath "<string>$home_dir/.local/log/osquery/firewall-gatekeeper-monitor.log</string>"
assert_plist_kv StandardErrorPath "<string>$home_dir/.local/log/osquery/firewall-gatekeeper-monitor.log</string>"

# --- the loader: darwin gate, onchange trigger, gui user target, bootout+bootstrap retry ----

# The loader re-runs when the plist CONTENT changes: the plist-hash include line
# is the onchange trigger, and it must name the poller's own plist template.
grep -qF 'include "Library/LaunchAgents/com.webdavis.osquery-firewall-gatekeeper-monitor.plist.tmpl"' "$LOADER" ||
  fail "loader is missing the plist-hash onchange trigger for the poller plist"
grep -qE 'eq \.chezmoi\.os "darwin"' "$LOADER" ||
  fail "loader is not darwin-gated like the sibling osquery loaders"

# The rendered loader targets the poller's label and plist as a gui/<uid> USER
# agent (NOT a system daemon: the screenlock table needs the user session), and
# keeps the sibling bootout-then-bootstrap shape with the retry loop. The needles
# below are literal loader lines; $HOME and $(id -u) must NOT expand here (same
# rule as the feature-set-location test's source-line needles).
# shellcheck disable=SC2016
printf '%s\n' "$rendered_loader" |
  grep -qF 'PLIST="$HOME/Library/LaunchAgents/com.webdavis.osquery-firewall-gatekeeper-monitor.plist"' ||
  fail "rendered loader does not point at the poller plist"
# shellcheck disable=SC2016
printf '%s\n' "$rendered_loader" |
  grep -qF 'TARGET="gui/$(id -u)/com.webdavis.osquery-firewall-gatekeeper-monitor"' ||
  fail "rendered loader does not target the poller label as a gui/<uid> user agent"
# shellcheck disable=SC2016
printf '%s\n' "$rendered_loader" | grep -qF 'launchctl bootout "$TARGET"' ||
  fail "rendered loader is missing the bootout step"
# shellcheck disable=SC2016
printf '%s\n' "$rendered_loader" | grep -qF 'launchctl bootstrap "gui/$(id -u)" "$PLIST"' ||
  fail "rendered loader is missing the bootstrap step"
printf '%s\n' "$rendered_loader" | grep -qF 'for _ in 1 2 3; do' ||
  fail "rendered loader is missing the bootstrap retry loop"

printf 'osquery-firewall-gatekeeper-monitor-launchagent: OK (label + 60s interval + RunAtLoad true; gui/<uid> user agent; deployed libexec program; loader gated, hashed, bootout+bootstrap with retry)\n'
