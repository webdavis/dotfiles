#!/usr/bin/env bash

set -e # Exit immediately when there is an error.
set -u # Treat unset variables as errors, exiting when detected.
set -o pipefail # Fail if any command in a pipeline chain returns an error.

alert()
{
    notify-send --urgency=low --expire-time=2000 --icon="$1" 'Touchpad' "$2"
}

[[ -x "$(builtin command -v xinput)" ]] || {
    alert 'dialog-error' 'xinput: command not found. Is xorg-xinput installed?';
    exit 1;
}

touchpad_grep_template()
{
    local device='SynPS/2 Synaptics TouchPad'
    xinput list-props "$device" >/dev/null 2>&1 || {
        alert 'dialog-error' 'Device Disabled';
        exit 1;
    }
    xinput list-props "$device" | grep -Eo "Device\sEnabled.*[${1}]$"
}

device_property="$(touchpad_grep_template '01' | grep -o '(.*)' | tr -d '(|)|\n')"

set_touchpad()
{
    xinput set-prop 'SynPS/2 Synaptics TouchPad' "$device_property" $1
}

is_touchpad_enabled()
{
    if touchpad_grep_template '1'; then
        return 0
    else
        return 1
    fi
}

if is_touchpad_enabled; then
    set_touchpad 0
    alert 'input-mouse' 'Off'
else
    set_touchpad 1
    alert 'input-mouse' 'On'
fi
