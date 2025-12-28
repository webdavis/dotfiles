#!/usr/bin/env bash

##
# Description: Toggles an app managed by AeroSpace between focused and scratchpad (hide).
# Credit: https://github.com/nikitabobko/AeroSpace/issues/296#issuecomment-2326103392
#
# Example usage:
#
#   $ ./aerospace_toggle_app_focus.sh fantastical
##

# Exit immediately if any command fails or if any variable is unset.
set -euo pipefail

get_current_workspace() {
  aerospace list-workspaces --focused
}

get_app_window_id() {
  aerospace list-windows --all --format "%{window-id}%{right-padding} | %{app-name}" | grep -m 1 -i "$1" | cut -d' ' -f1 | sed '1p;d'
}

focus_app() {
  local app_name="$1"

  local current_workspace
  current_workspace="$(get_current_workspace)"

  local app_window_id
  app_window_id="$(get_app_window_id "$app_name")"

  aerospace focus --window-id "$app_window_id"
  aerospace move-node-to-workspace "$current_workspace"
  aerospace workspace "$current_workspace"
  aerospace move-mouse window-lazy-center
}

app_closed() {
  local app_name="$1"

  if aerospace list-windows --all --format '%{app-name}' | grep -iq -- "$app_name"; then
    return 1
  else
    return 0
  fi
}

get_focused_app_bundle_id() {
  aerospace list-windows --focused --format "%{app-bundle-id}"
}

get_target_app_bundle_id() {
  local app_name="$1"
  aerospace list-windows --all --format "%{app-bundle-id}" | grep -m 1 -i "$app_name"
}

app_focused() {
  local app_name="$1"

  local focused_bundle_id
  focused_bundle_id="$(get_focused_app_bundle_id)"

  local target_bundle_id
  target_bundle_id="$(get_target_app_bundle_id "$app_name")"

  [[ $focused_bundle_id == "$target_bundle_id" ]]
}

unfocus_app() {
  aerospace move-node-to-workspace scratchpad
}

main() {
  local app_name="$1"

  if app_closed "$app_name"; then
    open -a "$app_name"
    sleep 0.5
  else
    if app_focused "$app_name"; then
      unfocus_app
    else
      focus_app "$app_name"
    fi
  fi
}

main "$@"
