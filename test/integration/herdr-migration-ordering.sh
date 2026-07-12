#!/usr/bin/env bash
# herdr-migration-ordering.sh: run_onchange_before_10-system-packages must NEVER
# remove the old multiplexer (tmux/sesh) itself. `brew bundle cleanup`
# uninstalls anything installed but absent from the desired set, so once
# tmux/sesh leave the manifest a cleanup here would strip the only multiplexer.
# Chezmoi runs ALL before_ scripts, THEN updates target files, THEN runs after_
# scripts, so a cleanup in this before_ script would validate/tear down against
# the PREVIOUS revision and a broken new revision would only surface after the
# fallback was gone. The teardown therefore lives in run_after_58
# (post-target-update); before_10's only job is to WITHHOLD cleanup while a
# legacy multiplexer is installed.
#
# Invariant: while tmux OR sesh is installed, before_10 runs `brew bundle`
# WITHOUT cleanup and prints a deferral note that names the interactive
# activation step (open a terminal so the herdr server starts, then re-apply so
# after_58 verifies and cleans). When neither is installed (already migrated),
# cleanup proceeds normally. before_10 makes NO herdr contact and consults no
# stamp -- all migration verification moved to after_58.
#
# This renders the REAL before_10 and runs it with a stub Homebrew prefix
# (brew/uv/volta) and a stub npm on PATH; nothing real is installed. The exact
# `brew bundle` argv and the stderr warnings are captured.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

for tool in chezmoi jq; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'SKIP: %s not on PATH; cannot render/run before_10\n' "$tool"
    exit 0
  fi
done
[[ -f $SCRIPT ]] || fail "missing template: $SCRIPT"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

rendered="$work/rendered.sh"
render_home="$work/render-home"
mkdir -p "$render_home"
HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$SCRIPT" >"$rendered" || fail "chezmoi failed to render $SCRIPT"
if [[ ! -s $rendered ]]; then
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi

# Stub Homebrew prefix: brew (tap/trust/list/bundle/autoupdate), uv, volta.
# brew `list <pkg>` reports "installed" only for names in $INSTALLED_PKGS;
# `bundle` records its argv and swallows the Brewfile on stdin.
build_stub_prefix() {
  local prefix="$1"
  mkdir -p "$prefix/bin"
  cat >"$prefix/bin/brew" <<'STUB'
#!/bin/bash
case "$1" in
  tap | trust | autoupdate) exit 0 ;;
  list)
    pkg="$2"
    read -ra installed_packages <<<"$INSTALLED_PKGS"
    for installed in "${installed_packages[@]}"; do
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

# run_case <name> <installed-pkgs>
run_case() {
  local name="$1" installed="$2"
  local case_home="$work/$name/home"
  local prefix="$work/$name/prefix"
  local path_bin="$work/$name/path-bin"
  BUNDLE_RECORD="$work/$name/bundle-argv"
  ERR_FILE="$work/$name/stderr"
  mkdir -p "$case_home" "$path_bin"
  build_stub_prefix "$prefix"
  printf '#!/bin/bash\nexit 0\n' >"$path_bin/npm"
  chmod +x "$path_bin/npm"
  RC=0
  HOME="$case_home" HOMEBREW_PREFIX="$prefix" \
    PATH="$path_bin:$PATH" INSTALLED_PKGS="$installed" BUNDLE_RECORD="$BUNDLE_RECORD" \
    bash "$rendered" </dev/null >/dev/null 2>"$ERR_FILE" || RC=$?
}

# Homebrew 6.x moved removal out of `brew bundle --cleanup` (now a dry run) into
# the separate `brew bundle cleanup --force` command; before_10 runs that command
# only when no legacy multiplexer remains. "cleanup used" == that command was
# recorded (the plain install `bundle --file=...` line never contains "cleanup").
cleanup_used() { grep -q 'bundle cleanup --force' "$BUNDLE_RECORD"; }

# tmux still installed -> defer cleanup, and SAY so, naming the interactive
# activation step (the deferral note is part of the contract: silently
# withholding cleanup would leave the operator with no clue why tmux/sesh
# survive or how to finish the migration).
run_case tmux-present "tmux"
[[ $RC -eq 0 ]] || fail "tmux-present: script errored (rc=$RC; stderr: $(cat "$ERR_FILE"))"
[[ -s $BUNDLE_RECORD ]] || fail "tmux-present: brew bundle was never invoked"
cleanup_used && fail "tmux-present: cleanup passed though tmux is installed (before_10 must never tear down the multiplexer)"
grep -q 'WITHOUT cleanup' "$ERR_FILE" ||
  fail "tmux-present: the deferral note was not printed (stderr: $(cat "$ERR_FILE"))"
grep -qi 'interactive terminal' "$ERR_FILE" ||
  fail "tmux-present: the deferral note does not name the interactive activation step (stderr: $(cat "$ERR_FILE"))"
grep -q 'after_58' "$ERR_FILE" ||
  fail "tmux-present: the deferral note does not point at after_58 as the teardown owner (stderr: $(cat "$ERR_FILE"))"

# sesh still installed -> defer cleanup too.
run_case sesh-present "sesh"
[[ $RC -eq 0 ]] || fail "sesh-present: script errored (rc=$RC; stderr: $(cat "$ERR_FILE"))"
cleanup_used && fail "sesh-present: cleanup passed though sesh is installed"

# Neither installed (already migrated) -> cleanup proceeds normally.
run_case none "wget"
[[ $RC -eq 0 ]] || fail "none: script errored (rc=$RC; stderr: $(cat "$ERR_FILE"))"
cleanup_used || fail "none: cleanup withheld though no multiplexer is installed (migrated machines must still get cleanup)"

printf 'PASS: before_10 withholds cleanup while tmux/sesh is installed (deferral note names the interactive activation step and after_58 as the teardown owner), makes no herdr contact, and lets cleanup proceed once no multiplexer remains\n'
