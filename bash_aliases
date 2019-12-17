# vi: set filetype=sh:

alias ls='ls --color=auto'
alias ll='ls --color=auto -AFhl'
alias tree='tree -C'
alias grep='grep --color=auto'
alias dmesg='dmesg --color=auto'
alias ..='cd ..'
alias .c='cd ~/.config'
alias .d='cd ~/Documents'
alias .dl='cd ~/Downloads'
alias .p='cd ~/workspaces/projects'
alias .to='cd ~/workspaces/tools'
alias .te='cd ~/workspaces/test'
alias .v='cd ~/Videos'
alias .n='cd ~/Documents/notes'
alias .df='cd ~/.dotfiles-webdavis.git'
alias path='echo "${PATH}" | sed "s/:/\\n/g"'
alias ebrc='"${EDITOR}" ~/.dotfiles-webdavis.git/bashrc'
alias sbrc='\. ~/.bashrc'
alias eba='"${EDITOR}" ~/.dotfiles-webdavis.git/bash_aliases'
alias ebf='"${EDITOR}" ~/.dotfiles-webdavis.git/bash_functions'
alias ebk='"${EDITOR}" ~/.dotfiles-webdavis.git/bash_bindings'
alias ebd='"${EDITOR}" ~/.dotfiles-webdavis.git/docker_functions'
alias ebz='"${EDITOR}" ~/.dotfiles-webdavis.git/fzf_bindings'
alias ei='${"EDITOR"} ~/.dotfiles-webdavis.git/config/i3/config'
alias eib='"${EDITOR}" ~/.dotfiles-webdavis.git/config/i3blocks/config'
alias evrc='"${EDITOR}" ~/.dotfiles-webdavis.git/vimrc'
alias en='"${EDITOR}" ~/.dotfiles-webdavis.git/config/nvim/init.vim'
alias et='"${EDITOR}" ~/.dotfiles-webdavis.git/tmux.conf'
alias eg='"${EDITOR}" ~/.dotfiles-webdavis.git/gitconfig'
alias eclipse="~/workspaces/tools/eclipse-jee-2019-09_R_4.13.0-linux-gtk-x86_64/eclipse"
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
