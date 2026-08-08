#!/usr/bin/env bash
# pns channel: the clickable macOS banner. See channels/moshi.sh for the
# contract every channel in this directory implements.
#
# Clicking focuses the exact herdr pane the event came from, which is the whole
# reason this channel beats a plain notification.
set -uo pipefail

event="$(cat)"
command -v terminal-notifier >/dev/null 2>&1 || exit 0

pns_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# A MISSING HELPER MUST FIRE THE BANNER, never exit quietly. Without the idle
# probe this channel cannot tell where the operator is, and unknown presence
# fires: a spare banner is spam, a missing one is a dropped notification. So
# the source is tolerated rather than fatal, and pns_idle_secs simply stays
# undefined, which reads below as an unknown idle and fails open on its own.
# shellcheck source=dot_local/libexec/pns/helpers/presence.sh
source "${PNS_HELPERS_DIR:-$pns_dir/helpers}/presence.sh" 2>/dev/null || true

title="$(jq -r '.title // ""' <<<"$event" 2>/dev/null || true)"
preview="$(jq -r '.preview // ""' <<<"$event" 2>/dev/null || true)"
pane="$(jq -r '.pane // ""' <<<"$event" 2>/dev/null || true)"

# WHICH terminal houses this pane is SELF-DETECTED, not hardcoded: every
# process a Mac app launches inherits __CFBundleIdentifier naming that app, and
# it rides the whole chain (terminal to herdr to pane shell to agent to hook to
# this channel), so the code below works under any terminal unchanged. An
# explicit override wins over the inherited value. Empty means the terminal is
# UNKNOWN, and a suppression condition that cannot be evaluated cannot be met.
terminal_id="${PNS_TERMINAL_BUNDLE_ID:-${__CFBundleIdentifier:-}}"

# front_bundle_id: the bundle id of the frontmost app, or EMPTY when that
# cannot be read. lsappinfo is Launch Services' own CLI: it ships with macOS,
# needs no Accessibility grant (osascript would raise a TCC prompt, and a hook
# context can be denied one silently) and couples this decision to no window
# manager. Bundle id, never display name.
front_bundle_id() {
  local front
  command -v lsappinfo >/dev/null 2>&1 || return 0
  front="$(lsappinfo front 2>/dev/null || true)"
  [[ -n $front ]] || return 0
  lsappinfo info -only bundleid "$front" 2>/dev/null |
    sed -n '/CFBundleIdentifier/{s/.*"CFBundleIdentifier"="\([^"]*\)".*/\1/p;q;}'
}

# operator_is_watching <pane>
# 0 only when ALL THREE presence conditions hold at once. Any one of them false,
# unreadable or unknown returns non-zero and the banner fires.
#
# The Stop hook fires on every turn end, so an unconditional banner narrates
# the conversation the operator is having: one spam banner per agent reply.
# Focus alone proves nothing, though, which is the ruling this encodes. Ghostty
# buried under a browser means the pane is focused inside a window nobody can
# see, and a pane left focused while the operator walked away means the same
# thing for a different reason. Only all three together say "they are looking
# at this right now"; the away case is the phone leg's job.
#
# Ordered cheapest first, bailing on the first failure: an env comparison and
# an arithmetic test before an 11ms Launch Services call before a round trip to
# the herdr server.
operator_is_watching() {
  local pane="${1:-}" desk="${RELAY_DESK_IDLE_SECS:-120}" idle focused
  [[ -n $pane ]] || return 1
  # 1. the operator touched this Mac recently.
  idle="$(pns_idle_secs 2>/dev/null || true)"
  [[ $idle =~ ^[0-9]+$ && $desk =~ ^[0-9]+$ ]] || return 1
  ((idle < desk)) || return 1
  # 2. the terminal that houses the pane is the key window.
  [[ -n $terminal_id ]] || return 1
  [[ "$(front_bundle_id)" == "$terminal_id" ]] || return 1
  # 3. the event's pane is the pane herdr has focused.
  command -v herdr >/dev/null 2>&1 || return 1
  focused="$(herdr pane list 2>/dev/null |
    jq -r '.result.panes[]? | select(.focused == true) | .pane_id' 2>/dev/null | head -1 || true)"
  [[ -n $focused && $focused == "$pane" ]]
}

# -execute takes a SHELL STRING, so an unsafe pane id would run on click. The
# ENGINE sanitizes it (pns_pane_is_safe, pinned in pns-event.bats) and drops an
# unsafe one from the event before dispatch, so a pane that arrives here has
# already been vetted. Re-checking it in every channel is how the guard drifts:
# one copy gets tightened, the rest rot, and a channel in another language could
# not share it anyway.
operator_is_watching "$pane" && exit 0

# TWO steps on the click, and only on the click: one herdr server renders many
# workspaces in one Ghostty window, and `agent focus` alone moves focus INSIDE
# the pane's workspace while the screen keeps showing whichever workspace the
# operator was in (measured 2026-08-06: a cross-workspace click went nowhere).
# The workspace id is the pane id's prefix. Focus must never move at notify
# time; deciding to jump is what the click IS.
#
# THE ABSOLUTE PATH IS THE FIX FOR A DEAD CLICK. A clicked banner runs its
# -execute string in a bare launchd context whose PATH is the /etc/paths
# default, and herdr lives in ~/.local/bin, so a bare name resolved to nothing
# and every click died there in silence (proven 2026-08-07; what looked like
# "jumps to the last-touched pane" was -activate working alone). This channel
# runs with the full environment, so it resolves the path once here and bakes
# it in. No herdr on PATH leaves the click to -activate, exactly as before.
exec_cmd=":"
herdr_bin="$(command -v herdr 2>/dev/null || true)"
if [[ -n $pane && -n $herdr_bin ]]; then
  exec_cmd="$herdr_bin workspace focus ${pane%%:*}; $herdr_bin agent focus $pane"
fi

terminal-notifier -title "$title" -message "$preview" -sound default \
  -activate "${terminal_id:-com.mitchellh.ghostty}" -execute "$exec_cmd" >/dev/null 2>&1 || true
exit 0
