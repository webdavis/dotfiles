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
Usage: ${script} [-l <launcher>|-m|-d|-p|-h]

This script runs xrandr to set display modes. It has builtin support for any launcher
utility. Adjust the xrandr commands below per the available devices.

Options:

    -x <launcher>  Accepts any launcher utility, (e.g. Rofi), as an argument.

    -m	 Configure the laptop monitor only.
    -M	 Configure the laptop monitor only and the portable monitor.
    -d   Configure the docked monitors only.
    -D   Configure the docked monitors and the laptop monitor.
    -h	 Prints helpful text that explains how to use ${script}.

Examples:

To configure the docked monitors only, run the following command:

    display-mode.bash -d

To select the display mode with Rofi, run the following command

    display-mode.bash -l 'rofi -eh 2 -no-fixed-num-lines -dmenu -i -p Display\ Mode:'

(Hint: bind this so an i3 keyboard shortcut.)"
}

declare -a launcher_options=("Mobile (eDP1)" \
			     "Mobile Plus (eDP1, DP-3)" \
			     "Docked (DP-2, DP-0.1)" \
			     "Docked Mobile (DP-2, DP-0.1, eDP1)")
launcher=""
xrandrcommand="Mobile (eDP-1-1)"
optstring=':x:smMdDh'
while getopts "$optstring" option; do
    case "$option" in
	x ) launcher="$OPTARG" ;;
	m ) xrandrcommand='Mobile (eDP1)' ;;
	M ) xrandrcommand='Mobile Plus (eDP1, DP-3)' ;;
	d ) xrandrcommand='Docked (DP-2, DP-0.1)' ;;
	D ) xrandrcommand='Docked Mobile (DP-2, DP-0.1, eDP1)' ;;
	h ) help_message; exit 0 ;;
	* ) help_message; exit 1 ;;
    esac
done
unset -v option

# When docking, setxkbmap reverts to the default layout.
# (E.g. "docked" or "docked plus mobile".)
keyboard_map() {
    [[ -x "$(builtin command -v setxkbmap)" ]] && setxkbmap -option 'ctrl:nocaps' >/dev/null
    [[ -x "$(builtin command -v xcape)" ]] && nohup xcape -t 200 -e 'Control_L=Escape' </dev/null >/dev/null 2>&1 &
}

# Mobile
##########################################################################################
#
#   Workspaces: All
#   +------------------+
#   | Primary eDP1  |
#   | 3840x2160 60Hz   |
#   | ThinkPad P51 15" |
#   +------------------+
#
#
# Mobile Plus
##########################################################################################
#
#   Workspaces: n=2k+1   Workspaces: n=2k
#   +------------------+ +------------------+
#   | Primary eDP1  | | DP-3             |
#   | 3840x2160 60Hz   | | 1920x1080 60Hz   |
#   | ThinkPad P51 15" | | GeChic 15"       |
#   +------------------+ +------------------+
#
#
# Docked
##########################################################################################
#
#   Workspaces: n=2k
#   +--------------------------------+
#   |                                |
#   |      DP-0.1                    |
#   |      3840x2160 60Hz            |
#   |      ASUS PA32Q 32" 4K/UHD     |
#   |                                |
#   +--------------------------------+
#   Workspaces: n=2k+1
#   +--------------------------------------+
#   |                                      |
#   |        Primary DP-2                  |
#   |        3800x1600 60Hz                |
#   |        LG 38UC99-W 38" Curved        |
#   |                                      |
#   +--------------------------------------+
#
#
# Docked Mobile
##########################################################################################
#
#                        Workspaces: n=2k
#                        +--------------------------------+
#                        |                                |
#   Workspaces: None     |      DP-0.1                    |
#   +------------------+ |      3840x2160 60Hz            |
#   | eDP1          | |      ASUS PA32Q 32" 4K/UHD     |
#   | ThinkPad P51 15" | |                                |
#   +------------------+ +----- key=${OP}-----------------+
#                        Workspaces: n=2k+1
#                        +--------------------------------------+
#                        |                                      |
#                        |        Primary DP-2                  |
#                        |        3800x1600 60Hz                |
#                        |        LG 38UC99-W 38" Curved        |
#                        |                                      |
#                        +--------------------------------------+

arrange_workspaces() {
    local workspace
    local output1="$1"
    local output2="$2"
    while read -r workspace; do
        if [[ $(("$workspace" < 5)) -eq 0 ]]; then
            i3-msg "workspace ${workspace}, move workspace to output "$output2";" >/dev/null 2>&1
        else
            i3-msg "workspace ${workspace}, move workspace to output "$output1";" >/dev/null 2>&1
        fi
    done < <(i3-msg -t get_workspaces | grep -Eo "num\":[0-9]+" | grep -Eo "[0-9]+")
}

# Support any launcher, (e.g. Rofi), but not required.
if [[ -n "$launcher" ]]; then
    xrandrcommand="$(printf "%s\\n" "${launcher_options[@]}" | eval "$launcher")"
fi

# ThinkPad P51 15" (Mobile)
# ThinkPad P51 15" + GeChic 15" (Mobile Plus)
# ASUS PA32Q 32" + LG 38&C99-W 38" (Docked)
# ASUS PA32Q 32" + LG 38&C99-W 38" + ThinkPad P51 15" (Docked Mobile)
# The sleep command is necessary to give the monitors time to connect when docking.
if [[ $xrandrcommand == 'Mobile (eDP1)' ]]; then
    xrandr --output 'eDP1' --auto --mode 2560x1440 --rotate normal \
	--output 'DP-0.1' --off \
	--output 'DP-5' --off \
	--output 'DP-4' --off \
	--output 'DP-3' --off \
	--output 'DP-2' --off \
	--output 'DP-1' --off \
	--output 'DP-1' --off
elif [[ $xrandrcommand == 'Mobile Plus (eDP1, DP-3)' ]]; then
    xrandr --output 'eDP1' --auto --primary --mode 2560x1440 --rotate normal \
	--output 'DP-0.1' --off \
	--output 'DP-5' --off \
	--output 'DP-4' --off \
	--output 'DP-3' --auto --mode 1920x1080 --rate '60.00' --right-of 'eDP1' --rotate normal \
	--output 'DP-2' --off \
	--output 'DP-1' --off \
	--output 'DP-0' --off
    keyboard_map
    arrange_workspaces 'eDP1' 'DP-3'
elif [[ $xrandrcommand == 'Docked (DP-2, DP-0.1)' ]]; then
    xrandr --output 'eDP1' --off \
	--output 'DP-0.1' --auto --mode 3840x2160 --rate '60.00' --rotate normal \
	--output 'DP-5' --off \
	--output 'DP-4' --off \
	--output 'DP-3' --off \
	--output 'DP-2' --auto --primary --mode 3840x1600 --rate '75.00' --below 'DP-0.1' --rotate normal \
	--output 'DP-1' --off \
	--output 'DP-0' --off
    sleep 2
    keyboard_map
    arrange_workspaces 'DP-2' 'DP-0.1'
elif [[ $xrandrcommand == 'Docked Mobile (DP-2, DP-0.1, eDP1)' ]]; then
    xrandr --output 'eDP1' --auto --mode 2560x1440 --left-of 'DP-0.1' --rotate normal \
	--output 'DP-0.1' --auto --mode 3840x2160 --above 'DP-2' --rate '60.00' --rotate normal \
	--output 'DP-5' --off \
	--output 'DP-4' --off \
	--output 'DP-3' --off \
	--output 'DP-2' --auto --primary --mode 3840x1600 --rate '75.00' --below 'DP-0.1' --rotate normal \
	--output 'DP-1' --off \
	--output 'DP-0' --off
    sleep 2
    keyboard_map
    arrange_workspaces 'DP-2' 'DP-0.1'
fi
