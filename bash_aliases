# vi: set filetype=sh:

alias ls='ls --color=auto'
alias ll='ls --color=auto -AFhl'
alias tree='tree -C'
alias grep='grep --color=auto'
alias dmesg='dmesg --color=auto'
alias ..='cd ..'
alias .c='cd "${HOME}/.config"'
alias .d='cd "${HOME}/Documents"'
alias .dl='cd "${HOME}/Downloads"'
alias .p='cd "${HOME}/workspaces/projects"'
alias .t='cd "${HOME}/workspaces/tools"'
alias .v='cd "${HOME}/Videos"'
alias .n='cd "${HOME}/Documents/notes"'
alias .df='cd "${HOME}/.dotfiles-webdavis.git"'
alias path='echo "${PATH}" | sed "s/:/\\n/g"'
alias nvim='/usr/local/bin/nvim.appimage'
alias ebrc='/usr/local/bin/nvim.appimage "${HOME}/.bashrc"'
alias sbrc='\. "${HOME}/.bashrc"'
alias eba='/usr/local/bin/nvim.appimage "${HOME}/.bash_aliases"'
alias ebf='/usr/local/bin/nvim.appimage "${HOME}/.bash_functions"'
alias ei='/usr/local/bin/nvim.appimage "${HOME}/.config/i3/config"'
alias eib='/usr/local/bin/nvim.appimage "${HOME}/.config/i3blocks/config"'
alias evrc='/usr/local/bin/nvim.appimage "${HOME}/.vimrc"'
alias en='/usr/local/bin/nvim.appimage "${HOME}/.config/nvim/init.vim"'
alias et='/usr/local/bin/nvim.appimage "${HOME}/.tmux.conf"'
alias eg='/usr/local/bin/nvim.appimage "${HOME}/.gitconfig"'
alias o='xdg-open'
alias p='python'
alias g='git'
alias prn='pipenv run ${EDITOR}'
alias prp='pipenv run python'
alias vs='vagrant status'
alias vgs='vagrant global-status'
alias vu='vagrant up'
alias vsh='vagrant ssh'
alias vh='vagrant halt'
alias vd='vagrant destroy'
alias vdf='vagrant destroy --force'
alias vbl='vagrant box list'
alias vcs='vagrant cloud search'

# Apply aliases for this user to root user.
alias sudo='sudo '

# Ask for permission before moving or removing a file.
alias mv='mv --interactive'
alias rm='rm --interactive'

# Add color to manual pages, the respective settings are:
# - mb is begin bold
# - md is begin blink
# - me is reset bold/blink
# - so is begin reverse video
# - se is reset reverse video
# - us is begin underline
# - ue is reset underline
alias man=' \
    LESS="-R" \
    LESS_TERMCAP_mb="$(printf "\E[38;5;197m")" \
    LESS_TERMCAP_md="$(printf "\E[38;5;197m")" \
    LESS_TERMCAP_me="$(printf "\E[0m")" \
    LESS_TERMCAP_so="$(printf "\E[0;36;0;44m")" \
    LESS_TERMCAP_se="$(printf "\E[0m")" \
    LESS_TERMCAP_us="$(printf "\E[38;5;36m")" \
    LESS_TERMCAP_ue="$(printf "\E[0m")" \
	man "${@}" \
'

# Toggle debugging.
alias d='set -o nounset; set -o xtrace'
alias D='set +o nounset; set +o xtrace'

# Securely delete. Use this with caution.
alias srm='/usr/bin/shred --verbose --random-source=/dev/urandom --iterations=1 -u'

# `hub` provides GitHub specific commands.
alias git='/usr/bin/hub'

# This ensures that Tmux displays truecolors (24-bit colors) in terminals that support it.
alias tmux='env TERM=xterm-256color tmux'
