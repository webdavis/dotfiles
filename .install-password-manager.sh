#!/bin/sh

# chezmoi runs this as the `hooks.read-source-state.pre` command declared in
# .chezmoi.toml.tmpl, so it runs before EVERY chezmoi command that reads the
# source state, the read-only ones included (execute-template, managed, status,
# diff), and its exit status becomes that command's exit status.
#
# That makes it best-effort by construction: it installs KeePassXC when it can
# and reports why when it cannot. Failing instead takes the whole chezmoi CLI
# down with it. On a host with no Homebrew, `brew` is not found, the hook exits
# 127, and every template render fails for a reason that has nothing to do with
# the template. A fresh Mac is in exactly that state until
# .chezmoiscripts/run_once_before_00-install-homebrew.sh has run, and this hook
# runs before any script does, so aborting here would deadlock the bootstrap the
# hook exists to serve; the next `chezmoi apply` installs the cask through the
# Homebrew the first one laid down.
#
# The tradeoff: a template that reads the vault now fails at its own call site,
# naming itself, instead of every chezmoi command failing up front.

# Exit immediately if password-manager-binary is already in $PATH.
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
    # commands to install password-manager-binary on Linux
    ;;
  *)
    # No install branch exists for this platform, so there is no bootstrap to
    # protect and the error is the only signal the operator gets.
    echo "Error: unsupported OS '$os'" >&2
    exit 1
    ;;
esac
