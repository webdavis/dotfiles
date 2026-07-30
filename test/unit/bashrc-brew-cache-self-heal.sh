#!/usr/bin/env bash
#
# The brew-shellenv cache self-heal in the rendered ~/.bashrc.
#
# This block is the ONLY automatic writer of the cache that ~/.bashrc sources for
# Homebrew's PATH. `chezmoi apply --exclude=templates` (what `just a` runs, and
# what CLAUDE.md requires of automation) does not run templated scripts, so the
# old apply-time regen script only fired on a rare full interactive apply and was
# removed. Deleting or weakening this block therefore has no backstop, and every
# property below must fail the suite if it disappears:
#
#   1. the block exists at all
#   2. it names the same cache file ~/.bashrc sources
#   3. it regenerates when Homebrew's shellenv GENERATOR is newer than the cache
#      (which also covers a missing cache: `-nt` is true when its right-hand
#      operand does not exist)
#   4. the guard forks NOTHING: it is stat tests only, on every interactive shell
#   5. regeneration runs the DEPLOYED writer, not a second copy of the write
#   6. regeneration is detached (`( ... & )`) and logged, so a fresh host sees no
#      job-control noise and no error text at the prompt
#
# Unit test: render dot_bashrc.tmpl and inspect the block. Darwin-only, because
# the block lives inside the template's `{{ if eq .chezmoi.os "darwin" }}` guard.
#
# Almost every needle below is a literal fragment of the rendered shell source
# being inspected, so single quotes and unexpanded `$` are the point of the file.
# shellcheck disable=SC2016
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/dot_bashrc.tmpl"
WRITER_SOURCE="$REPO_ROOT/dot_local/bin/executable_brew-shellenv-cache-refresh.sh"
WRITER_TARGET_PATH='.local/bin/brew-shellenv-cache-refresh.sh'
CACHE_PATH_EXPRESSION='${XDG_CACHE_HOME:-$HOME/.cache}/brew-shellenv.sh'
DELETED_APPLY_TIME_SCRIPT='run_after_44-cache-brew-shellenv'

fail() {
  printf 'bashrc-brew-cache-self-heal: FAIL -- %s\n' "$*" >&2
  exit 1
}

assert_contains() {
  local haystack="$1" needle="$2" description="$3"
  grep -qF -- "$needle" <<<"$haystack" || fail "$description (missing '$needle')"
}

# Refute helper: a bare `! grep` only decides a test in final position under
# `set -e`, so negative assertions go through this.
refute_contains() {
  local haystack="$1" needle="$2" description="$3"
  if grep -qF -- "$needle" <<<"$haystack"; then
    fail "$description (found '$needle')"
  fi
}

if [[ "$(uname -s)" != Darwin ]]; then
  printf 'bashrc-brew-cache-self-heal: SKIP (block is darwin-only; host is %s)\n' "$(uname -s)"
  exit 0
fi
command -v chezmoi >/dev/null 2>&1 || {
  printf 'SKIP: chezmoi not on PATH; cannot render dot_bashrc.tmpl\n'
  exit 0
}
[[ -f $TEMPLATE ]] || fail "missing template: $TEMPLATE"
[[ -x $WRITER_SOURCE ]] || fail "missing deployed writer source: $WRITER_SOURCE"

home="$(mktemp -d)"
trap 'rm -rf "$home"' EXIT
rendered="$(HOME="$home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$TEMPLATE")" ||
  fail 'chezmoi failed to render dot_bashrc.tmpl'

# 1. The block, sliced between its first assignment and its closing `unset`.
block="$(awk '/^[[:space:]]*__be_cache=/,/^[[:space:]]*unset __be_cache/' <<<"$rendered")"
[[ -n ${block//[[:space:]]/} ]] ||
  fail 'the brew-shellenv cache self-heal block is gone from the rendered ~/.bashrc'

# Assert on CODE, not on the comments that explain it.
block_code="$(grep -vE '^[[:space:]]*#' <<<"$block")"

# The guard header: everything from `if` through the line that ends in `then`.
guard="$(awk '/^[[:space:]]*if /,/then[[:space:]]*$/' <<<"$block_code")"
[[ -n ${guard//[[:space:]]/} ]] || fail 'the self-heal block has no if-guard'

# 2. Same cache file ~/.bashrc sources earlier in the same file.
assert_contains "$block_code" "__be_cache=\"$CACHE_PATH_EXPRESSION\"" \
  'self-heal does not point at the cache path ~/.bashrc sources'
assert_contains "$rendered" "\"$CACHE_PATH_EXPRESSION\"" \
  'the sourced brew cache path and the self-heal cache path disagree'

# 3. Generator-staleness term (also the missing-cache case).
assert_contains "$guard" '$__be_gen -nt $__be_cache' \
  'guard no longer regenerates when the shellenv generator is newer than the cache'

# 4. No fork in the guard: it runs on every interactive shell.
refute_contains "$guard" '$(' \
  'guard contains a command substitution, so it forks on every interactive shell'
refute_contains "$guard" '`' \
  'guard contains a backtick substitution, so it forks on every interactive shell'

# 5. Regeneration runs the DEPLOYED writer, not an inlined copy of the write.
assert_contains "$block_code" "\$HOME/$WRITER_TARGET_PATH" \
  'self-heal does not run the deployed brew-shellenv cache writer'
assert_contains "$block_code" '"$__be_writer"' \
  'self-heal never invokes the writer it resolved'
for inlined in 'mktemp' 'shellenv >' 'command mv'; do
  refute_contains "$block_code" "$inlined" \
    'self-heal still inlines its own copy of the atomic write'
done

# 6. Detached and logged.
assert_contains "$block_code" '&)' \
  'regeneration is not launched in a detached ( ... & ) subshell, so the prompt shows job noise'
assert_contains "$block_code" '>>"$__be_log" 2>&1' \
  'regeneration output is not redirected to the log, so errors reach the terminal'
assert_contains "$guard" 'mkdir -p "${__be_log%/*}"' \
  'the guard does not create the log dir before the redirect that needs it'

# The apply-time regen script is gone; nothing may still point at it.
refute_contains "$rendered" "$DELETED_APPLY_TIME_SCRIPT" \
  'rendered ~/.bashrc still references the deleted apply-time regen script'

printf 'bashrc-brew-cache-self-heal: OK (fork-free generator-staleness guard; detached, logged, deployed writer)\n'
