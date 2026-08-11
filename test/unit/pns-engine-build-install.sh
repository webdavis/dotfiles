#!/usr/bin/env bash
# The apply-time build script is what puts the engine binary on the machine.
# Its three behaviors are load-bearing and none of them is visible anywhere
# else: install the binary WHERE THE PRODUCERS LOOK, leave the run_onchange
# trigger retryable when the build could not happen, and settle the trigger
# once it did.
#
# The script resolves cargo at a fixed $HOME-relative path, so a sandboxed
# HOME with a stub cargo runs the real rendered script end to end.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

script="$scratch/build-pns.sh"
CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$REPO_ROOT/.chezmoiscripts/run_onchange_after_58-build-pns-engine.sh.tmpl" \
  >"$script" 2>/dev/null
[[ -s $script ]] || {
  echo "the build script rendered empty" >&2
  exit 1
}
chmod +x "$script"

home="$scratch/home"
marker="$home/.cache/pns-build/engine.retry"
installed="$home/.local/libexec/pns/pns"

run_script() { HOME="$home" "$script" >/dev/null 2>&1; }

# --- no toolchain: nothing installed, and the trigger stays retryable ------
mkdir -p "$home"
run_script || {
  echo "a missing toolchain must not fail the apply" >&2
  exit 1
}
[[ ! -e $installed ]] || {
  echo "nothing may be installed without a toolchain" >&2
  exit 1
}
[[ -s $marker ]] || {
  echo "a deferred build must leave the trigger retryable" >&2
  exit 1
}
first_attempt="$(cat "$marker")"

# --- toolchain, no crate: still deferred, and the marker keeps counting ----
mkdir -p "$home/.cargo/bin"
cat >"$home/.cargo/bin/cargo" <<STUB
#!/usr/bin/env bash
# Stand-in for the real build: honor --manifest-path and produce the binary
# the script installs.
manifest=""
while [[ \$# -gt 0 ]]; do
  [[ \$1 == --manifest-path ]] && manifest="\$2"
  shift
done
crate="\$(dirname "\$manifest")"
mkdir -p "\$crate/target/release"
if [[ -f "\$HOME/.stub-build-sleeper" ]]; then
  # A real Mach-O, so running it holds the text lock that makes an in-place
  # overwrite fail.
  cp /bin/sleep "\$crate/target/release/pns"
else
  printf '#!/usr/bin/env bash\nprintf pns-engine\n' >"\$crate/target/release/pns"
fi
chmod +x "\$crate/target/release/pns"
STUB
chmod +x "$home/.cargo/bin/cargo"
run_script || {
  echo "a missing crate must not fail the apply" >&2
  exit 1
}
[[ ! -e $installed ]] || {
  echo "nothing may be installed without the crate source" >&2
  exit 1
}
[[ "$(cat "$marker")" -gt "$first_attempt" ]] || {
  echo "each deferral must change the trigger, or it stops re-firing" >&2
  exit 1
}

# --- toolchain and crate: the binary lands where the producers look --------
mkdir -p "$home/.local/share/pns"
printf '[package]\nname = "pns"\n' >"$home/.local/share/pns/Cargo.toml"
run_script || {
  echo "the build must succeed with a toolchain and a crate" >&2
  exit 1
}
[[ -x $installed ]] || {
  echo "the binary must install where the producers look: $installed" >&2
  exit 1
}
[[ "$("$installed")" == pns-engine ]] || {
  echo "the installed file is not the built binary" >&2
  exit 1
}
[[ ! -e $marker ]] || {
  echo "a successful build must settle the trigger" >&2
  exit 1
}

# --- a rebuild while the old binary is RUNNING still replaces it -----------
# The real mid-apply hazard: a producer (an agent hook, a long command, a
# LaunchAgent) is executing the installed engine when the next apply lands.
# Replacing the file in place fails with ETXTBSY; unlinking and creating
# anew does not.
touch "$home/.stub-build-sleeper"
run_script
"$installed" 5 >/dev/null 2>&1 &
running=$!
sleep 0.3
run_script || {
  kill "$running" 2>/dev/null || true
  echo "a rebuild must replace the binary even while it is running" >&2
  exit 1
}
kill "$running" 2>/dev/null || true
wait "$running" 2>/dev/null || true

exit 0
