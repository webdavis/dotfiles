# vi: set filetype=sh:

# Start an HTTP server from a directory, optionally specifying the port. Credit:
# https://github.com/jessfraz/dotfiles/blob/master/.functions
server() {
	local port="${1:-8000}"
	sleep 1 && xdg-open "http://localhost:${port}/" &
	# Set the default Content-Type to `text/plain` instead of `application/octet-stream`
	# And serve everything as UTF-8 (although not technically correct, this doesn’t break anything for binary files)
	python2.7 -c $'import SimpleHTTPServer;\nmap = SimpleHTTPServer.SimpleHTTPRequestHandler.extensions_map;\nmap[""] = "text/plain";\nfor key, value in map.items():\n\tmap[key] = value + ";charset=UTF-8";\nSimpleHTTPServer.test();' "$port"
}


open_last_committed_files() {
    # You must be in a directory relative to the Git path.
    files=($(git diff-tree --no-commit-id --name-only -r HEAD))
    for file in ${files[@]}; do
        echo ${file##*/}
        files+=($(echo "${file}" | sed "s/${}//g"))
    done
    $EDITOR -O "${files[*]}"
}


mcd() {
    [[ $# -gt 1 || -d $1 ]] && return 1
    mkdir -p "$@"
    cd -- "$@"
}


# Make a temporary directory and enter it
tmpd() {
    local dir
    if [[ $# -eq 0 ]]; then dir=$(mktemp -d); else dir=$(mktemp -d -t "${1}.XXXXXXXXXX"); fi
    cd -- "$dir" || return 1
}


# Get the size of a file or the total size of a directory.
fs() {
    local arg
    if du -b /dev/null > /dev/null 2>&1; then arg=-sbh; else arg=-sh; fi;
    if [[ -n "$@" ]]; then du $arg -- "$@"; else du $arg -- .[^.]* *; fi;
}

# fkill - kill process
# Credit: https://github.com/atweiden/fzf-extras/blob/master/fzf-extras.sh
fkill() {
    local pid
    pid="$(ps -ef | sed 1d | fzf -m | awk '{print $2}')" || return
    kill -"${1:-9}" "$pid"
}
