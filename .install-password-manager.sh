#!/bin/sh

# chezmoi runs this before every command that reads the source, and its exit status becomes theirs.
# So it warns and exits 0: a fresh Mac has no Homebrew until a later script installs it.

type keepassxc-cli >/dev/null 2>&1 && exit 0

os="$(uname -s)"

case "$os" in
  Darwin)
    if type brew >/dev/null 2>&1; then
      brew install --cask keepassxc ||
        echo "Warning: 'brew install --cask keepassxc' failed. Install KeePassXC by hand before applying any template that reads the vault." >&2
    else
      echo "Warning: KeePassXC is missing and Homebrew is not on PATH, so it cannot be installed here. Templates that read the vault will fail until it is installed." >&2
    fi
    ;;
  Linux)
    ;;
  *)
    # This platform has no bootstrap to protect, so it fails loudly.
    echo "Error: unsupported OS '$os'" >&2
    exit 1
    ;;
esac
