#!/usr/bin/env bash

set -euo pipefail

dotfiles_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cli_print_style_lib="$dotfiles_dir/.chezmoitemplates/cli-print-style-lib.sh.tmpl"

# shellcheck source=.chezmoitemplates/cli-print-style-lib.sh.tmpl
source "$cli_print_style_lib"

nixos_nix_installer_url="https://artifacts.nixos.org/nix-installer"
default_profile_nix="/nix/var/nix/profiles/default/bin/nix"

nix_is_installed() {
  [[ -x $default_profile_nix ]]
}

install_nix_unattended_with_flakes() {
  curl -sSfL "$nixos_nix_installer_url" | sh -s -- install --no-confirm --enable-flakes
}

main() {
  report_section 'nix' 'NixOS nix-installer'

  if nix_is_installed; then
    report_line "already installed, skipping."
    return
  fi

  report_line "installing..."
  install_nix_unattended_with_flakes
  local nix_version
  nix_version="$("$default_profile_nix" --version)"
  report_line "installed $nix_version."
}

main "$@"
