#!/usr/bin/env bash

set -euo pipefail

readonly needs_you_exit_code=100

cargo_is_installed() {
  command -v cargo >/dev/null 2>&1
}

registry_crates_in() {
  local install_list=$1
  awk 'NF == 2 && /^[^[:space:]]/ && $2 ~ /^v.+:$/ { print $1 "\t" substr($2, 2, length($2) - 2) }' <<<"$install_list"
}

newest_release_of() {
  local crate=$1
  local search_result
  search_result="$(cargo search "$crate" --limit 1)" || return 1
  awk -v crate="$crate" '$1 == crate && $2 == "=" { gsub(/"/, "", $3); print $3; exit }' <<<"$search_result"
}

release_is_newer() {
  local installed_version=$1 newest_version=$2
  [[ -n $newest_version && $newest_version != "$installed_version" ]]
}

main() {
  if ! cargo_is_installed; then
    printf 'error[missing-tool]: cargo is not installed.\n' >&2
    exit 1
  fi

  local install_list
  if ! install_list="$(cargo install --list)"; then
    printf 'error[list-failed]: cargo install --list failed.\n' >&2
    exit 1
  fi

  local crate installed_version newest_version
  local search_failures=0 newer_releases=0
  while IFS=$'\t' read -r crate installed_version <&3; do
    if ! newest_version="$(newest_release_of "$crate")"; then
      printf 'error[search-failed]: cargo search %s failed.\n' "$crate" >&2
      search_failures=$((search_failures + 1))
      continue
    fi
    if release_is_newer "$installed_version" "$newest_version"; then
      printf '%s: %s → %s\n' "$crate" "$installed_version" "$newest_version"
      newer_releases=$((newer_releases + 1))
    fi
  done 3< <(registry_crates_in "$install_list")

  if ((search_failures > 0)); then
    exit 1
  fi
  if ((newer_releases > 0)); then
    exit "$needs_you_exit_code"
  fi
}

main "$@"
