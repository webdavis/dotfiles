#!/usr/bin/env bash

# Exit immediately if a "simple" command, a "compound" command, a list, or the last
# command in a pipeline exits with a non-zero exit status.
set -e

# Treat unset variables as errors, exiting when detected.
set -u

# Fail if any command in a pipeline chain returns with a non-zero exit status.
set -o pipefail

# Count the number of Docker containers running. If there aren't any running then don't
# display the status-line.
declare -i count="$(sudo docker ps -q | wc -l | sed -r 's/^0$//g')"
(( $count < 1 )) && exit 0

# Try to get the IP address of the most recent docker container, if there isn't one fall
# back to whatever is next.
ip="$(sudo docker inspect -f "{{ .NetworkSettings.IPAddress }}" $(sudo docker ps -ql))"
[[ -z "$ip" ]] && ip="$(sudo docker inspect -f "{{ .NetworkSettings.IPAddress }}" $(sudo docker ps -q) | sed -n 1p)"

printf "%s\\n" "${count} - ${ip}"
