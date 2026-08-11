#!/usr/bin/env bash
# The R2d differential gate as a standing check: the SAME dispatch assertions
# that pin the bash engine must pass against the Rust engine binary, so a
# broken or no-op binary fails the build instead of shipping behind a green
# bash-only suite. HOME is isolated so a live operator config cannot change
# the roster under the assertions.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

cargo build --quiet --manifest-path "$REPO_ROOT/dot_local/share/pns/Cargo.toml"

HOME="$scratch" PNS_RELAY_BIN="$REPO_ROOT/dot_local/share/pns/target/debug/pns" \
  bats "$REPO_ROOT/test/unit/pns-channel-dispatch.bats"
