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
# A missing helper must FIRE the banner, not exit quietly: a spare banner is
# spam, a dropped one is a lost notification. So a failed source is tolerated
# and the undefined function reads below as unknown idle, which fires.
# shellcheck source=dot_local/libexec/pns/helpers/presence.sh
source "${PNS_HELPERS_DIR:-$pns_dir/helpers}/presence.sh" 2>/dev/null || true

title="$(jq -r '.title // ""' <<<"$event" 2>/dev/null || true)"
preview="$(jq -r '.preview // ""' <<<"$event" 2>/dev/null || true)"
pane="$(jq -r '.pane // ""' <<<"$event" 2>/dev/null || true)"

# Which terminal houses the pane is self-detected: macOS gives every process
# an inherited __CFBundleIdentifier naming the app it was launched from, and
# it rides the whole chain down to this channel. Override wins; empty means
# unknown, and an unknown terminal can never satisfy the suppression check.
terminal_id="${PNS_TERMINAL_BUNDLE_ID:-${__CFBundleIdentifier:-}}"

# Bundle id of the frontmost app, or empty when unreadable. lsappinfo ships
# with macOS and needs no Accessibility grant, unlike osascript.
front_bundle_id() {
  local front
  command -v lsappinfo >/dev/null 2>&1 || return 0
  front="$(lsappinfo front 2>/dev/null || true)"
  [[ -n $front ]] || return 0
  lsappinfo info -only bundleid "$front" 2>/dev/null |
    sed -n '/CFBundleIdentifier/{s/.*"CFBundleIdentifier"="\([^"]*\)".*/\1/p;q;}'
}

# operator_is_watching <pane>
# True only when all three hold at once: the Mac was touched recently, the
# pane's terminal is the frontmost app, and the pane is herdr's focused pane.
# Anything false, unreadable or unknown fires the banner. All three are needed
# because each alone lies: a focused pane can sit in a buried window, and a
# front terminal can be showing a pane nobody is reading.
# Checks are ordered cheapest first.
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

operator_is_watching "$pane" && exit 0

# The click does two things, in order: focus the pane's WORKSPACE (the pane id
# prefix), then the pane. `agent focus` alone moves focus inside a workspace
# the screen may not be showing, so a cross-workspace click would go nowhere.
# Focus only ever moves on the click, never at notify time.
#
# -execute runs a SHELL STRING on click, so the pane id must be safe to
# interpolate. relay.sh vets it before dispatch (pns_pane_is_safe) and drops
# unsafe ones, so what arrives here is already clean.
#
# herdr's path is resolved HERE and baked into the string: the click runs in a
# bare launchd context whose PATH cannot find ~/.local/bin, so a bare `herdr`
# dies silently. No herdr at all leaves the click to -activate.
exec_cmd=":"
herdr_bin="$(command -v herdr 2>/dev/null || true)"
if [[ -n $pane && -n $herdr_bin ]]; then
  exec_cmd="$herdr_bin workspace focus ${pane%%:*}; $herdr_bin agent focus $pane"
fi

terminal-notifier -title "$title" -message "$preview" -sound default \
  -activate "${terminal_id:-com.mitchellh.ghostty}" -execute "$exec_cmd" >/dev/null 2>&1 || true
exit 0
