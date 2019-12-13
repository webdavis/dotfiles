#
# ~/.profile
#

[[ -f "${HOME}/.bashrc" ]] && \. ~/.bashrc

# Set i3 default to Alacritty.
export TERMINAL='/usr/bin/alacritty'

# Autoload pyenv.
eval "$(pyenv init -)"

# Use node site wide.
nvm use default
