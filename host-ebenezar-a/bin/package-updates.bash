#!/usr/bin/env bash

# Exit immediately if a "simple" command, a "compound" command, a list, or the last
# command in a pipeline exits with a non-zero exit status.
set -e

# Treat unset variables as errors, exiting when detected.
set -u

# Fail if any command in a pipeline chain returns with a non-zero exit status.
set -o pipefail

fa_heartbeat="<span font='FontAwesome' color='#ff005f'></span>"
count="$(checkupdates | grep "\(linux-lts\|ca-certificates\)" | wc -l)"
if ((${count} > 0)); then
    printf "%b\\n" "${fa_heartbeat}  ${count}"
fi
