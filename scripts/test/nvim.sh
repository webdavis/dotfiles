#!/usr/bin/env bash

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SPEC_RUNNER="${REPO_ROOT}/dot_config/nvim/tests/run.lua"

usage() {
  cat >&2 <<'USAGE'
usage: nvim.sh
USAGE
  exit 1
}

git_variable_names() {
  env | sed -n 's/^\(GIT_[A-Za-z0-9_]*\)=.*/\1/p'
}

# Specs build temporary repositories, which must not inherit the hook variables git exports.
unset_git_variables() {
  local name
  while IFS= read -r name; do
    unset "$name"
  done < <(git_variable_names)
}

run_specs() {
  nvim --headless --clean -l "$SPEC_RUNNER"
}

main() {
  [[ $# -eq 0 ]] || usage
  unset_git_variables
  run_specs
}

[[ ${BASH_SOURCE[0]} == "$0" ]] && main "$@"
