#!/usr/bin/env bash

# This script lets the user map audio sink symbolic names to data audio streams. An audio
# sink is any device that is accepting traffic flow (in this case sound bytes) and
# terminating them. Running `pactl list short sinks` list all of the available audio
# sinks availabe to the audio source on the computer. The audio source may or may not be
# streaming to an audio sink. Try playing a YouTube video and open `pavucontrol`.
# `pavucontrol` should indicate that an audio stream is open on the audio source. Now,
# open another YouTube video and play is alongside the already playing video.
# `pavucontrol` should display two streams. If you change the playback device then
# essentially you are changing the audio sink that the audio stream is signaling to.
#
# Dependencies:
# - libpulse
#
# TODO: Allow individual stream-to-sink mapping on the command-line.
# TODO: Utilize Rofi multi-selection to handle individual stream to sink mappings. (if possible)

# Exit immediately if a "simple" command, a "compound" command, a list, or the last
# command in a pipeline exits with a non-zero exit status.
set -e

# Treat unset variables as errors, exiting when detected.
set -u

# Fail if any command in a pipeline chain returns with a non-zero exit status.
set -o pipefail

# The name of this script.
script="${BASH_SOURCE[0]##*/}"

help_message() {
    printf "%s\\n\\n" "\
Usage: ${script} [-s <sink>]... [-l]

This script allows the user to change the audio output by setting the sink device of the
current audio streams.

Options:

	-s <sink>  Set the audio sink. Can be used more than once to specify fallback sinks.
	-l	   Print the available audio sinks using notify-send.
	-h	   Prints helpful text that explains how to use ${script}.

Example:

To attempt to set the bluez sink and then fallback to the usb if bluez is unavailable, run
the following command:

	streamsink.bash -s bluez -s usb

Bind it to an i3 key combo, like so:

	bindsym \$mod+p exec --no-startup-id \"${HOME}/.config/i3/scripts/streamsink.bash -s bluez -s usb\""
}

# Global variables
OPTIND=1
declare -a sinkdevice=()
declare -A sinks=()
declare -A streams=()
declare -a formatted_sinks2=()
launcher=""
list_sinks='false'
optstring=":s:x:lh"
while getopts "$optstring" option; do
    case "$option" in
	x ) launcher="$OPTARG" ;;
	s ) sinkdevice+=("$OPTARG") ;;
	l ) list_sinks='true' ;;
	h ) help_message; exit 0 ;;
	* ) exit 1 ;;
    esac
done
unset -v option

# Match the "sink index" to the "sink symbolic name".
cache_sinks() {
    local line
    while read -r line; do
	[[ -n "$line" ]] && sinks+=(["$(printf "%s\\n" "$line" | awk '{ print $2 }')"]="$(echo "$line" | awk '{ print $1 }')")
    done < <(pactl list short sinks)
}

# Match the "stream" to the "sink index".
cache_streams() {
    local line
    while read -r line; do
	[[ -n "$line" ]] && streams+=(["$(printf "%s\\n" "$line" | awk '{ print $1 }')"]="$(echo "$line" | awk '{ print $2 }')")
    done < <(pactl list short sink-inputs)
}

# Match the "stream" to the "sink symbolic name".
format_sinks() {
    cache_sinks
    cache_streams
    local stream sink
    declare -a active_sinks=()
    for sink in "${!sinks[@]}"; do
	for stream in "${!streams[@]}"; do
	    if (("${streams["${stream}"]}" == "${sinks["${sink}"]}"))
	    then
		active_sinks+=("$stream")
	    fi
	done
	if [[ -n "${active_sinks[@]}" ]]
	then
	    formatted_sinks1+=("$(echo -e "${active_sinks[@]}\\t${sink}")")
	else
	    formatted_sinks1+=("$(echo -e "NS\\t${sink}")")
	fi
	unset -v active_sinks
    done

    # Get the current default audio sink.
    local default_sink
    default_sink="$(pactl info | grep "[Dd]efault [Ss]ink" 2>/dev/null)"

    # Prepend an asterick (*) to the current default audio sink.
    local formatted_sink
    for formatted_sink in "${formatted_sinks1[@]}"; do
	[[ -z "$formatted_sink" ]] && continue
	formatted_sink_field2="$(echo "$formatted_sink" | awk '{ print $NF }')"
	if [[ $default_sink =~ "$formatted_sink_field2" ]]
	then
	    formatted_sinks2+=("*${formatted_sink}")
	else
	    formatted_sinks2+=("$formatted_sink")
	fi
    done
}

# Print.
list_sink_inputs() {
    format_sinks
    printf "%s\\n" "${formatted_sinks2[@]}"
}

# Query the sink symbolic names with an array of names. Return the first successful
# lookup.
query_sink() {
    local sink line
    cache_sinks
    for sink in "$@"; do
	while read -r line; do
	    printf "%s\\n" "$line"
	    return 0
	done < <(printf "%s\\n" "${sinks[@]}" | grep -E ".*"$sink".*" 2>/dev/null)
    done
    return 1
}

# List out sink inputs in the following form:
#
# Audio Sinks:
# stream-index	  sink
# stream-index	  sink
# ...
#
# All streams will move to the sink that the user selects. Supports any launcher
# (e.g. Rofi), but not required.
if [[ -n "$launcher" ]]; then
    sinkdevice=("$(list_sink_inputs | eval "$launcher" | awk '{ print $2 }')")
elif [[ -n "${sinkdevice[@]}" ]]; then
    sinkdevice="$(query_sink "${sinkdevice[@]}")" || { echo "Terminating"; exit 1; }
fi

if [[ -n "${sinkdevice[@]}" ]]; then
    cache_streams
    while read -r stream; do
	[[ -n "$stream" ]] || continue
	pactl move-sink-input "$stream" "${sinkdevice[@]}"
    done < <(printf "%s\\n" "${!streams[@]}")
    pactl set-default-sink "${sinkdevice[@]}"
fi

# User feedback: cache, format, and print to notification.
if [[ $list_sinks == 'true' ]]; then
    list_sink_inputs
fi
