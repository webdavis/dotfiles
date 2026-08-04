#!/usr/bin/env bash
#
# The com.webdavis.report-plugin-updates LaunchAgent must fire weekly on a
# calendar schedule (launchd Weekday 1 == Monday), must NOT run at load time, and
# must pass --scheduled. The marker is the load-bearing part: only a scheduled
# run posts an entry, so an agent that runs every Monday without it produces the
# same silence as an agent that is not loaded at all, which is the exact
# ambiguity the record exists to end.
#
# It must also NOT collide with com.webdavis.homebrew-weekly-upgrade's slot. The
# two post to the same channel, and two entries stamped the same minute read as
# one job that posted twice.
#
# Unit test: render the plist template with the host chezmoi and assert the
# schedule fields. No launchctl, no side effects.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PLIST="$REPO_ROOT/Library/LaunchAgents/com.webdavis.report-plugin-updates.plist.tmpl"
SIBLING_PLIST="$REPO_ROOT/Library/LaunchAgents/com.webdavis.homebrew-weekly-upgrade.plist.tmpl"
LOADER="$REPO_ROOT/.chezmoiscripts/run_onchange_after_69-load-report-plugin-updates-launchagent.sh.tmpl"

fail() {
  printf 'report-plugin-updates-plist: FAIL -- %s\n' "$*" >&2
  exit 1
}

command -v chezmoi >/dev/null 2>&1 || {
  printf 'SKIP: chezmoi not on PATH; cannot render the plist\n'
  exit 0
}
[[ -f $PLIST ]] || fail "missing plist template: $PLIST"
[[ -f $SIBLING_PLIST ]] || fail "missing sibling plist template: $SIBLING_PLIST"
[[ -f $LOADER ]] || fail "missing loader script: $LOADER"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# render <template> <destination> -- the host chezmoi, with HOME pointed at a
# throwaway directory so the render cannot read or write the operator's own.
render() {
  local template="$1" destination="$2" render_home
  render_home="$(mktemp -d)"
  HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
    <"$template" >"$destination" || fail "chezmoi failed to render $template"
  rm -rf "$render_home"
  [[ -s $destination ]] || fail "empty render of $template"
}

rendered="$work/plist.xml"
render "$PLIST" "$rendered"

# Parsed as real plist data (plutil -> json -> jq) rather than grepped, so a
# field lost inside a malformed dict cannot pass a text match.
plist_json="$work/plist.json"
plutil -convert json -o "$plist_json" - <"$rendered" 2>/dev/null ||
  fail "the rendered plist did not parse as a plist"

label="$(jq -r '.Label' "$plist_json")"
[[ $label == "com.webdavis.report-plugin-updates" ]] ||
  fail "the agent carries the wrong Label (got $label)"

[[ "$(jq -r 'has("StartCalendarInterval")' "$plist_json")" == "true" ]] ||
  fail "no StartCalendarInterval (not a calendar-scheduled agent)"
weekday="$(jq -r '.StartCalendarInterval.Weekday' "$plist_json")"
[[ $weekday == "1" ]] || fail "the agent does not fire on Monday (Weekday $weekday)"
[[ "$(jq -r '.StartCalendarInterval.Minute' "$plist_json")" == "0" ]] ||
  fail "the agent does not fire on the hour"

[[ "$(jq -r '.RunAtLoad' "$plist_json")" == "false" ]] ||
  fail "RunAtLoad is not false; loading the agent would post an entry out of schedule"

prog_scheduled="$(jq -r '[.ProgramArguments[] | select(. == "--scheduled")] | length' "$plist_json")"
[[ $prog_scheduled == "1" ]] ||
  fail "ProgramArguments does not pass exactly one --scheduled marker (got $prog_scheduled); the record would never be posted and the agent would look dead"
helper_arg="$(jq -r '.ProgramArguments[1]' "$plist_json")"
[[ $helper_arg == */.local/bin/report-plugin-updates.sh ]] ||
  fail "the agent does not run the deployed helper (got $helper_arg)"

# The log path the loader creates must be the one the agent writes to, or the
# agent fails to start with no output anywhere explaining why.
log_path="$(jq -r '.StandardOutPath' "$plist_json")"
[[ $log_path == */.local/log/plugins/report-updates.log ]] ||
  fail "the agent logs somewhere unexpected (got $log_path)"
[[ "$(jq -r '.StandardErrorPath' "$plist_json")" == "$log_path" ]] ||
  fail "stdout and stderr go to different files, so half the run's output is unfindable"
log_directory=".local${log_path#*/.local}"
log_directory="$(dirname "$log_directory")"
grep -qF -- "$log_directory" "$LOADER" ||
  fail "the loader does not create $log_directory, the directory the agent writes its log into; launchd refuses to start an agent whose log directory is absent"

# The sibling weekly job posts to the same channel, so the two must not share a
# minute. Compared against the rendered sibling rather than a hardcoded hour, so
# moving either job keeps this honest.
sibling_rendered="$work/sibling.xml"
sibling_json="$work/sibling.json"
render "$SIBLING_PLIST" "$sibling_rendered"
plutil -convert json -o "$sibling_json" - <"$sibling_rendered" 2>/dev/null ||
  fail "the rendered sibling plist did not parse as a plist"
this_slot="$(jq -r '"\(.StartCalendarInterval.Weekday):\(.StartCalendarInterval.Hour):\(.StartCalendarInterval.Minute)"' "$plist_json")"
sibling_slot="$(jq -r '"\(.StartCalendarInterval.Weekday):\(.StartCalendarInterval.Hour):\(.StartCalendarInterval.Minute)"' "$sibling_json")"
[[ $this_slot != "$sibling_slot" ]] ||
  fail "this agent shares its slot ($this_slot) with com.webdavis.homebrew-weekly-upgrade; two entries stamped the same minute on one channel read as one job posting twice"

printf 'report-plugin-updates-plist: OK (Weekday:Hour:Minute %s; RunAtLoad false; --scheduled passed once)\n' "$this_slot"
