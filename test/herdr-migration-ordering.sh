#!/usr/bin/env bash
# herdr-migration-ordering.sh — run_onchange_before_10-system-packages must NOT
# let `brew bundle --cleanup` remove the old multiplexer (tmux/sesh) before herdr
# is installed AND verified. `--cleanup` uninstalls anything installed but absent
# from the desired set, so once tmux/sesh leave the manifest a bare --cleanup on
# a not-yet-migrated machine would strip the only multiplexer the moment before
# herdr's install/verify — if that fails, the machine is stranded.
#
# The invariant: pass --cleanup UNLESS (tmux or sesh is still installed AND the
# herdr-verified stamp is absent). The stamp is written by run_after_58 (proven
# in herdr-migration-verify.sh). This renders the REAL before_10 and runs it with
# a stub Homebrew prefix (brew/uv/volta) and a stub npm on PATH — nothing real is
# installed — capturing the exact `brew bundle` argv.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_ROOT/.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

if ! command -v chezmoi >/dev/null 2>&1; then
  printf 'SKIP: chezmoi not on PATH; cannot render before_10\n'
  exit 0
fi
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

# Stub Homebrew prefix: brew (tap/trust/list/bundle/autoupdate), uv, volta. brew
# `list <pkg>` reports "installed" only for names in $INSTALLED_PKGS; `bundle`
# records its argv and swallows the Brewfile on stdin.
build_stub_prefix() {
  local prefix="$1"
  mkdir -p "$prefix/bin"
  cat >"$prefix/bin/brew" <<'STUB'
#!/bin/bash
case "$1" in
  tap | trust | autoupdate) exit 0 ;;
  list)
    pkg="$2"
    for installed in $INSTALLED_PKGS; do
      [[ $pkg == "$installed" ]] && exit 0
    done
    exit 1 ;;
  bundle)
    printf '%s\n' "$*" >>"$BUNDLE_RECORD"
    cat >/dev/null
    exit 0 ;;
esac
exit 0
STUB
  printf '#!/bin/bash\nexit 0\n' >"$prefix/bin/uv"
  printf '#!/bin/bash\nexit 0\n' >"$prefix/bin/volta"
  chmod +x "$prefix/bin/brew" "$prefix/bin/uv" "$prefix/bin/volta"
}

# run_case <name> <installed-pkgs> <stamp?>
run_case() {
  local name="$1" installed="$2" stamp="$3"
  local case_home="$work/$name/home"
  local prefix="$work/$name/prefix"
  local path_bin="$work/$name/path-bin"
  BUNDLE_RECORD="$work/$name/bundle-argv"
  mkdir -p "$case_home" "$path_bin"
  build_stub_prefix "$prefix"
  printf '#!/bin/bash\nexit 0\n' >"$path_bin/npm"
  chmod +x "$path_bin/npm"
  if [[ $stamp == stamp ]]; then
    mkdir -p "$case_home/.cache/herdr-migration"
    : >"$case_home/.cache/herdr-migration/verified"
  fi
  RC=0
  HOME="$case_home" HOMEBREW_PREFIX="$prefix" \
    PATH="$path_bin:$PATH" INSTALLED_PKGS="$installed" BUNDLE_RECORD="$BUNDLE_RECORD" \
    bash "$rendered" >/dev/null 2>&1 || RC=$?
}

cleanup_used() { grep -qw -- '--cleanup' "$BUNDLE_RECORD"; }

# tmux still installed, herdr NOT verified -> defer cleanup (no --cleanup).
run_case tmux-no-stamp "tmux" no-stamp
[[ $RC -eq 0 ]] || fail "tmux-no-stamp: script errored (rc=$RC)"
[[ -s $BUNDLE_RECORD ]] || fail "tmux-no-stamp: brew bundle was never invoked"
cleanup_used && fail "tmux-no-stamp: --cleanup passed though tmux is installed and herdr is unverified (would strand the machine)"

# sesh still installed, herdr NOT verified -> defer cleanup.
run_case sesh-no-stamp "sesh" no-stamp
cleanup_used && fail "sesh-no-stamp: --cleanup passed though sesh is installed and herdr is unverified"

# tmux installed, herdr verified (stamp) -> cleanup proceeds.
run_case tmux-stamp "tmux" stamp
cleanup_used || fail "tmux-stamp: --cleanup not passed though herdr is verified (migration would never complete)"

# No multiplexer installed -> cleanup proceeds regardless of stamp.
run_case none-no-stamp "" no-stamp
cleanup_used || fail "none-no-stamp: --cleanup withheld though no multiplexer is installed (nothing to strand)"

run_case none-stamp "" stamp
cleanup_used || fail "none-stamp: --cleanup withheld though no multiplexer is installed"

printf 'PASS: before_10 defers brew bundle --cleanup while tmux/sesh is installed and herdr is unverified, and passes it once herdr is verified or no multiplexer is present\n'
