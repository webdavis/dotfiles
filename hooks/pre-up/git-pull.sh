#!/bin/sh

cd ~/.dotfiles-webdavis.git

# Only pull if there are no local changes.
if git diff-index --quiet HEAD --; then
    git pull
else
    >&2 echo "Local ~/.dotfiles-webdavis.git repo not clean; won't pull."
fi
