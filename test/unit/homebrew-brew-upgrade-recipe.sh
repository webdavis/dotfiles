#!/usr/bin/env bash
#
# Fix 2: `just brew-upgrade` must invoke the DEPLOYED helper
# (~/.local/bin/homebrew-weekly-upgrade.sh) -- the exact artifact launchd runs
# every Monday -- not the repo SOURCE copy (dot_local/bin/executable_...). An
# ad-hoc upgrade has to exercise what the LaunchAgent exercises.
#
# Unit test: read the recipe body with `just --show` (parses the justfile, runs
# nothing) and assert which path it names.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

fail() {
  printf 'homebrew-brew-upgrade-recipe: FAIL -- %s\n' "$*" >&2
  exit 1
}

command -v just >/dev/null 2>&1 || {
  printf 'SKIP: just not on PATH; cannot inspect the brew-upgrade recipe\n'
  exit 0
}

body="$(just --justfile "$REPO_ROOT/justfile" --show brew-upgrade 2>/dev/null)" ||
  fail "no brew-upgrade recipe in the justfile"

grep -qF '.local/bin/homebrew-weekly-upgrade.sh' <<<"$body" ||
  fail "recipe does not invoke the deployed ~/.local/bin/homebrew-weekly-upgrade.sh"

if grep -qF 'dot_local/bin/executable_homebrew-weekly-upgrade.sh' <<<"$body"; then
  printf 'recipe body:\n%s\n' "$body" >&2
  fail "recipe invokes the repo SOURCE copy instead of the deployed one"
fi

printf 'homebrew-brew-upgrade-recipe: OK (runs the deployed ~/.local/bin/homebrew-weekly-upgrade.sh)\n'
