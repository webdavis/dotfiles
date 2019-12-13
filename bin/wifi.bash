#!/bin/bash

# Exit immediately on error.
set -e

# Treat unset variables as errors, exiting when detected.
set -u

# Fail if any command in a pipeline chain returns with a non-zero exit status.
set -o pipefail

if [[ -z "${INTERFACE:-}" ]]; then
    INTERFACE="${BLOCK_INSTANCE:-wlp4s0}"
fi

# As per #36 -- it is transparent: [e.g. if the machine has no battery or wireless
# connection (think desktop), the corresponding block should not be displayed].
if [[ -d /sys/class/net/${INTERFACE}/wireless ]]; then
    [[ "$(cat /sys/class/net/${INTERFACE}/operstate)" == 'down' ]] && exit 0
fi

# Font Awesome WiFi.
fa_wifi="<span font='FontAwesome'></span>"

# WiFi quality.
declare -i quality="$(grep ${INTERFACE} /proc/net/wireless | awk '{ print int($3 * 100 / 70) }')"

# Color.
if [[ $quality -ge 80 ]]; then fulltext="<span color='#00ff00'>"${quality}%"</span>"
elif [[ $quality -ge 60 ]]; then fulltext="<span color='#fff600'>"${quality}%"</span>"
elif [[ $quality -ge 40 ]]; then fulltext="<span color='#ffae00'>"${quality}%"</span>"
else fulltext="<span color='#ff0000'>"${quality}%"</span>"; fi

# If there is no ethernet connection and a WiFi connection then print the signal strength.
if ! ip route list | grep --quiet --no-messages -Po "default.*dev \Ke" &&
	ip route list | grep --quiet --no-messages -Po "default.*dev \Kw"; then
    printf "%b\\n" "${fa_wifi}  ${fulltext}"
fi
