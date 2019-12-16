# vi: set filetype=sh:

builtin bind '"\C-x0": kill-whole-line'     # Clears the entire line.
builtin bind '"\C-x1": magic-space'         # Performs history expansion and inserts a space.
builtin bind '"\C-x2": redraw-current-line' # Refreshes the current line.

# Prints the command to the command-line so that command-line history is logged.
__ehc() {
    READLINE_LINE="${READLINE_LINE:+${READLINE_LINE:0:READLINE_POINT}}${1}${READLINE_LINE:+${READLINE_LINE:READLINE_POINT}}"
    READLINE_POINT="$((READLINE_POINT + ${#1}))"
}

__build_edit_command() {
    local key tool file line checkout optstring option
    key='default'
    tool=""
    file=""
    line=""
    checkout=""
    optstring=':k:t:f:l:c:'
    while getopts "$optstring" option; do
        case $option in
            k ) key="${OPTARG}" ;;
            t ) tool="${OPTARG}" ;;
            f ) file="${OPTARG}" ;;
            l ) line="+${OPTARG}" ;;
            c ) checkout="git checkout ${OPTARG};" ;;
            * ) builtin printf "%s\\n" "fzf-bindings: flag '$OPTARG' doesn't exist. Terminating."; return 1 ;;
        esac
    done

    # Allocate the command namespace.
    local _command=""

    [[ -n $tool ]] && _command="$tool"

    # Build the command.
    [[ -n "$file" ]] && _command="${_command} ${file}"

    if [[ -n "$checkout" ]]; then
        _command="${checkout} ${_command}"
    fi

    [[ -n "$line" ]] && _command="${_command} ${line}"

    # Choose the operation.
    case $key in
	'alt-c' )
	    _command="cd ${file%/*}"
	    if [[ -n $checkout ]]; then
            _command="${checkout} ${_command}"
	    fi
	    ;;
	'ctrl-o' )
            if [[ -n "$OPENER" ]]; then
                _command="${OPENER} $file"
            else
                # Attempt to determine open-utility on the fly.
                local os kernel_name
                kernel_name="$(command uname -s)"
                case "${kernel_name}" in
                    'Linux' | 'GNU'* )
                        os='linux' ;;
                    'Darwin' )
                        os="$(command sw_vers -productname)" ;;
                    *'BSD' )
                        os='bsd' ;;
                    'CYGWIN'* | 'MSYS'* | 'MINGW'* )
                        os='windows' ;;
                    'SunOS' )
                        os='solaris' ;;
                    * )
                        os='unknown'
                        ;;
                esac
                case "${os}" in
                    'linux' | 'bsd' )
                        _command="xdg-open $file" ;;
                    'Mac OS X' )
                        _command="open $file" ;;
                    'windows' )
                        _command="start $file" ;;
                    * )
                        printf "%s\\n" "Unknown Operating System detected: \"${kernel_name}\". Terminating." 1>&2
                        return 1
                esac
            fi
            ;;
	'ctrl-s' )
	    _command="sudo ${_command}"
	    ;;
	'ctrl-x' )
	    [[ -n "$TMUX" ]] || { builtin printf "%s\\n" 'fzf-bindings: Tmux session not detected. Terminating.' 2>&1; return 1; }
	    _command="tmux split-window -v \"${_command}; exec bash\""
	    ;;
	'ctrl-v' )
	    [[ -n "$TMUX" ]] || { builtin printf "%s\\n" 'fzf-bindings: Tmux session not detected. Terminating.' 2>&1; return 1; }
	    _command="tmux split-window -h \"${_command}; exec bash\""
	    ;;
	'ctrl-t' )
	    [[ -n "$TMUX" ]] || { builtin printf "%s\\n" 'fzf-bindings: Tmux session not detected. Terminating.' 2>&1; return 1; }
	    _command="tmux new-window \"${_command}; exec bash\""
	    ;;
    esac

    builtin echo "$_command"
}

__catch_interrupt() {
    trap '' ERR INT TERM QUIT

    if [[ -n "$1" ]]; then
	cd -- "$1"
    fi
    return
}

# Directly open a file.
__fzf_file_open() {
    local selection key file
    selection=("$(FZF_DEFAULT_COMMAND="${FZF_DEFAULT_COMMAND}" \
	command fzf --height='40%' -0 --expect='ctrl-o,ctrl-x,ctrl-v,ctrl-t,ctrl-s,alt-c')")
    key="$(command head -1 <<< "$selection")"
    file="$(command head -2 <<< "$selection" | command tail -1)"

    local path
    if [[ -n "$dir" ]]; then
	path="${dir}/${file}"
    else
	path="${file}"
    fi

    [[ -f "$path" ]] || { cd -- "$cwd"; return 1; }

    local _command
    _command="$(__build_edit_command -k "$key" -t "$EDITOR" -f "$path")"
    [[ $? -eq 1 ]] && return 1
    __ehc "$_command"
}

# Press "Ctrl-x + Ctrl-x" (double tap) to open a file.
builtin bind -x '"\C-x3": __fzf_file_open'
builtin bind '"\C-x\C-x": "\C-x0\C-x3\C-x1\C-x2\015"'

# Directly open a file in the project.
__fzf_project_open() {
    local cwd
    cwd="$(pwd)"
    trap '__catch_interrupt "$cwd"' ERR INT TERM QUIT
    local dir=""
    if git rev-parse --show-toplevel &>/dev/null; then
	dir="$(git rev-parse --show-toplevel)"
	cd -- "$dir"
    fi

    local selection key file
    selection=("$(FZF_DEFAULT_COMMAND="${FZF_DEFAULT_COMMAND}" \
	command fzf --height='40%' -0 --expect='ctrl-o,ctrl-x,ctrl-v,ctrl-t,ctrl-s,alt-c')")
    key="$(command head -1 <<< "$selection")"
    file="$(command head -2 <<< "$selection" | command tail -1)"

    local path
    if [[ -n "$dir" ]]; then
	path="${dir}/${file}"
    else
	path="${file}"
    fi

    [[ -f "$path" ]] || { cd -- "$cwd"; return 1; }

    local _command
    _command="$(__build_edit_command -k "$key" -t "$EDITOR" -f "$path")"
    [[ $? -eq 1 ]] && return 1
    __ehc "$_command"

    [[ $key != 'alt-c' ]] && cd -- "$cwd"
    trap '' ERR INT TERM QUIT
}

# Press "Ctrl-x + x" to open a file.
builtin bind -x '"\C-_5": __fzf_project_open'
builtin bind '"\C-xx": "\C-x0\C-_5\C-x1\C-x2\015"'

__fzf_checkout_commit() {
    if git rev-parse --show-toplevel &>/dev/null; then
        local path
        path="$(git rev-parse --show-toplevel)"
    else
        builtin printf "%s\\n" "Git project not detected." 2>&1
        return 1
    fi

    local fzf_output key selection commit_hash file file_path
    fzf_output=("$(git log --oneline | fzf --height='40%' -0 --expect='ctrl-x,ctrl-v,ctrl-t')")
    key="$(command head -1 <<< "$fzf_output")"; [[ -z "$key" ]] && key='default'
    selection="$(command head -2 <<< "$fzf_output" | command tail -1)"
    commit_hash="$(echo $selection | awk '{ print $1 }')"

    # Verify the commit hash.
    local hash_regex='^[0-9a-f]{7,40}'
    if [[ $commit_hash =~ $hash_regex ]]; then
        commit_hash="${BASH_REMATCH[@]}"
    fi

    local _command
    _command="$(__build_edit_command -k "$key" -c "$commit_hash")"
    [[ $? -eq 1 ]] && return 1
    __ehc "$_command"
}

# Press "Ctrl-x + c" to checkout to a commit.
builtin bind -x '"\C-_4": __fzf_checkout_commit'
builtin bind '"\C-xc": "\C-x0\C-_4\C-x1\C-x2\015"'

# Open file attached to previous commit.
__fzf_open_file_at_commit() {
    if git rev-parse --show-toplevel &>/dev/null; then
        local path
        path="$(git rev-parse --show-toplevel)"
    else
        builtin printf "%s\\n" "Git project not detected." 2>&1
        return 1
    fi

    # Get number of commits.
    local commit_count
    commit_count="$(git rev-list --no-merges --count HEAD)"
    commit_count=$((commit_count - 1))

    # Build array of file diffs paired with their respective commit hashes.
    local _a hash_regex line _hash message
    hash_regex='(^[0-9a-f]{7,40})\ (.*)'
    while read -r line; do
        if [[ $line =~ $hash_regex ]]; then
            _hash="${BASH_REMATCH[1]}"
            message="${BASH_REMATCH[2]}"
        else
            _a+=("$_hash - $message: $line")
        fi
    done < <(git --no-pager show --stat --oneline --name-only HEAD~${commit_count}..HEAD)

    local fzf_output key selection commit_hash file file_path
    fzf_output=("$(printf "%s\\n" "${_a[@]}" | fzf --height='40%' -0 --expect='ctrl-o,ctrl-x,ctrl-v,ctrl-t,ctrl-s,alt-c')")
    key="$(command head -1 <<< "$fzf_output")"; [[ -z "$key" ]] && key='default'
    selection="$(command head -2 <<< "$fzf_output" | command tail -1)"
    commit_hash="$(echo $selection | awk '{ print $1 }')"
    file="$(echo $selection | awk '{ print $NF }')"
    file_path="${path}/${file}"

    # Turn off case-sensitivity for the following prompt.
    shopt -s nocasematch

    local answer checkout='false'
    while true; do
	# This fails without the '-n' flag.
        read -p "Do you want to checkout this commit_hash? (Y/N): " -n 1 answer
        case "${answer}" in
            'Y' ) echo $answer; checkout='true'; break ;;
            'N' ) echo $answer; break ;;
            * ) echo $answer; printf "%s\\n" "Please answer [Y]ES or [N]O (case sensitive)." ;;
        esac
    done
    # Turn case-sensitivity back on.
    shopt -u nocasematch

    local tool build_arguments
    if [[ "$checkout" == 'true' ]]; then
	tool="${EDITOR}"
	build_arguments='-k "$key" -t "$tool" -f "$file_path" -c "$commit_hash"'
    else
        # Fail gracefully if file doesn't exist in current branch.
        [[ -f "$file" ]] || { printf "%s\\n" "${file} doesn't exist in the working branch." 2>&1; return 1; }

        tool="git difftool --no-prompt ${commit_hash}"
        build_arguments='-k "$key" -t "$tool" -f "$file"'
    fi

    local _command
    _command="$(eval __build_edit_command "$build_arguments")"
    [[ $? -eq 1 ]] && return 1
    __ehc "$_command"
}

# Press "Ctrl-x + C" to open one of the previously committed files.
builtin bind -x '"\C-_3": __fzf_open_file_at_commit'
builtin bind '"\C-xC": "\C-x0\C-_3\C-x1\C-x2\015"'

# Open a file, listing all files below the Home directory.
__fzf_file_open_home() {
    local cwd home
    cwd="$(pwd)"
    trap '__catch_interrupt "$cwd"' ERR INT TERM QUIT
    home="$(builtin eval echo ~)"
    cd -- "$home"

    local selection key file
    selection=("$(FZF_DEFAULT_COMMAND="${FZF_DEFAULT_COMMAND}" \
	command fzf --height='40%' -0 --expect='ctrl-o,ctrl-x,ctrl-v,ctrl-t,ctrl-s,alt-c')")
    key="$(command head -1 <<< "$selection")"
    file="${home}/$(command head -2 <<< "$selection" | command tail -1)"

    [[ -d "$file" || ! -f "$file" ]] && { cd -- "$cwd"; return 1; }

    local _command
    _command="$(__build_edit_command -k "$key" -t "$EDITOR" -f "$file")"
    [[ $? -eq 1 ]] && return 1
    __ehc "$_command"

    [[ $key != 'alt-c' ]] && cd -- "$cwd"
    trap '' ERR INT TERM QUIT
}

# Press "Ctrl-x + Ctrl-n" to call __fzf_file_open_home() and cd to the selection.
builtin bind -x '"\C-_0": __fzf_file_open_home'
builtin bind '"\C-x\C-N": "\C-x0\C-_0\C-x1\C-x2\015"'

# Open a file, listing all files below the root directory.
__fzf_file_open_root() {
    local cwd
    cwd="$(pwd)"
    trap '__catch_interrupt "$cwd"' ERR INT TERM QUIT
    cd -- '/'

    local selection key file
    selection=("$(FZF_DEFAULT_COMMAND="${FZF_DEFAULT_COMMAND}" \
	command fzf --height='40%' -0 --expect='ctrl-o,ctrl-x,ctrl-v,ctrl-t,ctrl-s,alt-c')")
    key="$(command head -1 <<< "$selection")"
    file="/$(command head -2 <<< "$selection" | command tail -1)"

    [[ -d "$file" || ! -f "$file" ]] && { cd -- "$cwd"; return 1; }

    local _command
    _command="$(__build_edit_command -k "$key" -t "$EDITOR" -f "$file")"
    [[ $? -eq 1 ]] && return 1
    __ehc "$_command"

    [[ $key != 'alt-c' ]] && cd -- "$cwd"
    trap '' ERR INT TERM QUIT
}

# Press "Ctrl-x + /" to call __fzf_file_open_root() and open the selection.
builtin bind -x '"\C-_1": __fzf_file_open_root'
builtin bind '"\C-x/": "\C-x0\C-_1\C-x1\C-x2\015"'

# Recursively search the contents of files using Ripgrep and then open the file(s) to the
# selected line.
__fzf_grep() {
    local cwd
    cwd="$(pwd)"
    trap '__catch_interrupt "$cwd"' ERR INT TERM QUIT
    local dir=""
    if git rev-parse --show-toplevel &>/dev/null; then
	dir="$(git rev-parse --show-toplevel)"
	cd -- "$dir"
    fi

    local selection key
    selection=("$($FINDER | command fzf --height='40%' -0 --expect='ctrl-o,ctrl-x,ctrl-v,ctrl-t,ctrl-s,alt-c')")
    key="$(command head -1 <<< "$selection")"

    local awkcommand
    if command -v gawk &>/dev/null; then awkcommand='gawk'; else awkcommand='awk'; fi

    local tokens
    tokens="$(builtin echo "${selection[@]}" | command $awkcommand -F: '{ print $1, $2}')"

    local file line
    builtin read -r file line <<< "$(command head -2 <<< "$tokens" | command tail -1)"
    [[ $line =~ ^[0-9]+$ ]] || { builtin printf "%s\\n" "fzf-bindings: detected wrong line number format. Terminating."; return 1; }

    local path
    if [[ -n "$dir" ]]; then
	path="${dir}/${file}"
    else
	path="${file}"
    fi

    [[ -f "$path" ]] || { cd -- "$cwd"; return 1; }

    local _command
    _command="$(__build_edit_command -k "$key" -t "$EDITOR" -f "$path" -l "$line")"
    [[ $? -eq 1 ]] && return 1
    __ehc "$_command"

    [[ $key != 'alt-c' ]] && cd -- "$cwd"
    trap '' ERR INT TERM QUIT
}

# Press "Ctrl-x + Ctrl-f" to call __fzf_grep() and open the file of the selected line, at
# that line.
builtin bind -x '"\C-x4": __fzf_grep'
builtin bind '"\C-x\C-f": "\C-x0\C-x4\C-x1\C-x2\015"'

__build_cd_comand() {
    local key directory
    key="$1"
    directory="$2"

    local executable _return=0
    case $key in
	'ctrl-o' ) executable=""$OPENER" "$directory" &" ;;
	'ctrl-x' ) if [[ -n "$TMUX" ]]; then executable="tmux split-window -v -c ${directory}"; else _return=1; fi ;;
	'ctrl-v' ) if [[ -n "$TMUX" ]]; then executable="tmux split-window -h -c ${directory}"; else _return=1; fi ;;
	'ctrl-t' ) if [[ -n "$TMUX" ]]; then executable="tmux new-window -c ${directory}"; else _return=1; fi ;;
	* ) executable="cd -- "$directory"" ;;
    esac

    [[ $_return -eq 1 ]] && { builtin printf "%s\\n" "tmux: Tmux session not detected." 2>&1; return 1; }
    builtin echo "$executable"
}

# Lists all of the directories inside of a Git project.
__fzf_cd_git() {
    trap '__catch_interrupt' ERR INT TERM QUIT
    if git rev-parse --show-toplevel &>/dev/null; then
        local proot
	proot="$(git rev-parse --show-toplevel)"
    else
	builtin printf "%s\\n" "Git project not detected." 2>&1
	return 1
    fi

    local selection key dir
    selection=("$(command find -L "$proot" \( -path '*/\.*' -o -fstype dev -o -fstype proc \) -prune \
	-o -type d -print 2>/dev/null |
        command sed "s#${proot}/##" |
	command sed '1d; s#^\./##' |
	command fzf --height='40%' -0 --no-multi --expect='ctrl-o,ctrl-x,ctrl-v,ctrl-t')")
    key="$(command head -1 <<< "$selection")"
    dir="$(command head -2 <<< "$selection" | command tail -1)"

    local path
    path="${proot}/${dir}"
    [[ -d "$path" ]] || return 1

    local _command
    _command="$(__build_cd_comand "$key" "$path")"
    [[ $? -eq 1 ]] && return 1
    __ehc "$_command"

    trap '' ERR INT TERM QUIT
}

# When inside a Git project, press Alt-g to call __fzf_cd_git() and cd to the selection.
builtin bind -x '"\C-x5": __fzf_cd_git'
builtin bind '"\M-g": "\C-x0\C-x5\C-x1\C-x2\015"'

# Like Fzf's Alt-c default, this searches all directories from the current working
# directory recursively. The only difference is that this includes hidden directories.
__fzf_cd_all() {
    trap '__catch_interrupt' ERR INT TERM QUIT
    local selection key dir

    selection=("$(command find -L . \( -fstype dev -o -fstype proc \) -prune \
	-o -type d -print 2>/dev/null |
	command sed '1d; s#^\./##' |
	command fzf --height='40%' -0 --no-multi --expect='ctrl-o,ctrl-x,ctrl-v,ctrl-t')")
    key="$(command head -1 <<< "$selection")"
    dir="$(command head -2 <<< "$selection" | command tail -1)"

    path="$(pwd)/${dir}"
    [[ -d "$path" ]] || return 1

    local _command
    _command="$(__build_cd_comand "$key" "$path")"
    [[ $? -eq 1 ]] && return 1
    __ehc "$_command"

    trap '' ERR INT TERM QUIT
}

# Press Alt-c to call __fzf_cd_all() and cd to the selection.
builtin bind -x '"\C-_2": __fzf_cd_all'
builtin bind '"\M-C": "\C-x0\C-_2\C-x1\C-x2\015"'

# Lists all of the directories inside of the filesystem.
__fzf_cd_root() {
    trap '__catch_interrupt' ERR INT TERM QUIT
    local selection key dir
    selection=("$(command find -L / \( -fstype dev -o -fstype proc \) -prune \
	-o -type d -print 2>/dev/null |
	command sed '1d; s#^\./##' |
	command fzf --height='40%' -0 --no-multi --expect='ctrl-o,ctrl-x,ctrl-v,ctrl-t')")
    key="$(command head -1 <<< "$selection")"
    dir="$(command head -2 <<< "$selection" | command tail -1)"

    [[ -d "$dir" ]] || return 1

    local _command
    _command="$(__build_cd_comand "$key" "$dir")"
    [[ $? -eq 1 ]] && return 1
    __ehc "$_command"

    trap '' ERR INT TERM QUIT
}

# Press Alt-r to call __fzf_cd_root() and cd to the selection.
builtin bind -x '"\C-x6": __fzf_cd_root'
builtin bind '"\M-r": "\C-x0\C-x6\C-x1\C-x2\015"'

# cd to one of the selected parent directories from the working path.
__fzf_cd_parent() {
    trap '__catch_interrupt' ERR INT TERM QUIT
    declare directories=()
    get_parent_directories() {
	if [[ -d "$1" ]]; then directories+=("$1"); else return 1; fi

	if [[ $1 == '/' ]]; then
	    local directory
	    for directory in "${directories[@]}"; do builtin echo "$directory"; done
	else
	    get_parent_directories "$(dirname "$1")"
	fi
    }

    local selection key directory
    selection=("$(get_parent_directories "$(realpath "${1:-$PWD}")" |
	command fzf --height='40%' -0 --tac --no-multi --expect='ctrl-o,ctrl-x,ctrl-v,ctrl-t')")
    key="$(command head -1 <<< "$selection")"
    directory="$(command head -2 <<< "$selection" | command tail -1)"

    [[ -d "$directory" ]] || return 1

    local _command
    _command="$(__build_cd_comand "$key" "$directory")"
    [[ $? -eq 1 ]] && return 1
    __ehc "$_command"

    trap '' ERR INT TERM QUIT
}

# Press Alt-p to call __fzf_cd_parent() and cd to the selection.
builtin bind -x '"\C-x7": __fzf_cd_parent'
builtin bind '"\M-p": "\C-x0\C-x7\C-x1\C-x2\015"'

# Bookmark directories using pushd. Each terminal session will have it's own set of bookmarks.
bookmark() {
    # Return if the directory has already been bookmarked.
    if builtin dirs -v | command grep -v " 0" | command grep --quiet --no-messages "$(command pwd)"; then return 1; fi

    local _command
    if (( $# > 0)); then
	_command="builtin pushd "$@" 2>/dev/null && builtin dirs -v"
    else
	_command="builtin pushd . 2>/dev/null"
    fi

    __ehc "$_command"
}

# Press "Ctrl-x + Ctrl-b" to bookmark the current working directory.
builtin bind -x '"\C-x8": bookmark'
builtin bind '"\C-x\C-b": "\C-x0\C-x8\C-x1\C-x2\015"'

# Lists all of the bookmarked directories to jump to.
__fzf_goto_bookmark() {
    trap '__catch_interrupt' ERR INT TERM QUIT
    local awkcommand
    if command -v gawk &>/dev/null; then awkcommand='gawk'; else awkcommand='awk'; fi

    local selection home
    home="$(builtin eval echo ~)"
    selection=("$(builtin dirs -v |
	grep -v " 0" |
	command $awkcommand "{ gsub(/~/, \"${home}\"); print \$2 }" |
	command fzf --height='40%' -0 --no-multi --expect='ctrl-o,ctrl-x,ctrl-v,ctrl-t')")

    local key directory
    key="$(command head -1 <<< "$selection")"
    directory="$(command head -2 <<< "$selection" | command tail -1)"

    [[ -d "$directory" ]] || return 1

    local _command
    _command="$(__build_cd_comand "$key" "$directory")"
    [[ $? -eq 1 ]] && return 1
    __ehc "$_command"

    trap '' ERR INT TERM QUIT
}

# Press "Ctrl-x + Ctrl-g" to pull up a listing of the currently bookmarked directories to navigate to.
builtin bind -x '"\C-x9": __fzf_goto_bookmark'
builtin bind '"\C-x\C-g": "\C-x0\C-x9\C-x1\C-x2\015"'
