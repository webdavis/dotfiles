#!/usr/bin/env bash

# Copyright (C) 2014 Julien Bonjean <julien@bonjean.info>
# Copyright (C) 2014 Alexander Keller <github@nycroth.com>

# This program is free software: you can redistribute it and/or modify it under the terms
# of the GNU General Public License as published by the Free Software Foundation, either
# version 3 of the License, or (at your option) any later version.

# This program is distributed in the hope that it will be useful, but WITHOUT ANY
# WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
# PARTICULAR PURPOSE. See the GNU General Public License for more details.

# You should have received a copy of the GNU General Public License along with this
# program. If not, see <http://www.gnu.org/licenses/>.

# Exit immediately if a "simple" command, a "compound" command, a list, or the last
# command in a pipeline exits with a non-zero exit status.
set -e

# Treat unset variables as errors, exiting when detected.
set -u

# Fail if any command in a pipeline chain returns with a non-zero exit status.
set -o pipefail

# The name of this script.
script="${BASH_SOURCE[0]##*/}"

# The second parameter overrides the mixer selection:
#   For PulseAudio users, eventually use "pulse".
#   For Jack/Jack2 users, use "jackplug".
#   For ALSA users, you may use "default" for your primary card or you may use "hw:#"
#   where "#" is the number of the card desired.
help_message() {
    printf "%s\\n\\n" "\
Usage: ${script} [-m <device>] [-s <simple_control>]

This script prints the temperature of a sensor chip. If no options are specified then
the default settings will be used.

Options:

    -m <device>	          Change the default device to control. The default device name is \"default\".
    -s <simple_control>   Change the default simple control to <simple_control>. The default simple control is \"Master\".
    -h			  Prints helpful text that explains how to use this script.

Example:

To change the default device that \`amixer\` controls, run the following command:

	${script} -m hw:2 -s Headphone"
}

mixer="default"
scontrol='Master'
declare -i step='2'
optstring=':m:c:s:'
while getopts "$optstring" option; do
    case "$option" in
	m ) mixer="$OPTARG" ;;
	c ) scontrol="$OPTARG" ;;
	s ) step="$OPTARG" ;;
	h ) exit 0 ;;
	* ) exit 33 ;;
    esac
done
unset -v option

if [[ -z "$mixer" ]]; then
    mixer="default"
    if command -v pulseaudio >/dev/null 2>&1 && pulseaudio --check; then
	# Pulseaudio is running, but not all installations use "pulse"
	if amixer -D pulse info >/dev/null 2>&1; then
	    mixer="pulse"
	fi
    fi
    [ -n "$(lsmod | grep jack)" ] && mixer="jackplug"
    mixer="${2:-${mixer}}"
fi

# The instance option sets the control to report and configure. This defaults to the
# first control of your selected mixer.
# For a list of the available, use `amixer -D $your_mixer scontrols`.
if [[ -z "${scontrol}" ]]; then
    scontrol="${block_instance:-$(amixer -D ${mixer} scontrols |
	sed -n "s/Simple mixer control '\([^']*\)',0/\1/p" |
	head -n1
    )}"
fi

# The first parameter sets the step to change the volume by (and units to display). This
# may be in in "%" or "dB" (eg. "5%" or "3dB").
if [[ -z "$step" ]]; then
    step="${1:-5%}"
fi

# Return "Capture" if the device is a capture device.
capability() {
    amixer -D "$mixer" get "$scontrol" |
	sed -n "s/  Capabilities:.*cvolume.*/Capture/p"
}

volume() {
    amixer -D "$mixer" get "$scontrol" $(capability)
}

format() {
    filter='if (/.*\[(\d+%)\] (\[(-?\d+.\d+dB)\] )?\[(on|off)\]/)'
    filter+='{CORE::say $4 eq "off" ? "MUTE" : "'
    # If "dB" was selected, print that instead.
    filter+="$([[ $step = *dB ]] && echo '$3' || echo '$1')"
    filter+='"; exit}'
    fulltext="><span font='FontAwesome'></span>"
    perl -ne "$filter"
}

# Right click (mute/unmute)
# Scroll Up (increase)
# Scroll Down (decrease)
block_button="${BLOCK_BUTTON:-}"
case "$block_button" in
    3 ) amixer -q -D ${mixer} sset ${scontrol} $(capability) toggle ;;
    4 ) amixer -q -D ${mixer} sset ${scontrol} $(capability) ${step}+ unmute ;;
    5 ) amixer -q -D ${mixer} sset ${scontrol} $(capability) ${step}- unmute ;;
esac

volume | format
