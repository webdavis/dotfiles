#!/usr/bin/env bash
#
# osquery-converge.sh -- make /var/osquery hold OUR desired state, and restart
# osqueryd only when something had drifted.
#
# WHY THIS EXISTS. The osquery cask reinstalls the vendor package on upgrade and
# wipes our files out of /var/osquery (osquery.conf, packs/*.conf,
# osquery.flags) while repopulating the vendor's own. That upgrade runs
# UNATTENDED from the weekly Homebrew job, so before this tool the machine could
# lose its whole detection config with nobody present, and the setup script that
# wrote those files refired only when its OWN content changed. Nothing converged
# after an external wipe.
#
# THE CONTRACT. No drift means no privileged call, no restart, and NO OUTPUT: a
# no-op apply prints nothing (operator ruling 2026-08-05). Drift means one line
# per repaired path naming what was wrong, and every failure is loud, on stderr,
# with a non-zero exit. Success is only ever claimed on measured evidence, which
# is why the restart ends in a verified daemon rather than in a printed sentence.
#
# TWO CALLERS, both safe on every run because this is idempotent and silent when
# there is nothing to do:
#   - .chezmoiscripts/run_after_50-setup-osquery.sh, on every apply;
#   - the weekly Homebrew upgrade job, right after its upgrade pass, which is
#     what closes the unattended wipe window.
#
# THE DESIRED STATE is a set of ordinary chezmoi targets under
# osquery-converge/desired/, so every repair channel reads one rendered source
# of truth and this file holds no heredocs. KNOWN LIMIT: `chezmoi apply
# --exclude=templates` does not refresh the two templated ones
# (desired/osquery.conf and desired/packs/agent-attack-surface.conf), so config
# CHANGES ship on a full apply. The wipe-repair case needs only the staging that
# is already there, which every apply flavor reads.
#
# WHAT IS NOT OURS. /var/osquery/io.osquery.agent.plist, the certs and the
# lenses belong to the vendor package. They are never repaired here. The plist
# is checked before any restart because `osqueryctl start` copies it into
# /Library/LaunchDaemons and `osqueryctl stop` deletes that copy, so a stop with
# no plist behind it leaves the daemon GONE rather than stale.
#
# ROOT BLAST RADIUS. osqueryd runs the queries in these files AS ROOT. That is
# why they live in root-owned /var/osquery rather than under ~/.config (any
# process at the user's privilege level could rewrite those, and the daemon
# would then run them as root), and it is why the drift check compares mode,
# owner and group as well as content: right bytes under a 0666 mode is the same
# escalation with an extra step. Every decision lives in
# osquery-converge/drift-verdict.sh as a pure function of observed state; this
# file owns the probing, the privileged calls and the restart.
set -euo pipefail

# The desired state, beside this script in both the source tree and the deployed
# one, so a sandbox copy resolves the same way the deployment does.
OSQUERY_CONVERGE_DESIRED_DIR="${OSQUERY_CONVERGE_DESIRED_DIR:-$(dirname "${BASH_SOURCE[0]}")/osquery-converge/desired}"
OSQUERY_CONVERGE_TARGET_DIR="${OSQUERY_CONVERGE_TARGET_DIR:-/var/osquery}"
# osqueryd's filesystem logger writes here (osquery.conf's logger_path), so the
# directory has to exist before the daemon loads that config.
OSQUERY_CONVERGE_LOG_DIR="${OSQUERY_CONVERGE_LOG_DIR:-$HOME/.local/log/osquery}"
# `sudo -n` and never a bare sudo: the weekly job runs under launchd with no
# terminal, so a sudo configuration that started asking for a password must fail
# loudly here rather than block a scheduled job forever. This host's operator has
# passwordless sudo (recorded in run_after_05's docblock), so -n is free today.
OSQUERY_CONVERGE_SUDO="${OSQUERY_CONVERGE_SUDO:-sudo}"
OSQUERY_CONVERGE_OSQUERYCTL="${OSQUERY_CONVERGE_OSQUERYCTL:-osqueryctl}"

# The desired-state files, relative to both directories. NAMED, never globbed:
# this list is what gets installed root-owned into a directory the root daemon
# reads, so a file that appears in the staging tree without also appearing here
# is inert rather than promoted. Adding a pack is a deliberate edit here, beside
# the entry it also needs in desired/osquery.conf.
#
# osquery.flags carries the CLI-only flags. The event flags in it are ignored
# when set only in the config's "options" block, which is why they live in a
# flagfile at all; --disable_extensions removes the extension-autoload attack
# surface (unused here) and the logger_rotate* trio caps filesystem-logger growth
# at 5 files of 10 MB.
OSQUERY_CONVERGE_FILES=(
  osquery.conf
  osquery.flags
  packs/agent-attack-surface.conf
  packs/installed-software-drift.conf
  packs/intrusion-detection.conf
  packs/security-policy-regression.conf
)

# How long to wait for the restarted daemon to appear, and how long it then has
# to stay up before the restart counts as verified. Both validated by shape and
# defaulted on anything surprising, so a fat-fingered environment cannot turn a
# bound into unbounded waiting (the ssh-hardening deadline idiom).
OSQUERY_CONVERGE_RESTART_DEADLINE="${OSQUERY_CONVERGE_RESTART_DEADLINE:-30}"
[[ $OSQUERY_CONVERGE_RESTART_DEADLINE =~ ^[1-9][0-9]{0,3}$ ]] || OSQUERY_CONVERGE_RESTART_DEADLINE=30
OSQUERY_CONVERGE_SETTLE_SECONDS="${OSQUERY_CONVERGE_SETTLE_SECONDS:-5}"
[[ $OSQUERY_CONVERGE_SETTLE_SECONDS =~ ^[1-9][0-9]{0,3}$ ]] || OSQUERY_CONVERGE_SETTLE_SECONDS=5
# The poll interval, and the ticks that make a second of it. Bash has no
# wait-with-timeout and stock macOS ships no timeout(1), so the bound is counted
# in ticks rather than measured, which is what keeps it from drifting away from
# the seconds the operator-facing messages quote.
OSQUERY_CONVERGE_POLL_INTERVAL='0.25'
OSQUERY_CONVERGE_POLL_TICKS_PER_SECOND=4

# shellcheck source=dot_local/libexec/osquery/osquery-converge/drift-verdict.sh
source "$(dirname "${BASH_SOURCE[0]}")/osquery-converge/drift-verdict.sh"

usage() {
  printf 'usage: osquery-converge.sh\n' >&2
}

fail() {
  printf 'osquery-converge: %s\n' "$1" >&2
}

report() {
  printf 'osquery-converge: %s\n' "$1"
}

# An unknown argument is an ERROR, never a silent fallthrough to a full converge:
# a typo in a caller would otherwise run privileged work nobody asked for.
for argument in "$@"; do
  usage
  printf 'osquery-converge: unknown argument: %s\n' "$argument" >&2
  exit 2
done

# osquery not installed at all: there is no daemon to converge for and no vendor
# layout to converge into. Quiet, so a machine that simply does not run osquery
# adds nothing to an apply.
command -v "$OSQUERY_CONVERGE_OSQUERYCTL" >/dev/null 2>&1 || exit 0

run_privileged() {
  "$OSQUERY_CONVERGE_SUDO" -n "$@"
}

# probe_kind <path>: the path's type as the verdict functions name it. A symlink
# is reported BEFORE anything else and never followed, because `install` follows
# it: a link planted at /var/osquery/osquery.conf would redirect the root
# daemon's config to wherever it points.
probe_kind() {
  if [[ -L $1 ]]; then
    printf 'symlink'
  elif [[ ! -e $1 ]]; then
    printf 'absent'
  elif [[ -f $1 ]]; then
    printf 'file'
  elif [[ -d $1 ]]; then
    printf 'directory'
  else
    printf 'other'
  fi
}

# probe_attributes <path>: "<mode> <uid> <gid>" with the mode as exactly four
# octal digits, or NOTHING when any of the three could not be read. One stat
# call, GNU form first and BSD second (the portable order this feature-set uses).
#
# BSD is asked for %p, never %Lp: %Lp prints only the low NINE permission bits,
# so a setuid, setgid or sticky bit on a file the root daemon reads would come
# back looking like an ordinary mode. %p carries the file type as well, so the
# low four octal digits are taken from whichever form answered and both platforms
# yield the same string. The value is shape-checked BEFORE it is sliced.
probe_attributes() {
  local raw mode uid gid
  raw="$(stat -c '%a %u %g' "$1" 2>/dev/null || stat -f '%p %u %g' "$1" 2>/dev/null)" || return 0
  read -r mode uid gid <<<"$raw"
  [[ $mode =~ ^[0-7]{1,7}$ && $uid =~ ^[0-9]{1,10}$ && $gid =~ ^[0-9]{1,10}$ ]] || return 0
  mode="000$mode"
  printf '%s %s %s' "${mode: -4}" "$uid" "$gid"
}

# probe_content_equality <desired> <live>: 1 when the bytes match, 0 when they
# differ, EMPTY when the comparison could not be made. cmp answers 2 on an error
# (an unreadable file), which must never be read as "differs" or as "matches":
# the verdict treats an empty answer as unreadable and reinstalls. The deployed
# files are world-readable 0644, so this needs no privilege.
probe_content_equality() {
  local status=0
  cmp -s "$1" "$2" || status=$?
  case "$status" in
    0) printf '1' ;;
    1) printf '0' ;;
    *) ;;
  esac
}

# drift_label <verdict>: the operator-facing phrase for a verdict token. A CLOSED
# vocabulary, so a repair line carries one of these literals and never a token
# lifted out of something observed.
drift_label() {
  case "$1" in
    absent) printf 'missing' ;;
    irregular) printf 'not a regular file' ;;
    unreadable) printf 'unreadable' ;;
    content) printf 'content drift' ;;
    mode) printf 'mode drift' ;;
    owner) printf 'owner drift' ;;
    group) printf 'group drift' ;;
    *) printf 'drift' ;;
  esac
}

# The desired state must be COMPLETE before anything privileged happens. A
# missing staging file means the deploy is broken, and passing over it would
# leave a wiped /var/osquery file wiped while the run reported success; a symlink
# there would install through to the referent. Both are refusals, and both are
# checked before the first sudo so a broken staging tree costs no privileged call.
assert_desired_state_is_complete() {
  local relative desired incomplete=0
  for relative in "${OSQUERY_CONVERGE_FILES[@]}"; do
    desired="$OSQUERY_CONVERGE_DESIRED_DIR/$relative"
    if [[ -L $desired ]]; then
      fail "the desired state at $desired is a symlink; refusing to install through it"
      incomplete=1
    elif [[ ! -f $desired ]]; then
      fail "the desired state for $relative is not deployed at $desired, so /var/osquery cannot be converged; run a full 'chezmoi apply'"
      incomplete=1
    fi
  done
  [[ $incomplete -eq 0 ]]
}

# directory_verdict_for <path> / file_verdict_for <relative-path>: probe, then
# ask the decision core. They print ONLY the verdict token, and repair nothing:
# the repair pair below owns the privileged calls and the reporting.
#
# The split is not cosmetic. A function that both printed a verdict for its
# caller to capture AND reported its repairs on stdout would have every repair
# line swallowed into the captured verdict, which is a repair that happened in
# silence: the exact failure this whole tool exists to end.
directory_verdict_for() {
  local path="$1" kind attributes mode uid gid
  kind="$(probe_kind "$path")"
  attributes="$(probe_attributes "$path")"
  read -r mode uid gid <<<"$attributes"
  osquery_converge_directory_verdict "$kind" "${mode:-}" "${uid:-}" "${gid:-}"
}

file_verdict_for() {
  local relative="$1" desired live kind attributes mode uid gid content_equal
  desired="$OSQUERY_CONVERGE_DESIRED_DIR/$relative"
  live="$OSQUERY_CONVERGE_TARGET_DIR/$relative"
  kind="$(probe_kind "$live")"
  attributes="$(probe_attributes "$live")"
  read -r mode uid gid <<<"$attributes"
  content_equal=''
  [[ $kind == file ]] && content_equal="$(probe_content_equality "$desired" "$live")"
  osquery_converge_file_verdict "$kind" "$content_equal" "${mode:-}" "${uid:-}" "${gid:-}"
}

# repair_directory <path> <verdict>: create the directory, or put its attributes
# back. This carries the role the old setup script's UNCONDITIONAL
# `sudo install -d` had; making it conditional on a verdict is what buys the
# quiet no-op, so the check moved into the verdict rather than the repair being
# dropped.
repair_directory() {
  run_privileged install -d -o root -g wheel -m 0755 "$1"
  report "repaired $1 ($(drift_label "$2"))"
}

# repair_file <relative-path> <verdict>: install the desired state over whatever
# is there.
#
# ONE `install` call carrying owner, group AND mode, never a tee-then-chmod pair:
# a file that exists between those two steps carries the creating umask and the
# invoking owner for that window, and this file is read by a root daemon.
repair_file() {
  local live="$OSQUERY_CONVERGE_TARGET_DIR/$1"
  run_privileged install -o root -g wheel -m 0644 "$OSQUERY_CONVERGE_DESIRED_DIR/$1" "$live"
  report "installed $live ($(drift_label "$2"))"
}

# daemon_parent_pid: the pid of the osqueryd the LaunchDaemon owns, or nothing.
#
# THE PARENT, selected by ppid 1, is the subject of every liveness statement
# here. osqueryd is a watchdog that respawns its own worker, so a worker pid
# changing proves nothing about the restart, and a bare `pgrep -x osqueryd`
# picks between the two arbitrarily. The pidfile is root-owned 0600, so it is
# deliberately not read: pgrep needs no privilege.
daemon_parent_pid() {
  local pid
  pid="$(pgrep -P 1 -x osqueryd 2>/dev/null | head -1)" || true
  [[ $pid =~ ^[1-9][0-9]{0,9}$ ]] || return 0
  printf '%s' "$pid"
}

# wait_for_daemon_parent: poll until the parent appears, printing its pid, or
# return 1 at the deadline. `launchctl load` returns before the spawn, so there
# is nothing to read at the instant `osqueryctl start` returns.
wait_for_daemon_parent() {
  local ticks=0 deadline_ticks pid
  deadline_ticks=$((OSQUERY_CONVERGE_RESTART_DEADLINE * OSQUERY_CONVERGE_POLL_TICKS_PER_SECOND))
  while :; do
    pid="$(daemon_parent_pid)"
    if [[ -n $pid ]]; then
      printf '%s' "$pid"
      return 0
    fi
    [[ $ticks -ge $deadline_ticks ]] && return 1
    sleep "$OSQUERY_CONVERGE_POLL_INTERVAL"
    ticks=$((ticks + 1))
  done
}

# daemon_parent_stays_up <pid>: the same parent is still there for the whole
# settle window. PRESENT ONCE IS NOT ALIVE: the vendor plist sets KeepAlive with
# ThrottleInterval 60, so a daemon that starts and dies immediately is replaced a
# minute later, long after this run has reported success. Polled across the
# window rather than checked once at its end, so a death anywhere inside it is
# caught, and compared by PID so a crash-and-respawn is a failure too.
daemon_parent_stays_up() {
  local pid="$1" ticks=0 settle_ticks current
  settle_ticks=$((OSQUERY_CONVERGE_SETTLE_SECONDS * OSQUERY_CONVERGE_POLL_TICKS_PER_SECOND))
  while [[ $ticks -lt $settle_ticks ]]; do
    sleep "$OSQUERY_CONVERGE_POLL_INTERVAL"
    ticks=$((ticks + 1))
    current="$(daemon_parent_pid)"
    [[ $current == "$pid" ]] || return 1
  done
  return 0
}

# restart_daemon: bounce osqueryd onto the converged config, and prove it came
# back. Returns non-zero, loudly, on anything it could not establish.
restart_daemon() {
  local pid

  # The vendor plist FIRST, before anything is stopped. `osqueryctl stop` is
  # `launchctl unload` PLUS `rm` of /Library/LaunchDaemons/io.osquery.agent.plist,
  # and `start` re-copies it from here, so stopping without this file present
  # leaves the machine with no daemon and no plist to start one from. It is the
  # vendor package's file, so this is a refusal, never a repair.
  if [[ ! -f "$OSQUERY_CONVERGE_TARGET_DIR/io.osquery.agent.plist" ]]; then
    fail "$OSQUERY_CONVERGE_TARGET_DIR/io.osquery.agent.plist is missing, so 'osqueryctl start' would have no LaunchDaemon plist to install and a stop would leave osqueryd GONE. That file belongs to the osquery package; reinstall the cask. The daemon was NOT stopped."
    return 1
  fi

  # The config the daemon is about to load has to parse. `osqueryctl start` runs
  # this check itself, but it runs it AFTER the stop, so a config it rejects
  # would take the daemon down with it. Running it first turns that into a loud
  # refusal with the previous daemon still up on its previous configuration.
  if ! run_privileged "$OSQUERY_CONVERGE_OSQUERYCTL" config-check >/dev/null 2>&1; then
    fail "the converged configuration at $OSQUERY_CONVERGE_TARGET_DIR/osquery.conf does not pass 'osqueryctl config-check', so restarting would stop a working daemon and fail to start it again. The files are installed; the running daemon was NOT stopped and is still on its previous configuration."
    return 1
  fi

  # GUARDED, and only this one: a fresh host has no loaded LaunchDaemon and no
  # plist in /Library/LaunchDaemons, so the vendor stop legitimately fails there.
  run_privileged "$OSQUERY_CONVERGE_OSQUERYCTL" stop >/dev/null 2>&1 || true

  # UNGUARDED, deliberately. This is the half that used to be silenced too, and
  # a stop-succeeds/start-fails pair left the daemon GONE while the script still
  # printed "osquery setup complete."
  if ! run_privileged "$OSQUERY_CONVERGE_OSQUERYCTL" start; then
    fail "'osqueryctl start' FAILED after a successful stop, so osqueryd is DOWN and this machine is not being monitored. Diagnose with 'sudo osqueryctl status' and 'sudo launchctl print system/io.osquery.agent'."
    return 1
  fi

  pid="$(wait_for_daemon_parent)" || {
    fail "osqueryd did not come back within ${OSQUERY_CONVERGE_RESTART_DEADLINE}s of 'osqueryctl start', so this machine is not being monitored. Diagnose with 'sudo osqueryctl status'."
    return 1
  }
  if ! daemon_parent_stays_up "$pid"; then
    fail "osqueryd started (pid $pid) and was gone again within ${OSQUERY_CONVERGE_SETTLE_SECONDS}s, which is a daemon crashing on startup. KeepAlive will not retry for another 60 seconds (ThrottleInterval), so treat this machine as unmonitored until it is fixed."
    return 1
  fi
  report "restarted osqueryd (parent pid $pid, still up after ${OSQUERY_CONVERGE_SETTLE_SECONDS}s)"
  return 0
}

main() {
  assert_desired_state_is_complete || return 1

  local verdicts=() relative directory verdict

  # The directories first: a pack cannot be installed into a packs/ that is not
  # there, and the target directory's own mode is what everything below inherits
  # its reachability from.
  for directory in "$OSQUERY_CONVERGE_TARGET_DIR" "$OSQUERY_CONVERGE_TARGET_DIR/packs"; do
    verdict="$(directory_verdict_for "$directory")"
    verdicts+=("$verdict")
    [[ $verdict == ok ]] || repair_directory "$directory" "$verdict"
  done
  for relative in "${OSQUERY_CONVERGE_FILES[@]}"; do
    verdict="$(file_verdict_for "$relative")"
    verdicts+=("$verdict")
    [[ $verdict == ok ]] || repair_file "$relative" "$verdict"
  done

  # The daemon's log directory, ours and unprivileged. Deliberately NOT folded
  # into the restart decision: creating it does not change what a running daemon
  # holds in memory, and bouncing the root daemon over a missing directory would
  # be a heavier act than the condition warrants.
  if [[ ! -d $OSQUERY_CONVERGE_LOG_DIR ]]; then
    mkdir -p "$OSQUERY_CONVERGE_LOG_DIR"
    report "created $OSQUERY_CONVERGE_LOG_DIR"
  fi

  [[ "$(osquery_converge_restart_verdict "${verdicts[@]}")" == restart ]] || return 0
  restart_daemon
}

main
