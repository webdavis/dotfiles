#!/usr/bin/env bash
# restore-age-key-secret-free: proves the age-key restore chezmoiscript renders a SECRET-FREE body.
# The private AGE-SECRET-KEY must be fetched at execution time (keepassxc-cli), never templated into the
# script, because chezmoi writes rendered scripts to temp executables and echoes them in verbose/diff runs.
#
# Two layers, ordered so a secret-bearing template is NEVER rendered:
#   1. SOURCE (always, incl. CI): the template contains no keepassxc-FUNCTION call with a string entry arg
#      and no .Password selector -- the only ways a KeePassXC secret reaches the rendered body. A config
#      read like `.chezmoi.config.keepassxc.database` (a non-secret path) is allowed and does NOT match.
#   2. RENDER (only when chezmoi exists AND layer 1 passed): render headless (CI=1) and assert the output
#      carries no real age private-key material (marker + long bech32 tail). Layer 1 gates this so the old
#      secret-bearing template can never be rendered by this test.
set -uo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root" || exit 1
fail() {
  echo "restore-age-key-secret-free: FAIL -- $1" >&2
  exit 1
}

# Locate the restore template by function, resilient to the run_once_/run_before_ prefix.
tmpl=""
for f in .chezmoiscripts/*restore-age-key*.sh.tmpl; do
  [[ -f $f ]] && tmpl="$f"
done
[[ -n $tmpl ]] || fail "no *restore-age-key*.sh.tmpl found under .chezmoiscripts/"

# Layer 1: no keepassxc-function call (function name followed by a quoted entry) and no .Password selector.
if grep -nE 'keepassxc(Attribute)?[[:space:]]+"' "$tmpl"; then
  fail "$tmpl calls the keepassxc template function -- the secret would render into the script body"
fi
if grep -nF '.Password' "$tmpl"; then
  fail "$tmpl selects .Password in a template action -- a KeePassXC secret would render into the body"
fi
echo "restore-age-key-secret-free: source has no render-time KeePassXC secret pull"

# Layer 2: render headless and confirm no real private key material landed in the body.
if ! command -v chezmoi >/dev/null 2>&1; then
  echo "restore-age-key-secret-free: render check SKIPPED (no chezmoi)"
  echo "restore-age-key-secret-free: OK (source only)"
  exit 0
fi
rendered="$(CI=1 chezmoi execute-template --no-tty <"$tmpl" 2>/dev/null || true)"
# A real age identity is the marker plus a long bech32 tail; split so this line cannot self-match.
if grep -qE 'AGE-SECRET-KEY-''(PQ-)?1[A-Z0-9]{40,}' <<<"$rendered"; then
  fail "the rendered $tmpl contains age private-key material"
fi
echo "restore-age-key-secret-free: OK (rendered body is secret-free)"
