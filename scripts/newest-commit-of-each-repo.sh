#!/usr/bin/env bash

set -euo pipefail

readonly last_seen_directory="$HOME/.cache/cargo-git-tools/newest-commits"

last_seen_file_of() {
  local repo=$1
  printf '%s/%s' "$last_seen_directory" "${repo//\//-}"
}

newest_commit_on_github() {
  local repo=$1
  GIT_TERMINAL_PROMPT=0 git -c http.lowSpeedLimit=1000 -c http.lowSpeedTime=5 \
    ls-remote "https://github.com/$repo" HEAD 2>/dev/null | cut -f1
}

# An unreachable GitHub leaves the last commit seen in place, so an apply offline rebuilds nothing.
remember_newest_commit_of() {
  local repo=$1 newest
  newest="$(newest_commit_on_github "$repo")" || return 0
  [[ -n $newest ]] || return 0
  printf '%s\n' "$newest" >"$(last_seen_file_of "$repo")"
}

print_last_seen_commit_of() {
  local repo=$1 last_seen_file
  last_seen_file="$(last_seen_file_of "$repo")"
  if [[ -s $last_seen_file ]]; then
    printf '# newest commit of %s: %s\n' "$repo" "$(<"$last_seen_file")"
  else
    printf '# newest commit of %s: never seen\n' "$repo"
  fi
}

main() {
  local space_separated_repos=$1 repo
  local -a repos
  read -ra repos <<<"$space_separated_repos"
  mkdir -p "$last_seen_directory"
  for repo in "${repos[@]}"; do
    remember_newest_commit_of "$repo" &
  done
  wait
  for repo in "${repos[@]}"; do
    print_last_seen_commit_of "$repo"
  done
}

main "$@"
