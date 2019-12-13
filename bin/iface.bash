#!/bin/bash

# Exit immediately if a "simple" command, a "compound" command, a list, or the last
# command in a pipeline exits with a non-zero exit status.
set -e

# Treat unset variables as errors, exiting when detected.
set -u

# Fail if any command in a pipeline chain returns with a non-zero exit status.
set -o pipefail

# Use the provided interface, otherwise the device used for the default route.
interface="${_iface:-}"
interface="${interface:-$(ip route | awk '/^default/ { print $5; exit }')}"

# As per #36 -- it is transparent: [e.g. if the machine has no battery or wireless
# connection (think desktop), the corresponding block should not be displayed].
[[ ! -d /sys/class/net/${interface} ]] && exit 33

ADDRESS_FAMILY="${ADDRESS_FAMILY:-inet6?}"
label="${label:-}"

optstring=":46Lh"
while getopts "$optstring" option; do
    case "$option" in
        4 ) ADDRESS_FAMILY="inet" ;;
        6 ) ADDRESS_FAMILY="inet6" ;;
        L ) if [[ -z "$interface" ]]; then label="iface "; else label="${interface} "; fi ;;
        h ) exit 0 ;;
        * ) exit 33 ;;
    esac
done
unset -v option

if [[ $interface == "" ]] || [[ "$(cat /sys/class/net/${interface}/operstate)" == 'down' ]]; then
    state="<span color'#ff0000'>"${label}"</span>"
    printf "%s\\n" "${label}${state}"
    exit
fi

# If no interface is found, use the first device with a global scope.
# Colorize output
ipaddr="$(ip addr show ${interface} | perl -n -e "/${ADDRESS_FAMILY} ([^\/]+).* scope global/ && print \$1 and exit")"
ipaddr="<span color='#00ff00'>"${ipaddr}"</span>"

BLOCK_BUTTON="${BLOCK_BUTTON:-}"
[[ $BLOCK_BUTTON -eq 3 ]] && printf "%s" "$ipaddr" | xclip -q -se c

# Full text
# Short text
printf "%s\\n" "${label} ${ipaddr}"
printf "%s\\n" "${label} ${ipaddr}"
