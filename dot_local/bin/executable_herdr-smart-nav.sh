#!/usr/bin/env bash
# Smart Ctrl-h/j/k/l: seamless navigation across Neovim splits and herdr panes.
#
# herdr has no conditional keybindings, so this script IS the condition. Bound to
# ctrl+h/j/k/l, it inspects the focused pane's foreground process:
#   - Neovim     -> forward the keystroke into the pane, so smart-splits.nvim
#                   moves between nvim splits and, at a split edge, calls
#                   `herdr pane focus` to cross into the adjacent herdr pane.
#   - anything else -> move herdr pane focus directly.
# One set of keys navigates nvim splits and herdr panes whether you are in Neovim
# or not. (Replaces the dead devxplay/herdr.nvim herdr-navigator.)
#
# `herdr pane send-keys` injects at the pane PTY, below herdr's keybinding layer,
# so forwarding ctrl+<letter> does NOT re-trigger this binding (no recursion) --
# the same pattern the prefix+shift+l clear binding already relies on.
#
# Usage: herdr-smart-nav.sh left|down|up|right
set -euo pipefail

direction=${1:-}
herdr=${HERDR_BIN_PATH:-herdr}
pane=${HERDR_ACTIVE_PANE_ID:-}

case "$direction" in
  left) chord="ctrl+h" ;;
  down) chord="ctrl+j" ;;
  up) chord="ctrl+k" ;;
  right) chord="ctrl+l" ;;
  *)
    echo "herdr-smart-nav: usage: $(basename "$0") left|down|up|right" >&2
    exit 2
    ;;
esac

# Branch on the focused pane, targeting $HERDR_ACTIVE_PANE_ID explicitly (the pane
# herdr injects into the keybinding's environment -- the same var the prefix+shift+l
# clear binding relies on) rather than --current, whose resolution in a keybinding
# shell is less certain.
if [[ -n $pane ]]; then
  if "$herdr" pane process-info --pane "$pane" 2>/dev/null |
    jq -e '[.result.process_info.foreground_processes[].name] | any(. == "nvim")' >/dev/null 2>&1; then
    # Focused pane runs Neovim: forward the chord so smart-splits.nvim handles it.
    "$herdr" pane send-keys "$pane" "$chord"
  else
    # Otherwise move herdr pane focus relative to the focused pane.
    "$herdr" pane focus --direction "$direction" --pane "$pane"
  fi
else
  # No active-pane hint (e.g. run manually outside a keybinding): best effort.
  "$herdr" pane focus --direction "$direction" --current
fi
