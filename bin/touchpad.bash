#!/usr/bin/env bash

# Exit immediately if a "simple" command, a "compound" command, a list, or the last
# command in a pipeline exits with a non-zero exit status.
set -e

# Treat unset variables as errors, exiting when detected.
set -u

# Fail if any command in a pipeline chain returns with a non-zero exit status.
set -o pipefail

[[ -x "$(builtin command -v xinput)" ]] ||
    notify-send --urgency=low --expire-time 3000 --icon=dialog-error \
	'Synaptics TouchPad' 'xinput: command not found. Is xorg-xinput installed?'

# The device property (172), may be different. Run `xinput list-props 'SynPS/2 Synaptics
# TouchPad'` to see what device property the TouchPad has and adjust this script
# accordingly.
if xinput list-props 'SynPS/2 Synaptics TouchPad' | grep -Eo "Device\sEnabled.*[1]$" >/dev/null 2>&1; then
    xinput set-prop 'SynPS/2 Synaptics TouchPad' 136 0
    notify-send --urgency=low --expire-time=2000 --icon=input-mouse 'TouchPad' 'Off'
else
    xinput set-prop 'SynPS/2 Synaptics TouchPad' 136 1
    notify-send --urgency=low --expire-time=2000 --icon=input-mouse 'TouchPad' 'On'
fi
