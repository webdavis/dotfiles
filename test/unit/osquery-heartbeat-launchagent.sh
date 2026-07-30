#!/usr/bin/env bash
#
# The daily heartbeat LaunchAgent: com.webdavis.osquery-heartbeat runs the deployed
# heartbeat detector once a day at a time DECLARED IN DATA (.chezmoidata/osquery.yaml
# heartbeatHour/heartbeatMinute), never hardcoded in the plist. RunAtLoad is false: a
# daily proof-of-life fires at its StartCalendarInterval, not at apply time. A
# load-time run would fire the heartbeat at an arbitrary hour, and right after a boot
# (before osqueryd has written its first 600s canary) it would false-report the
# canary MISSING; the 09:00 daily fire plus the uptime watchdog (real-time liveness)
# cover the job. It is a gui/<uid> USER agent, not a system daemon: the heartbeat
# calls send_alert (the user's local notifier) and reads the user-scoped snapshot
# log, not any osquery table. Its loader chezmoiscript is wired like the sibling
# osquery agents (darwin gate, plist-hash onchange trigger, bootout + bootstrap
# retry), PLUS a rendered schedule line so a DATA-only osquery.yaml change re-fires
# it: the plist-hash include hashes the template SOURCE, whose {{ }} placeholders do
# not change when their values do.
#
# Unit test: render the plist and loader with the host chezmoi and assert their
# content, including that the schedule is DATA-DRIVEN (a different osquery.yaml
# renders a different StartCalendarInterval). No launchctl, no side effects.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PLIST="$REPO_ROOT/Library/LaunchAgents/com.webdavis.osquery-heartbeat.plist.tmpl"
LOADER="$REPO_ROOT/.chezmoiscripts/run_onchange_after_60-load-osquery-heartbeat-launchagent.sh.tmpl"
YAML="$REPO_ROOT/.chezmoidata/osquery.yaml"

fail() {
  printf 'osquery-heartbeat-launchagent: FAIL -- %s\n' "$*" >&2
  exit 1
}

command -v chezmoi >/dev/null 2>&1 || {
  printf 'SKIP: chezmoi not on PATH; cannot render the templates\n'
  exit 0
}
[[ -f $PLIST ]] || fail "missing plist template: $PLIST"
[[ -f $LOADER ]] || fail "missing loader chezmoiscript: $LOADER"
[[ -f $YAML ]] || fail "missing schedule data: $YAML"

# Render both templates exactly as at apply time. --source pins the render to THIS
# checkout (hermetic), mirroring the sibling launchagent tests.
rendered_plist="$(CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$PLIST")" ||
  fail "chezmoi failed to render the plist"
[[ -n $rendered_plist ]] || fail "empty plist render"
rendered_loader="$(CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$LOADER")" ||
  fail "chezmoi failed to render the loader"

home_dir="$(chezmoi --source "$REPO_ROOT" execute-template --no-tty <<<'{{ .chezmoi.homeDir }}')"

# assert_plist_kv <key> <expected-value-line-fragment> -- a plist pairs a <key> with
# the value element on the FOLLOWING line (same helper as the sibling agent tests).
assert_plist_kv() {
  local key="$1" want="$2"
  if ! printf '%s\n' "$rendered_plist" | grep -A1 -F "<key>$key</key>" | grep -qF "$want"; then
    printf 'rendered plist:\n%s\n' "$rendered_plist" >&2
    fail "expected <key>$key</key> followed by '$want'"
  fi
}

# --- the plist: label, program, logs, RunAtLoad ------------------------------

assert_plist_kv Label '<string>com.webdavis.osquery-heartbeat</string>'
# RunAtLoad false: a daily proof-of-life fires at its StartCalendarInterval, not at
# apply time (a load-time run would fire at an arbitrary hour, and could false-report
# the canary MISSING right after a boot before osqueryd has written its first one).
assert_plist_kv RunAtLoad '<false/>'

# ProgramArguments runs the DEPLOYED detector from the libexec home (the executable_
# source prefix is dropped in the target).
printf '%s\n' "$rendered_plist" |
  grep -qF "<string>$home_dir/.local/libexec/osquery/heartbeat.sh</string>" ||
  fail "ProgramArguments does not run $home_dir/.local/libexec/osquery/heartbeat.sh"

# Both log streams land in the osquery log dir, like every sibling agent.
assert_plist_kv StandardOutPath "<string>$home_dir/.local/log/osquery/heartbeat.log</string>"
assert_plist_kv StandardErrorPath "<string>$home_dir/.local/log/osquery/heartbeat.log</string>"

# --- the schedule is DATA-DRIVEN, not a hardcoded plist literal --------------

# Linkage: the rendered Hour/Minute equal the shipped osquery.yaml values.
shipped_hour="$(chezmoi --source "$REPO_ROOT" execute-template --no-tty <<<'{{ .osquery.heartbeatHour }}')"
shipped_minute="$(chezmoi --source "$REPO_ROOT" execute-template --no-tty <<<'{{ .osquery.heartbeatMinute }}')"
assert_plist_kv Hour "<integer>$shipped_hour</integer>"
assert_plist_kv Minute "<integer>$shipped_minute</integer>"

# Data DRIVES it: render the SAME plist template against a source whose osquery.yaml
# carries different values; the StartCalendarInterval must follow the data, which a
# hardcoded <integer> literal never would. (The plist has no `include`, so a minimal
# probe source suffices.)
probe_src="$(mktemp -d)"
trap 'rm -rf "$probe_src"' EXIT
mkdir -p "$probe_src/.chezmoidata"
printf 'osquery:\n  heartbeatHour: 4\n  heartbeatMinute: 8\n' >"$probe_src/.chezmoidata/osquery.yaml"
probe_plist="$(CI=1 chezmoi --source "$probe_src" execute-template --no-tty <"$PLIST")" ||
  fail "chezmoi failed to render the plist against the probe source"
printf '%s\n' "$probe_plist" | grep -A1 -F '<key>Hour</key>' | grep -qF '<integer>4</integer>' ||
  fail "a different osquery.yaml did not change the rendered Hour: the schedule is hardcoded, not data-driven"
printf '%s\n' "$probe_plist" | grep -A1 -F '<key>Minute</key>' | grep -qF '<integer>8</integer>' ||
  fail "a different osquery.yaml did not change the rendered Minute: the schedule is hardcoded, not data-driven"

# --- the loader: darwin gate, onchange triggers, gui user target, bootout+bootstrap retry ----

grep -qF 'include "Library/LaunchAgents/com.webdavis.osquery-heartbeat.plist.tmpl"' "$LOADER" ||
  fail "loader is missing the plist-hash onchange trigger for the heartbeat plist"
grep -qE 'eq \.chezmoi\.os "darwin"' "$LOADER" ||
  fail "loader is not darwin-gated like the sibling osquery loaders"
# The rendered schedule line is the SECOND (data) onchange trigger: the plist-hash
# include hashes the template SOURCE (a {{ }} literal that never changes on a value
# change), so the loader also renders the schedule, which DOES change with
# osquery.yaml and re-fires run_onchange. Assert the shipped schedule is rendered in.
printf '%s\n' "$rendered_loader" | grep -qF "schedule: $shipped_hour:$shipped_minute" ||
  fail "rendered loader is missing the schedule ($shipped_hour:$shipped_minute) data-onchange trigger"

# gui/<uid> USER agent (the detector needs the user session for the local notifier),
# with the sibling bootout-then-bootstrap shape + retry loop. $HOME and $(id -u)
# must NOT expand in these literal loader lines.
# shellcheck disable=SC2016
printf '%s\n' "$rendered_loader" |
  grep -qF 'PLIST="$HOME/Library/LaunchAgents/com.webdavis.osquery-heartbeat.plist"' ||
  fail "rendered loader does not point at the heartbeat plist"
# shellcheck disable=SC2016
printf '%s\n' "$rendered_loader" |
  grep -qF 'TARGET="gui/$(id -u)/com.webdavis.osquery-heartbeat"' ||
  fail "rendered loader does not target the heartbeat label as a gui/<uid> user agent"
# shellcheck disable=SC2016
printf '%s\n' "$rendered_loader" | grep -qF 'launchctl bootout "$TARGET"' ||
  fail "rendered loader is missing the bootout step"
# shellcheck disable=SC2016
printf '%s\n' "$rendered_loader" | grep -qF 'launchctl bootstrap "gui/$(id -u)" "$PLIST"' ||
  fail "rendered loader is missing the bootstrap step"
printf '%s\n' "$rendered_loader" | grep -qF 'for _ in 1 2 3; do' ||
  fail "rendered loader is missing the bootstrap retry loop"

printf 'osquery-heartbeat-launchagent: OK (label + RunAtLoad false; gui/<uid> user agent; deployed libexec detector; data-driven StartCalendarInterval from osquery.yaml; loader gated, plist-hash + schedule onchange, bootout+bootstrap with retry)\n'
