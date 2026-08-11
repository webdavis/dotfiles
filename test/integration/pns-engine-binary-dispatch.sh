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

# Phase 2: native dispatch reachability. With no channels-dir override, the
# banner leg must deliver NATIVELY: the PATH-stubbed terminal-notifier records
# the spawn and a decoy executable channel must stay silent. This is the one
# branch the stub-driven bats above can never reach, because they set
# PNS_CHANNELS_DIR, the exact condition under which executables win by design.
stub_bin="$scratch/bin"
decoy_dir="$scratch/.local/libexec/pns/channels"
mkdir -p "$stub_bin" "$decoy_dir"
printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$*" >"%s/notifier.args"\n' "$scratch" \
  >"$stub_bin/terminal-notifier"
printf '#!/usr/bin/env bash\ncat >"%s/decoy.event"\n' "$scratch" \
  >"$decoy_dir/macos-banner.sh"
chmod +x "$stub_bin/terminal-notifier" "$decoy_dir/macos-banner.sh"

env -u PNS_CHANNELS_DIR HOME="$scratch" PATH="$stub_bin:$PATH" RELAY_IDLE_SECS=99999 \
  "$REPO_ROOT/dot_local/share/pns/target/debug/pns" \
  --agent claude --state 'done' --detail x --local-only

[[ -f "$scratch/notifier.args" ]] || {
  echo "native banner did not spawn the notifier" >&2
  exit 1
}
grep -q -- '-title' "$scratch/notifier.args" || {
  echo "the notifier spawn carried no title" >&2
  exit 1
}
[[ ! -f "$scratch/decoy.event" ]] || {
  echo "the decoy executable channel fired; native did not win" >&2
  exit 1
}
