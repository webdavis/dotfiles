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

# The writer only reaches ~/.local/bin through `chezmoi apply`, so on any host
# that has not applied yet (a fresh machine, or this very branch before its first
# apply) the recipe runs a path that does not exist. Say so, instead of letting
# the shell report a bare "No such file or directory" for a path the reader has
# no reason to recognize. Driven with a HOME that deliberately has no writer.
undeployed_home="$(mktemp -d)"
trap 'rm -rf "$undeployed_home"' EXIT
recipe_status=0
recipe_output="$(HOME="$undeployed_home" just --justfile "$REPO_ROOT/justfile" \
  --working-directory "$REPO_ROOT" "$RECIPE" 2>&1)" || recipe_status=$?
((recipe_status != 0)) ||
  fail "$RECIPE exited 0 on a host where the writer is not deployed"
grep -qF 'is not deployed' <<<"$recipe_output" ||
  fail "$RECIPE did not report that the writer is missing (output: $recipe_output)"
grep -qF 'chezmoi apply' <<<"$recipe_output" ||
  fail "$RECIPE did not name the step that deploys the writer (output: $recipe_output)"

printf 'brew-cache-refresh-recipe: OK (delegates to the deployed ~/%s; names chezmoi apply when it is missing)\n' \
  "$DEPLOYED_WRITER"
