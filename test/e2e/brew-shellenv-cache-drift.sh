#!/usr/bin/env bash
#
# Drift check for the cached `brew shellenv` output.
#
# ~/.bashrc sources ${XDG_CACHE_HOME:-~/.cache}/brew-shellenv.sh -- a verbatim copy
# of `brew shellenv` written by ~/.local/bin/brew-shellenv-cache-refresh.sh --
# instead of running `eval "$(brew shellenv)"` on every shell. Because the cache is
# an exact copy of the generator output, sourcing it is identical to
# `eval "$(brew shellenv)"` only while the two match byte-for-byte. They diverge if
# Homebrew changes its shellenv output and no shell has self-healed the cache since.
# This test asserts that byte-identity and prints fix instructions on drift.
#
# This is also the SAFETY NET for the self-heal, which is the only automatic
# writer: if its staleness guard ever stops noticing an upstream change, this
# check is what reports the stale cache.
#
# Run: `just test-brew-cache` (or ./test/e2e/brew-shellenv-cache-drift.sh); it is
# part of `just test`.
set -euo pipefail

prefix='/opt/homebrew'
cache="${XDG_CACHE_HOME:-$HOME/.cache}/brew-shellenv.sh"

if [[ "$(uname -s)" != "Darwin" || ! -x "$prefix/bin/brew" ]]; then
  echo "brew-shellenv cache drift: skipped (not Darwin, or brew not installed)"
  exit 0
fi

if [[ ! -r $cache ]]; then
  # Not a failure: ~/.bashrc falls back to a live `eval "$(brew shellenv)"` when
  # the cache is absent (correct, just slower), so a missing cache must not block
  # commits. Skip with a hint to generate it for the fast path.
  echo "brew-shellenv cache drift: skipped -- cache not generated yet ($cache)."
  echo "  Generate it with 'just brew-cache-refresh', or start a new shell."
  exit 0
fi

# The cache is a verbatim copy of `brew shellenv` stdout: both callers (the bashrc
# self-heal and `just brew-cache-refresh`) run the one writer,
# ~/.local/bin/brew-shellenv-cache-refresh.sh, which does
# `brew shellenv >tmp && mv tmp cache`. So it must be BYTE-identical to a fresh
# run. cmp against the live stream directly: capturing both through command
# substitution would strip trailing newlines and hide a real drift the
# byte-identity invariant forbids.
if "$prefix/bin/brew" shellenv | cmp -s - "$cache"; then
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
diff "$cache" <("$prefix/bin/brew" shellenv) >&2 || true
cat >&2 <<EOF

  Fix: run \`just brew-cache-refresh\` to regenerate the cache now. A new
  interactive shell also heals it on its own; the writer both use is
  ~/.local/bin/brew-shellenv-cache-refresh.sh.
EOF
exit 1
