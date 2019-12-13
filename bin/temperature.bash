#!/usr/bin/env bash

# The user can pick which chip to query. To find out what chips are available run the
# following command:
#
#   sensors -u
#
# The chips will be be printed with some diagnostic fields. Choose one of the chips
# from the output and put it in your ~/.bashrc or respective shell configuration file as:
#
#   SENSOR_CHIP="iwlwifi-virtual-0" && export SENSOR_CHIP
#
# The following are some example chips:
#
#   1. iwlwifi-virtual-0
#   2. acpitz-virtual-0
#   3. coretemp-isa-0000
#   4. pch_skylake-virtual-0

# Exit immediately if a "simple" command, a "compound" command, a list, or the last
# command in a pipeline exits with a non-zero exit status.
set -e

# Treat unset variables as errors, exiting when detected.
set -u

# Fail if any command in a pipeline chain returns with a non-zero exit status.
set -o pipefail

# The name of this script.
script="${BASH_SOURCE[0]##*/}"

# Default variables and Font Awesome symbols
chip="${SENSOR_CHIP:-"acpi.*"}"
fa_thermometer_quarter="<span font='FontAwesome'></span>"
fa_thermometer_three_quarters="<span font='FontAwesome'></span>"
fa_fire="<span font='FontAwesome'></span>"
declare -i warning_temp='70'
declare -i critical_temp='90'

# Print error messages.
error() {
    if [[ -n "${2}" ]]; then
	message="${script} (line ${1}): ${2}"
    else
	message="${script}: ${1}"
    fi

    printf "%s\\n" "${message}" 20&
    exit 33
}

# Trap any errors, calling error() when they're caught.
trap 'error "${LINENO}" "error reported"' ERR

help_message() {
    printf "%s\\n\\n" "\
Usage: ${script} [-s <chip>] [-c <percent>] [-w <percent>]

This script prints the temperature of a sensor chip. If no options are specified then
the default settings will be used.

Options:

    -s <chip>	   Print the temperature of <chip>.
    -w <percent>   Set the warning temperature percentage at which the output turns yellow (e.g. ${script} -w 70).
    -c <percent>   Set the critical temperature percentage at which the output turns orange (e.g. ${script} -c 90).
    -h		   Prints helpful text that explains how to use ${script}.

Example:

To print the temperature of the Nvidia SkyLake GPU and change the critical warning to 80,
run the following command:

    ${script} -c pch_skylake-virtual-0"
}

optstring=':s:c:w:h'
while getopts "${optstring}" option; do
    case "${option}" in
	s ) chip="${OPTARG}" ;;
	w ) warning_temp="${OPTARG}" ;;
	c ) critical_temp="${OPTARG}" ;;
	h ) help_message; exit 0 ;;
	* ) help_message; exit 33 ;;
    esac
done
unset -v option

# Check if lm-sensors has been installed.
if [[ -x "$(builtin command -v sensors)" ]]; then
    # Store the matching chip and it's respective fields. Default to the ACPI thermal zone.
    chip_grep="$(sensors -u | grep -E -A 5 "${chip}")" || error "${LINENO}" "could not find chip name \"${chip}\""
else
    error "${LINENO}" "the command sensor was not found; install lm-sensors"
fi

# Get system temperature.
while read line; do
    if echo $line | grep -E "temp1_input:.*" >/dev/null 2>&1; then
	temperature="$(echo ${line} | awk '{ gsub(/[0-9][0-9]$/, ""); print $2 }')"
    fi
done < <(echo "${chip_grep}")

# Exit if the temperature cannot be detected.
[[ -z "${temperature}" ]] && error "could not find temp1_input temperature"

# Build the temperature indicator. (Fulltext is a required variable according to i3blocks
# documentation.)
fulltext="${fa_thermometer_quarter}  ${temperature} °C"

# Redshift the indicator as the temperature rises. (Note: will only show if temperature
# exceeds warning level.
if (("${temperature:0:2}" >= "${critical_temp}")); then
    fulltext="<span color='#ff5f00'>${fa_fire} ${temperature} °C</span>"
    printf "%b\\n" "${fulltext}"
elif (("${temperature:0:2}" >= "${warning_temp}")); then
    fulltext="<span color='#fffc00'>${fa_thermometer_three_quarters} ${temperature} °C</span>"
    printf "%b\\n" "${fulltext}"
fi
