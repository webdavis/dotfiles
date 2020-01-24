#
# ~/.bashrc
#
# This file is triggered when bash is started as a non-login shell in interactive mode.
# Only Bash specific settings should go in this file.
#

# Disallows third parties from accessing files and directories by default.
umask 077

# Append to PATH. {{{1
path_append() {
    [[ -d "$1" ]] || return 1
    case ":${PATH:=$1}:" in *:${1}:* ) : ;; * ) export PATH="${PATH}:${1}" ;; esac;
}

# Prepend to PATH. (Used to override system binaries.)
path_prepend() {
    [[ -d "$1" ]] || return 1
    case ":${PATH:=$1}:" in *:${1}:* ) : ;; * ) export PATH="${1}:${PATH}" ;; esac;
}

# Custom tools.
path_prepend "${HOME}/bin"

# If not running interactively, exit. {{{1
# This has to be after PATH additions so that i3 can use custom PATH.
[[ $- != *i* ]] && { printf "%s\\n" 'The shell must be run interactively. Terminating bashrc.'; return 1; }


# bashrc_local (Source host specific settings.) {{{1
[[ -s $HOME/.bashrc_local ]] && \. $HOME/.bashrc_local


# Environment variables {{{1
export SUDOEDITOR='/usr/bin/rvim'
export EDITOR="$(which nvim)"
export XDG_CONFIG_HOME="${HOME}/.config"
export XDG_DATA_HOME="${HOME}/.local/share"
export FIGNORE='.o'
export HISTCONTROL='ignoredups:ignoreboth'
export HISTFILESIZE='-1'
export HISTIGNORE='h:history:__projectrt'
export HISTSIZE=-1
export HISTTIMEFORMAT='%F %T %z '
export LANG='en_US.UTF-8'
export LC_ALL='en_US.UTF-8'
export MANPAGER="$(which nvim) -c 'set ft=man' -"
export TERM=screen-256color
export TERMINAL="$(which alacritty)"
export IGNOREOF=1
export TMUXP_CONFIGDIR="${HOME}/.tmuxp"
export BROWSER='/usr/bin/firefox-developer-edition'
export ANKI_NOHIGHDPI=1

# AWS default profile.
export AWS_PROFILE='default'

# Move WeeChat home out of home directory. There's too much going on there.
export WEECHAT_HOME="${HOME}/.config/weechat"

# Jave environment.
JSHELLEDITOR="$(which nvim)"
export JSHELLEDITOR
export MAVEN_OPTS='-Xmx1024m'

# Place Python projects in project directories.
export PIPENV_VENV_IN_PROJECT=1


# Shell settings {{{1
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

# Redraw when the consoles window size changes.
[[ -n "$DISPLAY" ]] && shopt -s checkwinsize

# Disable flow-control. For example, Ctrl+s suspends flow-control, and Ctrl+q resumes
# flow-control. An explanation is given here: http://stackoverflow.com/questions/791765.
stty -ixon

# Report the status of terminated background jobs immediately. (Only effective when job
# control is enabled.)
set -o notify

# Bash will not overwrite existing file with >, >&, and <>.
set -o noclobber


# rsync {{{1
rsync_path="$(which rsync)"
[[ -x "$rsync_path" ]] && path_append "$rsync_path"


# Launch gpg-agent. {{{1
GPG_TTY="$(tty)" && export GPG_TTY
[[ -z "$SSH_AUTH_SOCK" ]] && SSH_AUTH_SOCK="$(gpgconf --list-dirs agent-ssh-socket)" && export SSH_AUTH_SOCK
gpgconf --launch gpg-agent


# PS1 {{{1

# Colors. {{{2

darkcolor() { tput sgr0 && tput setaf "$@"; }
brightcolor() { tput sgr0 && tput setaf "$@"; }

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


# ASCIINEMA_REC {{{2
[[ -n "$ASCIINEMA_REC" ]] && asciinema_indicator='(rec)'


# jobcount {{{2
jobcount() {
    local processes
    processes="$(jobs -p | wc -l | tr -d 0)"
    if [[ $processes =~ [1-9] ]]; then
	printf "%s\\n" "${reset}:${lightred}${processes}${reset}"
    fi
}


# Append history lines from current session to the history file. {{{2
append_history() { history -a; }


# PROMPT_COMMAND executes as a command prior to issuing each primary prompt. {{{2
PROMPT_COMMAND='append_history;'


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
PS1="\${asciinema_indicator}"
PS1+="[+\${SHLVL}\$(jobcount)${reset} "
PS1+="\\u:"
PS1+="${MachineColor}\\W\\[\\033[00m\\]]"
PS1+="\\n\\[\\033[00m\\]\\$ "
export PS1


# Git prompt {{{2
if [[ -f $HOME/.bash-git-prompt/gitprompt.sh ]]; then
    PS1_BEGINNING="${reset}\${asciinema_status}[+\${SHLVL}\$(jobcount) \\u:${MachineColor}\\W${reset}]"
    GIT_PROMPT_ONLY_IN_REPO=1
    GIT_PROMPT_WITH_USERNAME_AND_REPO=1
    GIT_PROMPT_FETCH_REMOTE_STATUS=1
    GIT_PROMPT_IGNORE_SUBMODULES=0
    GIT_PROMPT_WITH_VIRTUAL_ENV=1
    GIT_PROMPT_SHOW_UPSTREAM=0
    GIT_PROMPT_SHOW_UNTRACKED_FILES=no # Values: no, normal or all.
    GIT_PROMPT_START_USER="${MachineColor}${PS1_BEGINNING}${reset}"
    GIT_PROMPT_START_ROOT="${GIT_PROMPT_START_USER}"
    GIT_PROMPT_END_USER="\n${ResetColor}$ "
    GIT_PROMPT_END_ROOT="\n${ResetColor}# "
    GIT_PROMPT_SHOW_CHANGED_FILES_COUNT=1
    GIT_PROMPT_THEME_FILE=$HOME/.git-prompt-colors.sh

    source $HOME/.bash-git-prompt/gitprompt.sh
fi


# terminal_title {{{2
# Called by the DEBUG signal to set the terminal title as the previously executed command.
terminal_title() { history 1 | awk '{ $1=$2=$3=$4=""; gsub(/^[[:space:]]*/, ""); print }'; }

# functrace ensures calls to DEBUG are inherited by subshells. However, it breaks rvm.
#set -o functrace
trap 'echo -ne "\\033]0;"$(terminal_title)"\\007";' DEBUG


# Sources {{{1
[[ -s $HOME/.bash_functions   ]] && \. $HOME/.bash_functions   # Useful functions.
[[ -s $HOME/.bash_bindings    ]] && \. $HOME/.bash_bindings    # Keyboard shortcuts.
[[ -s $HOME/.bash_aliases     ]] && \. $HOME/.bash_aliases     # Command aliases.
[[ -s $HOME/.bash_completions ]] && \. $HOME/.bash_completions # Command completion scripts.
[[ -s $HOME/.fzf_bindings     ]] && \. $HOME/.fzf_bindings     # Powerful Fzf bindings.
[[ -s $HOME/.docker_functions ]] && \. $HOME/.docker_functions # Functions that spin up docker machines.

# Add Google Cloud SDK to PATH.
[[ -f $HOME/workspaces/tools/google-cloud-sdk/path.bash.inc ]] && \. $HOME/workspaces/tools/google-cloud-sdk/path.bash.inc

# `gcloud` autocompletion.
[[ -f $HOME/workspaces/tools/google-cloud-sdk/completion.bash.inc ]] && \. $HOME/workspaces/tools/google-cloud-sdk/completion.bash.inc

# `heroku` autocomplete.
HEROKU_AC_BASH_SETUP_PATH=/home/stephen/.cache/heroku/autocomplete/bash_setup && test -f $HEROKU_AC_BASH_SETUP_PATH && source $HEROKU_AC_BASH_SETUP_PATH;

[ -f $HOME/.fzf.bash ] && source $HOME/.fzf.bash
