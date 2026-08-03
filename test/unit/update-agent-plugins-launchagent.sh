#!/usr/bin/env bash
#
# update-agent-plugins-launchagent.sh, the com.webdavis.update-agent-plugins
# LaunchAgent must actually be able to run the weekly updater, and must be its
# OWN agent rather than a second command bolted onto the skills one.
#
# WHY EACH ASSERTION IS HERE:
#
#   24 hourly Monday slots  the updater's idle gate defers a slot on which a
#                           Claude Code session was recently active, so one slot
#                           a week would mean the vertical never runs on a
#                           machine that is busy at that hour.
#   RunAtLoad false         loading the agent (which every apply does) must never
#                           trigger an unattended third-party code update.
#   --scheduled, exactly    only a scheduled run posts the weekly record. A
#   once                    typo'd or missing marker runs every Monday and posts
#                           nothing, which reads exactly like a dead agent, and
#                           a doubled one is an argument the updater refuses.
#   the DEPLOYED helper     the agent must run ~/.local/bin, not a path inside a
#                           checkout that may not exist on a fresh machine.
#   PATH covers claude+jq   launchd hands a job a minimal PATH. The updater calls
#                           `claude` and `jq` by bare name, and both live under
#                           Homebrew here; relay.sh is called by absolute path
#                           but ~/.local/bin is on the PATH for the same reason
#                           the skills agent puts it there.
#   its OWN label and log   plugins and skills are separate verticals by operator
#                           ruling, so they may not share an agent: one plist
#                           means one vertical's deferral logic governs the
#                           other's.
#   a loader exists         a plist that ships and never loads leaves the whole
#                           vertical inert, which is the gap the first version of
#                           this design shipped with.
#
# Unit test: render the plist with the host chezmoi and read it as real plist
# data. No launchctl, no side effects.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PLIST="$REPO_ROOT/Library/LaunchAgents/com.webdavis.update-agent-plugins.plist.tmpl"
LOADER="$REPO_ROOT/.chezmoiscripts/run_onchange_after_69-load-update-agent-plugins-launchagent.sh.tmpl"
SKILLS_PLIST="$REPO_ROOT/Library/LaunchAgents/com.webdavis.update-skills.plist.tmpl"

readonly EXPECTED_LABEL='com.webdavis.update-agent-plugins'
readonly EXPECTED_HELPER_SUFFIX='/.local/bin/update-agent-plugins.sh'
readonly EXPECTED_SLOT_COUNT=24
readonly -a REQUIRED_PATH_DIRS=('/opt/homebrew/bin')

fail() {
  printf 'update-agent-plugins-launchagent: FAIL -- %s\n' "$*" >&2
  exit 1
}

command -v chezmoi >/dev/null 2>&1 ||
  fail "chezmoi is not on PATH, so the plist cannot be rendered. Run inside the flake run shell: nix develop .#run"
command -v plutil >/dev/null 2>&1 || {
  printf 'SKIP: plutil is not available (non-darwin host); the plist cannot be parsed as plist data\n'
  exit 0
}
[[ -f $PLIST ]] || fail "missing plist template: $PLIST"
[[ -f $LOADER ]] || fail "missing loader: $LOADER; a plist that never loads leaves the vertical inert"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# --source pins the render to THIS checkout: a bare chezmoi reads the machine's
# configured source directory, which is a different checkout on a different
# branch.
rendered="$work/plist.xml"
CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$PLIST" >"$rendered" ||
  fail "chezmoi failed to render the plist"
[[ -s $rendered ]] || fail "empty plist render"

# Parsed as real plist data rather than grepped: a marker lost inside a
# malformed array passes a text search and fails launchd.
plist_json="$work/plist.json"
plutil -convert json -o "$plist_json" - <"$rendered" 2>/dev/null ||
  fail "the rendered plist did not parse as a plist"

label="$(jq -r '.Label' "$plist_json")"
[[ $label == "$EXPECTED_LABEL" ]] || fail "the agent's label is '$label', expected '$EXPECTED_LABEL'"

# Its OWN agent. Sharing the skills label would recouple the two verticals at
# the schedule level, which the operator ruling forbids.
if [[ -f $SKILLS_PLIST ]]; then
  skills_label="$(CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$SKILLS_PLIST" |
    plutil -convert json -o - - 2>/dev/null | jq -r '.Label')"
  [[ $label != "$skills_label" ]] ||
    fail "the plugin agent shares the skills agent's label ($label); the two verticals must not share a schedule"
fi

run_at_load="$(jq -r '.RunAtLoad' "$plist_json")"
[[ $run_at_load == "false" ]] ||
  fail "RunAtLoad is '$run_at_load'; loading the agent would trigger an unattended plugin update"

slot_count="$(jq -r '.StartCalendarInterval | length' "$plist_json")"
[[ $slot_count == "$EXPECTED_SLOT_COUNT" ]] ||
  fail "the agent declares $slot_count calendar slots, expected $EXPECTED_SLOT_COUNT; a deferring slot needs later ones to retry into"

non_monday="$(jq -r '[.StartCalendarInterval[] | select(.Weekday != 1)] | length' "$plist_json")"
[[ $non_monday == "0" ]] || fail "$non_monday calendar slot(s) are not on Monday (launchd Weekday 1)"
distinct_hours="$(jq -r '[.StartCalendarInterval[].Hour] | unique | length' "$plist_json")"
[[ $distinct_hours == "$EXPECTED_SLOT_COUNT" ]] ||
  fail "the $EXPECTED_SLOT_COUNT slots cover only $distinct_hours distinct hours, so some of them fire at the same moment"
off_minute="$(jq -r '[.StartCalendarInterval[] | select(.Minute != 0)] | length' "$plist_json")"
[[ $off_minute == "0" ]] || fail "$off_minute calendar slot(s) do not fire on the hour"

scheduled_markers="$(jq -r '[.ProgramArguments[] | select(. == "--scheduled")] | length' "$plist_json")"
[[ $scheduled_markers == "1" ]] ||
  fail "ProgramArguments passes $scheduled_markers --scheduled markers, expected exactly 1; without one no weekly record is ever posted, and a second one is an argument the updater refuses"

helper_arg="$(jq -r '.ProgramArguments[1]' "$plist_json")"
[[ $helper_arg == *"$EXPECTED_HELPER_SUFFIX" ]] ||
  fail "the agent does not run the deployed helper (got '$helper_arg', expected a path ending in $EXPECTED_HELPER_SUFFIX)"

path_value="$(jq -r '.EnvironmentVariables.PATH // ""' "$plist_json")"
[[ -n $path_value ]] || fail "the agent declares no PATH; launchd's default would not resolve claude or jq"
for required_dir in "${REQUIRED_PATH_DIRS[@]}"; do
  case ":$path_value:" in
    *":$required_dir:"*) ;;
    *) fail "the agent's PATH does not carry $required_dir, where the updater's bare-name tools live: $path_value" ;;
  esac
done

log_path="$(jq -r '.StandardOutPath // ""' "$plist_json")"
[[ -n $log_path ]] || fail "the agent writes its output nowhere, so a failing run leaves no local trace at all"
[[ $log_path == *"/.local/log/"* ]] ||
  fail "the agent's log is at '$log_path', outside ~/.local/log where the log rotation reaches"

# The loader must load THIS agent, and must re-hash the plist so a changed plist
# actually re-bootstraps. A run_onchange script whose hash line does not name
# the plist never re-fires when the plist changes.
grep -qF -- "$EXPECTED_LABEL" "$LOADER" ||
  fail "the loader does not name $EXPECTED_LABEL, so it loads some other agent"
grep -qF -- 'com.webdavis.update-agent-plugins.plist.tmpl' "$LOADER" ||
  fail "the loader does not hash the plist template, so a changed plist would never re-bootstrap"
grep -qF -- 'launchctl bootstrap' "$LOADER" || fail "the loader never bootstraps the agent"

printf 'update-agent-plugins-launchagent: OK (its own label %s; %s Monday slots on distinct hours; RunAtLoad false; one --scheduled marker; the deployed helper; a PATH carrying %s; a log under ~/.local/log; a loader that hashes and bootstraps it)\n' \
  "$EXPECTED_LABEL" "$EXPECTED_SLOT_COUNT" "${REQUIRED_PATH_DIRS[*]}"
