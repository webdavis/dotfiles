#!/usr/bin/env bash

# Exit immediately if a "simple" command, a "compound" command, a list, or the last
# command in a pipeline exits with a non-zero exit status.
set -e

# Treat unset variables as errors, exiting when detected.
set -u

# Fail if any command in a pipeline chain returns with a non-zero exit status.
set -o pipefail

# Default variables and Font Awesome symbols
fa_microchip="<span font='FontAwesome'></span>"
declare -i warning='70'
declare -i critical='90'

help_message() {
    printf "%s\\n\\n" "\
Usage: nvidia-gpu-load.bash [-c <percent>] [-w <percent>]

This script prints the GPU load. If no options are specified then the default settings
will be used.

Options:

    -c <percent>   Set the critical load percentage at which the output turns red (e.g.  nvidia-gpu-load.bash -c 90).
    -w <percent>   Set the warning load percentage at which the output turns yellow (e.g.  nvidia-gpu-load.bash -w 70).
    -h		   Prints helpful text that explains how to use this script.

Example:

To print the GPU load of the Nvidia graphics card and lower the warning threshold to 60
and the critical threshold to 80, run the following command:

    nvidia-gpu-load.bash -w 60 -c 80"
}

optstring=':w:c:h'
while getopts "$optstring" option; do
    case "$option" in
        w ) warning="$OPTARG" ;;
        c ) critical="$OPTARG" ;;
        h ) help_message; exit 0 ;;
        * ) help_message; exit 33 ;;
    esac
done
unset -v option

# Print error messages.
error_exit() {
    if [[ -n "$2" ]]; then
	message="nvidia-gpu-load.bash (line ${1}): ${2}"
    else
	message="nvidia-gpu-load.bash: ${1}"
    fi

    printf "%s\\n" "$message" 2>&1
    exit 33
}
trap 'error_exit ${LINENO} "error reported"' ERR

# Get system GPU load.
while read line; do
    declare -i gpu_usage="$(echo "$line" | awk -F', ' '{ print $1 }')"
    declare -i gpu_memory="$(echo "$line" | awk -F', ' '{ print $2 }')"
    declare -i gpu_video="$(echo "$line" | awk -F', ' '{ print $3 }')"
    declare -i gpu_pcie="$(echo "$line" | awk -F', ' '{ print $4 }')"
done < <(nvidia-settings -q GPUUtilization -t | awk '{ gsub(/[A-Za-z]+=/, ""); print $0 }')

# Build the GPU load indicator. (Fulltext is a required variable according to i3blocks
# documentation.)
fulltext="$(printf "%b %.0f%% %.0f%% %.0f%% %.0f%%\\n" "${fa_microchip} " "${gpu_usage}" "${gpu_memory}" "${gpu_video}" "${gpu_pcie}")"

# Redshift the indicator as the GPU load rises.
if [[ $gpu_usage -ge "$critical" ||
	$gpu_memory -ge "$critical" ||
	$gpu_video -ge "$critical" ||
	$gpu_pcie -ge "$critical" ]]; then
    fulltext="<span color='#ff5f00'>${fulltext}</span>"
elif [[ $gpu_usage -ge "$warning" ||
	$gpu_memory -ge "$warning" ||
	$gpu_video -ge "$warning" ||
	$gpu_pcie -ge "$warning" ]]; then
    fulltext="<span color='#fffc00'>${fulltext}</span>"
fi

printf "%b\\n" "$fulltext"
