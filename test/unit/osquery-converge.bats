#!/usr/bin/env bats
# osquery-converge.sh, the EXECUTION EDGE: it probes /var/osquery, installs the
# desired state over whatever drifted, and restarts the root daemon only when
# something did. The verdicts it folds over are pinned separately in
# osquery-converge-drift-verdict.bats; what is pinned here is the privileged
# work and the restart, because those are the parts that can leave the machine
# unmonitored.
#
# Nothing here reaches the real /var/osquery, the real sudo or the real
# osqueryd. `sudo` and `osqueryctl` are recording stubs, and the whole target
# tree is a sandbox directory.
#
# ONE thing the sandbox cannot produce: a root-owned file. So `stat` is stubbed
# to answer the REAL mode from the real stat and a substituted owner pair,
# which is what lets a correctly-installed file be modelled at all. The mode is
# never faked, since mode drift is the case that matters most here.

# The desired-state file set, exactly as the tool names it. Written out here
# rather than discovered, so a file quietly dropped from the tool's list fails
# this suite instead of silently never being installed again.
DESIRED_FILES=(
  osquery.conf
  osquery.flags
  packs/agent-attack-surface.conf
  packs/installed-software-drift.conf
  packs/intrusion-detection.conf
  packs/security-policy-regression.conf
)

# The stubs and the converged prototype tree are built ONCE and copied per test:
# every stub reads its behavior out of the environment, so nothing about them is
# per-test, and a fresh set of forks for each of them is most of what this file
# would otherwise cost.
setup_file() {
  TOOL="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)/dot_local/libexec/osquery/executable_osquery-converge.sh"
  BIN="$BATS_FILE_TMPDIR/bin"
  PROTOTYPE="$BATS_FILE_TMPDIR/prototype"
  mkdir -p "$BIN" "$PROTOTYPE/desired/packs" "$PROTOTYPE/var-osquery/packs" "$PROTOTYPE/log/osquery"

  local relative
  for relative in "${DESIRED_FILES[@]}"; do
    printf '{"desired": "%s"}\n' "$relative" >"$PROTOTYPE/desired/$relative"
    cp "$PROTOTYPE/desired/$relative" "$PROTOTYPE/var-osquery/$relative"
  done
  # The vendor's own file: `osqueryctl start` copies it into /Library/LaunchDaemons
  # and `stop` deletes that copy, so it is a precondition of any restart and is
  # never a repair target.
  printf 'vendor plist\n' >"$PROTOTYPE/var-osquery/io.osquery.agent.plist"

  # sudo: record the whole argument vector, then run the command WITHOUT the
  # ownership flags, which a non-root sandbox cannot honor. Recording the argv
  # is what lets a test prove owner, group and mode ride in ONE install call.
  cat >"$BIN/sudo" <<'STUB'
#!/bin/bash
printf '%s\n' "$*" >>"$SUDO_LOG"
[[ ${1:-} == -n ]] && shift
if [[ ${1:-} == install ]]; then
  shift
  args=()
  while (($#)); do
    case "$1" in
      -o | -g) shift 2 ;;
      *)
        args+=("$1")
        shift
        ;;
    esac
  done
  exec install "${args[@]}"
fi
exec "$@"
STUB

  # osqueryctl: record each subcommand and answer with the programmed status. A
  # successful `start` is what publishes the daemon pid the restart check reads.
  cat >"$BIN/osqueryctl" <<'STUB'
#!/bin/bash
printf '%s\n' "$*" >>"$OSQUERYCTL_LOG"
case "${1:-}" in
  config-check) exit "${OSQUERYCTL_CONFIG_CHECK_EXIT:-0}" ;;
  stop)
    [[ ${OSQUERYCTL_STOP_EXIT:-0} == 0 ]] || exit "$OSQUERYCTL_STOP_EXIT"
    : >"$DAEMON_PID_FILE"
    ;;
  start)
    if [[ ${OSQUERYCTL_START_EXIT:-0} != 0 ]]; then
      printf 'osqueryctl: start failed\n' >&2
      exit "$OSQUERYCTL_START_EXIT"
    fi
    [[ ${OSQUERYCTL_START_LEAVES_DAEMON_DOWN:-0} == 1 ]] ||
      printf '%s\n' "${OSQUERYD_PID:-4242}" >"$DAEMON_PID_FILE"
    ;;
esac
exit 0
STUB

  # pgrep: only `pgrep -P 1 -x osqueryd` is asked for. The pid file IS the
  # daemon's liveness. DAEMON_LIVES models a daemon that answers a few probes
  # and is then gone, which is what a crash inside the settle window looks like.
  cat >"$BIN/pgrep" <<'STUB'
#!/bin/bash
printf '%s\n' "$*" >>"$PGREP_ARGV"
[[ -s $DAEMON_PID_FILE ]] || exit 1
if [[ -f $DAEMON_LIVES ]]; then
  remaining=$(cat "$DAEMON_LIVES")
  remaining=$((remaining - 1))
  printf '%s' "$remaining" >"$DAEMON_LIVES"
  ((remaining < 0)) && exit 1
fi
cat "$DAEMON_PID_FILE"
STUB

  # stat: the REAL mode, a substituted owner pair. See the file docblock.
  cat >"$BIN/stat" <<'STUB'
#!/bin/bash
case "${1:-}" in
  -c) exit 1 ;; # a BSD host has no -c; keep the tool on the branch it really takes
  -f) ;;
  *) exit 1 ;;
esac
path="${3:-}"
mode="$(/usr/bin/stat -c '%a' "$path" 2>/dev/null || /usr/bin/stat -f '%p' "$path" 2>/dev/null)" || exit 1
printf '%s %s %s\n' "$mode" "${FAKE_STAT_UID:-0}" "${FAKE_STAT_GID:-0}"
STUB

  # An instant sleep: the restart poll waits in quarter seconds and nothing here
  # asserts on the waiting.
  ln -sf /usr/bin/true "$BIN/sleep"
  chmod +x "$BIN/sudo" "$BIN/osqueryctl" "$BIN/pgrep" "$BIN/stat"

  export TOOL BIN PROTOTYPE
}

setup() {
  DESIRED="$BATS_TEST_TMPDIR/desired"
  TARGET="$BATS_TEST_TMPDIR/var-osquery"
  LOG_DIR="$BATS_TEST_TMPDIR/log/osquery"
  SUDO_LOG="$BATS_TEST_TMPDIR/sudo.log"
  OSQUERYCTL_LOG="$BATS_TEST_TMPDIR/osqueryctl.log"
  DAEMON_PID_FILE="$BATS_TEST_TMPDIR/daemon.pid"
  DAEMON_LIVES="$BATS_TEST_TMPDIR/daemon.lives"
  PGREP_ARGV="$BATS_TEST_TMPDIR/pgrep.argv"

  cp -R "$PROTOTYPE/." "$BATS_TEST_TMPDIR/"
  : >"$SUDO_LOG"
  : >"$OSQUERYCTL_LOG"
  : >"$PGREP_ARGV"
  printf '4242\n' >"$DAEMON_PID_FILE"

  export SUDO_LOG OSQUERYCTL_LOG DAEMON_PID_FILE DAEMON_LIVES PGREP_ARGV
}

converge() {
  PATH="$BIN:$PATH" \
    OSQUERY_CONVERGE_DESIRED_DIR="$DESIRED" \
    OSQUERY_CONVERGE_TARGET_DIR="$TARGET" \
    OSQUERY_CONVERGE_SUDO="$BIN/sudo" \
    OSQUERY_CONVERGE_OSQUERYCTL="$BIN/osqueryctl" \
    OSQUERY_CONVERGE_LOG_DIR="$LOG_DIR" \
    OSQUERY_CONVERGE_RESTART_DEADLINE=1 \
    OSQUERY_CONVERGE_SETTLE_SECONDS=1 \
    "$TOOL" "$@"
}

privileged_call_count() { grep -c . "$SUDO_LOG" || true; }

# The REAL mode on disk, GNU form first and BSD second (the portable order the
# repo enforces): the stubbed stat on the sandbox PATH is deliberately not used
# here, because it substitutes the owner pair.
deployed_mode() { /usr/bin/stat -c '%a' "$1" 2>/dev/null || /usr/bin/stat -f '%Lp' "$1"; }

restarted() { grep -qx 'start' "$OSQUERYCTL_LOG"; }

refute_log_has() { # <fixed-substring> <file>
  # A bare `! grep` is exempted from set -e inside bats and silently no-ops, so
  # every negative assertion goes through a function whose nonzero return counts.
  if grep -qF -- "$1" "$2"; then
    printf 'expected %q NOT to appear in %s, but it does:\n%s\n' "$1" "$2" "$(cat "$2")" >&2
    return 1
  fi
  return 0
}

# --- the three-state converge ----------------------------------------------

@test "a converged tree is a silent no-op: nothing printed, nothing privileged, no restart" {
  run converge
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  [ "$(privileged_call_count)" -eq 0 ]
  [ ! -s "$OSQUERYCTL_LOG" ]
}

@test "a wiped file is reinstalled and the daemon is restarted" {
  rm -f "$TARGET/osquery.conf"
  run converge
  [ "$status" -eq 0 ]
  [ -f "$TARGET/osquery.conf" ]
  cmp -s "$DESIRED/osquery.conf" "$TARGET/osquery.conf"
  restarted
}

@test "a wiped pack is reinstalled too, so a partial wipe is fully repaired" {
  rm -f "$TARGET/packs/intrusion-detection.conf"
  run converge
  [ "$status" -eq 0 ]
  cmp -s "$DESIRED/packs/intrusion-detection.conf" "$TARGET/packs/intrusion-detection.conf"
}

@test "correct bytes under a world-writable mode are reinstalled, not passed over" {
  # The escalation vector: osqueryd reads this file as root.
  chmod 0666 "$TARGET/osquery.conf"
  run converge
  [ "$status" -eq 0 ]
  [ "$(deployed_mode "$TARGET/osquery.conf")" = 644 ]
  restarted
}

@test "the install carries owner, group and mode in ONE call" {
  # Not a tee-then-chmod pair: a file that exists between the two carries the
  # creating umask and the invoking owner for that window.
  rm -f "$TARGET/osquery.flags"
  run converge
  [ "$status" -eq 0 ]
  grep -qF -- "-n install -o root -g wheel -m 0644 $DESIRED/osquery.flags $TARGET/osquery.flags" "$SUDO_LOG"
}

@test "only the drifted file is reinstalled, so a repair is not a rewrite of everything" {
  rm -f "$TARGET/osquery.flags"
  run converge
  [ "$status" -eq 0 ]
  refute_log_has "$TARGET/osquery.conf" "$SUDO_LOG"
}

@test "a repair says which file it repaired and why" {
  rm -f "$TARGET/osquery.conf"
  run converge
  [ "$status" -eq 0 ]
  [[ $output == *"$TARGET/osquery.conf"* ]]
  [[ $output == *missing* ]]
}

# --- the directories --------------------------------------------------------

@test "a group-writable target directory is repaired to 0755 root:wheel" {
  chmod 0775 "$TARGET"
  run converge
  [ "$status" -eq 0 ]
  grep -qF -- "-n install -d -o root -g wheel -m 0755 $TARGET" "$SUDO_LOG"
}

@test "a missing packs directory is created before the packs are installed into it" {
  rm -rf "$TARGET/packs"
  run converge
  [ "$status" -eq 0 ]
  [ -f "$TARGET/packs/intrusion-detection.conf" ]
}

# --- refusals ---------------------------------------------------------------

@test "a desired file that is not deployed is a loud failure, never a silent skip" {
  # Without it there is nothing to converge toward, and passing over it would
  # leave a wiped /var/osquery file wiped while reporting success.
  rm -f "$DESIRED/packs/intrusion-detection.conf"
  rm -f "$TARGET/osquery.conf"
  run converge
  [ "$status" -ne 0 ]
  [[ $output == *"packs/intrusion-detection.conf"* ]]
  [ "$(privileged_call_count)" -eq 0 ]
}

@test "a file in the desired tree the tool does not install is refused, not ignored" {
  # The list of files is NAMED rather than globbed, so that nothing planted in
  # the staging tree is promoted root-owned into the daemon's directory. The
  # cost of naming it is a file that could sit there being ignored forever, so
  # the mismatch is a loud refusal instead.
  printf 'planted\n' >"$DESIRED/packs/planted.conf"
  rm -f "$TARGET/osquery.conf"
  run converge
  [ "$status" -ne 0 ]
  [[ $output == *planted.conf* ]]
  [ "$(privileged_call_count)" -eq 0 ]
}

@test "a symlink in the desired tree is refused rather than installed through" {
  rm -f "$DESIRED/osquery.conf"
  ln -s /etc/passwd "$DESIRED/osquery.conf"
  run converge
  [ "$status" -ne 0 ]
  [ "$(privileged_call_count)" -eq 0 ]
}

@test "osquery not being installed at all is a quiet no-op" {
  rm -f "$TARGET/osquery.conf"
  run env PATH="$BIN:$PATH" \
    OSQUERY_CONVERGE_DESIRED_DIR="$DESIRED" \
    OSQUERY_CONVERGE_TARGET_DIR="$TARGET" \
    OSQUERY_CONVERGE_SUDO="$BIN/sudo" \
    OSQUERY_CONVERGE_OSQUERYCTL="$BIN/definitely-not-installed" \
    OSQUERY_CONVERGE_LOG_DIR="$LOG_DIR" \
    "$TOOL"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  [ "$(privileged_call_count)" -eq 0 ]
}

@test "an unknown argument is a usage error, never a silent full converge" {
  run converge --dry-run
  [ "$status" -ne 0 ]
  [[ $output == *usage* ]]
  [ "$(privileged_call_count)" -eq 0 ]
}

# --- the restart ------------------------------------------------------------

@test "a stop that fails does not stop the run, because a fresh host has nothing to stop" {
  rm -f "$TARGET/osquery.conf"
  OSQUERYCTL_STOP_EXIT=1 run converge
  [ "$status" -eq 0 ]
  restarted
}

@test "a start that fails is FATAL even while a daemon is still running" {
  # The case that separates an UNGUARDED start from a guarded one. Both calls
  # used to be silenced, so a stop that failed to unload followed by a start
  # that failed left the PREVIOUS daemon up on its PREVIOUS configuration while
  # the script printed 'osquery setup complete.': the liveness check alone
  # cannot catch that, because a daemon really is running. The start's own
  # status is what says the new configuration was never loaded.
  rm -f "$TARGET/osquery.conf"
  OSQUERYCTL_STOP_EXIT=1 OSQUERYCTL_START_EXIT=1 run converge
  [ "$status" -ne 0 ]
  [[ $output == *"'osqueryctl start' FAILED"* ]]
}

@test "a start that fails after a successful stop leaves no success line" {
  # The other half: here the stop worked, so the daemon really is gone. A
  # `[[ != ]]` and not a `! grep`, which set -e ignores inside bats.
  rm -f "$TARGET/osquery.conf"
  OSQUERYCTL_START_EXIT=1 run converge
  [ "$status" -ne 0 ]
  [[ $output != *"restarted osqueryd"* ]]
}

@test "a missing vendor plist refuses the restart and never stops the daemon" {
  # `osqueryctl start` copies that plist into /Library/LaunchDaemons. Without
  # it the stop would succeed and the start could not, so the check runs first.
  rm -f "$TARGET/io.osquery.agent.plist"
  rm -f "$TARGET/osquery.conf"
  run converge
  [ "$status" -ne 0 ]
  refute_log_has stop "$OSQUERYCTL_LOG"
  [[ $output == *io.osquery.agent.plist* ]]
}

@test "a config the daemon cannot parse refuses the restart and never stops the daemon" {
  rm -f "$TARGET/osquery.conf"
  OSQUERYCTL_CONFIG_CHECK_EXIT=1 run converge
  [ "$status" -ne 0 ]
  refute_log_has stop "$OSQUERYCTL_LOG"
}

@test "a daemon that never comes back is a loud failure, not a reported success" {
  rm -f "$TARGET/osquery.conf"
  OSQUERYCTL_START_LEAVES_DAEMON_DOWN=1 run converge
  [ "$status" -ne 0 ]
  [[ $output == *osqueryd* ]]
}

@test "a daemon that dies inside the settle window is a loud failure" {
  # launchctl load returns before the spawn, and the vendor plist sets KeepAlive
  # with ThrottleInterval 60, so a daemon present at t+1s and gone at t+3s is a
  # minute away from its next respawn. Present-once is not alive.
  rm -f "$TARGET/osquery.conf"
  printf '1\n' >"$DAEMON_LIVES"
  run converge
  [ "$status" -ne 0 ]
}

@test "the restart is judged on the ppid-1 parent, never on an arbitrary worker" {
  # The watchdog respawns workers on its own, so a worker pid proves nothing.
  rm -f "$TARGET/osquery.conf"
  run converge
  [ "$status" -eq 0 ]
  grep -qF -- '-P 1 -x osqueryd' "$BATS_TEST_TMPDIR/pgrep.argv"
}

# --- the log directory ------------------------------------------------------

@test "a missing log directory is created, because the daemon logs into it" {
  rm -rf "$LOG_DIR"
  run converge
  [ "$status" -eq 0 ]
  [ -d "$LOG_DIR" ]
}
