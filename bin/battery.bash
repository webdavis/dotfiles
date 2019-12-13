#!/usr/bin/env bash

# Exit immediately if a "simple" command, a "compound" command, a list, or the last
# command in a pipeline exits with a non-zero exit status.
set -e

# Treat unset variables as errors, exiting when detected.
set -u

# Fail if any command in a pipeline chain returns with a non-zero exit status.
set -o pipefail

# Force Bash pattern matching to be case insensitive in help assign Font Awesome unicode
# characters to the state of the /sys/../status file.
shopt -s nocasematch

# The name of this script.
script="${BASH_SOURCE[0]##*/}"

# This function logs useful error messages.
error() {
    local line_number="$1"
    local message="${2:-}"
    local exit_code="${3:-1}"

    local full_message="${script}: error on or near line ${line_number}: ${message}. Exiting with status: ${exit_code}."
    printf "%s\\n" "$full_message" 2>&1
    [[ -e "$lock_file" ]] && rm "$lock_file"
    exit "$exit_code"
}

# Trap any errors, calling error() when they're caught.
trap 'error "${LINENO}" "unknown"' ERR

# This function logs user interruptions.
interrupt() {
    local exit_code="$?"
    trap '' EXIT
    printf "%s\\n" "${script}: received interrupt signal from user. The last command finished with exit status ${exit_code}."
    [[ -e "$lock_file" ]] && rm "$lock_file"
    exit "$exit_code"
}

# Trap any user interruptions, calling interrupt() when they're caught.
trap interrupt INT
trap interrupt QUIT
trap interrupt TERM

# Cache battery data.
for battery in '/sys/class/power_supply/'{BAT,axp288_fuel_gauge,CMD}*; do
    if [[ -d "$battery" ]]; then
	# Calculate this some setups don't maintain a /sys/../capacity file.
	declare -i energy_now="$(cat "${battery}/energy_now" 2>/dev/null)"
	declare -i energy_full="$(cat "${battery}/energy_full" 2>/dev/null)"
	declare -i energy_percentage=$(($((energy_now * 100)) / energy_full))
	energy_percentage="${energy_percentage:-00}"

	# Power state (discharging, charging, full, fully-charged, unknown)
	status="$(cat "${battery}/status" 2>/dev/null)"

	# Check upower because power_supply reports a false negative when the battery is
	# fully charged.
	if [[ -x "$(builtin command -v upower)" ]] \
		&& upower -i /org/freedesktop/UPower/devices/battery_BAT0 |
		    grep -o "state:.*fully-charged" >/dev/null 2>&1
	then
	    status='full'
	    energy_percentage='100'
	fi
    fi
done
unset -v battery

# Default if power_supply isn't detected.
block="<span color='red'><span font='FontAwesome'> </span></span>"

# lightning = charging
#   battery = discharging
#  question = unknown
fa_charging_station="<span color='yellow'><span font='FontAwesome'></span></span>"
fa_battery="<span font='FontAwesome'></span>"
fa_question="<span font='FontAwesome'></span>"

case "$status" in
    "discharging" ) block="${fa_battery}" ;;
    "charging" ) block="${fa_charging_station}" ;;
    "unknown" | * ) block="${fa_question}" ;;
esac

# `exit 33`: i3blocks prints white text on a red background.
red='#ff005f'
yellow='#ffff5f'
white='#ffffff'

if [[ $energy_percentage -le 15 ]]; then color="$red"
elif [[ $energy_percentage -le 30 ]]; then color="$yellow"
elif [[ $energy_percentage -lt 100 ]]; then color="$white"; fi

# Inject values into pango format.
block="${block}  <span foreground=\"${color}\">"${energy_percentage}"%</span>"

# Only print if less than 100% battery.
if [[ ! $status =~ "full"* ]]; then
    printf "%b\\n" "$block"
    printf "%b\\n" "$block"
fi

# Lock file name
# File descriptor
declare -r lock_file="/var/lock/$(basename ${0}).lock"
declare -ir lock_fd=200

# Set a mutex on this script and create a lock file to prevent certain parts of this
# script from executing concurrently and repeatedly.
lock() {
    eval "exec ${lock_fd}>${lock_file}"
    flock -n "$lock_fd" || return 1
    return 0
}

# Notify at 10% battery.
if [[ ! -e "$lock_file" ]] &&
	[[ $status == 'discharging' ]] &&
	[[ $energy_percentage -eq 10 ]]
then
    lock || exit 1
    notify-send \
	--urgency=critical \
	--expire-time=0 \
	--icon=battery-caution \
	'Laptop battery low' '10% remaining. The laptop will suspend at 5%.'
fi

# Ensure lock file has been removed.
if [[ -e "$lock_file" && $energy_percentage -ne 10 ]]; then
    rm "$lock_file"
fi

# Suspend at 5% battery to prevent shutdown and loss of work session.
if [[ ! -e "$lock_file" && $status == 'discharging' ]] &&
	[[ $energy_percentage -eq 5 ]]
then
    lock || exit 0
    notify-send \
	--urgency=critical \
	--expire-time=118000 \
	--icon=battery-caution \
	'Laptop battery critical' 'Laptop will suspend in 2 minutes.'
    sleep 120

    if [[ "$(cat "${battery}/status" 2>/dev/null)" =~ 'discharging' ]] &&
	    [[ $energy_percentage -le 5 ]]
    then
	systemctl suspend
    fi
fi
