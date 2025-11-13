#!/bin/sh

# Exit immediately if password-manager-binary is already in $PATH.
type keepassxc-cli >/dev/null 2>&1 && exit

os="$(uname -s)"

case "$os" in
  Darwin)
    brew install --cask keepassxc
    ;;
  Linux)
    # commands to install password-manager-binary on Linux
    ;;
  *)
    echo "Error: unsupported OS '$os'" >&2
    exit 1
    ;;
esac
