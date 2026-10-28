#!/usr/bin/env bash

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BREWFILE="${REPO_ROOT}/Brewfile.dev"
# Pinned here and in CI, because mdformat rewrites Markdown.
MDFORMAT="mdformat==0.7.22"
MDFORMAT_PLUGINS=(
  "mdformat-gfm==0.4.1"
  "mdformat-gfm-alerts==2.0.0"
  "mdformat-frontmatter==2.0.8"
  "mdformat-footnote==0.1.1"
  "mdformat-tables==1.0.0"
  "mdformat-config==0.2.1"
)

usage() {
  cat >&2 <<'USAGE'
usage: setup.sh
USAGE
  exit 1
}

fail() {
  local message="$1"
  echo "error: ${message}" >&2
  exit 1
}

require_commands() {
  local command_name
  for command_name in "$@"; do
    command -v "$command_name" >/dev/null || fail "${command_name} is required but not installed"
  done
}

install_homebrew_tools() {
  brew bundle --file="$BREWFILE"
}

mdformat_plugin_arguments() {
  local plugin
  for plugin in "${MDFORMAT_PLUGINS[@]}"; do
    echo "--with"
    echo "$plugin"
  done
}

install_mdformat() {
  local plugin_arguments=()
  mapfile -t plugin_arguments < <(mdformat_plugin_arguments)
  uv tool install "$MDFORMAT" "${plugin_arguments[@]}"
}

main() {
  [[ $# -eq 0 ]] || usage
  require_commands brew uv
  install_homebrew_tools
  install_mdformat
}

[[ ${BASH_SOURCE[0]} == "$0" ]] && main "$@"
