#!/usr/bin/env bash
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
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit: both belong to a script that runs on its own, and either one
# here would reach into the runner's own shell. `just test-bashunit` runs it;
# test/validate-tests.sh pins the shape.
#
# assert_same, never assert_equals. bashunit's assert_equals normalizes away
# ANSI and control characters before comparing, so a verdict of `o<TAB>k` passes
# as `ok` (measured on 0.50.1); assert_same compares the bytes, which is what
# the `[ "$(...)" = ok ]` this file was converted from did. Every token below is
# a single word the caller dispatches on, so a stray control character in one is
# a real defect and must not pass.

subject_under_test() {
  printf '%s' "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/dot_local/libexec/osquery/osquery-converge/drift-verdict.sh"
}

# shellcheck source=dot_local/libexec/osquery/osquery-converge/drift-verdict.sh
source "$(subject_under_test)"

# --- files ------------------------------------------------------------------

function test_a_file_matching_on_content_mode_owner_and_group_has_not_drifted() {
  assert_same ok "$(osquery_converge_file_verdict file 1 0644 0 0)"
}

function test_nothing_at_the_path_reads_as_absent() {
  assert_same absent "$(osquery_converge_file_verdict absent '' '' '' '')"
}

function test_a_symlink_standing_where_the_config_belongs_reads_as_irregular() {
  # Never installed THROUGH: `install` would follow the link and write the
  # daemon's root-owned config wherever the link points.
  assert_same irregular "$(osquery_converge_file_verdict symlink 1 0644 0 0)"
}

function test_a_directory_standing_where_the_config_belongs_reads_as_irregular() {
  assert_same irregular "$(osquery_converge_file_verdict other '' 0755 0 0)"
}

function test_differing_bytes_read_as_content_drift() {
  assert_same content "$(osquery_converge_file_verdict file 0 0644 0 0)"
}

function test_correct_bytes_under_a_world_writable_mode_read_as_drift_not_as_ok() {
  # The privilege-escalation vector: osqueryd reads this file as root, so a
  # 0666 config with the right bytes is a file any user process can rewrite
  # between now and the daemon's next config load.
  assert_same mode "$(osquery_converge_file_verdict file 1 0666 0 0)"
}

function test_correct_bytes_owned_by_a_non_root_user_read_as_drift() {
  assert_same owner "$(osquery_converge_file_verdict file 1 0644 501 0)"
}

function test_correct_bytes_owned_by_a_non_wheel_group_read_as_drift() {
  assert_same group "$(osquery_converge_file_verdict file 1 0644 0 20)"
}

function test_a_state_that_could_not_be_read_reads_as_unreadable_never_as_ok() {
  # Fail-safe: an unreadable mode is not evidence of a healthy file, and the
  # repair is idempotent, so the safe direction is to reinstall.
  assert_same unreadable "$(osquery_converge_file_verdict file 1 '' 0 0)"
  assert_same unreadable "$(osquery_converge_file_verdict file '' 0644 0 0)"
}

function test_content_drift_is_reported_ahead_of_an_attribute_that_also_drifted() {
  # One token per path, and the caller reinstalls either way; the token is what
  # the operator reads, so the most serious dimension wins.
  assert_same content "$(osquery_converge_file_verdict file 0 0666 501 20)"
}

# --- directories ------------------------------------------------------------

function test_a_directory_at_0755_root_wheel_has_not_drifted() {
  assert_same ok "$(osquery_converge_directory_verdict directory 0755 0 0)"
}

function test_a_group_writable_directory_reads_as_drift() {
  # A writable /var/osquery lets a user process replace the config wholesale,
  # which is the same escalation the file modes close.
  assert_same mode "$(osquery_converge_directory_verdict directory 0775 0 0)"
}

function test_a_missing_directory_reads_as_absent() {
  assert_same absent "$(osquery_converge_directory_verdict absent '' '' '')"
}

function test_a_file_standing_where_the_packs_directory_belongs_reads_as_irregular() {
  assert_same irregular "$(osquery_converge_directory_verdict file 0644 0 0)"
}

# --- the restart fold -------------------------------------------------------

function test_nothing_drifted_means_no_restart() {
  assert_same no-restart "$(osquery_converge_restart_verdict ok ok ok)"
}

function test_any_single_drifted_path_warrants_a_restart() {
  assert_same restart "$(osquery_converge_restart_verdict ok mode ok)"
}

function test_a_drifted_path_in_the_last_position_still_warrants_a_restart() {
  # A fold that stopped at the first entry, or overwrote its accumulator each
  # pass, would pass the middle case above and fail this one.
  assert_same restart "$(osquery_converge_restart_verdict ok ok content)"
}

function test_an_empty_verdict_list_means_no_restart() {
  # Nothing was examined, so nothing justifies bouncing the root daemon.
  assert_same no-restart "$(osquery_converge_restart_verdict)"
}
