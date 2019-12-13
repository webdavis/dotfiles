#!/usr/bin/env bash

# This script opens the instance of whatever program is passed to it.
# (Note! This script is hacky and probably doesn't work for many edge cases.)

# Exit immediately if a "simple" command, a "compound" command, a list, or the last
# command in a pipeline exits with a non-zero exit status.
set -e

# Treat unset variables as errors, exiting when detected.
set -u

# Fail if any command in a pipeline chain returns with a non-zero exit status.
set -o pipefail

# The title and window_role scopes are included as an option because some programs can run
# multiple instances at once. Options will be added as the need arises.
program=""
instance=""
window_role=""
title=""
optstring=':p:i:w:t:'
while getopts "$optstring" option; do
    case "$option" in
	p ) program="$OPTARG" ;;
	i ) instance="$OPTARG" ;;
        w ) window_role="$OPTARG" ;;
	t ) title="$OPTARG" ;;
	* ) exit 1 ;;
    esac
done

# status captures the error message if there is one.
tree=false
if [[ -n "$instance" && -n "$window_role" && -n "$title" ]]; then
    if /usr/bin/i3-msg -t get_tree | grep --quiet --no-messages "\"instance\":\"${instance}\",\"window_role\":\"${window_role}\",\"title\":\"${title}"; then
        tree=true
        status="$(/usr/bin/i3-msg "[instance=\"${instance}*\" window_role=\"${window_role}\" title=\"^${title}*\"] scratchpad show; sticky enable" 2>&1 >/dev/null)"
    fi
elif [[ -n "$instance" && -n "$title" ]]; then
    if /usr/bin/i3-msg -t get_tree | grep --quiet --no-messages "\"instance\":\"${instance}\",\"title\":\"${title}"; then
        tree=true
        status="$(/usr/bin/i3-msg "[instance=\"${instance}*\" title=\"^${title}*\"] scratchpad show; sticky enable" 2>&1 >/dev/null)"
    fi
elif [[ -n "$window_role" && -n "$title" ]]; then
    if /usr/bin/i3-msg -t get_tree | grep --quiet --no-messages "\"window_role\":\"${window_role}\",\"title\":\"${title}"; then
        tree=true
        status="$(/usr/bin/i3-msg "[window_role=\"${window_role}*\" title=\"^${title}*\"] scratchpad show; sticky enable" 2>&1 >/dev/null)"
    fi
elif [[ -n "$instance" ]]; then
    if /usr/bin/i3-msg -t get_tree | grep --quiet --no-messages "\"instance\":\"${instance}\""; then
        tree=true
        status="$(/usr/bin/i3-msg "[instance=\"${instance}*\"] scratchpad show; sticky enable" 2>&1 >/dev/null)"
    fi
fi

if [[ $tree == 'true' && ! $status =~ "ERROR" ]]; then
    exit 0
fi

# Launch the program.
eval exec "$program" &

# It takes a second for the Json entry to populate in the i3 tree.
sleep 1.0

# Make the program a scratchpad.
/usr/bin/i3-msg "[id="$(/usr/bin/xdotool getactivewindow)"] move scratchpad, resize set 2460 1340, move absolute position center"
