#!/usr/bin/env bats
# The converge tool's DECISION CORE: given the observed state of one path in
# /var/osquery, has it drifted from the desired state, and does anything that
# drifted warrant restarting the root daemon.
#
# Pure functions, called directly. Every input arrives as an ARGUMENT, so no
# test here stats a file, reads /var/osquery, or needs a privilege. The stat and
# cmp probing lives in the execution edge, which its own suite drives.
#
# The mode/owner/group dimensions are the reason the files sit in /var/osquery
# at all: osqueryd runs those queries AS ROOT, so a config carrying the right
# bytes under the wrong permissions is the privilege-escalation vector the
# placement exists to close. A file with correct content and a 0666 mode MUST
# read as drift, and that is pinned below rather than left to the installer.

setup() {
  # shellcheck source=dot_local/libexec/osquery/osquery-converge/drift-verdict.sh
  source "$(cd "$BATS_TEST_DIRNAME/../.." && pwd)/dot_local/libexec/osquery/osquery-converge/drift-verdict.sh"
}

# --- files ------------------------------------------------------------------

@test "a file matching on content, mode, owner and group has not drifted" {
  [ "$(osquery_converge_file_verdict file 1 0644 0 0)" = ok ]
}

@test "nothing at the path reads as absent" {
  [ "$(osquery_converge_file_verdict absent '' '' '' '')" = absent ]
}

@test "a symlink standing where the config belongs reads as irregular" {
  # Never installed THROUGH: `install` would follow the link and write the
  # daemon's root-owned config wherever the link points.
  [ "$(osquery_converge_file_verdict symlink 1 0644 0 0)" = irregular ]
}

@test "a directory standing where the config belongs reads as irregular" {
  [ "$(osquery_converge_file_verdict other '' 0755 0 0)" = irregular ]
}

@test "differing bytes read as content drift" {
  [ "$(osquery_converge_file_verdict file 0 0644 0 0)" = content ]
}

@test "correct bytes under a world-writable mode read as drift, not as ok" {
  # The privilege-escalation vector: osqueryd reads this file as root, so a
  # 0666 config with the right bytes is a file any user process can rewrite
  # between now and the daemon's next config load.
  [ "$(osquery_converge_file_verdict file 1 0666 0 0)" = mode ]
}

@test "correct bytes owned by a non-root user read as drift" {
  [ "$(osquery_converge_file_verdict file 1 0644 501 0)" = owner ]
}

@test "correct bytes owned by a non-wheel group read as drift" {
  [ "$(osquery_converge_file_verdict file 1 0644 0 20)" = group ]
}

@test "a state that could not be read reads as unreadable, never as ok" {
  # Fail-safe: an unreadable mode is not evidence of a healthy file, and the
  # repair is idempotent, so the safe direction is to reinstall.
  [ "$(osquery_converge_file_verdict file 1 '' 0 0)" = unreadable ]
  [ "$(osquery_converge_file_verdict file '' 0644 0 0)" = unreadable ]
}

@test "content drift is reported ahead of an attribute that also drifted" {
  # One token per path, and the caller reinstalls either way; the token is what
  # the operator reads, so the most serious dimension wins.
  [ "$(osquery_converge_file_verdict file 0 0666 501 20)" = content ]
}

# --- directories ------------------------------------------------------------

@test "a directory at 0755 root:wheel has not drifted" {
  [ "$(osquery_converge_directory_verdict directory 0755 0 0)" = ok ]
}

@test "a group-writable directory reads as drift" {
  # A writable /var/osquery lets a user process replace the config wholesale,
  # which is the same escalation the file modes close.
  [ "$(osquery_converge_directory_verdict directory 0775 0 0)" = mode ]
}

@test "a missing directory reads as absent" {
  [ "$(osquery_converge_directory_verdict absent '' '' '')" = absent ]
}

@test "a file standing where the packs directory belongs reads as irregular" {
  [ "$(osquery_converge_directory_verdict file 0644 0 0)" = irregular ]
}

# --- the restart fold -------------------------------------------------------

@test "nothing drifted means no restart" {
  [ "$(osquery_converge_restart_verdict ok ok ok)" = no-restart ]
}

@test "any single drifted path warrants a restart" {
  [ "$(osquery_converge_restart_verdict ok mode ok)" = restart ]
}

@test "a drifted path in the last position still warrants a restart" {
  # A fold that stopped at the first entry, or overwrote its accumulator each
  # pass, would pass the middle case above and fail this one.
  [ "$(osquery_converge_restart_verdict ok ok content)" = restart ]
}

@test "an empty verdict list means no restart" {
  # Nothing was examined, so nothing justifies bouncing the root daemon.
  [ "$(osquery_converge_restart_verdict)" = no-restart ]
}
