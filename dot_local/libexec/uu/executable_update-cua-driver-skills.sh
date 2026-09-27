#!/usr/bin/env bash

set -euo pipefail

readonly exit_failed=1
readonly skill_pack="$HOME/.agents/skills/cua-driver"

cua_driver_is_installed() {
  command -v cua-driver >/dev/null 2>&1
}

skill_pack_fingerprint() {
  if [[ -d $skill_pack ]]; then
    find "$skill_pack/" -type f ! -name .DS_Store -exec shasum -a 256 {} + | sort | shasum -a 256 | cut -c 1-7
  fi
}

update_skill_pack() {
  cua-driver skills update </dev/null
}

main() {
  if ! cua_driver_is_installed; then
    return
  fi

  local old new output
  old="$(skill_pack_fingerprint)"
  if ! output="$(update_skill_pack 2>&1)"; then
    printf 'error[update-failed]: cua-driver could not update its skills:\n%s\n' "$output" >&2
    exit "$exit_failed"
  fi
  new="$(skill_pack_fingerprint)"
  if [[ -z $old && -n $new ]]; then
    printf 'cua-driver: added\n'
  elif [[ $old != "$new" ]]; then
    printf 'cua-driver: %s → %s\n' "$old" "$new"
  fi
}

main "$@"
