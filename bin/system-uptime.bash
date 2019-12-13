#!/usr/bin/env bash

# Exit immediately if a "simple" command, a "compound" command, a list, or the last
# command in a pipeline exits with a non-zero exit status.
set -e

# Treat unset variables as errors, exiting when detected.
set -u

# Fail if any command in a pipeline chain returns with a non-zero exit status.
set -o pipefail

# Convert epoch to DD:HH:MM:SS time.
epoch="$(cat /proc/uptime 2>/dev/null)"
declare -i epoch="${epoch%%.*}"
declare -i days="$(($(($((epoch / 60)) / 60)) / 24))"
declare -i hours="$(($(($((epoch / 60)) / 60)) % 24))"
declare -i minutes="$(($((epoch / 60)) % 60))"
declare -i seconds="$((epoch % 60))"

# Color reward for longer uptimes.
if [[ $days -gt 90 ]]; then text="<span color='#00ff5f'>$(printf "%02d:%02d:%02d:%02d\\n" "$days" "$hours" "$minutes" "$seconds")</span>"
elif [[ $days -gt 15 ]]; then text="<span color='#00875f'>$(printf "%02d:%02d:%02d:%02d\\n" "$days" "$hours" "$minutes" "$seconds")</span>"
elif [[ $days -gt 0 ]]; then text="<span color='#ffff5f'>$(printf "%02d:%02d:%02d:%02d\\n" "$days" "$hours" "$minutes" "$seconds")</span>"
else text="<span color='#ff005f'>$(printf "%02d:%02d:%02d\\n" "${hours}" "${minutes}" "${seconds}")</span>"; fi

# Print fulltext.
fa_power_off="<span font='FontAwesome'></span>"
fulltext="${fa_power_off}  ${text}"
printf "%b\\n" "$fulltext"
