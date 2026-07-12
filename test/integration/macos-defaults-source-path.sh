#!/usr/bin/env bash
# macos-defaults-source-path.sh -- the macos-defaults-{apply,capture,drift} tools
# must resolve their .chezmoidata/macos_defaults.yaml for the CURRENT context, NOT
# a hardcoded primary-checkout path. The old tools hardcoded
# "${HOME}/workspaces/Ivy/webdavis/dotfiles/.chezmoidata/macos_defaults.yaml", so a
# capture/apply/drift run from a SECONDARY git worktree wrote (or read) the PRIMARY
# tree instead of the worktree the operator is standing in: the
# worktree-writes-primary bug. The shared lib now resolves the source dir from the
# current git worktree (falling back to `chezmoi source-path`), so a run from a
# worktree targets THAT worktree.
#
# Everything below lives under one sandbox temp dir. HOME is pointed there and a
# throwaway chezmoi config sets sourceDir to a sandbox "primary", so NEITHER the
# real primary checkout NOR the buggy hardcoded ${HOME}/workspaces/... path can be
# touched by this test -- the red run (buggy code) writes only the sandbox.
#
# capture is the write path (the strongest proof): a capture from the worktree must
# land in the WORKTREE yaml and leave BOTH sandbox "primaries" untouched. drift is
# the read path: pointed at an unreadable worktree yaml it must exit 2 naming the
# WORKTREE file, proving it read the worktree and not a primary.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CAPTURE="$REPO_ROOT/dot_local/bin/executable_macos-defaults-capture.sh"
DRIFT="$REPO_ROOT/dot_local/bin/executable_macos-defaults-drift.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# Host-tool guards: plain test/*.sh scripts run with host tools, outside the Nix
# shell. The de-homebrewed CI-faithful run has no chezmoi/yq on PATH -> SKIP.
for tool in chezmoi yq git; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'SKIP: %s not on PATH; cannot exercise source-dir resolution\n' "$tool"
    exit 0
  fi
done
[[ -f $CAPTURE ]] || fail "missing script: $CAPTURE"
[[ -f $DRIFT ]] || fail "missing script: $DRIFT"

# Canonicalize away macOS's /var -> /private/var symlink so paths match what
# `git rev-parse --show-toplevel` (used by the resolver under test) returns.
sandbox="$(cd "$(mktemp -d)" && pwd -P)"
trap 'chmod -R u+rwX "$sandbox" 2>/dev/null; rm -rf "$sandbox"' EXIT

# A throwaway chezmoi config so the lib's `chezmoi source-path` fallback resolves
# to a SANDBOX primary, never the operator's real checkout.
mkdir -p "$sandbox/.config/chezmoi" "$sandbox/primary-src/.chezmoidata"
printf 'sourceDir = "%s/primary-src"\n' "$sandbox" >"$sandbox/.config/chezmoi/chezmoi.toml"
printf 'macos:\n  defaults: []\n  killall: []\n' >"$sandbox/primary-src/.chezmoidata/macos_defaults.yaml"

# The buggy hardcoded path is ${HOME}/workspaces/Ivy/webdavis/dotfiles/.chezmoidata/
# macos_defaults.yaml; with HOME=sandbox it lands here. Seed it so the red run has a
# readable target and mutates ONLY the sandbox.
hardcoded_dir="$sandbox/workspaces/Ivy/webdavis/dotfiles/.chezmoidata"
mkdir -p "$hardcoded_dir"
printf 'macos:\n  defaults: []\n  killall: []\n' >"$hardcoded_dir/macos_defaults.yaml"

# The secondary worktree the operator is standing in: a real git repo carrying its
# own macos_defaults.yaml.
wt="$sandbox/wt"
mkdir -p "$wt/.chezmoidata"
printf 'macos:\n  defaults: []\n  killall: []\n' >"$wt/.chezmoidata/macos_defaults.yaml"
git -C "$wt" init -q
# Pre-flight: the worktree branch of resolution only fires when git sees this dir as
# its own top-level. Assert that so a green-looking pass cannot hide a silent
# fallback to a primary.
[[ "$(git -C "$wt" rev-parse --show-toplevel)" == "$wt" ]] ||
  fail "test setup: $wt is not its own git top-level"

# Stub `defaults` so capture appends a deterministic record without reading live
# system state. read-type -> boolean, read -> 1 (normalizes to true).
stub_bin="$sandbox/bin"
mkdir -p "$stub_bin"
cat >"$stub_bin/defaults" <<'STUB'
#!/bin/bash
case "$1" in
  read-type) echo "Type is boolean"; exit 0 ;;
  read) echo "1"; exit 0 ;;
  *) exit 1 ;;
esac
STUB
chmod +x "$stub_bin/defaults"

DOMAIN="com.example.s10test"
KEY="s10flag"

# ---- capture (write path): must land in the WORKTREE, not a primary ----------
(
  cd "$wt" || exit 1
  HOME="$sandbox" PATH="$stub_bin:$PATH" bash "$CAPTURE" "$DOMAIN" "$KEY"
) || fail "capture exited non-zero from the worktree"

grep -qF "$DOMAIN" "$wt/.chezmoidata/macos_defaults.yaml" ||
  fail "capture did NOT write the worktree yaml (worktree-writes-primary bug: the record went to a primary)"
if grep -qF "$DOMAIN" "$hardcoded_dir/macos_defaults.yaml"; then
  fail "capture wrote the HARDCODED ~/workspaces/... primary instead of the worktree"
fi
if grep -qF "$DOMAIN" "$sandbox/primary-src/.chezmoidata/macos_defaults.yaml"; then
  fail "capture wrote the chezmoi-configured primary instead of the worktree"
fi

# ---- drift (read path): unreadable worktree yaml -> exit 2 naming the worktree -
chmod 000 "$wt/.chezmoidata/macos_defaults.yaml"
drift_err="$sandbox/drift.err"
drift_rc=0
(
  cd "$wt" || exit 1
  HOME="$sandbox" PATH="$stub_bin:$PATH" bash "$DRIFT"
) 2>"$drift_err" || drift_rc=$?
chmod u+rw "$wt/.chezmoidata/macos_defaults.yaml"

[[ $drift_rc -eq 2 ]] ||
  fail "drift from the worktree with an unreadable yaml must exit 2 (got rc=$drift_rc); it read a primary, not the worktree"
grep -qF "$wt/.chezmoidata/macos_defaults.yaml" "$drift_err" ||
  fail "drift's exit-2 message must name the WORKTREE yaml, proving it resolved the worktree (stderr: $(cat "$drift_err"))"

printf 'macos-defaults-source-path: OK (capture writes the worktree; drift reads the worktree; both primaries untouched)\n'
