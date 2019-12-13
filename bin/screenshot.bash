#!/usr/bin/env bash

# Exit immediately if a "simple" command, a "compound" command, a list, or the last
# command in a pipeline exits with a non-zero exit status.
set -e

# Treat unset variables as errors, exiting when detected.
set -u

# Fail if any command in a pipeline chain returns with a non-zero exit status.
set -o pipefail

# Case insensitive matching.
shopt -s nocasematch

# The name of this script.
script="${BASH_SOURCE[0]##*/}"

help_message() {
    printf "%s\\n\\n" "\
Usage: ${script} [-l <launcher>|-f|-w|-W|-a|-h]

This script runs systemctl power management commands. It has builtin support for any
launcher utility.

Options:

    -l <launcher>  Accepts any launcher utility, (e.g. Rofi), as an argument.

    -f   Capture a screenshot of the entire screen.
    -w   Capture a screenshot of a window with its borders removed (e.g. remove the titlebar).
    -W   Capture a screenshot of a window with its borders included (e.g. include the titlebar).
    -a   Open gnome-screenshot to select an area to capture. The screenshot will be captured when the mouse click is released.
    -h   Prints helpful text that explains how to use ${script}.

Example:

To capture a screenshot of the entire screen, run the following command:

    screenshot.bash -f

To select the type of screenshot to take with Rofi, run the following command:

    screenshot.bash -l 'rofi -eh 2 -no-fixed-num-lines -dmenu -i -p Screenshot:'

(Hint: bind this so an i3 keyboard shortcut.)"
}

declare -a launcher_options=("Capture Fullscreen" "Capture Window (remove border)" "Capture Window (include border)" "Capture Area")
launcher=""
optstring=':x:fwWah'
capturecommand=""
while getopts "$optstring" option; do
    case "$option" in
        x ) launcher="$OPTARG" ;;
        f ) capturecommand='fullscreen' ;;
        w ) capturecommand='window (remove border)' ;;
        W ) capturecommand='window (include border)' ;;
        a ) capturecommand='area' ;;
        h ) help_message; exit 0 ;;
        * ) help_message; exit 1 ;;
    esac
done
unset -v option

# Support any launcher, (e.g. Rofi), but not required.
if [[ -n "${launcher}" ]]
then
    capturecommand="$(printf "%s\\n" "${launcher_options[@]}" | eval "$launcher" | cut --fields=1 --delimiter=' ' --complement)"
fi

if [[ ! -d "${HOME}/Pictures/screenshots" ]]; then
    mkdir --parents -- "${HOME}/Pictures/screenshots"
fi

case "$capturecommand" in
    "fullscreen" ) eval "gnome-screenshot --file=Pictures/screenshots/screenshot-fullscreen-$(date +%FT%T).png" ;;
    "window (remove border)" ) eval "gnome-screenshot --window --remove-border --file=Pictures/screenshots/screenshot-window-rmborder-$(date +%FT%T).png" ;;
    "window (include border)" ) eval "gnome-screenshot --window --include-border --file=Pictures/screenshots/screenshot-window-border-$(date +%FT%T).png" ;;
    "area" ) eval "gnome-screenshot --area --file=Pictures/screenshots/screenshot-area-$(date +%FT%T).png" ;;
    * ) ;;
esac
