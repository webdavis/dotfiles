#!/usr/bin/env bash
#
# Drift check for the cached `brew shellenv` output.
#
# ~/.bashrc sources ${XDG_CACHE_HOME:-~/.cache}/brew-shellenv.sh -- a verbatim copy
# of `brew shellenv` regenerated on every `chezmoi apply` by
# .chezmoiscripts/run_after_44-cache-brew-shellenv.sh.tmpl -- instead of running
# `eval "$(brew shellenv)"` on every shell. Because the cache is an exact copy of
# the generator output, sourcing it is identical to `eval "$(brew shellenv)"` only
# while the two match byte-for-byte. They diverge if Homebrew changes its shellenv
# output and no `chezmoi apply` has run since to regenerate the cache. This test
# asserts that byte-identity and prints fix instructions on drift.
#
# Run: `just test-brew-cache` (or ./test/brew-shellenv-cache-drift.sh). Also run
# at the end of the regen chezmoiscript as a post-apply sanity check.
set -euo pipefail

prefix='/opt/homebrew'
cache="${XDG_CACHE_HOME:-$HOME/.cache}/brew-shellenv.sh"

if [[ "$(uname -s)" != "Darwin" || ! -x "$prefix/bin/brew" ]]; then
  echo "brew-shellenv cache drift: skipped (not Darwin, or brew not installed)"
  exit 0
fi

if [[ ! -r $cache ]]; then
  cat >&2 <<EOF
brew-shellenv cache drift: FAIL -- cache file is missing.
  expected: $cache
  Fix: run a full \`chezmoi apply\` (NOT --exclude=templates) so that
  .chezmoiscripts/run_after_44-cache-brew-shellenv.sh.tmpl regenerates it.
EOF
  exit 1
fi

# The cache is a verbatim copy of `brew shellenv`, so it must be byte-identical to
# a fresh run. Command substitution strips trailing newlines from both sides, so a
# trailing-newline-only difference does not false-alarm.
live="$("$prefix/bin/brew" shellenv)"
cached="$(cat "$cache")"

if [[ $live == "$cached" ]]; then
  echo "brew-shellenv cache drift: OK -- cache matches live brew shellenv"
  exit 0
fi

cat >&2 <<EOF
brew-shellenv cache drift: FAIL -- the cache no longer matches \`brew shellenv\`.

Homebrew changed its shellenv output since the cache was generated, so ~/.bashrc
is sourcing a stale brew environment from:
  $cache

Diff (cached vs live brew shellenv):
EOF
diff <(printf '%s\n' "$cached") <(printf '%s\n' "$live") >&2 || true
cat >&2 <<EOF

  Fix: run a full \`chezmoi apply\` (NOT --exclude=templates) to regenerate the
  cache from the current brew shellenv. The regen script is
  .chezmoiscripts/run_after_44-cache-brew-shellenv.sh.tmpl.
EOF
exit 1
