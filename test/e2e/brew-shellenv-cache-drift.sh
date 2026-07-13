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
# Run: `just test-brew-cache` (or ./test/e2e/brew-shellenv-cache-drift.sh). Also
# run at the end of the regen chezmoiscript as a post-apply sanity check.
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
  echo "  Generate it with 'just brew-cache-refresh' (or a full 'chezmoi apply')."
  exit 0
fi

# The cache is a verbatim copy of `brew shellenv` stdout (run_after_44, the
# `just brew-cache-refresh` recipe, and the bashrc self-heal all write it via
# `brew shellenv >tmp && mv tmp cache`), so it must be BYTE-identical to a fresh
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

  Fix: run \`just brew-cache-refresh\` to regenerate the cache now (or a full
  \`chezmoi apply\`, NOT --exclude=templates). The regen script is
  .chezmoiscripts/run_after_44-cache-brew-shellenv.sh.tmpl.
EOF
exit 1
