#!/usr/bin/env bash
#
# `just brew-cache-refresh` must DELEGATE to the deployed writer
# (~/.local/bin/brew-shellenv-cache-refresh.sh), the same artifact ~/.bashrc's
# self-heal runs, and must not carry a second copy of the atomic
# mktemp/generate/rename sequence.
#
# The recipe used to reimplement that write, which is how it drifted: it shipped
# non-atomic and had to be fixed separately from the other writers. One
# implementation, two callers, no drift.
#
# Unit test: read the recipe body with `just --show` (parses the justfile, runs
# nothing) and assert what it names.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RECIPE='brew-cache-refresh'
DEPLOYED_WRITER='.local/bin/brew-shellenv-cache-refresh.sh'
REPO_SOURCE_WRITER='dot_local/bin/executable_brew-shellenv-cache-refresh.sh'

fail() {
  printf 'brew-cache-refresh-recipe: FAIL -- %s\n' "$*" >&2
  exit 1
}

# Refute helper: a bare `! grep` only decides a test in final position under
# `set -e`, so negative assertions go through this.
refute_contains() {
  local haystack="$1" needle="$2" description="$3"
  if grep -qF -- "$needle" <<<"$haystack"; then
    printf 'recipe body:\n%s\n' "$haystack" >&2
    fail "$description (found '$needle')"
  fi
}

command -v just >/dev/null 2>&1 || {
  printf 'SKIP: just not on PATH; cannot inspect the %s recipe\n' "$RECIPE"
  exit 0
}

body="$(just --justfile "$REPO_ROOT/justfile" --show "$RECIPE" 2>/dev/null)" ||
  fail "no $RECIPE recipe in the justfile"

grep -qF "$DEPLOYED_WRITER" <<<"$body" ||
  fail "recipe does not invoke the deployed ~/$DEPLOYED_WRITER"

refute_contains "$body" "$REPO_SOURCE_WRITER" \
  'recipe invokes the repo SOURCE copy instead of the deployed one'

# No second implementation of the write hiding in the recipe body.
for inlined in 'mktemp' 'shellenv >' 'shellenv >>'; do
  refute_contains "$body" "$inlined" \
    'recipe still reimplements the cache write instead of delegating to the writer'
done

printf 'brew-cache-refresh-recipe: OK (delegates to the deployed ~/%s)\n' "$DEPLOYED_WRITER"
