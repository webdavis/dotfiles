#!/usr/bin/env bash
#
# Fix 4 (SKIP_SYSTEM_PACKAGES must cover every cleanup owner): the literal-1 skip
# guard existed only in before_10, but after_58 independently regenerates the
# worktree's Brewfile and can run `brew bundle cleanup --force` -- the exact
# secondary-worktree scenario SKIP_SYSTEM_PACKAGES exists for. The same
# `{{ if eq (env "SKIP_SYSTEM_PACKAGES") "1" }}` guard must gate after_58's
# cleanup path (the cleanup, NOT the herdr verification the script also does).
#
# Integration test: render after_58 twice (SKIP=1 and unset), run each against a
# healthy herdr stub with tmux installed, and assert the forced cleanup argv is
# absent under SKIP=1 and present when unset. herdr is verified in BOTH renders.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/.chezmoiscripts/run_after_58-herdr-migration-verify.sh.tmpl"

fail() {
  printf 'homebrew-after58-skip-cleanup: FAIL -- %s\n' "$*" >&2
  exit 1
}

for tool in chezmoi jq; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'SKIP: %s not on PATH; cannot render/run after_58\n' "$tool"
    exit 0
  }
done
[[ -f $SCRIPT ]] || fail "missing template: $SCRIPT"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Healthy herdr stub: version ok, one running session, reload applied with empty
# diagnostics, both required plugins enabled+warning-free by exact id.
make_healthy_herdr() {
  local dir="$1"
  cat >"$dir/herdr" <<'STUB'
#!/bin/bash
printf '%s\n' "$*" >>"$HERDR_RECORD"
[[ $1 == --version ]] && { echo "herdr 0.7.0-test"; exit 0; }
case "$1 $2" in
  "session list") printf '{"sessions":[{"default":true,"name":"default","running":true}]}\n'; exit 0 ;;
  "server reload-config") printf '{"id":"cli:server:reload-config","result":{"diagnostics":[],"status":"applied","type":"config_reload"}}\n'; exit 0 ;;
  "plugin list") printf '{"id":"cli:plugin","result":{"plugins":[{"plugin_id":"%s","enabled":true}],"type":"plugin_list"}}\n' "$4"; exit 0 ;;
esac
exit 0
STUB
  chmod +x "$dir/herdr"
}

# Brew stub: `list <pkg>` installed only for names in $INSTALLED_PKGS; `bundle`
# records its argv (non-forced preview prints nothing -> 0 removals -> the guard
# proceeds when the cleanup path runs at all).
make_brew() {
  local dir="$1"
  cat >"$dir/brew" <<'STUB'
#!/bin/bash
case "$1" in
  list)
    read -ra pkgs <<<"$INSTALLED_PKGS"
    for p in "${pkgs[@]}"; do [[ $2 == "$p" ]] && exit 0; done
    exit 1 ;;
  bundle) printf '%s\n' "$*" >>"$BUNDLE_RECORD"; exit 0 ;;
esac
exit 0
STUB
  chmod +x "$dir/brew"
}

# run_render <name> <skip-value|__unset__>: render after_58 with the given
# SKIP_SYSTEM_PACKAGES, run it healthy+tmux, and set BUNDLE_RECORD/ERR/RC.
run_render() {
  local name="$1" skip="$2"
  local dir="$work/$name"
  local prefix="$dir/prefix" path_bin="$dir/path-bin" case_home="$dir/home"
  local rendered="$dir/rendered.sh"
  BUNDLE_RECORD="$dir/bundle-argv"
  HERDR_RECORD="$dir/herdr-argv"
  ERR="$dir/stderr"
  mkdir -p "$prefix/bin" "$path_bin" "$case_home/.config/herdr"
  make_brew "$prefix/bin"
  make_healthy_herdr "$path_bin"
  printf 'x = 1\n' >"$case_home/.config/herdr/config.toml"
  : >"$BUNDLE_RECORD"
  : >"$HERDR_RECORD"

  local render_home="$dir/render-home"
  mkdir -p "$render_home"
  if [[ $skip == __unset__ ]]; then
    HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
      <"$SCRIPT" >"$rendered" || fail "$name: render failed"
  else
    HOME="$render_home" CI=1 SKIP_SYSTEM_PACKAGES="$skip" chezmoi --source "$REPO_ROOT" \
      execute-template --no-tty <"$SCRIPT" >"$rendered" || fail "$name: render failed"
  fi
  if [[ ! -s $rendered ]]; then
    printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
    exit 0
  fi
  RC=0
  HOME="$case_home" HOMEBREW_PREFIX="$prefix" PATH="$path_bin:$PATH" \
    INSTALLED_PKGS="tmux" HERDR_RECORD="$HERDR_RECORD" BUNDLE_RECORD="$BUNDLE_RECORD" \
    bash "$rendered" >"$dir/stdout" 2>"$ERR" || RC=$?
}

herdr_contacted() { [[ -s $HERDR_RECORD ]]; }
cleanup_ran() { grep -q 'bundle cleanup --force' "$BUNDLE_RECORD"; }

# --- SKIP=1: herdr still verified, but the forced cleanup must NOT run ---------
run_render skip-on 1
[[ $RC -eq 0 ]] || fail "skip-on: expected exit 0, got $RC ($(cat "$ERR"))"
herdr_contacted || fail "skip-on: herdr was not verified (the SKIP guard must gate the cleanup, not the verification)"
cleanup_ran && fail "skip-on: brew bundle cleanup --force ran under SKIP_SYSTEM_PACKAGES=1"

# --- unset: the forced cleanup runs normally ----------------------------------
run_render skip-off __unset__
[[ $RC -eq 0 ]] || fail "skip-off: expected exit 0, got $RC ($(cat "$ERR"))"
cleanup_ran || fail "skip-off: brew bundle cleanup --force did not run with SKIP unset"

printf 'homebrew-after58-skip-cleanup: OK (SKIP=1 gates the cleanup, herdr still verified; unset runs the cleanup)\n'
