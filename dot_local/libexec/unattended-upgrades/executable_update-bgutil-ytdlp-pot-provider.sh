#!/bin/bash

set -euo pipefail

POT_REPO="${HOME}/.local/share/yt-dlp/bgutil-ytdlp-pot-provider"
POT_SERVER="${POT_REPO}/server"
POT_API="https://api.github.com/repos/Brainicism/bgutil-ytdlp-pot-provider/releases/latest"
POT_PLUGIN_URL="https://github.com/Brainicism/bgutil-ytdlp-pot-provider/releases/latest/download/bgutil-ytdlp-pot-provider.zip"
PLUGIN_ZIP="${HOME}/.config/yt-dlp/plugins/bgutil-ytdlp-pot-provider.zip"
LAUNCHAGENT_LABEL="com.webdavis.yt-dlp-pot-provider"
LAUNCHAGENT_DOMAIN="gui/$(id -u)"

if [[ ! -d ${POT_REPO} ]]; then
  printf 'bgutil provider is not installed; deferring update.\n' >&2
  exit 75
fi

fetch_latest_pot_tag() {
  curl -sf "${POT_API}" | jq -r .tag_name 2>/dev/null
}

current_pot_tag() {
  git -C "${POT_REPO}" describe --tags --exact-match 2>/dev/null
}

download_plugin() {
  local tmpfile
  tmpfile="$(mktemp)"
  if curl --proto '=https' --tlsv1.2 -L --fail --retry 3 -s \
    -o "${tmpfile}" "${POT_PLUGIN_URL}"; then
    mkdir -p "$(dirname "${PLUGIN_ZIP}")"
    mv "${tmpfile}" "${PLUGIN_ZIP}"
  else
    rm -f "${tmpfile}"
    return 1
  fi
}

build_pot_server() {
  (cd "${POT_SERVER}" && deno install --allow-scripts=npm:canvas,npm:@swc/core --frozen)
}

latest="$(fetch_latest_pot_tag)"
current="$(current_pot_tag || true)"

if [[ -z ${latest} ]]; then
  printf 'could not determine the latest bgutil provider release.\n' >&2
  exit 1
fi

if [[ ${current} == "${latest}" ]]; then
  printf 'bgutil provider is already at %s.\n' "${latest}"
  exit 0
fi

printf 'updating bgutil provider from %s to %s.\n' "${current:-unknown}" "${latest}"
git -C "${POT_REPO}" fetch --tags origin
git -C "${POT_REPO}" checkout "${latest}"
build_pot_server
download_plugin
launchctl kickstart -k "${LAUNCHAGENT_DOMAIN}/${LAUNCHAGENT_LABEL}"
printf 'bgutil provider updated to %s.\n' "${latest}"
