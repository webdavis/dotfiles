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

# -execute takes a SHELL STRING, so an unsafe pane id would run on click. The
# ENGINE sanitizes it (pns_pane_is_safe, pinned in pns-event.bats) and drops an
# unsafe one from the event before dispatch, so a pane that arrives here has
# already been vetted. Re-checking it in every channel is how the guard drifts:
# one copy gets tightened, the rest rot, and a channel in another language could
# not share it anyway.
# TWO steps on the click, and only on the click: one herdr server renders many
# workspaces in one Ghostty window, and `agent focus` alone moves focus INSIDE
# the pane's workspace while the screen keeps showing whichever workspace the
# operator was in (measured 2026-08-06: a cross-workspace click went nowhere).
# The workspace id is the pane id's prefix. Focus must never move at notify
# time; deciding to jump is what the click IS.
exec_cmd=":"
[[ -n $pane ]] && exec_cmd="herdr workspace focus ${pane%%:*}; herdr agent focus $pane"

terminal-notifier -title "$title" -message "$preview" -sound default \
  -activate com.mitchellh.ghostty -execute "$exec_cmd" >/dev/null 2>&1 || true
exit 0
