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
  #
  # The install arm matches an ABSOLUTE path as well as a bare name, because the
  # tool now names /usr/bin/install rather than letting the caller's PATH pick.
  cat >"$BIN/sudo" <<'STUB'
#!/bin/bash
printf '%s\n' "$*" >>"$SUDO_LOG"
[[ ${1:-} == -n ]] && shift
if [[ ${1:-} == install || ${1:-} == */install ]]; then
  shift
  creates_directory=''
  args=()
  while (($#)); do
    case "$1" in
      -o | -g) shift 2 ;;
      -d)
        creates_directory=1
        args+=("$1")
        shift
        ;;
      *)
        args+=("$1")
        shift
        ;;
    esac
  done
  # The SOURCE path and the mode of the directory holding it, recorded at the
  # instant root would read it. That pair is what proves the privileged read
  # happens out of a private copy rather than out of the deployed staging tree.
  if [[ -z $creates_directory ]]; then
    source_path="${args[@]: -2:1}" # install ... SOURCE DESTINATION
    source_directory="$(dirname "$source_path")"
    # GNU form first, BSD second: the portable order this repo enforces, because
    # the BSD form does not fail under GNU coreutils, it succeeds with garbage.
    source_mode="$(/usr/bin/stat -c '%a' "$source_directory" 2>/dev/null ||
      /usr/bin/stat -f '%Lp' "$source_directory" 2>/dev/null)"
    printf '%s %s\n' "$source_path" "$source_mode" >>"$INSTALL_SOURCE_LOG"
  fi
  exec install "${args[@]}"
fi
exec "$@"
STUB

  # osqueryctl: record each subcommand and answer with the programmed status. A
  # successful `start` is what publishes the daemon pid the restart check reads,
  # and it publishes a DIFFERENT pid from the one setup() leaves running, because
  # a real restart produces a new process. A test that wants to model a daemon
  # that never actually went away sets OSQUERYD_PID_AFTER_START to the old pid.
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
      printf '%s\n' "${OSQUERYD_PID_AFTER_START:-5150}" >"$DAEMON_PID_FILE"
    ;;
esac
exit 0
STUB

  # pgrep: only `pgrep -P 1 -x osqueryd` is asked for. The pid file IS the
  # daemon's liveness. DAEMON_LIVES models a daemon that answers a few probes
  # and is then gone, which is what a crash inside the settle window looks like.
  # DAEMON_RESPAWN_PID models the other shape of the same crash: the daemon is
  # never absent, but the pid answering is a NEW one from the first probe on.
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
if [[ -n ${DAEMON_RESPAWN_PID:-} ]]; then
  cat "$DAEMON_PID_FILE"
  printf '%s\n' "$DAEMON_RESPAWN_PID" >"$DAEMON_PID_FILE"
  exit 0
fi
cat "$DAEMON_PID_FILE"
STUB

  # stat: the REAL mode, a substituted owner pair. See the file docblock.
  #
  # The MODE TOKEN is honored rather than assumed. %p carries the file type and
  # the setuid, setgid and sticky bits; %Lp carries only the low nine
  # permission bits. Which one the tool asks for is the difference between
  # seeing a setuid config in the root daemon's directory and reading it as an
  # ordinary 0644 file, so a stub that answered the same thing either way would
  # make that choice untestable.
  cat >"$BIN/stat" <<'STUB'
#!/bin/bash
case "${1:-}" in
  -c) exit 1 ;; # a BSD host has no -c; keep the tool on the branch it really takes
  -f) ;;
  *) exit 1 ;;
esac
path="${3:-}"
mode="$(/usr/bin/stat -c '%a' "$path" 2>/dev/null || /usr/bin/stat -f '%p' "$path" 2>/dev/null)" || exit 1
[[ ${2:-} == *%Lp* ]] && mode="$(printf '%o' "$((8#$mode & 0777))")"
# The owner substitution is SCOPED to one subtree, defaulting to the target
# directory the stub exists for. The tool probes owners for two unrelated
# reasons now: the live files under /var/osquery, and the directory holding the
# privileged binary it is about to hand to sudo. One global override could not
# model a non-root config without also declaring the stub's own bin directory
# untrustworthy, which is a different test's subject.
uid="${FAKE_STAT_UID:-0}"
gid="${FAKE_STAT_GID:-0}"
if [[ -n ${FAKE_STAT_UID_SCOPE:-} && $path != "$FAKE_STAT_UID_SCOPE"* ]]; then
  uid=0
  gid=0
fi
printf '%s %s %s\n' "$mode" "$uid" "$gid"
STUB

  # cmp: record the argv, then answer with the real cmp. The tool compares the
  # desired bytes against the live ones exactly here, so the recorded FIRST path
  # is the only evidence of WHICH desired copy decided the verdict. Delegating
  # rather than emulating keeps every content verdict a real byte comparison.
  cat >"$BIN/cmp" <<'STUB'
#!/bin/bash
printf '%s\n' "$*" >>"$CMP_LOG"
exec /usr/bin/cmp "$@"
STUB

  # An instant sleep: the restart poll waits in quarter seconds and nothing here
  # asserts on the waiting.
  ln -sf /usr/bin/true "$BIN/sleep"
  chmod +x "$BIN/sudo" "$BIN/osqueryctl" "$BIN/pgrep" "$BIN/stat" "$BIN/cmp"

  export TOOL BIN PROTOTYPE
}

setup() {
  # The sandbox root, CANONICAL. The tool refuses a staging directory reached
  # through a symlink at any component, and on macOS TMPDIR sits under /var,
  # which IS a symlink to /private/var. Resolving the root once here is what
  # keeps that refusal a real assertion instead of something every test trips on.
  ROOT="$(cd -P "$BATS_TEST_TMPDIR" && pwd -P)"
  DESIRED="$ROOT/desired"
  TARGET="$ROOT/var-osquery"
  LOG_DIR="$ROOT/log/osquery"
  SUDO_LOG="$ROOT/sudo.log"
  OSQUERYCTL_LOG="$ROOT/osqueryctl.log"
  INSTALL_SOURCE_LOG="$ROOT/install-source.log"
  DAEMON_PID_FILE="$ROOT/daemon.pid"
  DAEMON_LIVES="$ROOT/daemon.lives"
  PGREP_ARGV="$ROOT/pgrep.argv"
  CMP_LOG="$ROOT/cmp.log"

  cp -R "$PROTOTYPE/." "$ROOT/"
  : >"$SUDO_LOG"
  : >"$OSQUERYCTL_LOG"
  : >"$INSTALL_SOURCE_LOG"
  : >"$PGREP_ARGV"
  : >"$CMP_LOG"
  printf '4242\n' >"$DAEMON_PID_FILE"

  # The default subtree the stat stub substitutes an owner for: the live tree,
  # which is what a sandbox cannot make root-owned. A test about the trust check
  # on a privileged binary moves the scope to the stub bin directory instead.
  FAKE_STAT_UID_SCOPE="$TARGET"

  export SUDO_LOG OSQUERYCTL_LOG INSTALL_SOURCE_LOG DAEMON_PID_FILE DAEMON_LIVES PGREP_ARGV
  export CMP_LOG
  export FAKE_STAT_UID_SCOPE
}

# The sandbox drive. OSQUERY_CONVERGE_TEST_SEAM is what unlocks the overrides
# below: without it the tool refuses to be pointed at a different desired state,
# a different target directory or a different privileged binary, because in
# production those are an escalation rather than a knob. The trusted uid is NOT
# among them: the stat stub already answers uid 0 for every path, so the tool's
# own production comparison against root is what runs here, and a test that wants
# the untrusted case moves FAKE_STAT_UID instead.
converge() {
  PATH="$BIN:$PATH" \
    OSQUERY_CONVERGE_TEST_SEAM=1 \
    OSQUERY_CONVERGE_DESIRED_DIR="$DESIRED" \
    OSQUERY_CONVERGE_TARGET_DIR="$TARGET" \
    OSQUERY_CONVERGE_SUDO="$BIN/sudo" \
    OSQUERY_CONVERGE_OSQUERYCTL="${OSQUERYCTL_STUB:-$BIN/osqueryctl}" \
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

# The recorded argv of the ONE privileged install that wrote <destination>, or
# nothing. Read out of the sudo log rather than reconstructed, so an assertion
# about the privileged call is an assertion about the call that really happened.
install_line_for() { # <destination>
  grep -F -- " $1" "$SUDO_LOG" | grep -F -- '/install ' | head -1
}

# The SOURCE the privileged install read <destination> from, taken from the same
# recorded argv.
install_source_for() { # <destination>
  local line source
  line="$(install_line_for "$1")"
  [[ -n $line ]] || return 1
  source="${line% "$1"}"
  printf '%s' "${source##* }"
}

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

@test "a path owned by a non-root user is reinstalled, files and directories alike" {
  # The dimension the stubbed stat exists for: a sandbox cannot make a
  # root-owned file, so the owner pair is substituted. osqueryd runs these
  # queries AS ROOT, so a config owned by the login user is one that user can
  # rewrite before the daemon's next config load, whatever its bytes say.
  FAKE_STAT_UID=501 run converge
  [ "$status" -eq 0 ]
  [[ "$(install_line_for "$TARGET/osquery.conf")" == "-n /usr/bin/install -o root -g wheel -m 0644 "*" $TARGET/osquery.conf" ]]
  grep -qF -- "-n /usr/bin/install -d -o root -g wheel -m 0755 $TARGET" "$SUDO_LOG"
}

@test "a setuid bit on a live file reads as drift, not as a matching 0644" {
  # Why the probe asks stat for %p and not %Lp: %Lp prints only the low nine
  # permission bits, so a setuid file in the root daemon's directory would come
  # back looking like an ordinary 0644 config and be passed over. The repair
  # line is the assertion rather than the resulting mode, because %Lp reads 644
  # off a setuid file too.
  chmod u+s "$TARGET/osquery.conf"
  run converge
  [ "$status" -eq 0 ]
  [[ "$(install_line_for "$TARGET/osquery.conf")" == "-n /usr/bin/install -o root -g wheel -m 0644 "*" $TARGET/osquery.conf" ]]
}

@test "a symlink standing at the config path is replaced by a regular file" {
  # A crafted link is the case the type check exists for: this one wears the
  # desired bytes and a 0644 mode of its own, so every other dimension reads as
  # converged and only its TYPE says otherwise. Leaving it would point the root
  # daemon's config at a path its author still controls. `install` replaces the
  # link rather than writing through it (measured), so the repair is safe once
  # the link is refused as a file.
  cp "$DESIRED/osquery.conf" "$BATS_TEST_TMPDIR/referent.conf"
  rm -f "$TARGET/osquery.conf"
  ln -s "$BATS_TEST_TMPDIR/referent.conf" "$TARGET/osquery.conf"
  chmod -h 0644 "$TARGET/osquery.conf"
  run converge
  [ "$status" -eq 0 ]
  [ ! -L "$TARGET/osquery.conf" ]
  cmp -s "$DESIRED/osquery.conf" "$TARGET/osquery.conf"
}

@test "the install carries owner, group and mode in ONE call" {
  # Not a tee-then-chmod pair: a file that exists between the two carries the
  # creating umask and the invoking owner for that window.
  rm -f "$TARGET/osquery.flags"
  run converge
  [ "$status" -eq 0 ]
  [[ "$(install_line_for "$TARGET/osquery.flags")" == "-n /usr/bin/install -o root -g wheel -m 0644 "*" $TARGET/osquery.flags" ]]
}

@test "the privileged install names /usr/bin/install, never a PATH lookup" {
  # `sudo -n` preserves the caller's PATH, and this host's PATH leads with
  # /opt/homebrew/bin, which is group-writable and owned by the operator. A bare
  # `install` in a privileged call is therefore a name any process at the
  # operator's privilege level can answer, and root would run their binary.
  rm -f "$TARGET/osquery.flags"
  run converge
  [ "$status" -eq 0 ]
  [[ "$(install_line_for "$TARGET/osquery.flags")" == "-n /usr/bin/install "* ]]
  refute_log_has '-n install ' "$SUDO_LOG"
}

@test "an osqueryctl resolved into a directory root does not own is refused, never handed to sudo" {
  # The same PATH hole, on the other privileged binary: /opt/homebrew/bin is
  # owned by the operator, so a name resolved there is a name a user-level
  # process can answer for root.
  rm -f "$TARGET/osquery.conf"
  FAKE_STAT_UID=501 FAKE_STAT_UID_SCOPE="$BIN" run converge
  [ "$status" -ne 0 ]
  [[ $output == *osqueryctl* ]]
  [ "$(privileged_call_count)" -eq 0 ]
}

@test "an osqueryctl resolved into a group-writable directory is refused too" {
  # /opt/homebrew/bin is drwxrwxr-x, so ownership alone is not the whole test:
  # a directory anyone in the group can write is a directory whose entries they
  # can replace.
  mkdir -p "$ROOT/loose-bin"
  cp "$BIN/osqueryctl" "$ROOT/loose-bin/osqueryctl"
  chmod 0775 "$ROOT/loose-bin"
  rm -f "$TARGET/osquery.conf"
  OSQUERYCTL_STUB="$ROOT/loose-bin/osqueryctl" run converge
  [ "$status" -ne 0 ]
  [[ $output == *loose-bin* ]]
  [ "$(privileged_call_count)" -eq 0 ]
}

@test "a seam variable set without the test seam is refused, so it is not a production knob" {
  # Pointing the desired state at another tree is root installing THAT tree's
  # bytes into the root daemon's configuration directory, so the override has to
  # be unavailable to anything but a test that says so.
  #
  # `env -u` SCRUBS the seam rather than trusting it to be absent. This case names
  # one override and no target directory and no sudo, which is the whole point of
  # it; inherit OSQUERY_CONVERGE_TEST_SEAM=1 from the surrounding environment and
  # those two fall back to /var/osquery and /usr/bin/sudo, and the case that
  # asserts a refusal converges the REAL machine instead. That is not theoretical:
  # it happened, and it is why the tool now also refuses a seam engaged without
  # them (pinned below).
  run env -u OSQUERY_CONVERGE_TEST_SEAM PATH="$BIN:$PATH" OSQUERY_CONVERGE_DESIRED_DIR="$DESIRED" "$TOOL"
  [ "$status" -ne 0 ]
  [[ $output == *OSQUERY_CONVERGE_DESIRED_DIR* ]]
  [ "$(privileged_call_count)" -eq 0 ]
}

@test "the test seam without a target directory is refused, never defaulted to /var/osquery" {
  # The seam engaged with only SOME overrides is the shape that converged the live
  # machine out of a bats sandbox: every value is individually legal, and the two
  # that were omitted default to production.
  run env PATH="$BIN:$PATH" OSQUERY_CONVERGE_TEST_SEAM=1 \
    OSQUERY_CONVERGE_DESIRED_DIR="$DESIRED" "$TOOL"
  [ "$status" -ne 0 ]
  [[ $output == *OSQUERY_CONVERGE_TARGET_DIR* ]]
  refute_log_has '/var/osquery' "$SUDO_LOG"
  [ "$(privileged_call_count)" -eq 0 ]
}

@test "the test seam without a sudo is refused too, so root is never the real one" {
  run env PATH="$BIN:$PATH" OSQUERY_CONVERGE_TEST_SEAM=1 \
    OSQUERY_CONVERGE_DESIRED_DIR="$DESIRED" OSQUERY_CONVERGE_TARGET_DIR="$TARGET" "$TOOL"
  [ "$status" -ne 0 ]
  [[ $output == *OSQUERY_CONVERGE_SUDO* ]]
  [ "$(privileged_call_count)" -eq 0 ]
}

@test "the privileged install reads from a private staging copy, not from the deployed tree" {
  # The source symlink race: the -L check on a deployed staging file and the
  # `install` that reads it are far apart, and `install` reads its source AS
  # ROOT. Copying to a private 0700 directory first means the path root reads is
  # one no other process can substitute between the check and the read.
  rm -f "$TARGET/osquery.flags"
  run converge
  [ "$status" -eq 0 ]
  local source
  source="$(install_source_for "$TARGET/osquery.flags")"
  [[ -n $source ]]
  [[ $source != "$DESIRED"/* ]]
  # The mode of the directory root read from, recorded by the sudo stub at the
  # moment of the call.
  grep -qE -- "^$source 700$" "$INSTALL_SOURCE_LOG"
}

@test "the content comparison reads the private copy, not the deployed staging tree" {
  # The install side of this is pinned above; this is the VERDICT side. A
  # comparison against the deployed tree while the install reads the private copy
  # lets the two disagree: the bytes that decided "converged" are then not the
  # bytes that would have been written, so a staging file swapped after the copy
  # is taken produces a verdict for content that never gets installed.
  printf 'drifted\n' >"$TARGET/osquery.conf"
  run converge
  [ "$status" -eq 0 ]
  # Something really was compared, or the two assertions below are vacuous.
  [ -s "$CMP_LOG" ]
  # Every comparison names the private stage as its desired side, and none of
  # them names the deployed staging tree.
  refute_log_has "$DESIRED/" "$CMP_LOG"
  grep -q '/osquery-converge\.' "$CMP_LOG"
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
  grep -qF -- "-n /usr/bin/install -d -o root -g wheel -m 0755 $TARGET" "$SUDO_LOG"
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

@test "a symlink planted in the desired tree under an unlisted name is refused too" {
  # The staging scan is `! -type d` rather than `-type f` precisely so a link is
  # seen: `-type f` would look straight past this one, and the file the tool
  # never installs would sit there being ignored forever.
  ln -s /etc/passwd "$DESIRED/packs/planted-link.conf"
  rm -f "$TARGET/osquery.conf"
  run converge
  [ "$status" -ne 0 ]
  [[ $output == *planted-link.conf* ]]
  [ "$(privileged_call_count)" -eq 0 ]
}

@test "a planted file is matched on its exact relative path, not on a pattern or a basename" {
  # Two ways that match could weaken and still pass the plain case above: an
  # unquoted right-hand side turns the staged name into a glob, so a file
  # called `*.conf` would match a listed pack and be waved through; and a
  # basename comparison would wave through a pack sitting in the wrong
  # directory. Both end in a staged file that is never installed and never
  # mentioned.
  printf 'planted\n' >"$DESIRED/packs/*.conf"
  printf 'planted\n' >"$DESIRED/intrusion-detection.conf"
  rm -f "$TARGET/osquery.conf"
  run converge
  [ "$status" -ne 0 ]
  [[ $output == *"packs/*.conf"* ]]
  [[ $output == *"desired/intrusion-detection.conf"* ]]
  [ "$(privileged_call_count)" -eq 0 ]
}

@test "a desired file that cannot be read is never treated as converged" {
  # Once the desired state is copied into a private directory before anything is
  # compared, an unreadable STAGING file is caught by the copy rather than by the
  # comparison: this pins the refusal, and that it costs no privileged call. The
  # comparison's own error direction is the LIVE case below, which is where cmp
  # can still fail.
  chmod 000 "$DESIRED/osquery.conf"
  run converge
  [ "$status" -ne 0 ]
  [[ $output != *"restarted osqueryd"* ]]
  [ "$(privileged_call_count)" -eq 0 ]
}

@test "a LIVE file that cannot be compared is reinstalled, never counted as converged" {
  # cmp answers 2 when it could not compare, which is not evidence that the
  # bytes match. Reading it as a match would pass over the file in silence and
  # report a converged tree, so an unreadable comparison reinstalls instead.
  #
  # The live file is the case that still reaches this once the desired state is
  # copied privately first, and it is the realistic one: /var/osquery is
  # root-owned and this tool runs unprivileged until the install.
  #
  # An ACL rather than `chmod 000`, deliberately. Removing the mode bits would
  # drift the MODE as well, so the file would be reinstalled either way and the
  # test could only ever assert which label the repair line carried. A deny-read
  # ACL leaves the mode 0644 and the owner intact, so unreadability is the only
  # thing wrong: read as a match, the tree is fully converged and NOTHING
  # happens, which is the silent pass this pins.
  chmod +a "everyone deny read" "$TARGET/osquery.conf"
  run converge
  [ "$status" -eq 0 ]
  [[ "$(install_line_for "$TARGET/osquery.conf")" == "-n /usr/bin/install "* ]]
  [[ $output == *unreadable* ]]
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
    OSQUERY_CONVERGE_TEST_SEAM=1 \
    OSQUERY_CONVERGE_TRUSTED_UID="$(id -u)" \
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

@test "a symlink standing where the target directory belongs is refused, never repaired through" {
  # MEASURED: `install -d` on a preplanted symlink exits 0, chmods the REFERENT
  # and leaves the link in place. Repairing an irregular directory would
  # therefore hand the root daemon's configuration directory to wherever the
  # link points, so the only safe answer is to refuse before any privileged call.
  mkdir -p "$ROOT/decoy"
  chmod 0700 "$ROOT/decoy"
  rm -rf "$TARGET"
  ln -s "$ROOT/decoy" "$TARGET"
  run converge
  [ "$status" -ne 0 ]
  [ "$(privileged_call_count)" -eq 0 ]
  [[ $output == *"$TARGET"* ]]
  [ "$(deployed_mode "$ROOT/decoy")" = 700 ]
  [ -L "$TARGET" ]
}

@test "a symlink standing where the packs directory belongs claims nothing and repairs nothing" {
  # The measured end state before this refusal existed: `install -d` exits 0
  # without replacing the link, the tool prints "repaired ... (not a regular
  # file)" and exits 0, and all four packs are then written THROUGH the link.
  # Three separate lies in one run, so all three are pinned here: no repair line,
  # no success, and nothing written into the referent.
  #
  # The target directory is left needing a repair of its own, which the tool
  # would reach FIRST. Zero privileged calls is therefore also the assertion that
  # both directory verdicts are taken before either one is acted on.
  mkdir -p "$ROOT/decoy-packs"
  rm -rf "$TARGET/packs"
  ln -s "$ROOT/decoy-packs" "$TARGET/packs"
  chmod 0775 "$TARGET"
  run converge
  [ "$status" -ne 0 ]
  [ "$(privileged_call_count)" -eq 0 ]
  [[ $output != *repaired* ]]
  [[ $output != *installed* ]]
  [ ! -s "$OSQUERYCTL_LOG" ]
  [ -L "$TARGET/packs" ]
  [ -z "$(ls -A "$ROOT/decoy-packs")" ]
}

@test "a staging directory that is itself a symlink is refused, not followed" {
  # Both completeness halves are defeated by this one move: `find <dir>` does not
  # descend a symlinked directory ARGUMENT, and `[[ -L $dir/file ]]` resolves the
  # directory component before testing the leaf. So a planted file in the
  # referent is neither listed nor seen as a link, and the tool would install
  # from a tree it never checked.
  mkdir -p "$ROOT/substitute/packs"
  cp -R "$DESIRED/." "$ROOT/substitute/"
  printf 'planted\n' >"$ROOT/substitute/packs/planted.conf"
  rm -rf "$DESIRED"
  ln -s "$ROOT/substitute" "$DESIRED"
  rm -f "$TARGET/osquery.conf"
  run converge
  [ "$status" -ne 0 ]
  [ "$(privileged_call_count)" -eq 0 ]
}

@test "a staging directory reached through a symlinked PARENT is refused too" {
  # The same defeat one component higher up, which a check on the leaf alone
  # would pass.
  mkdir -p "$ROOT/elsewhere"
  mv "$DESIRED" "$ROOT/elsewhere/desired"
  ln -s "$ROOT/elsewhere" "$ROOT/parent-link"
  rm -f "$TARGET/osquery.conf"
  DESIRED="$ROOT/parent-link/desired" run converge
  [ "$status" -ne 0 ]
  [ "$(privileged_call_count)" -eq 0 ]
}

@test "a staging tree the tool cannot fully read is a refusal, not a silent pass" {
  # find's exit status is what says the listing is complete. Read through a
  # process substitution it is discarded, so an unreadable subdirectory hid
  # whatever it held and the run reported success over a tree it never saw.
  mkdir -p "$DESIRED/packs/unreadable"
  printf 'planted\n' >"$DESIRED/packs/unreadable/planted.conf"
  chmod 0000 "$DESIRED/packs/unreadable"
  rm -f "$TARGET/osquery.conf"
  run converge
  chmod 0755 "$DESIRED/packs/unreadable"
  [ "$status" -ne 0 ]
  [ "$(privileged_call_count)" -eq 0 ]
}

@test "an unknown argument is a usage error, never a silent full converge" {
  run converge --dry-run
  [ "$status" -ne 0 ]
  [[ $output == *usage* ]]
  [ "$(privileged_call_count)" -eq 0 ]
}

# --- the restart ------------------------------------------------------------

@test "a stop that fails with no daemon running does not stop the run, because a fresh host has nothing to stop" {
  # The legitimate half of a failing stop: no LaunchDaemon is loaded and no plist
  # sits in /Library/LaunchDaemons, so the vendor stop really does fail and the
  # start that follows is the first daemon this host has had.
  rm -f "$TARGET/osquery.conf"
  : >"$DAEMON_PID_FILE"
  OSQUERYCTL_STOP_EXIT=1 run converge
  [ "$status" -eq 0 ]
  restarted
}

@test "a daemon whose parent pid never changed is not a restart, however the stop exited" {
  # `osqueryctl stop` is `launchctl unload` plus an rm, and launchctl LOGS a
  # failure while exiting 0, so neither the stop's status nor a liveness check
  # can tell a bounced daemon from one that never went away. The pid recorded
  # BEFORE the stop is what can: an unchanged parent is the old process still
  # running the old configuration.
  rm -f "$TARGET/osquery.conf"
  OSQUERYCTL_STOP_EXIT=1 OSQUERYD_PID_AFTER_START=4242 run converge
  [ "$status" -ne 0 ]
  [[ $output != *"restarted osqueryd"* ]]
  [[ $output == *4242* ]]
}

@test "a restart is claimed only when the parent pid actually changed" {
  rm -f "$TARGET/osquery.conf"
  run converge
  [ "$status" -eq 0 ]
  [[ $output == *"restarted osqueryd"* ]]
  [[ $output == *5150* ]]
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

@test "a symlink standing in for the vendor plist refuses the restart" {
  # `-f` follows a link, so a preplanted symlink here reads as a healthy vendor
  # plist and `osqueryctl start` would copy its referent into
  # /Library/LaunchDaemons and load it as root. The file belongs to the osquery
  # package, so anything but a regular file at that path is a refusal.
  rm -f "$TARGET/io.osquery.agent.plist"
  printf 'attacker plist\n' >"$ROOT/attacker.plist"
  ln -s "$ROOT/attacker.plist" "$TARGET/io.osquery.agent.plist"
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

@test "a daemon that comes back under a NEW pid inside the settle window is a failure" {
  # The other shape of a crash: nothing is ever absent, so a check for "is an
  # osqueryd there" is satisfied throughout. What happened is that the daemon
  # this run started died and KeepAlive replaced it, which is why liveness is
  # the SAME parent staying up rather than any parent being present.
  rm -f "$TARGET/osquery.conf"
  DAEMON_RESPAWN_PID=9999 run converge
  [ "$status" -ne 0 ]
  [[ $output == *"gone again"* ]]
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

@test "creating the log directory does not bounce the root daemon" {
  # Deliberately outside the restart decision: a directory that did not exist
  # changes nothing a running daemon holds in memory, and it is unprivileged
  # and ours. Folding it in would restart osqueryd over a mkdir, which is a far
  # heavier act than the condition warrants.
  rm -rf "$LOG_DIR"
  run converge
  [ "$status" -eq 0 ]
  [ ! -s "$OSQUERYCTL_LOG" ]
  [ "$(privileged_call_count)" -eq 0 ]
}
