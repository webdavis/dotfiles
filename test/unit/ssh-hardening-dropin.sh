#!/usr/bin/env bash
# ssh-hardening-dropin.sh -- the two pure modes of ssh-hardening.sh, the test
# seam everything else stands on (slice 7).
#
# The properties pinned, one per acceptance criterion:
#   1. --print-config emits EVERY accepted directive (asserted one by one:
#      completeness over counting), each exactly once among the non-comment
#      lines with its accepted value, so no conflicting directive can hide.
#      No privilege escalation, no write.
#   2. --print-path names 000-ssh-hardening.conf under the configured drop-in
#      directory, and an LC_ALL=C sort places that name before Apple's
#      100-macos.conf (sshd's Include is lexical and first-value-wins, so
#      sorting first is what keeps the drop-in authoritative). No privilege
#      escalation, no write.
#   3. --print-config emits the tailnet restriction as ONE Match block keyed on
#      LocalAddress, whose pattern list ends in a positive '*' so a typo in a
#      negated term refuses rather than admits.
#   4. --print-config is byte-identical to the drop-in template posture's own
#      generator emits. Two tools install this one file and nothing else makes
#      them agree, so this is the gate that catches an edit to one of them.
#
# Runs through the sandbox harness: seams point at a scratch tree, a failing
# sudo stub on PATH blocks escalation, and the script runs under /bin/bash so
# a bash 3.2 regression fails here.
set -euo pipefail

# Scrubbed at SCRIPT scope. Git exports GIT_DIR to every hook it runs and this
# suite runs from the pre-commit hook.
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

# Every accepted line, each asserted individually. The list is the shared one,
# so a directive added to policy cannot leave this guard checking a short set.
for accepted_line in \
  'PasswordAuthentication no' \
  'KbdInteractiveAuthentication no' \
  'UsePAM yes' \
  'PubkeyAuthentication yes' \
  'PermitRootLogin no' \
  'GSSAPIAuthentication no' \
  'HostbasedAuthentication no'; do
  grep -qxF "$accepted_line" <<<"$SSH_RUN_OUT" ||
    fail "--print-config must emit '$accepted_line' (got: $SSH_RUN_OUT)"
done

# No conflicting directive: among non-comment lines, each protected keyword
# appears EXACTLY once (case-insensitive, '=' separator counted too), so a
# second occurrence with a hostile value cannot ride along.
noncomment_lines="$(grep -Ev '^[[:space:]]*(#|$)' <<<"$SSH_RUN_OUT")"
for keyword in PasswordAuthentication KbdInteractiveAuthentication UsePAM \
  PubkeyAuthentication PermitRootLogin GSSAPIAuthentication \
  HostbasedAuthentication; do
  occurrences="$(grep -icE "^[[:space:]]*${keyword}([[:space:]=]|$)" \
    <<<"$noncomment_lines")" || true
  [[ $occurrences -eq 1 ]] ||
    fail "--print-config must set '$keyword' exactly once among non-comment lines, found $occurrences"
done

# The tailnet restriction: one Match block, keyed on the address the connection
# ARRIVED on (a client can claim its own address but not this host's interface),
# and a pattern list whose last term is a positive '*'. The '*' is what makes a
# typo in a negated pattern refuse that address instead of admitting it; a list
# carrying no positive term matches nothing and refuses nobody.
expected_match_line='Match LocalAddress "!127.0.0.0/8,!::1,!100.64.0.0/10,!fd7a:115c:a1e0::/48,*"'
match_lines="$(grep -cE '^[[:space:]]*Match([[:space:]=]|$)' <<<"$noncomment_lines")" || true
[[ $match_lines -eq 1 ]] ||
  fail "--print-config must emit exactly one Match block, found $match_lines"
grep -qxF "$expected_match_line" <<<"$SSH_RUN_OUT" ||
  fail "--print-config must emit '$expected_match_line' (got: $SSH_RUN_OUT)"
grep -qxF '  RefuseConnection yes' <<<"$SSH_RUN_OUT" ||
  fail "--print-config must refuse connections inside that Match block (got: $SSH_RUN_OUT)"

# The two generators of this one file, held to the byte. Nothing else makes
# them agree: posture reads its copy through include_str! and cannot reach this
# script, so the equality is pinned here, from the repository that owns both.
posture_template="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/posture/crates/posture-domain/src/ssh_policy/dropin.conf"
[[ -f $posture_template ]] ||
  fail "the posture drop-in template is missing at '$posture_template'"
diff -u "$posture_template" <(printf '%s\n' "$SSH_RUN_OUT") ||
  fail '--print-config must be byte-identical to the posture drop-in template'

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

printf 'ssh-hardening-dropin: OK (both pure modes: every directive exactly once, one LocalAddress refusal block, byte-identical to the posture template, 000- name sorts before 100-macos.conf, no escalation, no writes)\n'
