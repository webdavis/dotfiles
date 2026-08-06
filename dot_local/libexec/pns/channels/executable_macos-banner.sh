#!/usr/bin/env bash
# pns channel: the clickable macOS banner. See channels/moshi.sh for the
# contract every channel in this directory implements.
#
# Clicking focuses the exact herdr pane the event came from, which is the whole
# reason this channel beats a plain notification.
set -uo pipefail

event="$(cat)"
command -v terminal-notifier >/dev/null 2>&1 || exit 0

title="$(jq -r '.title // ""' <<<"$event" 2>/dev/null || true)"
preview="$(jq -r '.preview // ""' <<<"$event" 2>/dev/null || true)"
pane="$(jq -r '.pane // ""' <<<"$event" 2>/dev/null || true)"

# -execute takes a SHELL STRING, so the pane id is validated rather than quoted:
# a pane carrying `; curl ... | sh` would otherwise run when the operator clicks
# the banner. relay-agent fills the pane from $HERDR_PANE_ID, which is
# environment pns does not own. herdr pane ids are word characters; anything
# else drops the click action rather than the banner.
exec_cmd=":"
if [[ -n $pane ]]; then
  if [[ $pane =~ ^[A-Za-z0-9._-]+$ ]]; then
    exec_cmd="herdr agent focus $pane"
  else
    printf 'relay: refusing a pane id with shell metacharacters; the banner will not focus a pane\n' >&2
  fi
fi

terminal-notifier -title "$title" -message "$preview" -sound default \
  -activate com.mitchellh.ghostty -execute "$exec_cmd" >/dev/null 2>&1 || true
exit 0
