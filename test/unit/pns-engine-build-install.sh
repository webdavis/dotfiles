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
pending="$home/.cache/pns-build/restart-pending"
installed="$home/.local/libexec/pns/pns"

# The script kickstarts the pns LaunchAgent after installing a CHANGED binary,
# and a sandboxed HOME does nothing to launchctl: without a stub on PATH this
# test bounces the operator's live daemon on every run. The stub records the
# exact invocation and answers whatever status the test puts in
# launchctl.status, defaulting to 113 ("Could not find service", the unloaded-
# label case) when that file is absent, so a phase that never sets it keeps
# the original quiet behavior.
stubbin="$scratch/stubbin"
kickstarts="$scratch/kickstarts"
launchctl_status="$scratch/launchctl.status"
marker_during_kickstart="$scratch/marker-during-kickstart"
mkdir -p "$stubbin"
: >"$kickstarts"
: >"$marker_during_kickstart"
cat >"$stubbin/launchctl" <<STUB
#!/usr/bin/env bash
printf '%s\n' "\$*" >>"$kickstarts"
if [[ -e "$pending" ]]; then
  printf 'present\n' >>"$marker_during_kickstart"
fi
if [[ -f "$launchctl_status" ]]; then
  exit "\$(cat "$launchctl_status")"
fi
exit 113
STUB
chmod +x "$stubbin/launchctl"

stdout_log="$scratch/stdout"
stderr_log="$scratch/stderr"
run_script() { HOME="$home" PATH="$stubbin:$PATH" "$script" >"$stdout_log" 2>"$stderr_log"; }

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
locked=0
while [[ \$# -gt 0 ]]; do
  [[ \$1 == --manifest-path ]] && manifest="\$2"
  [[ \$1 == --locked ]] && locked=1
  shift
done
# The committed lock is the build: without --locked cargo may rewrite the
# deployed lockfile and pull dependencies the lock never recorded.
if [[ \$locked -ne 1 ]]; then
  echo "cargo was invoked without --locked" >&2
  exit 1
fi

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
[[ "$(cat "$marker")" -gt $first_attempt ]] || {
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
# A cumulative total cannot tell a reversed change verdict from a correct one
# (skip-when-changed plus restart-when-identical can total the same count as
# restart-when-changed plus skip-when-identical), so each phase asserts its
# own running count rather than one check at the end.
[[ "$(wc -l <"$kickstarts" | tr -d ' ')" -eq 1 ]] || {
  echo "the first install must kickstart the daemon exactly once" >&2
  exit 1
}
expected_kickstart="kickstart -k gui/$(id -u)/com.webdavis.pns-daemon"
[[ "$(head -n1 "$kickstarts")" == "$expected_kickstart" ]] || {
  printf 'the kickstart must target the exact label; got: %s\n' "$(head -n1 "$kickstarts")" >&2
  exit 1
}
# The marker is armed before the binary is published, not after a kickstart
# failure, so it must already exist by the time the kickstart itself runs.
[[ -s $marker_during_kickstart ]] || {
  echo "the restart-pending marker must exist by the time the kickstart runs" >&2
  exit 1
}
[[ ! -e $pending ]] || {
  echo "a first-install kickstart (113) must clear the restart-pending marker" >&2
  exit 1
}
# From here on the binary is already installed, so a 113 is no longer the
# fresh-machine no-op; default every later phase to a clean success and let
# each one override the status it actually wants to test.
echo 0 >"$launchctl_status"

# --- the marker cannot be armed: refuse to publish rather than install a ---
# --- binary the daemon has no forced way to pick up -------------------------
kickstarts_before="$(wc -l <"$kickstarts" | tr -d ' ')"
touch "$home/.stub-build-sleeper"
rm -rf "$home/.cache/pns-build"
touch "$home/.cache/pns-build"
run_script && {
  echo "a marker that cannot be written must refuse to publish the binary" >&2
  exit 1
}
[[ "$("$installed")" == pns-engine ]] || {
  echo "a refused publish must leave the previously installed binary in place" >&2
  exit 1
}
[[ "$(wc -l <"$kickstarts" | tr -d ' ')" -eq $kickstarts_before ]] || {
  echo "a refused publish must never reach the kickstart" >&2
  exit 1
}
rm -f "$home/.cache/pns-build"
rm -f "$home/.stub-build-sleeper"

# --- a rebuild while the old binary is RUNNING still replaces it -----------
# The real mid-apply hazard: a producer (an agent hook, a long command, a
# LaunchAgent) is executing the installed engine when the next apply lands.
# macOS does not refuse an in-place overwrite of a running binary (no
# ETXTBSY; measured), so this phase only pins that a reinstall while the
# engine is running succeeds; the install(1) man page, not this test, is the
# evidence that it lands through a temporary file and a rename.
touch "$home/.stub-build-sleeper"
run_script
[[ "$(wc -l <"$kickstarts" | tr -d ' ')" -eq 2 ]] || {
  echo "a changed binary must kickstart again" >&2
  exit 1
}
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
[[ "$(wc -l <"$kickstarts" | tr -d ' ')" -eq 2 ]] || {
  echo "an identical reinstall must not kickstart the daemon" >&2
  exit 1
}

# --- a kickstart failure is loud: nonzero exit, a stderr line, and a marker
# that forces the next apply to retry regardless of what it rebuilds --------
echo 5 >"$launchctl_status"
# Drop the sleeper stub so the rebuild changes bytes again (back to the plain
# script binary) and the kickstart is attempted.
rm -f "$home/.stub-build-sleeper"
# The deferral phases above created the cache directory. A machine whose
# first apply already has the toolchain never ran a deferral, so the failure
# arm must create it itself; run this phase without it.
rmdir "$home/.cache/pns-build"
run_script && {
  echo "a kickstart failure must fail the apply" >&2
  exit 1
}
[[ "$(wc -l <"$kickstarts" | tr -d ' ')" -eq 3 ]] || {
  echo "a failed kickstart is still attempted once" >&2
  exit 1
}
grep -q 'daemon NOT restarted on the new binary (launchctl kickstart exited 5:' "$stderr_log" || {
  echo "a kickstart failure must print an attributed line to stderr" >&2
  exit 1
}
[[ -e $pending ]] || {
  echo "a kickstart failure must leave a restart-pending marker" >&2
  exit 1
}

# --- 113 on an EXISTING installation is a real failure, not the fresh-
# machine no-op: "not loaded" on a daemon that was already running means the
# kickstart never reached it, so it must fail the apply and keep the marker
echo 113 >"$launchctl_status"
run_script && {
  echo "a 113 on an existing installation must fail the apply" >&2
  exit 1
}
[[ "$(wc -l <"$kickstarts" | tr -d ' ')" -eq 4 ]] || {
  echo "the existing-installation 113 must still attempt the kickstart" >&2
  exit 1
}
grep -q 'daemon NOT restarted on the new binary (launchctl kickstart exited 113:' "$stderr_log" || {
  echo "a 113 on an existing installation must print an attributed line to stderr" >&2
  exit 1
}
[[ -e $pending ]] || {
  echo "a 113 on an existing installation must keep the restart-pending marker" >&2
  exit 1
}

# --- the pending marker forces a retry even on an IDENTICAL rebuild, and a
# successful kickstart clears it and prints the restarted line -------------
echo 0 >"$launchctl_status"
run_script || {
  echo "the retried kickstart must succeed and the apply must exit 0" >&2
  exit 1
}
[[ "$(wc -l <"$kickstarts" | tr -d ' ')" -eq 5 ]] || {
  echo "the pending marker must force one more kickstart on an unchanged binary" >&2
  exit 1
}
grep -q 'daemon restarted on a new binary' "$stdout_log" || {
  echo "a successful kickstart must print the restarted line" >&2
  exit 1
}
[[ ! -e $pending ]] || {
  echo "a successful kickstart must clear the restart-pending marker" >&2
  exit 1
}

exit 0
