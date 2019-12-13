#!/usr/bin/env bash

# Exit immediately if a "simple" command, a "compound" command, a list, or the last
# command in a pipeline exits with a non-zero exit status.
set -e

# Treat unset variables as errors, exiting when detected.
set -u

# Fail if any command in a pipeline chain returns with a non-zero exit status.
set -o pipefail

# Case insensitive matching.
shopt -s nocasematch

# The name of this script.
script="${BASH_SOURCE[0]##*/}"

help_message() {
    printf "%s\\n\\n" "\
Usage: ${script} [-e <logout-utility>|-l <launcher>|-x <screen-locker>|-s|-b|-r|-p|-h]

This script runs systemctl power management commands. It has builtin support for any
launcher utility.

Options:

    -e <logout-utility>  Specify any window manager logout utility to use.

    -l <launcher>	 Specify any launcher utility, (e.g. Rofi), to use; runs command
			 through launcher.

    -x <screen-locker>   Specify any screen locker utility to use; locks screen prior
			 to system suspension or hibernation.

    -s   Suspend the system. The will trigger activation of the special target unit
	 suspend.target. This command is asynchronous, and will return after the suspend
	 operation is successfully enqueued. It will not wait for the suspend/resume cycle
	 to complete.

    -b 	 Hibernate and suspend the system. This will trigger activation of the special
	 target unit hibernate.target. This command is asynchronous, and will return after
	 the hibernate operation is successfully enqueued. It will not wait for the
	 hibernate/thaw cycle to complete.

    -r 	 Shut down and reboot the system. This is mostly equivalent to systemctl start
	 reboot.target --job-mode=replace-irreversibly --no-block, but also prints a wall
	 message to all users. This command is asynchronous, and will return after the
	 reboot operation is enqueued, without waiting for it to complete.

    -p   Shut down and power-off the system. This is mostly equivalent to systemctl start
	 poweroff.target --job-mode=replace-irreversibly --no-block, but also prints a
	 wall message to all users. This command is asynchronous, and will return after
	 the power-off operation is enqueued, without waiting for it to complete.

    -h	 Prints helpful text that explains how to use ${script}.

Examples:

To suspend Linux, run the following command:

    systemctl-rofi.bash -s

To launch Rofi to select the type of
screenshot, run the following command:

    systemctl-rofi.bash \
	-e 'i3-msg exit' \
	-l 'rofi -eh 2 -no-fixed-num-lines -dmenu -i -p System\ Commands:' \
	-x 'i3lock -t -i ${HOME}/Pictures/endless-shapes.png'

(Hint: bind this so an i3 keyboard shortcut.)"
}

declare -a launcher_options=("Suspend System" "Hibernate System" "Reboot System" "Power-off System")
exit_option='false'
launcher=""
locker=""
syscommand=""
optstring=':e:x:l:sbrph'
while getopts "$optstring" option; do
    case "$option" in
	e ) exit_option="$OPTARG" ;;
	x ) launcher="$OPTARG" ;;
	l ) locker="$OPTARG" ;;
	s ) syscommand='suspend' ;;
	b ) syscommand='hibernate' ;;
	r ) syscommand='reboot' ;;
	p ) syscommand='poweroff' ;;
	h ) help_message; exit 0 ;;
	* ) help_message; exit 1 ;;
    esac
done
unset -v option

# Support any window manager logout utility, (e.g. i3-msg exit), but not required.
if [[ -n "$exit_option" ]]; then
    launcher_options=("${launcher_options[@]}" "Exit Window Manager")
fi

# Support any launcher, (e.g. Rofi), but not required.
if [[ -n "$launcher" ]]; then
    syscommand="$(printf "%s\\n" "${launcher_options[@]}" | eval "${launcher}" | awk '{ print $1 }')"
fi

case "${syscommand}" in
    "exit" ) eval "${exit_option}" exit ;;
    "suspend" ) [[ -n "${locker}" ]] && eval "${locker}" && sleep 1; /usr/bin/systemctl suspend ;;
    "hibernate" ) [[ -n "${locker}" ]] && eval "${locker}" && sleep 1; /usr/bin/systemctl hibernate ;;
    "reboot" ) /usr/bin/systemctl reboot ;;
    "poweroff" ) /usr/bin/systemctl poweroff ;;
    * ) ;;
esac
