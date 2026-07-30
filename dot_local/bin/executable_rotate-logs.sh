#!/usr/bin/env bash
#
# rotate-logs.sh -- bound the size of every log under ~/.local/log.
#
# Run hourly by the com.webdavis.rotate-logs LaunchAgent. For each regular file
# under the log root that has reached the size threshold: compress a copy into
# a numbered archive, then TRUNCATE THE ORIGINAL IN PLACE. Archives older than
# the keep count are discarded.
#
# WHY TRUNCATE RATHER THAN RENAME (and why not newsyslog(8))
#
# Every log under this root is a launchd StandardOutPath/StandardErrorPath
# redirect: launchd opens the file itself and hands the descriptor to the
# daemon as fd 1 and 2. The daemon never reopens it and has no SIGHUP reopen
# handler. newsyslog(8), the system mechanism, rotates by RENAMING the file and
# creating a new one, which leaves the daemon writing into the renamed inode
# forever while the fresh file stays empty. Worse, each subsequent newsyslog
# pass renames that inode further down the chain until it falls past the keep
# count and is unlinked while the daemon still holds it open -- at which point
# the output is unreachable and the disk space is not reclaimed until the
# process exits. Measured on macOS 26.2 against a live writer, all of it.
#
# Truncating in place keeps the inode, so the descriptor stays valid.
# launchd opens these files O_APPEND (measured), so the kernel seeks to
# end-of-file before every write and the daemon's next line lands at offset 0
# of the emptied file. No sparse hole, no lost descriptor.
#
# HONEST LIMITATION: copy-then-truncate is not atomic. A line written between
# the compress and the truncate is lost. The window is milliseconds and these
# are diagnostic daemon logs, not audit records; a log that needs
# exactly-once retention must not rely on this.
#
# All tunables are named constants, overridable by environment variable so the
# test suite can point the whole mechanism at a sandbox and never touch the
# real log root.
set -uo pipefail

# --- Named constants -------------------------------------------------------

# Log root, relative to HOME when not overridden. ROTATE_LOGS_ROOT is the seam
# the tests use. Empty is treated as unset on purpose: an empty root would mean
# "scan from /", which must never be guessable.
LOG_ROOT_RELATIVE_TO_HOME=".local/log"

# A file at or above this many bytes is rotated. 10 MiB matches the bound
# osquery already applies to its own logs, so one number describes the machine.
DEFAULT_ROTATE_AT_BYTES=10485760

# Compressed generations kept per log, numbered .1 (newest) to .N (oldest).
DEFAULT_ARCHIVES_KEPT=5

# Archive naming. ARCHIVE_EXTENSION_PATTERN is DERIVED from ARCHIVE_EXTENSION
# so the literal and its regex form cannot drift apart.
ARCHIVE_EXTENSION=".gz"
ARCHIVE_EXTENSION_PATTERN="${ARCHIVE_EXTENSION//./\\.}"

# Absolute paths: a launchd job runs with a minimal PATH.
COMPRESSOR="${ROTATE_LOGS_COMPRESSOR:-/usr/bin/gzip}"
FIND="${ROTATE_LOGS_FIND:-/usr/bin/find}"
STAT="${ROTATE_LOGS_STAT:-/usr/bin/stat}"

# Archives are created owner-only: a log may quote secrets that its producer
# printed, and an archive outlives the process that wrote it.
ARCHIVE_UMASK=077

# A byte count longer than this many digits is refused rather than fed to
# arithmetic evaluation, where a 64-bit comparison would wrap.
MAX_BYTE_COUNT_DIGITS=15

ROTATE_AT_BYTES="${ROTATE_LOGS_AT_BYTES:-$DEFAULT_ROTATE_AT_BYTES}"
ARCHIVES_KEPT="${ROTATE_LOGS_ARCHIVES_KEPT:-$DEFAULT_ARCHIVES_KEPT}"

if [[ -n ${ROTATE_LOGS_ROOT:-} ]]; then
  LOG_ROOT="$ROTATE_LOGS_ROOT"
elif [[ -n ${HOME:-} ]]; then
  LOG_ROOT="$HOME/$LOG_ROOT_RELATIVE_TO_HOME"
else
  printf 'rotate-logs: neither ROTATE_LOGS_ROOT nor HOME is set; refusing to guess a log root\n' >&2
  exit 2
fi

# --- Pure predicates -------------------------------------------------------

# is_valid_byte_count <value> -- can this value be compared arithmetically
# without surprises? Digits only, non-empty, and short enough not to wrap.
# Rejecting up front is what lets every later comparison force base 10.
is_valid_byte_count() {
  local value="${1-}"
  [[ -n $value ]] || return 1
  [[ $value =~ ^[0-9]+$ ]] || return 1
  [[ ${#value} -le $MAX_BYTE_COUNT_DIGITS ]] || return 1
  return 0
}

# exceeds_size_threshold <size_bytes> <threshold_bytes> -- has this file reached
# the size at which it should be rotated? Inclusive: a file of exactly the
# threshold rotates.
#
# Both operands are forced to base 10. Bash arithmetic reads a leading zero as
# OCTAL, so an unforced comparison would read a size of "010" as 8. Anything
# unparseable answers NO, because the safe direction for a rotator is to keep a
# file it could not measure, never to truncate on a value it did not understand.
exceeds_size_threshold() {
  local size="${1-}" threshold="${2-}"
  is_valid_byte_count "$size" || return 1
  is_valid_byte_count "$threshold" || return 1
  if ((10#$size >= 10#$threshold)); then
    return 0
  fi
  return 1
}

# is_archive_name <basename> -- is this one of the archives we produced? Used to
# keep a rotation pass from re-archiving its own output.
is_archive_name() {
  local name="${1-}"
  [[ $name =~ \.[0-9]+${ARCHIVE_EXTENSION_PATTERN}$ ]]
}

# archive_path <log_path> <index> -- where generation <index> of a log lives.
archive_path() {
  printf '%s.%s%s' "$1" "$2" "$ARCHIVE_EXTENSION"
}

# archive_index_of <archive_path> <log_path> -- the generation number encoded in
# an archive name, or nothing when the name does not encode one.
archive_index_of() {
  local archive="${1-}" log="${2-}" middle
  middle="${archive#"$log".}"
  middle="${middle%"$ARCHIVE_EXTENSION"}"
  [[ $middle =~ ^[0-9]+$ ]] || return 1
  printf '%s' "$middle"
}

# is_retainable_archive_index <index> <keep_count> -- will this generation still
# be inside the retention window after the shift that is about to happen?
#
# The window is 1 <= index <= keep-1: the shift moves each of those one slot
# older, and generation 1 is then rewritten from the live log. Every other
# index is stale and must go.
#
# Bounded on BOTH sides deliberately. Only checking the upper side leaves index
# 0 permanently unpruned and unshifted, because nothing ever moves it: the log
# would carry one extra generation forever while this script reported a
# retention count that did not include it. Index 0 is not hypothetical, it is
# how newsyslog(8) numbers its own archives, so a log this machine rotated by
# any other means arrives with one already in place.
is_retainable_archive_index() {
  local index="${1-}" keep="${2-}"
  is_valid_byte_count "$index" || return 1
  is_valid_byte_count "$keep" || return 1
  ((10#$index >= 1)) || return 1
  ((10#$index <= 10#$keep - 1)) || return 1
  return 0
}

# --- Filesystem predicates -------------------------------------------------

# is_rotatable_file <path> -- may this pass archive and truncate this file?
# Every NO here is the safe direction:
#   symlink       truncating through it would blank a file we do not manage
#                 (osquery publishes osqueryd.INFO as a symlink and holds it open)
#   not writable  a root-owned log is out of user scope; we cannot truncate it
#                 and must not claim to
#   archive       re-archiving our own output would compress forever
is_rotatable_file() {
  local path="${1-}"
  [[ -L $path ]] && return 1
  [[ -f $path ]] || return 1
  [[ -w $path ]] || return 1
  is_archive_name "$(basename "$path")" && return 1
  return 0
}

# --- Operations ------------------------------------------------------------

rotate_logs_failures=0
rotate_logs_rotated=0

report() { printf '%s\n' "$*"; }

report_failure() {
  printf 'rotate-logs: FAILED -- %s\n' "$*" >&2
  rotate_logs_failures=$((rotate_logs_failures + 1))
}

# prune_archives <log_path> -- discard every generation that will not be inside
# the retention window after the shift, making room for it.
#
# Framed as "keep exactly the retainable set" rather than "delete the one that
# falls off the end". A next-index-only rule leaves behind anything a LOWERED
# keep count orphaned, and anything numbered outside our own scheme, both of
# which then occupy disk that the stated retention count says is not in use.
prune_archives() {
  local log="$1" archive index
  for archive in "$log".*"$ARCHIVE_EXTENSION"; do
    [[ -e $archive ]] || continue
    index="$(archive_index_of "$archive" "$log")" || continue
    if ! is_retainable_archive_index "$index" "$ARCHIVES_KEPT"; then
      rm -f -- "$archive" || report_failure "could not remove stale archive $archive"
    fi
  done
}

# shift_archives <log_path> -- move every surviving generation one slot older,
# oldest first so nothing is overwritten on the way.
shift_archives() {
  local log="$1" index source target
  for ((index = ARCHIVES_KEPT - 1; index >= 1; index--)); do
    source="$(archive_path "$log" "$index")"
    [[ -e $source ]] || continue
    target="$(archive_path "$log" "$((index + 1))")"
    mv -f -- "$source" "$target" || report_failure "could not shift $source to $target"
  done
}

# archive_current <log_path> -- compress the live log into generation 1.
# Written to a private temp file and renamed into place, so a failed or
# interrupted compress never leaves a half-written archive that the next pass
# would mistake for a good generation.
#
# Returns non-zero WITHOUT truncating when anything fails: losing a log because
# its archive could not be written is the one outcome worth failing loudly for.
archive_current() {
  local log="$1" destination partial
  destination="$(archive_path "$log" 1)"
  partial="$destination.partial"
  (
    umask "$ARCHIVE_UMASK"
    "$COMPRESSOR" -c <"$log" >"$partial"
  ) || {
    rm -f -- "$partial"
    return 1
  }
  [[ -s $partial ]] || {
    rm -f -- "$partial"
    return 1
  }
  mv -f -- "$partial" "$destination" || {
    rm -f -- "$partial"
    return 1
  }
  return 0
}

# rotate_log <log_path> -- the whole sequence for one file.
rotate_log() {
  local log="$1"
  prune_archives "$log"
  shift_archives "$log"
  if ! archive_current "$log"; then
    report_failure "could not archive $log; leaving it untruncated"
    return 1
  fi
  # Truncate IN PLACE. Redirecting with > preserves the inode, so a descriptor
  # held open by launchd on behalf of the producing daemon stays valid.
  if ! : >"$log"; then
    report_failure "archived $log but could not truncate it"
    return 1
  fi
  rotate_logs_rotated=$((rotate_logs_rotated + 1))
  report "rotated: $log"
  return 0
}

# consider_file <path> -- classify one candidate and act.
#
# A file this pass cannot manage is NAMED in the report, but only once it has
# reached the threshold. That is the point at which "I cannot manage this" turns
# into "something here may be growing and nobody is bounding it", which is the
# fail-open case worth a human's attention. Naming every small unmanageable file
# every hour instead would bury it: this machine carries roughly fifty
# root-owned osquery files of a few hundred bytes each, and reporting them
# hourly would make this script's own log the noisiest thing under the root it
# is supposed to be keeping tidy.
consider_file() {
  local path="$1" size

  # Not ours to measure or manage, and following a symlink to stat its target is
  # exactly the access this script refuses to make.
  [[ -L $path ]] && return 0
  [[ -f $path ]] || return 0

  # Our own archives are already bounded by the retention window, so they are
  # deliberately silent rather than reported as a problem.
  is_archive_name "$(basename "$path")" && return 0

  size="$("$STAT" -f %z "$path" 2>/dev/null)"
  if ! is_valid_byte_count "$size"; then
    report_failure "could not read a usable size for $path"
    return 1
  fi

  exceeds_size_threshold "$size" "$ROTATE_AT_BYTES" || return 0

  if ! is_rotatable_file "$path"; then
    report "skipped (over threshold at ${size}B but not writable from user scope): $path"
    return 0
  fi

  rotate_log "$path"
  return 0
}

main() {
  if ! is_valid_byte_count "$ROTATE_AT_BYTES"; then
    printf 'rotate-logs: ROTATE_LOGS_AT_BYTES=%s is not a usable byte count\n' "$ROTATE_AT_BYTES" >&2
    exit 2
  fi
  if ! is_valid_byte_count "$ARCHIVES_KEPT" || [[ $ARCHIVES_KEPT -lt 1 ]]; then
    printf 'rotate-logs: ROTATE_LOGS_ARCHIVES_KEPT=%s must be a whole number of at least 1; refusing to run, because keeping zero archives would discard log content outright\n' "$ARCHIVES_KEPT" >&2
    exit 2
  fi
  if [[ ! -d $LOG_ROOT ]]; then
    report "rotate-logs: log root $LOG_ROOT does not exist; nothing to rotate"
    exit 0
  fi

  report "=== rotate-logs $(date -u +%Y-%m-%dT%H:%M:%SZ) root=$LOG_ROOT threshold=${ROTATE_AT_BYTES}B keep=$ARCHIVES_KEPT ==="

  # -print0 and a dedicated descriptor: a log path may contain spaces, and the
  # loop body runs a compressor, which must not be handed the path stream.
  local path
  while IFS= read -r -d '' -u3 path; do
    consider_file "$path"
  done 3< <("$FIND" "$LOG_ROOT" -type f -print0)

  report "=== rotated $rotate_logs_rotated file(s), $rotate_logs_failures failure(s) ==="
  [[ $rotate_logs_failures -eq 0 ]] || exit 1
  exit 0
}

# Entry-point guard: sourcing this file exposes its functions for unit tests
# without running a rotation pass.
if [[ ${BASH_SOURCE[0]} == "${0}" ]]; then
  main "$@"
fi
