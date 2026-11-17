#!/usr/bin/env bash
# Prints the commit each given GitHub repo's tools should be built from, one "repo commit" line per repo.
# An unreachable repo reports the commit already installed, so being offline asks for no rebuild.

set -euo pipefail

readonly installed_crates_record="$HOME/.cargo/.crates2.json"

upstream_head() {
  local repo=$1
  GIT_TERMINAL_PROMPT=0 git -c http.lowSpeedLimit=1000 -c http.lowSpeedTime=5 \
    ls-remote "https://github.com/$repo" HEAD 2>/dev/null | awk 'NR == 1 {print $1}'
}

installed_head() {
  local repo=$1
  [[ -f $installed_crates_record ]] || return 0
  jq -r --arg source "(git+https://github.com/${repo}#" \
    '[.installs | keys[] | select(contains($source)) | split("#")[1][0:40]][0] // empty' \
    "$installed_crates_record"
}

head_to_build() {
  local repo=$1 head
  head="$(upstream_head "$repo" || true)"
  if [[ -z $head ]]; then
    head="$(installed_head "$repo")"
  fi
  printf '%s %s\n' "$repo" "${head:-unknown}"
}

main() {
  local repo
  {
    for repo in "$@"; do
      head_to_build "$repo" &
    done
    wait
  } | sort
}

main "$@"
