#!/usr/bin/env bash
#
# drift-verdict.sh, sourced helper, not run directly. The DECISION CORE of
# osquery-converge.sh: given the observed state of one path under /var/osquery,
# it answers whether that path has drifted from the desired state, and whether
# anything that drifted warrants restarting the root daemon.
#
# Everything here is a total function of its arguments: no stat, no cmp, no
# filesystem, no clock, no privilege. The execution edge does all the probing
# and owns every `sudo`, which is what lets these decisions be tested one
# behavior at a time without a fixture in /var and without a root shell. It is
# the same split pns/helpers/event.sh and results-alerter/pipeline-verdict.sh
# use.
#
# Nothing here prints diagnostics, repairs anything, or exits. A caller decides
# what to do with a verdict.
#
# WHY MODE, OWNER AND GROUP ARE PART OF THE COMPARISON. osqueryd runs the
# queries in these files AS ROOT. That is the whole reason they live in
# root-owned /var/osquery rather than under ~/.config, so a file carrying the
# right bytes under the wrong permissions is not a cosmetic difference: it is a
# path a user-level process can rewrite before the daemon's next config load.
# Content equality alone would call that file healthy.

# The desired attributes, in one place so the comparisons below cannot disagree
# with the `install` flags the execution edge passes. Files are root:wheel 0644,
# the two directories root:wheel 0755. On macOS the wheel group is gid 0.
OSQUERY_CONVERGE_FILE_MODE='0644'
OSQUERY_CONVERGE_DIRECTORY_MODE='0755'
OSQUERY_CONVERGE_OWNER_UID='0'
OSQUERY_CONVERGE_OWNER_GID='0'

# _osquery_converge_attribute_verdict <want-mode> <mode> <uid> <gid>: the shared
# attribute comparison for files and directories. Prints a verdict token when an
# attribute disagrees with the desired one (or could not be read), and nothing
# at all when all three match.
#
# An UNREADABLE attribute is never treated as a match. A stat that failed is not
# evidence of a healthy file, the repair is idempotent, and reinstalling is the
# only direction that cannot leave a drifted file in place.
_osquery_converge_attribute_verdict() {
  local want_mode="$1" mode="$2" uid="$3" gid="$4"
  if [[ -z $mode || -z $uid || -z $gid ]]; then
    printf 'unreadable'
    return 0
  fi
  if [[ $mode != "$want_mode" ]]; then
    printf 'mode'
  elif [[ $uid != "$OSQUERY_CONVERGE_OWNER_UID" ]]; then
    printf 'owner'
  elif [[ $gid != "$OSQUERY_CONVERGE_OWNER_GID" ]]; then
    printf 'group'
  fi
  return 0
}

# osquery_converge_file_verdict <kind> <content-equal> <mode> <uid> <gid>:
# the verdict for one desired-state FILE installed under /var/osquery.
#
#   kind           the live path's type, as the caller's probe read it:
#                  absent | symlink | file | other
#   content-equal  1 when the live bytes equal the desired bytes, 0 when they
#                  differ, EMPTY when the comparison could not be made
#   mode           the live permission bits as four octal digits, empty if unread
#   uid, gid       the live owner ids in decimal, empty if unread
#
# Prints exactly one token: ok, absent, irregular, unreadable, content, mode,
# owner or group. Everything but `ok` means the caller reinstalls the file.
#
# A SYMLINK is `irregular`, and the reason is that the TYPE is the only column
# that can tell. Measured, on a link planted at /var/osquery/osquery.conf whose
# referent holds the desired bytes: `cmp` follows the link, so the content
# column compares EQUAL; BSD `stat -f '%p'` lstats, so the mode and owner
# columns describe the LINK, and its author sets those with `chmod -h 0644`.
# Every other dimension therefore reads as fully converged, while the root
# daemon goes on reading a file that author still controls.
#
# NOT because `install` would write through the link. It does not: on macOS
# `install` replaces a destination symlink and leaves the referent untouched
# (measured), which is what makes the repair that follows this verdict correct.
# The same refusal covers a directory or a device sitting at the path.
#
# One token per path, in a fixed precedence, because the token is what the
# operator reads in the repair line: a path that is both rewritten and chmod-ed
# reports the content change, which is the more serious of the two.
osquery_converge_file_verdict() {
  local kind="$1" content_equal="$2" mode="$3" uid="$4" gid="$5" attribute
  case "$kind" in
    absent)
      printf 'absent'
      return 0
      ;;
    file) ;;
    *)
      printf 'irregular'
      return 0
      ;;
  esac
  if [[ -z $content_equal ]]; then
    printf 'unreadable'
    return 0
  fi
  attribute="$(_osquery_converge_attribute_verdict "$OSQUERY_CONVERGE_FILE_MODE" "$mode" "$uid" "$gid")"
  if [[ $attribute == unreadable ]]; then
    printf 'unreadable'
    return 0
  fi
  if [[ $content_equal != 1 ]]; then
    printf 'content'
    return 0
  fi
  printf '%s' "${attribute:-ok}"
}

# osquery_converge_directory_verdict <kind> <mode> <uid> <gid>: the verdict for
# one desired-state DIRECTORY (/var/osquery and /var/osquery/packs). Same tokens
# as the file verdict minus `content`, since a directory has no bytes of ours.
#
# This carries the role the old setup script's unconditional `sudo install -d`
# had. That call repaired the directory modes on every apply whether or not
# anything had moved; making it conditional is what buys the quiet no-op, so the
# check has to be here or the repair would be silently dropped.
osquery_converge_directory_verdict() {
  local kind="$1" mode="$2" uid="$3" gid="$4" attribute
  case "$kind" in
    absent)
      printf 'absent'
      return 0
      ;;
    directory) ;;
    *)
      printf 'irregular'
      return 0
      ;;
  esac
  attribute="$(_osquery_converge_attribute_verdict "$OSQUERY_CONVERGE_DIRECTORY_MODE" "$mode" "$uid" "$gid")"
  printf '%s' "${attribute:-ok}"
}

# osquery_converge_restart_verdict <verdict>...: the fold over every per-path
# verdict. Prints `restart` when ANY path drifted, `no-restart` when none did.
#
# Separate from the per-path verdicts on purpose. osqueryd reads its config and
# flags at startup, so a repaired file changes nothing until the daemon is
# bounced; and a bounce is the one destructive act in the whole tool (the vendor
# `osqueryctl stop` unloads the LaunchDaemon and deletes its plist), so what
# justifies it is worth stating once, in a function a test can invert.
#
# No arguments means nothing was examined, which justifies no bounce.
osquery_converge_restart_verdict() {
  local verdict
  for verdict in "$@"; do
    if [[ $verdict != ok ]]; then
      printf 'restart'
      return 0
    fi
  done
  printf 'no-restart'
}
