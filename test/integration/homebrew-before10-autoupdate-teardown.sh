#!/usr/bin/env bash
#
# Fix 1 (atomic domt4/autoupdate replacement): run_onchange_before_10 must NOT
# start the old domt4/autoupdate daily upgrader anymore (the Monday-noon weekly
# LaunchAgent supersedes it). Instead it must TEAR DOWN autoupdate, and that
# teardown must run BEFORE any `brew bundle cleanup` could untap domt4/autoupdate
# (once the tap leaves the manifest a cleanup removes it, and then the
# `brew autoupdate` subcommand is gone). So:
#   1. no rendered path may contain `autoupdate start`;
#   2. the teardown argv (`autoupdate stop`/`delete`) must precede the cleanup
#      argv in a stubbed run.
#
# Integration test: render the real chezmoiscript, run it with brew stubbed at
# the boundary (the stub reports domt4/autoupdate as tapped so the guarded
# teardown fires and logs its argv), and read back the captured brew argv.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl"

fail() {
  printf 'homebrew-before10-autoupdate-teardown: FAIL -- %s\n' "$*" >&2
  exit 1
}

command -v chezmoi >/dev/null 2>&1 || {
  printf 'SKIP: chezmoi not on PATH; cannot render before_10\n'
  exit 0
}
[[ -f $SCRIPT ]] || fail "missing template: $SCRIPT"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

rendered="$work/rendered.sh"
render_home="$(mktemp -d)"
HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$SCRIPT" >"$rendered" || fail "chezmoi failed to render $SCRIPT"
rm -rf "$render_home"
if [[ ! -s $rendered ]]; then
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi

# (1) The daily-upgrader STARTER must be gone from the rendered script entirely.
if grep -qE 'autoupdate[[:space:]]+start' "$rendered"; then
  printf 'rendered before_10:\n' >&2
  grep -nE 'autoupdate' "$rendered" >&2
  fail "before_10 still starts domt4/autoupdate (found 'autoupdate start')"
fi

# Stub brew: logs every call's argv (one line each). `brew tap` with no further
# argument lists the live taps and reports domt4/autoupdate present so the
# teardown guard fires; `brew tap <name>`/`brew trust ...` (trust loop) succeed;
# `list` exits 1 so the multiplexer probe reports absent and the cleanup path is
# reached; `bundle cleanup` (non-forced preview) prints nothing (0 removals) so
# any bulk-cleanup guard proceeds; everything else exits 0.
prefix="$work/prefix"
mkdir -p "$prefix/bin"
brew_log="$work/brew-argv.log"
cat >"$prefix/bin/brew" <<MOCK
#!/usr/bin/env bash
printf '%s\n' "\$*" >>"$brew_log"
case "\${1:-}" in
  tap)
    if [[ -z \${2:-} ]]; then printf 'domt4/autoupdate\n'; fi
    exit 0 ;;
  list) exit 1 ;;
esac
exit 0
MOCK
for tool in uv npm volta mas; do
  printf '#!/usr/bin/env bash\nexit 0\n' >"$prefix/bin/$tool"
done
chmod +x "$prefix/bin/"*

: >"$brew_log"
HOMEBREW_PREFIX="$prefix" PATH="$prefix/bin:$PATH" bash "$rendered" >/dev/null 2>&1 ||
  fail "rendered before_10 exited non-zero under stubs"

[[ -s $brew_log ]] || fail "no brew invocations captured"

# (2) The teardown must precede the cleanup. First autoupdate stop/delete line
# number must be smaller than the first bundle-cleanup line number.
teardown_ln="$(grep -nE '^autoupdate (stop|delete)' "$brew_log" | head -1 | cut -d: -f1)"
cleanup_ln="$(grep -nE '^bundle cleanup' "$brew_log" | head -1 | cut -d: -f1)"
[[ -n $teardown_ln ]] || {
  printf 'captured brew argv:\n' >&2
  cat "$brew_log" >&2
  fail "no autoupdate teardown (stop/delete) captured; teardown did not run"
}
[[ -n $cleanup_ln ]] || {
  printf 'captured brew argv:\n' >&2
  cat "$brew_log" >&2
  fail "no bundle cleanup captured; cannot assert ordering"
}
[[ $teardown_ln -lt $cleanup_ln ]] ||
  fail "autoupdate teardown (line $teardown_ln) did not precede bundle cleanup (line $cleanup_ln)"

printf 'homebrew-before10-autoupdate-teardown: OK (no autoupdate start; teardown precedes cleanup)\n'
