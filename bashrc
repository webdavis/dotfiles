#
# ~/.bashrc
#

# Set the umask, disallowing third parties from accessing files and directories by
# default.
umask 077

# Append to PATH. {{{1
path_append() {
    case "${PATH}:" in
	*":${1}:"* ) : ;;
	* ) PATH="${PATH}:${1}" && export PATH ;;
    esac
}

# Prepend to PATH.
path_prepend() {
    case ":${PATH}" in
	*":${1}:"* ) : ;;
	* ) PATH="${1}:${PATH}" && export PATH ;;
    esac
}

# Custom tools -- only the files that are executable will be able to be run. Tmux is among
# the included commands. See https://github.com/tmux/tmux/releases for updates.
path_prepend "${HOME}/bin"

# Add local Anki to PATH.
export ANKI_NOHIGHDPI=1
path_append "${HOME}/workspaces/tools/anki-2.1.15-linux-amd64/bin"

# If not running interactively, exit. {{{2
# This has to be after PATH additions so that i3 can use custom PATH.
[[ $- != *i* ]] && { printf "%s\\n" 'The shell must be run interactively. Terminating bashrc.'; return 1; }


# Colors. {{{1

darkcolor() {
    tput sgr0 && tput setaf "$@"
}

brightcolor() {
    tput sgr0 && tput setaf "$@"
}

reset="$(tput sgr0)"
bold="$(tput bold)"
black="$(darkcolor 234 234 234)"
darkred="$(darkcolor 1 1 1)"
darkgreen="$(darkcolor 2 2 2)"
darkyellow="$(darkcolor 3 3 3)"
darkblue="$(darkcolor 4 4 4)"
darkmagenta="$(darkcolor 5 5 5)"
darkcyan="$(darkcolor 6 6 6)"
white="$(darkcolor 7 7 7)"
darkgray="$(brightcolor 244 244 244)"
lightred="$(brightcolor 9 9 9)"
lightgreen="$(brightcolor 10 10 10)"
lightyellow="$(brightcolor 11 11 11)"
lightblue="$(brightcolor 12 12 12)"
lightmagenta="$(brightcolor 13 13 13)"
lightcyan="$(brightcolor 14 14 14)"
lightgray="$(brightcolor 15 15 15)"
separator="$reset"


# Environment variables. {{{1
export BROWSER='/usr/bin/firefox-developer-edition'
# Just a security precaution.
[[ "$(command -v nvim.appimage)" == '/usr/local/bin/nvim.appimage' ]] && export EDITOR='nvim.appimage'
export SUDOEDITOR='/usr/bin/rvim'
export FIGNORE='.o'
export HISTCONTROL='ignoredups:ignoreboth'
export HISTFILESIZE='-1'
export HISTIGNORE='h:history:__projectrt'
export HISTSIZE=-1
export HISTTIMEFORMAT='%F %T %z '
export LANG='en_US.UTF-8'
export LC_ALL='en_US.UTF-8'
export MANPAGER="/usr/local/bin/nvim.appimage -c 'set ft=man' -"
export TERMINAL='/usr/bin/alacritty'
export TERM=screen-256color
export XDG_CONFIG_HOME="${HOME}/.config"
export XDG_DATA_HOME="${HOME}/.local/share"
export IGNOREOF=1
export TMUXP_CONFIGDIR="${HOME}/.tmuxp"

# Move WeeChat home out of home directory. There's too much going on there.
export WEECHAT_HOME="${HOME}/.config/weechat"

# AWS default profile.
export AWS_PROFILE='default'

shopt -s  force_fignore # Files with suffix from FIGNORE are ignored.
shopt -s        extglob # Pattern matching during pathname expansion enabled.
shopt -s       globstar # ** match files during pathname expansion.
shopt -s        dotglob # Pathname expansion inlcudes hidden files.
shopt -s     nocaseglob # Provides case insensitive pattern matching during pathname expansion.
shopt -s       dirspell # Corrects directory spelling during expansion.
shopt -s        cdspell # Autocorrects typos in path names when using `cd`.
shopt -s      checkjobs # List job state before exiting shell.
shopt -s expand_aliases # Aliases are expanded using TAB.
shopt -s        cmdhist # Lists multiline commands as one line in history.
shopt -s     histappend # Append to .bash_history.
shopt -s   hostcomplete # Attempt hostname completion.

# redraw when the consoles window size changes.
[[ -n "$DISPLAY" ]] && shopt -s checkwinsize

# Disable flow-control. For example, Ctrl+s suspends flow-control, and Ctrl+q resumes
# flow-control. An explanation is given here: http://stackoverflow.com/questions/791765.
stty -ixon

# Report the status of terminated background jobs immediately. (Only effective when job
# control is enabled.)
set -o notify

# Bash will not overwrite existing file with >, >&, and <>.
set -o noclobber


# PATH setup {{{1

[[ -x '/usr/bin/rsync' ]] && path_append '/usr/bin/rsync'


# Launch gpg-agent. {{{2
GPG_TTY="$(tty)" && export GPG_TTY
[[ -z "$SSH_AUTH_SOCK" ]] &&
    SSH_AUTH_SOCK="$(gpgconf --list-dirs agent-ssh-socket)" &&
    export SSH_AUTH_SOCK
gpgconf --launch gpg-agent


# Git configuration. {{{2

export GIT_EDITOR="${EDITOR:-vim}"
export GIT_AUTHOR_NAME='webdavis'
GIT_AUTHOR_DATE="$(date "+%A %F %T %z")" && export GIT_AUTHOR_DATE

# [[ -s '/usr/share/git/completion/git-prompt.sh' ]] && \. '/usr/share/git/completion/git-prompt.sh'

# Indicates difference between HEAD and its upstream using (<,>,<>,=).
export GIT_PS1_SHOWUPSTREAM='auto'

# Staged(+)/unstaged(-) indicators. For large repositories (e.g. Linux Kernel), set this
# to false in the local gitconfig.
export GIT_PS1_SHOWDIRTYSTATE='true'

# Stashed($) indicator.
export GIT_PS1_SHOWSTASHSTATE='true'

# Show untracked files.
export GIT_PS1_SHOWUNTRACKEDFILES='true'

# Show the branch name.
export GIT_PS1_DESCRIBE_STYLE='branch'

# Git auto completion. Download from https://github.com/git/git/tree/master/contrib/completion.
[[ -s '/usr/share/git/completion/git-completion.bash' ]] && \. '/usr/share/git/completion/git-completion.bash'


# Haskell's Stack, a Haskell version installer. {{{2
[[ -d "${HOME}/workspaces/tools/stack-1.9.3-linux-x86_64" ]] &&
    path_prepend "${HOME}/workspaces/tools/stack-1.9.3-linux-x86_64"


# Rust's Cargo, a Rust package manager. {{{2
[[ -d "${HOME}/.cargo/bin" ]] && path_prepend "${HOME}/.cargo/bin"


# Ripgrep, a Rust powered grep like search tool. {{{2
path_prepend "${HOME}/workspaces/tools/ripgrep/target/release"


# Java configuration. {{{2
# export JAVA_HOME="${HOME}/workspaces/tools/jdk1.8.0_202"
export JAVA_HOME="${HOME}/workspaces/tools/jdk-11.0.2"
path_append "${JAVA_HOME}/bin"

# Path to JetBrains` Intellij IDEA IDE.
path_append "${HOME}/workspaces/tools/idea-IC-191.6707.61/bin"

# Path to Eclipse IDE.
# path_append "~/workspaces/tools/eclipse-jee-2019-09_R_4.13.0-linux-gtk-x86_64/"

# Configure default jshell editor.
export JSHELLEDITOR='/usr/local/bin/nvim.appimage'


# Spring Boot CLI {{{2
spring_boot_cli="${HOME}/workspaces/tools/spring-2.2.1.RELEASE"
path_append "${spring_boot_cli}/bin"

[[ -f "${spring_boot_cli}/shell-completion/bash/spring" ]] && \. "${spring_boot_cli}/shell-completion/bash/spring"


# FlameGraph configuration. {{{2

# Connects to a running a JVM process and exports a map file which can be used by perf to
# generate the stack trace with the actual Java method names.
path_append "${HOME}/workspaces/tools/perf-map-agent/bin"

path_append "${HOME}/workspaces/tools/FlameGraph/"


# Apache Maven configuration. {{{2
export MAVEN_OPTS='-Xmx1024m'
path_append "${HOME}/workspaces/tools/apache-maven-3.6.0/bin"
[[ -f "${HOME}/workspaces/tools/maven-bash-completion/bash_completion.bash" ]] &&
    \. "${HOME}/workspaces/tools/maven-bash-completion/bash_completion.bash"


# Gradle configuration. {{{2
path_append "${HOME}/workspaces/tools/gradle-5.3.1"


# Python's Pew {{{2

# virtualenv location.
path_prepend "${HOME}/.local/bin"

# Place Python projects in project directories.
export PIPENV_VENV_IN_PROJECT=1


# _pip_completion {{{2
_pip_completion() {
    COMPREPLY=("$(COMP_WORDS="${COMP_WORDS[*]}" COMP_CWORD=${COMP_CWORD} PIP_AUTO_COMPLETE=1 ${1})")
}
complete -o default -F _pip_completion pip


# jobcount {{{2
jobcount() {
    local processes
    processes="$(jobs -p | wc -l | tr -d 0)"
    if [[ $processes =~ [1-9] ]]; then
	printf "%s\\n" "${separator}:${lightred}${processes}"
    fi
}


# Yarn {{{2

# Yarn manages Node/JavaScript packages.
# Recommend installing using the "Manual Install via tarball" instructions at:
# https://yarnpkg.com/lang/en/docs/install/#alternatives-stable
YARN_DIR="$HOME/workspaces/tools/yarn-v1.15.2/bin"
[[ -d "$YARN_DIR" ]] && export YARN_DIR && path_prepend "$YARN_DIR"


# nvm (Node Version Manager) {{{2
# Manages Node.js versions/installs. Recommend installing via:
# https://github.com/creationix/nvm#manual-install
#
# I Couldn't get "nvm.sh --no-use" to work so implemented this work around for lazy loading.
# For an in depth discussion checkout: https://github.com/creationix/nvm/issues/782
NVM_DIR="${HOME}/workspaces/tools/nvm"
if [[ -s "${NVM_DIR}/nvm.sh" ]] && [[ ! $(type -t _nvm_load) == 'function' ]]; then
    export NVM_DIR
    # Put all Node tools here.
    declare -a tools=('nvm' 'node' 'npm' 'yarn')

    nvm_load() {
	for executable in "${tools[@]}"; do
	    unalias "$executable"
	done

	[[ -s "${NVM_DIR}/nvm.sh" ]] && \. "${NVM_DIR}/nvm.sh"
	[[ -r "${NVM_DIR}/bash_completion" ]] && \. "${NVM_DIR}/bash_completion"

	unset tools
	unset -f nvm_load
    }

    for executable in "${tools[@]}"; do
	alias "$executable"="nvm_load && "$executable""
    done
fi

# Displays the active Node version: "(node/version)".
node_prompt() {
    node_version=""
    regex='node/v([0-9]+\.)+[0-9]/'
    [[ $PATH =~ $regex ]] && node_version="("${BASH_REMATCH%/}")"
}


# rvm {{{2

# Load rvm into a shell session *as a function*
# [[ -s "${HOME}/.rvm/scripts/rvm" ]] && \. "$HOME/.rvm/scripts/rvm"
# path_prepend "${HOME}/.rvm/gems/ruby-2.4.1/bin"

# Load rvm command completion. (This must be sourced after ~/.rvm/scripts/rvm.
# [[ -n "$rvm_path" && -r "${HOME}/.rvm/scripts/completion" ]] && \. "${HOME}/.rvm/scripts/completion"

# Call from the prompt to display the active rvm Gem set in the prompt.
# This slows down the bash prompt significantly.
# rvm_prompt() {
#     if rvm current | grep --quiet --no-messages '@'; then
# 	echo "($(~/.rvm/bin/rvm-prompt g)) "
#     fi
# }

# Add gems to path.
path_append "${HOME}/.gem/ruby/2.6.0/bin"


# PS1 {{{1

# Append history lines from current session to the history file. {{{2
append_history() {
    history -a
}


# PROMPT_COMMAND executes as a command prior to issuing each primary prompt. {{{2
PROMPT_COMMAND='append_history;node_prompt;'

# ASCIINEMA_REC {{{2
# Displays "(rec)" in the terminal prompt to indicate that asciinema is recording.
[[ -n "$ASCIINEMA_REC" ]] && asciinema_status='[rec] '


# Test Machine connection type and set the color. {{{2
if [[ -n "$SSH_CONNECTION" ]]; then
    # Connected via ssh (good).
    MachineColor="${lightgreen}${bold}"
elif [[ $DISPLAY != *:* ]]; then
    # Connected using something other than ssh.
    MachineColor="${lightred}${bold}"
else
    # Local machine.
    MachineColor="${lightcyan}${bold}"
fi


# PS1 {{{2
PS1="\${asciinema_status}"
# PS1+="\$([ ! "$VIRTUAL_ENV" ] && rvm_prompt)"
PS1+="\${node_version}"
PS1+="[+\${SHLVL}\$(jobcount)${separator} "
PS1+="${reset}\\u${separator}:"
PS1+="${MachineColor}\\W\\[\\033[00m\\]]"
# PS1+="${cloud}"
# PS1+="\$(__git_ps1 ' [${bold}%s${reset}]')"
PS1+="\\n\\[\\033[00m\\]\\$ "
export PS1

# Sources {{{1

# Displays the active Python virtual environment in the terminal prompt.
[[ -x "${HOME}/.local/bin/pew" ]] && \. "$(pew shell_config)"

# Called by the DEBUG signal to set the terminal title as the previously executed command.
terminal_title() {
    history 1 | awk '{ $1=$2=$3=$4=""; gsub(/^[[:space:]]*/, ""); print }'
}

# Add rvm to path for scripting. Make sure this is the last path variable change.
# path_append "${HOME}/.rvm/bin"

# functrace ensures calls to DEBUG are inherited by subshells, however, it breaks rvm.
#set -o functrace
trap 'echo -ne "\\033]0;"$(terminal_title)"\\007";' DEBUG

# Abstract out functions to a separate file for easier management.
[[ -s "${HOME}/.bash_functions" ]] && \. "${HOME}/.bash_functions"

# Abstract out aliases to a separate file for easier management.
[[ -s "${HOME}/.bash_aliases" ]] && \. "${HOME}/.bash_aliases"

[[ -s "${HOME}/.bash_completions" ]] && \. "${HOME}/.bash_completions"

# The next line updates PATH for the Google Cloud SDK.
[[ -f "${HOME}/workspaces/tools/google-cloud-sdk/path.bash.inc" ]] &&
    \. "${HOME}/workspaces/tools/google-cloud-sdk/path.bash.inc"

# The next line enables shell command completion for gcloud.
[[ -f "${HOME}/workspaces/tools/google-cloud-sdk/completion.bash.inc" ]] &&
    \. "${HOME}/workspaces/tools/google-cloud-sdk/completion.bash.inc"

# heroku autocomplete setup
HEROKU_AC_BASH_SETUP_PATH=/home/stephen/.cache/heroku/autocomplete/bash_setup &&
    test -f $HEROKU_AC_BASH_SETUP_PATH && source $HEROKU_AC_BASH_SETUP_PATH;

[ -f ~/.fzf.bash ] && source ~/.fzf.bash
