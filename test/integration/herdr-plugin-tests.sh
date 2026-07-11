#!/usr/bin/env bash
# herdr-plugin-tests.sh — gate the herdr plugin Rust suites in `just test`.
#
# The two vendored herdr plugins (herdr-last-workspace, herdr-smart-nav) carry
# cfg(test) unit suites, but nothing in `just test` ran them — a plugin
# regression would only surface at `chezmoi apply` time (the build script) or,
# worse, at runtime. This script runs `cargo test --locked` for BOTH plugins so
# the pre-commit hook (which runs `just test`) blocks the regression instead.
#
# cargo is resolved at the deterministic rustup path "$HOME/.cargo/bin/cargo" —
# the SAME path the build partial (.chezmoitemplates/herdr-plugin-build.sh.tmpl)
# uses; keep the two consistent so this test exercises the toolchain the build
# would actually use, never a stray PATH cargo.
#
# Why skip-not-fail when cargo is absent (CI parity): plain test/*.sh scripts
# run with host tools outside the Nix shell, and neither CI's nix devshell nor a
# fresh machine (before run_once_before_20 provisions rustup) has cargo at
# ~/.cargo/bin. Failing there would turn a missing host toolchain into a
# permanently red suite; the host pre-commit — where cargo exists — is where
# this gate bites. Same posture as the chezmoi/jq guards in the sibling tests.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CARGO_BIN="$HOME/.cargo/bin/cargo"

PLUGINS=(
  herdr-last-workspace
  herdr-smart-nav
)

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

if [[ ! -x $CARGO_BIN ]]; then
  printf 'SKIP: cargo not at %s (fresh machine or CI nix shell); plugin suites run on hosts with rustup\n' "$CARGO_BIN"
  exit 0
fi

status=0
for plugin in "${PLUGINS[@]}"; do
  plugin_dir="$REPO_ROOT/dot_local/share/herdr/plugins/$plugin"
  [[ -f $plugin_dir/Cargo.toml ]] || fail "missing plugin crate: $plugin_dir"
  if (cd "$plugin_dir" && "$CARGO_BIN" test --locked --quiet); then
    printf 'PASS: %s cargo suite\n' "$plugin"
  else
    printf 'FAIL: %s cargo suite\n' "$plugin" >&2
    status=1
  fi
done

exit "$status"
