#!/usr/bin/env bash
# ssh-hardening-dropin.sh -- the two pure modes of ssh-hardening.sh, the test
# seam everything else stands on (slice 7).
#
# The properties pinned, one per acceptance criterion:
#   1. --print-config emits ALL FIVE accepted directives (asserted one by one:
#      completeness over counting), each exactly once among the non-comment
#      lines with its accepted value, so no conflicting directive can hide.
#      No privilege escalation, no write.
#   2. --print-path names 000-ssh-hardening.conf under the configured drop-in
#      directory, and an LC_ALL=C sort places that name before Apple's
#      100-macos.conf (sshd's Include is lexical and first-value-wins, so
#      sorting first is what keeps the drop-in authoritative). No privilege
#      escalation, no write.
#
# Runs through the sandbox harness: seams point at a scratch tree, a failing
# sudo stub on PATH blocks escalation, and the script runs under /bin/bash so
# a bash 3.2 regression fails here.
set -euo pipefail

# Scrubbed at SCRIPT scope. Git exports GIT_DIR to every hook it runs and this
# suite runs from the pre-commit and pre-push hooks.
unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

# shellcheck source=../fixtures/ssh-hardening-lib.bash
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../fixtures" && pwd)/ssh-hardening-lib.bash"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

ssh_sandbox_setup
trap 'ssh_sandbox_teardown' EXIT

# The sandbox drop-in dir starts EMPTY; both pure modes must leave it so.
baseline_listing="$(ls -A "$SSHD_CONFIG_D")"

# --- Criterion 1: --print-config -------------------------------------------

run_ssh_hardening --print-config
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "--print-config must exit 0, got $SSH_RUN_STATUS (stderr: $SSH_RUN_ERR)"

# All five accepted lines, each asserted individually.
for accepted_line in \
  'PasswordAuthentication no' \
  'KbdInteractiveAuthentication no' \
  'UsePAM yes' \
  'PubkeyAuthentication yes' \
  'PermitRootLogin no'; do
  grep -qxF "$accepted_line" <<<"$SSH_RUN_OUT" ||
    fail "--print-config must emit '$accepted_line' (got: $SSH_RUN_OUT)"
done

# No conflicting directive: among non-comment lines, each protected keyword
# appears EXACTLY once (case-insensitive, '=' separator counted too), so a
# second occurrence with a hostile value cannot ride along.
noncomment_lines="$(grep -Ev '^[[:space:]]*(#|$)' <<<"$SSH_RUN_OUT")"
for keyword in PasswordAuthentication KbdInteractiveAuthentication UsePAM \
  PubkeyAuthentication PermitRootLogin; do
  occurrences="$(grep -icE "^[[:space:]]*${keyword}([[:space:]=]|$)" \
    <<<"$noncomment_lines")" || true
  [[ $occurrences -eq 1 ]] ||
    fail "--print-config must set '$keyword' exactly once among non-comment lines, found $occurrences"
done

assert_no_sudo_and_no_sandbox_write '--print-config' "$baseline_listing" ||
  fail '--print-config must be pure'

# --- Criterion 2: --print-path ----------------------------------------------

run_ssh_hardening --print-path
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "--print-path must exit 0, got $SSH_RUN_STATUS (stderr: $SSH_RUN_ERR)"

dropin_name="$(basename "$SSH_RUN_OUT")"
[[ $dropin_name == '000-ssh-hardening.conf' ]] ||
  fail "--print-path must name 000-ssh-hardening.conf, got '$SSH_RUN_OUT'"
[[ $SSH_RUN_OUT == "$SSHD_CONFIG_D/$dropin_name" ]] ||
  fail "--print-path must resolve under the SSHD_CONFIG_D seam, got '$SSH_RUN_OUT'"

# The precedence property itself: in sshd's lexical (LC_ALL=C) include order
# the drop-in must sort BEFORE Apple's file, or first-value-wins hands every
# directive to Apple.
first_sorted="$(printf '%s\n%s\n' "$dropin_name" '100-macos.conf' |
  LC_ALL=C sort | head -n 1)"
[[ $first_sorted == "$dropin_name" ]] ||
  fail "the drop-in name '$dropin_name' must sort before 100-macos.conf in LC_ALL=C order"

assert_no_sudo_and_no_sandbox_write '--print-path' "$baseline_listing" ||
  fail '--print-path must be pure'

printf 'ssh-hardening-dropin: OK (both pure modes: five directives exactly once, 000- name sorts before 100-macos.conf, no escalation, no writes)\n'
