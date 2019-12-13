#!/usr/bin/env bash

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
Usage: ${script} [-p <filesystem>]...

This script pretty prints a file systems disk space usage. If no options are specified
then the default settings will be used.

Options:

    -p <filesystem>   The <filesystem> to print. To print multiple file systems repeat this option.
    -h		      Prints helpful text that explains how to use this script.

Examples:

To print the disk space usage of the \"$HOME\" directory and the \"/\" directory run the
following command:

    ${script} -p /
    ${script} -p ${HOME}
    ${script} -p ${HOME} -p / -p /boot"
}

declare -a partition
fraction='false'
optstring=':p:fh'
while getopts "$optstring" option; do
    case "$option" in
        p ) partition+=("$OPTARG") ;;
        f ) fraction='true' ;;
        h ) help_message; exit 0 ;;
        * ) help_message; exit 33 ;;
    esac
done
unset -v option

for filesystem in ${partition[*]}; do
    # Parse `df -h`.
    used="$(df -h "$filesystem" | awk '{ print $(NF-3) }' | sed -n '2p')"
    size="$(df -h "$filesystem" | awk '{ print $(NF-4) }' | sed -n '2p')"
    percentage="$(df -h "$filesystem" | awk '/[0-9]+\%/ { print $(NF-1) }')"

    # Store in an array instead of using string concatenation to avoid extra whitespace.
    if [[ $fraction == 'true' ]]; then
	full_text="${percentage} - ${used} / ${size}"
    else
	full_text="$percentage"
    fi
done
unset -v filesystem

# Print the indexed array. (Note: will only show if usage exceeds 80 percent.)
if (($percentage >= 80)); then
    printf "%b\\n" "$full_text"
fi
