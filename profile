#
# ~/.profile
#

[[ -f "~/.bashrc" ]] && \. ~/.bashrc

# Set i3 default to Alacritty.
export TERMINAL="$(which alacritty)"

# Autoload pyenv.
eval "$(pyenv init -)"

# Use node site wide.
nvm use default
