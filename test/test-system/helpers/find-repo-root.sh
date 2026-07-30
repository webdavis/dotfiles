# shellcheck shell=bash
# find_repo_root -- print the repository root, failing closed when git cannot
# answer (rather than guessing a path). Sourced by the test-system suite; no main.

find_repo_root() {
  local root
  if ! root="$(git rev-parse --show-toplevel 2>/dev/null)"; then
    printf 'find_repo_root: not inside a git work tree\n' >&2
    return 1
  fi
  printf '%s\n' "$root"
}
