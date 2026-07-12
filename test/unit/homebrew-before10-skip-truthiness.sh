#!/usr/bin/env bash
#
# Fix 3: the SKIP_SYSTEM_PACKAGES guard in run_onchange_before_10 must skip the
# brew bundle sync ONLY for the literal value "1". A bare-truthiness guard
# (`{{ if env "SKIP_SYSTEM_PACKAGES" }}`) treats "0" and "false" as truthy and
# wrongly skips, so the correct form is `{{ if eq (env "...") "1" }}`.
#
# Unit test: render the template under a range of SKIP_SYSTEM_PACKAGES values
# and assert the skip block is present ONLY for "1".
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl"

fail() {
  printf 'homebrew-before10-skip-truthiness: FAIL -- %s\n' "$*" >&2
  exit 1
}

command -v chezmoi >/dev/null 2>&1 || {
  printf 'SKIP: chezmoi not on PATH; cannot render before_10\n'
  exit 0
}
[[ -f $SCRIPT ]] || fail "missing template: $SCRIPT"

# The skip block emits this sentinel comment; its presence == the bundle sync
# was skipped for this render.
SENTINEL='skipping the brew bundle sync'

# render_has_skip <value|__unset__> -> prints "yes"/"no". Empty render (non-darwin
# host) skips the whole test.
render_has_skip() {
  local value="$1"
  local home rendered
  home="$(mktemp -d)"
  rendered="$(
    if [[ $value == __unset__ ]]; then
      HOME="$home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$SCRIPT"
    else
      HOME="$home" CI=1 SKIP_SYSTEM_PACKAGES="$value" \
        chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$SCRIPT"
    fi
  )" || {
    rm -rf "$home"
    fail "chezmoi failed to render with SKIP_SYSTEM_PACKAGES=$value"
  }
  rm -rf "$home"
  if [[ -z ${rendered//[[:space:]]/} ]]; then
    printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
    exit 0
  fi
  if grep -qF "$SENTINEL" <<<"$rendered"; then printf 'yes'; else printf 'no'; fi
}

# Only "1" skips.
[[ "$(render_has_skip 1)" == yes ]] || fail 'SKIP_SYSTEM_PACKAGES=1 must skip the bundle sync'
# "0", "false", and unset must NOT skip (the bare-truthiness bug the fix closes).
[[ "$(render_has_skip 0)" == no ]] || fail 'SKIP_SYSTEM_PACKAGES=0 must NOT skip (bare-truthiness bug)'
[[ "$(render_has_skip false)" == no ]] || fail 'SKIP_SYSTEM_PACKAGES=false must NOT skip (bare-truthiness bug)'
[[ "$(render_has_skip __unset__)" == no ]] || fail 'unset SKIP_SYSTEM_PACKAGES must NOT skip'

printf 'homebrew-before10-skip-truthiness: OK (only the literal "1" skips; 0/false/unset run the sync)\n'
