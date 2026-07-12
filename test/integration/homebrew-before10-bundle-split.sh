#!/usr/bin/env bash
#
# Fix 1 (Homebrew 6.x bundle split): run_onchange_before_10 must install and
# clean up in TWO separate brew invocations against one rendered Brewfile --
# `brew bundle --file=<f>` then `brew bundle cleanup --force --file=<f>` -- not
# the old single `brew bundle --cleanup` form. Homebrew 6.x made `--cleanup` on
# `brew bundle` a dry-run that exits 1 whenever anything would be removed, which
# aborts the apply under set -e; removal is now its own `brew bundle cleanup`.
#
# Integration test: render the real chezmoiscript, run it with brew (and the
# ecosystem tools) stubbed at the boundary, and read back the captured brew
# argv. HOMEBREW_PREFIX points every brew/uv/volta call at the stub dir.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl"

fail() {
  printf 'homebrew-before10-bundle-split: FAIL -- %s\n' "$*" >&2
  exit 1
}

command -v chezmoi >/dev/null 2>&1 || {
  printf 'SKIP: chezmoi not on PATH; cannot render before_10\n'
  exit 0
}
[[ -f $SCRIPT ]] || fail "missing template: $SCRIPT"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Render the darwin-only script (scratch HOME, CI=1 -- same mechanics as the
# treefmt rendered-template lint). Empty render == non-darwin host: skip.
rendered="$work/rendered.sh"
render_home="$(mktemp -d)"
HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$SCRIPT" >"$rendered" || fail "chezmoi failed to render $SCRIPT"
rm -rf "$render_home"
if [[ ! -s $rendered ]]; then
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi

# Stub dir: brew logs its full argv (one line per call) and exits 0, except
# `list` exits non-zero so the multiplexer-present probe reports absent and the
# cleanup runs. uv/npm/volta/mas stubs exit 0 so the whole script completes.
prefix="$work/prefix"
mkdir -p "$prefix/bin"
brew_log="$work/brew-argv.log"
cat >"$prefix/bin/brew" <<MOCK
#!/usr/bin/env bash
printf '%s\n' "\$*" >>"$brew_log"
[[ \${1:-} == list ]] && exit 1
exit 0
MOCK
for tool in uv npm volta mas; do
  cat >"$prefix/bin/$tool" <<'MOCK'
#!/usr/bin/env bash
exit 0
MOCK
done
chmod +x "$prefix/bin/"*

: >"$brew_log"
HOMEBREW_PREFIX="$prefix" PATH="$prefix/bin:$PATH" bash "$rendered" >/dev/null 2>&1 ||
  fail "rendered before_10 exited non-zero under stubs"

[[ -s $brew_log ]] || fail "no brew invocations captured"

# The old single-command form must be gone.
if grep -qE '(^|[[:space:]])bundle[[:space:]]+--cleanup' "$brew_log"; then
  printf 'captured brew argv:\n' >&2
  cat "$brew_log" >&2
  fail "old 'brew bundle --cleanup' form still present (6.x split not applied)"
fi

install_line="$(grep -E '^bundle --file=' "$brew_log" | head -1 || true)"
cleanup_line="$(grep -E '^bundle cleanup --force --file=' "$brew_log" | head -1 || true)"
[[ -n $install_line ]] || fail "no 'brew bundle --file=<f>' install call captured"
[[ -n $cleanup_line ]] || fail "no 'brew bundle cleanup --force --file=<f>' call captured"

install_file="${install_line#bundle --file=}"
cleanup_file="${cleanup_line##*--file=}"
[[ $install_file == "$cleanup_file" ]] ||
  fail "install and cleanup ran against different Brewfiles ($install_file vs $cleanup_file)"

printf 'homebrew-before10-bundle-split: OK (install then cleanup --force against one Brewfile)\n'
