#!/usr/bin/env bash

set -euo pipefail

readonly exit_deferred=75
readonly exit_failure=1
readonly provider_checkout="$HOME/.local/share/yt-dlp/bgutil-ytdlp-pot-provider"
readonly provider_server_dir="$provider_checkout/server"
readonly latest_release_api_url="https://api.github.com/repos/Brainicism/bgutil-ytdlp-pot-provider/releases/latest"
readonly plugin_download_url="https://github.com/Brainicism/bgutil-ytdlp-pot-provider/releases/latest/download/bgutil-ytdlp-pot-provider.zip"
readonly installed_plugin_zip="$HOME/.config/yt-dlp/plugins/bgutil-ytdlp-pot-provider.zip"
readonly server_launch_agent_label="com.webdavis.yt-dlp-pot-provider"

provider_is_installed() {
  [[ -d $provider_checkout ]]
}

fetch_url() {
  local url=$1
  curl --proto '=https' --tlsv1.2 -L --fail --retry 3 -s "$url"
}

latest_release_tag() {
  fetch_url "$latest_release_api_url" | jq -r '.tag_name // empty' || true
}

installed_release_tag() {
  git -C "$provider_checkout" describe --tags --exact-match 2>/dev/null || true
}

check_out_release() {
  local tag=$1
  git -C "$provider_checkout" fetch --tags origin
  git -C "$provider_checkout" checkout "$tag"
}

build_server() {
  (cd "$provider_server_dir" && deno install --allow-scripts=npm:canvas,npm:@swc/core --frozen)
}

download_to_file() {
  local url=$1 destination=$2
  fetch_url "$url" >"$destination"
}

download_plugin() {
  local temp_file
  temp_file="$(mktemp)"
  if ! download_to_file "$plugin_download_url" "$temp_file"; then
    rm -f "$temp_file"
    return 1
  fi
  printf '%s' "$temp_file"
}

install_plugin() {
  local downloaded_plugin=$1
  mkdir -p "$(dirname "$installed_plugin_zip")"
  mv "$downloaded_plugin" "$installed_plugin_zip"
}

restart_server() {
  launchctl kickstart -k "gui/$(id -u)/$server_launch_agent_label"
}

main() {
  if ! provider_is_installed; then
    printf 'bgutil provider is not installed; deferring update.\n' >&2
    exit "$exit_deferred"
  fi

  local latest installed downloaded_plugin
  latest="$(latest_release_tag)"
  if [[ -z $latest ]]; then
    printf 'error[release-unknown]: could not determine the latest bgutil release.\n' >&2
    exit "$exit_failure"
  fi
  installed="$(installed_release_tag)"

  if [[ $installed == "$latest" ]]; then
    printf 'bgutil provider is already at %s.\n' "$latest"
    return
  fi

  printf 'updating bgutil provider from %s to %s.\n' "${installed:-unknown}" "$latest"
  check_out_release "$latest"
  build_server
  downloaded_plugin="$(download_plugin)"
  install_plugin "$downloaded_plugin"
  restart_server
  printf 'bgutil provider updated to %s.\n' "$latest"
}

main "$@"
