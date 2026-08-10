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
# is already there, which every apply flavor reads. That same limit is why those
# two files are carved out of the known-good manifest; run_after_05 states the
# trade.
#
# WHAT IS NOT OURS. /var/osquery/io.osquery.agent.plist, the certs and the
# lenses belong to the vendor package. They are never repaired here. The plist
# is required to be a REGULAR FILE before any restart because `osqueryctl start`
# copies it into /Library/LaunchDaemons and `osqueryctl stop` deletes that copy:
# a stop with no plist behind it leaves the daemon GONE rather than stale, and a
# SYMLINK there would have the vendor script publish its referent as a root
# LaunchDaemon.
#
# ROOT BLAST RADIUS. osqueryd runs the queries in these files AS ROOT. That is
# why they live in root-owned /var/osquery rather than under ~/.config (any
# process at the user's privilege level could rewrite those, and the daemon
# would then run them as root), and it is why the drift check compares mode,
# owner and group as well as content: right bytes under a 0666 mode is the same
# escalation with an extra step. Every decision lives in
# osquery-converge/drift-verdict.sh as a pure function of observed state; this
# file owns the probing, the privileged calls and the restart.
#
# WHAT ROOT IS ALLOWED TO TOUCH, the three rules the privileged half follows:
#
#   1. Every privileged command is named by ABSOLUTE PATH. `sudo -n` preserves
#      the caller's PATH, and this host's PATH leads with /opt/homebrew/bin,
#      which is drwxrwxr-x and owned by the operator (measured), so a bare
#      `install` would be a name any user-level process could answer for root.
#      The one command that is resolved rather than written literally is checked
#      by privileged_command_is_trustworthy before it is used.
#   2. Every byte root writes is read out of a PRIVATE 0700 COPY, never out of
#      the deployed staging tree. The check on a staging file and the `install`
#      that reads it are far apart, and `install` reads its source as root, so
#      copying first is what stops that gap being a root-file read redirected
#      through a swapped symlink.
#   3. Anything IRREGULAR is refused, never repaired. `install -d` follows a
#      preplanted symlink (measured: it chmods the referent, exits 0 and leaves
#      the link), so repairing an irregular directory would hand the root
#      daemon's configuration directory to wherever the link points.
set -euo pipefail

usage() {
  printf 'usage: osquery-converge.sh\n' >&2
}

fail() {
  printf 'osquery-converge: %s\n' "$1" >&2
}

report() {
  printf 'osquery-converge: %s\n' "$1"
}

# THE TEST SEAMS, and why they are gated rather than simply documented.
# Everything that selects WHAT is installed, WHERE it lands and WHICH binary
# runs under sudo is settable from the environment so the bats harness can drive
# this tool against a sandbox instead of /var/osquery. In production that same
# settability is an escalation: both callers reach root through passwordless
# sudo, so an environment pointing OSQUERY_CONVERGE_DESIRED_DIR at another tree
# is root installing that tree's bytes into the root daemon's configuration
# directory. They are TEST-ONLY, and setting one without the seam is refused
# before anything else happens.
#
# `${!name+set}` tests PRESENCE, so an override that happens to name the default
# is refused too: what is gated is who may steer this tool, not which values are
# harmful. OSQUERY_CONVERGE_LOG_DIR and the two restart bounds are deliberately
# NOT here; they select an unprivileged mkdir under $HOME and a bounded wait, and
# neither can redirect a privileged call.
OSQUERY_CONVERGE_TEST_SEAMS=(
  OSQUERY_CONVERGE_DESIRED_DIR
  OSQUERY_CONVERGE_TARGET_DIR
  OSQUERY_CONVERGE_SUDO
  OSQUERY_CONVERGE_OSQUERYCTL
)
if [[ ${OSQUERY_CONVERGE_TEST_SEAM:-} != 1 ]]; then
  osquery_converge_seam_refused=0
  for osquery_converge_seam in "${OSQUERY_CONVERGE_TEST_SEAMS[@]}"; do
    [[ -n ${!osquery_converge_seam+set} ]] || continue
    fail "$osquery_converge_seam is a TEST-ONLY seam and is set in this environment, but OSQUERY_CONVERGE_TEST_SEAM=1 is not. It selects what root installs and where, so it is refused rather than honored; unset it."
    osquery_converge_seam_refused=1
  done
  [[ $osquery_converge_seam_refused -eq 0 ]] || exit 2
fi

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
#
# ABSOLUTE, both of them. /usr/bin/sudo and /usr/bin/install are root-owned
# entries in a SIP-protected directory, and a literal path has no resolution for
# an attacker to answer, which is why neither needs the trust check that the
# resolved osqueryctl gets.
OSQUERY_CONVERGE_SUDO="${OSQUERY_CONVERGE_SUDO:-/usr/bin/sudo}"
OSQUERY_CONVERGE_INSTALL='/usr/bin/install'
# The uid a resolved privileged command's directory must belong to. NOT a seam:
# a sandbox cannot own anything as root, so the harness substitutes the OWNER
# READING instead (the same stat stub that lets a correctly-installed file be
# modelled at all), which keeps this comparison live rather than switching it off.
OSQUERY_CONVERGE_TRUSTED_UID='0'
# Resolved once by resolve_osqueryctl and used everywhere after, so the path that
# was checked is the path that runs.
OSQUERY_CONVERGE_OSQUERYCTL_COMMAND=''
# The private copy of the desired state, created per run and removed on exit.
OSQUERY_CONVERGE_STAGE=''

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

# An unknown argument is an ERROR, never a silent fallthrough to a full converge:
# a typo in a caller would otherwise run privileged work nobody asked for.
for argument in "$@"; do
  usage
  printf 'osquery-converge: unknown argument: %s\n' "$argument" >&2
  exit 2
done

run_privileged() {
  "$OSQUERY_CONVERGE_SUDO" -n "$@"
}

# privileged_command_is_trustworthy <path>: 0 when a command this tool is about
# to hand to sudo can be trusted to be the one it names.
#
# The check is on the CONTAINING DIRECTORY, not on the binary. A directory owned
# by the trusted uid and writable by nobody else is one only that uid can add to
# or replace entries in, which is precisely the property that makes a resolved
# name stable. /usr/local/bin/osqueryctl is a root-owned SYMLINK into
# /opt/osquery (measured on this host), so a check on the leaf's own attributes
# would first have to decide whether to follow it, and GNU stat follows by
# default where BSD lstats: the directory answer needs no such choice and is the
# same on both.
privileged_command_is_trustworthy() {
  local command_path="$1" directory attributes mode uid
  if [[ $command_path != /* ]]; then
    fail "the privileged command '$command_path' did not resolve to an absolute path, so what would run under sudo depends on this process's PATH; refusing."
    return 1
  fi
  directory="$(dirname "$command_path")"
  attributes="$(probe_attributes "$directory")"
  read -r mode uid _ <<<"$attributes"
  if [[ -z ${mode:-} || -z ${uid:-} ]]; then
    fail "the attributes of $directory could not be read, so $command_path cannot be shown to be a command only uid $OSQUERY_CONVERGE_TRUSTED_UID can replace; refusing."
    return 1
  fi
  if [[ $uid != "$OSQUERY_CONVERGE_TRUSTED_UID" ]]; then
    fail "$command_path resolves inside $directory, which belongs to uid $uid rather than uid $OSQUERY_CONVERGE_TRUSTED_UID, so a process at that uid could replace the binary sudo is about to run as root; refusing."
    return 1
  fi
  if (((8#${mode: -3} & 022) != 0)); then
    fail "$command_path resolves inside $directory, whose mode $mode grants write to its group or to everyone, so the binary sudo is about to run as root is replaceable; refusing."
    return 1
  fi
  return 0
}

# resolve_osqueryctl: settle on the ONE absolute osqueryctl this run will use,
# and refuse if it cannot be trusted.
#
# The availability probe and the privileged calls read the SAME resolved path.
# They used to differ: `command -v` answered a name and each `sudo` re-resolved
# it against a PATH that root's environment does not share, so a probe could
# succeed on one binary and the privileged call run another.
#
# An osqueryctl that is not there at all is NOT a failure. There is no daemon to
# converge for and no vendor layout to converge into, so the run is a quiet
# no-op and a machine that simply does not run osquery adds nothing to an apply.
resolve_osqueryctl() {
  local resolved
  if [[ -n ${OSQUERY_CONVERGE_OSQUERYCTL:-} ]]; then
    resolved="$OSQUERY_CONVERGE_OSQUERYCTL"
  else
    resolved="$(command -v osqueryctl 2>/dev/null)" || resolved=''
  fi
  [[ -n $resolved && -x $resolved ]] || return 0
  privileged_command_is_trustworthy "$resolved" || return 1
  OSQUERY_CONVERGE_OSQUERYCTL_COMMAND="$resolved"
  return 0
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

# assert_no_symlink_component <directory>: refuse a path whose own last
# component, or ANY component leading to it, is a symlink.
#
# MEASURED, and it defeats both halves of the completeness check at once: `find
# <dir> -mindepth 1` does not descend a symlinked directory ARGUMENT, so nothing
# planted in the referent is ever listed, and `[[ -L $dir/file ]]` resolves the
# directory component before testing the leaf, so nothing planted there looks
# like a link either. A staging tree swapped this way would be installed
# root-owned into /var/osquery without a single one of these checks seeing it.
#
# Walked component by component rather than compared against `pwd -P`, because a
# canonical-path comparison cannot say WHICH component was substituted, and on
# macOS it also reports a false positive for anything under /var, which is itself
# a symlink to /private/var.
assert_no_symlink_component() {
  local path="$1" walked='' component
  local -a components
  if [[ $path != /* ]]; then
    fail "the desired-state directory '$path' is not an absolute path, so the components leading to it cannot be checked for substitution; refusing."
    return 1
  fi
  IFS='/' read -r -a components <<<"${path#/}"
  for component in "${components[@]}"; do
    [[ -n $component ]] || continue
    walked="$walked/$component"
    if [[ -L $walked ]]; then
      fail "$walked is a symlink, and it leads to the desired state this tool installs root-owned into $OSQUERY_CONVERGE_TARGET_DIR; refusing to read through it."
      return 1
    fi
  done
  return 0
}

# create_private_stage: a 0700 directory this run owns, holding the listing and
# the copy of the desired state that every later step reads.
create_private_stage() {
  OSQUERY_CONVERGE_STAGE="$(mktemp -d "${TMPDIR:-/tmp}/osquery-converge.XXXXXX")" || {
    fail "could not create a private staging directory under ${TMPDIR:-/tmp}, so the desired state cannot be read out of a path no other process can substitute; refusing."
    return 1
  }
  # Explicit rather than inherited: mktemp already answers 0700 today, and this
  # is the mode the whole source-substitution argument rests on.
  chmod 0700 "$OSQUERY_CONVERGE_STAGE"
  mkdir -p "$OSQUERY_CONVERGE_STAGE/packs"
}

# remove_private_stage: the EXIT trap. `rm -rf` and not `trash`, matching the
# convention every other unattended script here follows for a workspace it
# created itself (run_after_05 and the weekly upgrade job both do this): trash(1)
# is a user-local binary that is not on the LaunchAgent PATH, and a weekly job
# filling ~/.Trash with staging directories is litter, not safety. Guarded on a
# non-empty path that is really a directory, so an early failure cannot turn this
# into a bare `rm -rf`.
remove_private_stage() {
  [[ -n $OSQUERY_CONVERGE_STAGE && -d $OSQUERY_CONVERGE_STAGE ]] || return 0
  rm -rf -- "$OSQUERY_CONVERGE_STAGE"
}

# stage_desired_state: validate the DEPLOYED staging tree, then take a private
# copy of the files this tool installs.
#
# The desired state must be COMPLETE before anything privileged happens. A
# missing staging file means the deploy is broken, and passing over it would
# leave a wiped /var/osquery file wiped while the run reported success; a symlink
# there would install through to the referent. Both are refusals, and both are
# checked before the first sudo so a broken staging tree costs no privileged call.
#
# THE COPY IS THE POINT, not a convenience. `install` reads its source AS ROOT,
# and the -L check on a deployed file and that read are far apart, so a process
# that swaps the file for a symlink in between has root read whatever it points
# at into a world-readable 0644 destination. Copying first closes that: `cp` runs
# at the INVOKING user's privilege, so the worst a won race yields is bytes the
# user could already read, and the path root then reads sits in a 0700 directory
# created by this run that no other process can substitute.
stage_desired_state() {
  local relative desired refused=0 listing entry listed candidate

  assert_no_symlink_component "$OSQUERY_CONVERGE_DESIRED_DIR" || return 1
  if [[ ! -d $OSQUERY_CONVERGE_DESIRED_DIR ]]; then
    fail "the desired state is not deployed at $OSQUERY_CONVERGE_DESIRED_DIR, so $OSQUERY_CONVERGE_TARGET_DIR cannot be converged; run a full 'chezmoi apply'"
    return 1
  fi

  # The listing is MATERIALIZED under an explicit status check, never read
  # through a process substitution, and its diagnostics are not silenced. A
  # process substitution discards the producer's status, so a find that listed
  # some of the tree and then failed (an unreadable subdirectory is enough) would
  # hand this loop a PARTIAL set, pass every check below, and converge from a
  # tree nothing had examined. The same reasoning run_after_05 records for
  # `chezmoi managed`.
  listing="$OSQUERY_CONVERGE_STAGE/deployed-staging-listing"
  if ! find "$OSQUERY_CONVERGE_DESIRED_DIR" -mindepth 1 -print0 >"$listing"; then
    fail "the desired-state tree at $OSQUERY_CONVERGE_DESIRED_DIR could not be listed completely (see the error above), so a file planted in the part that could not be read would go unnoticed; refusing."
    return 1
  fi

  # Every entry, at any depth: a symlink ANYWHERE in the tree is refused, and a
  # file the list does not name is refused too. The list is deliberately named
  # rather than globbed, so that nothing planted here is promoted root-owned into
  # the root daemon's directory; the cost of that choice is a file that could be
  # ignored forever, which is the same invisible divergence this tool exists to
  # end. Refusing turns it into one loud sentence, and the only two ways out are
  # to remove the file or to list it.
  while IFS= read -r -d '' entry; do
    if [[ -L $entry ]]; then
      fail "$entry in the desired-state tree is a symlink; refusing to install through it"
      refused=1
      continue
    fi
    [[ -d $entry ]] && continue
    listed=0
    for candidate in "${OSQUERY_CONVERGE_FILES[@]}"; do
      [[ $candidate == "${entry#"$OSQUERY_CONVERGE_DESIRED_DIR"/}" ]] && listed=1 && break
    done
    if [[ $listed -eq 0 ]]; then
      fail "$entry sits in the desired-state tree but is not one of the files this tool installs, so it would be ignored forever; remove it, or add it to OSQUERY_CONVERGE_FILES beside its entry in desired/osquery.conf"
      refused=1
    fi
  done <"$listing"

  for relative in "${OSQUERY_CONVERGE_FILES[@]}"; do
    desired="$OSQUERY_CONVERGE_DESIRED_DIR/$relative"
    if [[ -L $desired ]]; then
      fail "the desired state at $desired is a symlink; refusing to install through it"
      refused=1
    elif [[ ! -f $desired ]]; then
      fail "the desired state for $relative is not deployed at $desired, so $OSQUERY_CONVERGE_TARGET_DIR cannot be converged; run a full 'chezmoi apply'"
      refused=1
    fi
  done

  [[ $refused -eq 0 ]] || return 1

  for relative in "${OSQUERY_CONVERGE_FILES[@]}"; do
    if ! cp -- "$OSQUERY_CONVERGE_DESIRED_DIR/$relative" "$OSQUERY_CONVERGE_STAGE/$relative"; then
      fail "the desired state for $relative could not be copied out of $OSQUERY_CONVERGE_DESIRED_DIR, so there is nothing this run can vouch for installing; refusing."
      return 1
    fi
  done
  return 0
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

# The comparison reads the PRIVATE COPY, not the deployed staging tree, so the
# bytes that decided the verdict are the same bytes the repair installs.
file_verdict_for() {
  local relative="$1" desired live kind attributes mode uid gid content_equal
  desired="$OSQUERY_CONVERGE_STAGE/$relative"
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
  run_privileged "$OSQUERY_CONVERGE_INSTALL" -d -o root -g wheel -m 0755 "$1"
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
  run_privileged "$OSQUERY_CONVERGE_INSTALL" -o root -g wheel -m 0644 "$OSQUERY_CONVERGE_STAGE/$1" "$live"
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

# wait_for_new_daemon_parent <pid-before-the-stop>: poll until a parent that is
# NOT the one running before the stop appears, printing its pid, or return 1 at
# the deadline. `launchctl load` returns before the spawn, so there is nothing to
# read at the instant `osqueryctl start` returns.
#
# THE PID BEFORE THE STOP IS THE EVIDENCE. Nothing else in this sequence can tell
# a bounced daemon from one that never went away: `osqueryctl stop` is
# `launchctl unload` plus an rm, launchctl LOGS a failure while exiting 0, and a
# liveness check afterwards is satisfied by the daemon that was already there.
# Before this, an unload that silently did nothing left the old process running
# the OLD configuration while this tool reported a verified restart. A pid that
# did not change is that failure, so it is not accepted as one that did.
#
# An EMPTY pid before the stop means no daemon was running (a fresh host), and
# then any parent appearing is the restart: absent-then-present is a change too.
wait_for_new_daemon_parent() {
  local previous="$1" ticks=0 deadline_ticks pid
  deadline_ticks=$((OSQUERY_CONVERGE_RESTART_DEADLINE * OSQUERY_CONVERGE_POLL_TICKS_PER_SECOND))
  while :; do
    pid="$(daemon_parent_pid)"
    if [[ -n $pid && $pid != "$previous" ]]; then
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
  local pid previous_pid vendor_plist

  # The vendor plist FIRST, before anything is stopped. `osqueryctl stop` is
  # `launchctl unload` PLUS `rm` of /Library/LaunchDaemons/io.osquery.agent.plist,
  # and `start` re-copies it from here, so stopping without this file present
  # leaves the machine with no daemon and no plist to start one from. It is the
  # vendor package's file, so this is a refusal, never a repair.
  #
  # A REGULAR FILE, checked as `-f` AND `! -L`, because `-f` follows a link: a
  # symlink planted at this path reads as a healthy vendor plist, and the vendor
  # `osqueryctl start` would copy its referent into /Library/LaunchDaemons and
  # launchctl would load it AS ROOT. This tool never repairs the path either way,
  # so both cases end the run rather than the daemon.
  vendor_plist="$OSQUERY_CONVERGE_TARGET_DIR/io.osquery.agent.plist"
  if [[ -L $vendor_plist ]]; then
    fail "$vendor_plist is a symlink, and 'osqueryctl start' copies that file into /Library/LaunchDaemons and loads it as root, so starting would publish whatever it points at as a root LaunchDaemon. That file belongs to the osquery package; remove the link and reinstall the cask. The daemon was NOT stopped."
    return 1
  fi
  if [[ ! -f $vendor_plist ]]; then
    fail "$vendor_plist is missing, so 'osqueryctl start' would have no LaunchDaemon plist to install and a stop would leave osqueryd GONE. That file belongs to the osquery package; reinstall the cask. The daemon was NOT stopped."
    return 1
  fi

  # The config the daemon is about to load has to parse. `osqueryctl start` runs
  # this check itself, but it runs it AFTER the stop, so a config it rejects
  # would take the daemon down with it. Running it first turns that into a loud
  # refusal with the previous daemon still up on its previous configuration.
  if ! run_privileged "$OSQUERY_CONVERGE_OSQUERYCTL_COMMAND" config-check >/dev/null 2>&1; then
    fail "the converged configuration at $OSQUERY_CONVERGE_TARGET_DIR/osquery.conf does not pass 'osqueryctl config-check', so restarting would stop a working daemon and fail to start it again. The files are installed; the running daemon was NOT stopped and is still on its previous configuration."
    return 1
  fi

  # The pid of the daemon that is running NOW, read before anything is stopped.
  # It is the only thing that can prove the process was replaced; see
  # wait_for_new_daemon_parent.
  previous_pid="$(daemon_parent_pid)"

  # GUARDED, and only this one: a fresh host has no loaded LaunchDaemon and no
  # plist in /Library/LaunchDaemons, so the vendor stop legitimately fails there.
  # Its status is deliberately not evidence in either direction, which is exactly
  # why the pid above is taken.
  run_privileged "$OSQUERY_CONVERGE_OSQUERYCTL_COMMAND" stop >/dev/null 2>&1 || true

  # UNGUARDED, deliberately. This is the half that used to be silenced too, and
  # a stop-succeeds/start-fails pair left the daemon GONE while the script still
  # printed "osquery setup complete."
  if ! run_privileged "$OSQUERY_CONVERGE_OSQUERYCTL_COMMAND" start; then
    fail "'osqueryctl start' FAILED after a successful stop, so osqueryd is DOWN and this machine is not being monitored. Diagnose with 'sudo osqueryctl status' and 'sudo launchctl print system/io.osquery.agent'."
    return 1
  fi

  pid="$(wait_for_new_daemon_parent "$previous_pid")" || {
    if [[ -n $previous_pid ]] && [[ "$(daemon_parent_pid)" == "$previous_pid" ]]; then
      fail "osqueryd is still running as parent pid $previous_pid, the same process that was running before the stop, so it never restarted and is still on its PREVIOUS configuration. 'osqueryctl stop' is a launchctl unload, which logs a failure while exiting 0. Diagnose with 'sudo launchctl print system/io.osquery.agent'."
      return 1
    fi
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
  local verdicts=() relative directory verdict
  local directories=("$OSQUERY_CONVERGE_TARGET_DIR" "$OSQUERY_CONVERGE_TARGET_DIR/packs")
  local directory_verdicts=()

  resolve_osqueryctl || return 1
  # osquery not installed at all: there is no daemon to converge for and no
  # vendor layout to converge into. Quiet, so a machine that simply does not run
  # osquery adds nothing to an apply.
  [[ -n $OSQUERY_CONVERGE_OSQUERYCTL_COMMAND ]] || return 0

  create_private_stage || return 1
  stage_desired_state || return 1

  # The directories first: a pack cannot be installed into a packs/ that is not
  # there, and the target directory's own mode is what everything below inherits
  # its reachability from.
  #
  # BOTH verdicts are taken before EITHER is acted on, so an irregular second
  # directory is refused with no privileged call having been made on the first.
  for directory in "${directories[@]}"; do
    directory_verdicts+=("$(directory_verdict_for "$directory")")
  done
  local index=0
  for directory in "${directories[@]}"; do
    verdict="${directory_verdicts[$index]}"
    index=$((index + 1))
    if [[ $verdict == irregular ]]; then
      # REFUSED, never repaired. `install -d` follows a preplanted symlink
      # (measured: it chmods the referent, exits 0 and leaves the link in place),
      # so repairing this would put the root daemon's configuration wherever the
      # link points and report it as a repair.
      fail "$directory is not a directory: a symlink, a file or a device stands there. 'install -d' would FOLLOW it rather than replace it, so the root daemon's configuration would be written wherever it leads; refusing. Remove whatever is at that path by hand."
      return 1
    fi
  done
  index=0
  for directory in "${directories[@]}"; do
    verdict="${directory_verdicts[$index]}"
    index=$((index + 1))
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

trap remove_private_stage EXIT
main
