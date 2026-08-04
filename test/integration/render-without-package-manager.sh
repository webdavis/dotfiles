#!/usr/bin/env bash
# render-without-package-manager.sh -- rendering this repo's templates must not
# require Homebrew.
#
# `chezmoi execute-template` reads the source state, and reading the source
# state fires the `hooks.read-source-state.pre` command declared in
# .chezmoi.toml.tmpl (.install-password-manager.sh). The hook's exit status is
# the render's exit status, so when the hook shelled out to a `brew` that was
# not on PATH, every one of this repo's render tests failed on a host without
# Homebrew, for a reason none of them was testing.
#
# This test reproduces that path deliberately: a chezmoi config carrying the
# same hook declaration the real config carries, a PATH with neither
# keepassxc-cli nor brew, and a render that must still succeed.
#
# Two things keep it honest:
#   - it asserts .chezmoi.toml.tmpl still routes the hook at
#     .install-password-manager.sh, so the test cannot pass by testing a hook
#     the real config stopped using;
#   - it renders a second time through a hook that fails on purpose, proving the
#     harness DOES observe the hook and would catch a regression, rather than
#     passing because the hook never ran.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HOOK="$REPO_ROOT/.install-password-manager.sh"
CONFIG_TEMPLATE="$REPO_ROOT/.chezmoi.toml.tmpl"

fail() {
  printf 'render-without-package-manager: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -x $HOOK ]] || fail "missing or non-executable hook: $HOOK"
[[ -f $CONFIG_TEMPLATE ]] || fail "missing config template: $CONFIG_TEMPLATE"

# Resolve chezmoi from the AMBIENT PATH before the stripped PATH is built: on
# this host chezmoi itself is a Homebrew formula, and the point of the test is
# that the RENDER does not need Homebrew, not that chezmoi is installed
# somewhere else.
CHEZMOI_BIN="$(command -v chezmoi)" ||
  fail "chezmoi is not on PATH; this test renders with the host chezmoi"

# The hook this repo actually declares, read out of the config template rather
# than assumed. The template writes it as a Go-templated sourceDir path.
grep -q 'hooks.read-source-state.pre' "$CONFIG_TEMPLATE" ||
  fail "$CONFIG_TEMPLATE no longer declares hooks.read-source-state.pre; this test guards a mechanism that moved"
grep -q '\.install-password-manager\.sh' "$CONFIG_TEMPLATE" ||
  fail "$CONFIG_TEMPLATE no longer routes the read-source-state hook at .install-password-manager.sh"

work="$(mktemp -d)"
# `trash` is the operator's rule for interactive removals; a committed test has
# to run on a bare CI runner, where only coreutils exist.
trap 'rm -rf "$work"' EXIT

# A PATH the render cannot smuggle a package manager in through. Asserted, not
# assumed: a system-wide brew would turn this into a test of nothing.
BASE_PATH="/usr/bin:/bin"
for absent in brew keepassxc-cli; do
  if PATH="$BASE_PATH" command -v "$absent" >/dev/null 2>&1; then
    fail "$absent resolves under PATH=$BASE_PATH, so this test cannot show the render works without it"
  fi
done

# write_config <path> <hook-command> -- the minimum config that reproduces the
# real one's hook wiring. Nothing else from the authoring machine's config is
# copied: the render must not depend on it either.
write_config() {
  cat >"$1" <<CONFIG
[hooks.read-source-state.pre]
  command = "$2"
CONFIG
}

# render <config> <outfile> <errfile> -- render a template that needs no data of
# its own, so the only thing under test is whether reading the source state
# succeeded. Returns chezmoi's exit status.
render() {
  PATH="$BASE_PATH" HOME="$work/home" CI=1 "$CHEZMOI_BIN" \
    --config "$1" --source "$REPO_ROOT" --destination "$work/home" \
    execute-template --no-tty <<<'{{ .chezmoi.os }}' >"$2" 2>"$3"
}

mkdir -p "$work/home"

# ---- the behavior: the real hook must not block a render ---------------------
real_config="$work/real-hook.toml"
write_config "$real_config" "$HOOK"

status=0
render "$real_config" "$work/real.out" "$work/real.err" || status=$?
if [[ $status -ne 0 ]]; then
  printf 'render-without-package-manager: FAIL -- rendering through %s failed (exit %d) on a PATH with no Homebrew; chezmoi said:\n' \
    "$HOOK" "$status" >&2
  cat "$work/real.err" >&2
  exit 1
fi
[[ -s $work/real.out ]] ||
  fail "the render exited 0 but produced nothing, so it did not actually render"

# ---- the control: the harness must be able to SEE a failing hook -------------
# Without this, a hook that never ran would look identical to a hook that ran
# and behaved.
failing_hook="$work/failing-hook.sh"
printf '#!/bin/sh\necho "deliberate hook failure" >&2\nexit 1\n' >"$failing_hook"
chmod +x "$failing_hook"

control_config="$work/failing-hook.toml"
write_config "$control_config" "$failing_hook"

control_status=0
render "$control_config" "$work/control.out" "$work/control.err" || control_status=$?
[[ $control_status -ne 0 ]] ||
  fail "a hook that exits 1 must fail the render; it did not, so this test is not observing the hook at all"
grep -q 'deliberate hook failure' "$work/control.err" ||
  fail "the control render failed without running the hook, so the failure proves nothing (stderr: $(cat "$work/control.err"))"

printf 'render-without-package-manager: OK (renders through the real read-source-state hook with no brew and no keepassxc-cli on PATH; a failing hook still fails the render)\n'
