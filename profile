#
# ~/.profile
#
# This file works that same way that ~/.bash_profile does, except that it is not shell
# specific. Also, if ~/.bash_profile is not present in the users home directory, then this
# file will be sourced.
#

[[ -f $HOME/.bashrc ]] && \. $HOME/.bashrc

# Set i3 default to Alacritty.
export TERMINAL="$(which alacritty)"

# Autoload pyenv.
eval "$(pyenv init -)"

# Use node site wide.
nvm use default

[[ -s "$HOME/.rvm/scripts/rvm" ]] && source "$HOME/.rvm/scripts/rvm" # Load RVM into a shell session *as a function*
