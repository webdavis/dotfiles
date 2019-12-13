#!/usr/bin/env bash

# Exit immediately if a "simple" command, a "compound" command, a list, or the last
# command in a pipeline exits with a non-zero exit status.
set -e

# Treat unset variables as errors, exiting when detected.
set -u

end=$(($(date +%s) + $1));
while (($end >= $(date +%s))); do
    printf "\\t%s\\r" "$(date -u --date @$((${end} - $( date +%s ))) +%H:%M:%S)";
    sleep 0.1
done
paplay '/usr/share/sounds/freedesktop/stereo/complete.oga'
