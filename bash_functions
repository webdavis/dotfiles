# vi: set filetype=sh:

# The Fzf project hosted at https://github.com/junegunn/fzf/tree/master/shell.
[ -f ~/.fzf.bash ] && source ~/.fzf.bash

fzf_setup() {
    [[ -d "${HOME}/workspaces/tools/fzf/bin" ]] && path_append "${HOME}/workspaces/tools/fzf/bin"

    # Fzf must be installed either in the local user path or system-wide.
    local fzf_rootpath
    if [[ -x "${HOME}/workspaces/tools/fzf/bin/fzf" ]]; then
	fzf_rootpath="${HOME}/workspaces/tools/fzf"
    elif [[ -x '/usr/bin/fzf' ]]; then
	fzf_rootpath="/usr/share/fzf"
    else
	return 1
    fi

    [[ -s "${fzf_rootpath}/shell/completion.bash" ]] && \. "${fzf_rootpath}/shell/completion.bash"
    [[ -s "${fzf_rootpath}/shell/key-bindings.bash" ]] && \. "${fzf_rootpath}/shell/key-bindings.bash"

    if [[ -x "${HOME}/workspaces/tools/ripgrep-0.10.0/target/release/rg" ]]; then
	# FZF_DEFAULT_COMMAND='(rg --files --no-ignore --hidden --follow --glob "!.git/" 2>/dev/null)'
	FZF_DEFAULT_COMMAND='rg --files --no-ignore --hidden --follow --glob "!.git/" 2>/dev/null'
    elif [[ -x "/usr/bin/ag" ]]; then
        FZF_DEFAULT_COMMAND='ag -u --hidden --ignore ".git" -l -g ""'
    else
        FZF_DEFAULT_COMMAND='find . -path "*/\.*" -prune -o -type f -print -o -type l -print | sed s/^..// 2>/dev/null'
    fi
    export FZF_CTRL_T_COMMAND="$FZF_DEFAULT_COMMAND"

    # Command history with short descriptions.
    export FZF_CTRL_R_OPTS="--preview='echo {} | cut -d"'" "'" -f3 | xargs whatis' --preview-window=down:5"

    FZF_DEFAULT_OPTS='--reverse '
    FZF_DEFAULT_OPTS+='-m '
    FZF_DEFAULT_OPTS+='--cycle '
    # bat provides syntax highlighting.
    FZF_DEFAULT_OPTS+="--preview='(file --mime {} | grep --quiet --no-messages empty && echo \$(basename {}): is empty.) ||
        (file --mime {} | grep --quiet --no-messages binary && echo \$(basename {}): is a binary file.) ||
        (bat --style=numbers --color=always {} | head -150)' "
    FZF_DEFAULT_OPTS+='--preview-window="right:50%:hidden" '

    # Keyboard mappings.
    FZF_DEFAULT_OPTS+='--bind="?:toggle-preview" '
    FZF_DEFAULT_OPTS+='--bind="ctrl-n:down" '
    FZF_DEFAULT_OPTS+='--bind="ctrl-p:up" '
    FZF_DEFAULT_OPTS+='--bind="ctrl-alt-p:page-up" '
    FZF_DEFAULT_OPTS+='--bind="ctrl-alt-n:page-down" '
    FZF_DEFAULT_OPTS+='--bind="change:top" '
    FZF_DEFAULT_OPTS+='--bind="ctrl-r:toggle-sort" '
    FZF_DEFAULT_OPTS+='--bind="ctrl-g:kill-line" '

    # Set aside the keys that can be used to perform special actions when pressed during Fzf.
    # FZF_DEFAULT_OPTS+="--expect='ctrl-o,ctrl-e,ctrl-v,ctrl-x "
    #
    # # Colorscheme.
    FZF_DEFAULT_OPTS+='--color="bg+:#434748,bg:#2f3334,spinner:#af5f5f,hl:#ffdf5f" '
    FZF_DEFAULT_OPTS+='--color="fg:#a8a8a8,header:1,info:#af5f5f,pointer:15" '
    FZF_DEFAULT_OPTS+='--color="marker:#af875f,fg+:15,prompt:14,hl+:#50b6eb" '
    FZF_DEFAULT_OPTS+='--color="border:15" '
    export FZF_DEFAULT_OPTS

    export FZF_COMPLETION_OPTS="--height='40%'"

    # The command that opens default applications depending on the mime type of the file.
    [[ "$(command -v xdg-open)" == '/usr/bin/xdg-open' ]] && export OPENER='xdg-open'

    # Used in fzf-bindings to search the contents of files.
    export FINDER='rg --line-number --hidden --ignore-file /home/stephen/.gitignore_global .'

    # A plugin full of useful Fzf mappings.
    [[ -s "${HOME}/.fzf-bindings.bash" ]] && \. "${HOME}/.fzf-bindings.bash"
}
fzf_setup # Quick on/off switch.


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


mkd() {
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
