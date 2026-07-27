#!/usr/bin/env bash
# ssh-hardening-match-reenable.sh -- --verify catches every Match-shaped
# bypass (slice 7, acceptance criterion 5). `sshd -G` without a connection
# spec dumps only the pre-Match configuration, so a Match block re-enabling a
# protected directive passes it while the machine is wide open; that is the
# most idiomatic sshd bypass and each form below must fail the verify, loudly
# and nonzero.
#
# The cases, each on an otherwise fully hardened tree:
#   a. Match Address 0.0.0.0/0 re-enable        (raw scan names the file)
#   b. Match User * re-enable                   (raw scan names the file)
#   c. Match all re-enable, PermitRootLogin     (raw scan names the file)
#   d. Match User <invoking user> re-enable     (the CONNECTION-SPEC check
#      must catch it: asserted on the connection check's own attribution)
#   e. sibling that sorts FIRST (00-evil.conf) globally re-enabling UsePAM
#      and PubkeyAuthentication (the GLOBAL check catches first-value-wins)
#   f. Match Address on a subnet NO connection sample hits (only the raw scan
#      can catch this one; it pins the scan as the completeness net)
#   g. '=' separator form: Match=all with PasswordAuthentication=yes (sshd
#      parses it, verified on OpenSSH 10.0p2, so the scan must too)
#   h. ChallengeResponseAuthentication alias re-enable inside Match (sshd
#      still honors the deprecated alias, verified on OpenSSH 10.0p2)
#
# Between cases the hostile file is removed and --verify must PASS again, so
# a pass after a fail proves no state leaks and every failure is the hostile
# file's own doing. The tree starts clean and verifies clean (positive
# control), so "fails" is asserted on the right dimension.
set -euo pipefail

# Scrubbed at SCRIPT scope. Git exports GIT_DIR to every hook it runs and this
# suite runs from the pre-push hook.
unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

# shellcheck source=../fixtures/ssh-hardening-lib.bash
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../fixtures" && pwd)/ssh-hardening-lib.bash"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -x /usr/sbin/sshd ]] || {
  printf 'SKIP: /usr/sbin/sshd not present; cannot resolve effective configuration\n'
  exit 0
}

ssh_sandbox_setup
trap 'ssh_sandbox_teardown' EXIT

# Harden the sandbox tree through the script's own install, then the positive
# control: a clean tree must PASS.
run_ssh_hardening
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "install on a clean sandbox must succeed (stderr: $SSH_RUN_ERR)"
run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "positive control: --verify must PASS on the clean hardened tree (stderr: $SSH_RUN_ERR)"

hostile_file="$SSHD_CONFIG_D/500-hostile.conf"

# run_reenable_case <label> <expected-stderr-needle> <hostile-content...>
# Writes the hostile file, expects --verify to fail naming the needle, then
# removes it and expects --verify to pass again (no state leak).
run_reenable_case() {
  local label="$1" needle="$2"
  shift 2
  printf '%s\n' "$@" >"$hostile_file"
  run_ssh_hardening --verify
  [[ $SSH_RUN_STATUS -ne 0 ]] ||
    fail "$label: --verify must exit nonzero (stdout: $SSH_RUN_OUT)"
  grep -qF -- "$needle" <<<"$SSH_RUN_ERR" ||
    fail "$label: stderr must contain '$needle' (stderr: $SSH_RUN_ERR)"
  rm -f "$hostile_file"
  run_ssh_hardening --verify
  [[ $SSH_RUN_STATUS -eq 0 ]] ||
    fail "$label: --verify must PASS again once the hostile file is removed; state leaked (stderr: $SSH_RUN_ERR)"
}

# a. The idiomatic bypass. The raw scan must name the offending file.
run_reenable_case 'a: Match Address 0.0.0.0/0' "$hostile_file" \
  'Match Address 0.0.0.0/0' 'PasswordAuthentication yes'

# b. Match User * catches every user; scan names the file.
run_reenable_case 'b: Match User *' "$hostile_file" \
  'Match User *' 'KbdInteractiveAuthentication yes'

# c. Match all; also pins permitrootlogin as a protected directive.
run_reenable_case 'c: Match all PermitRootLogin' "$hostile_file" \
  'Match all' 'PermitRootLogin yes'

# d. Match User <invoking user>: the connection-spec resolution must catch it.
# The raw scan fires too; the assertion pins the CONNECTION check's own
# attribution so removing that check fails here even with the scan intact.
invoking_user="$(id -un)"
run_reenable_case 'd: Match User (invoking user)' \
  "connection check (user=$invoking_user" \
  "Match User $invoking_user" 'PasswordAuthentication yes'

# e. A sibling sorting BEFORE 000- with GLOBAL re-enables: first-value-wins
# hands these two directives to the sibling, and only the global check sees
# it (no Match block anywhere). Pins usepam and pubkeyauthentication.
sibling="$SSHD_CONFIG_D/00-evil.conf"
printf 'UsePAM no\nPubkeyAuthentication no\n' >"$sibling"
run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail 'e: a first-sorting sibling with global re-enables must fail --verify'
grep -qF 'usepam' <<<"$SSH_RUN_ERR" ||
  fail "e: stderr must name usepam (stderr: $SSH_RUN_ERR)"
grep -qF 'pubkeyauthentication' <<<"$SSH_RUN_ERR" ||
  fail "e: stderr must name pubkeyauthentication (stderr: $SSH_RUN_ERR)"
rm -f "$sibling"
run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail 'e: --verify must PASS again once the sibling is removed'

# f. A subnet neither connection sample hits: the raw scan is the ONLY net.
run_reenable_case 'f: Match Address off-sample subnet' "$hostile_file" \
  'Match Address 198.51.100.0/24' 'PermitRootLogin yes'

# g. The '=' separator form sshd accepts must not slip past the scan.
run_reenable_case "g: '=' separator form" "$hostile_file" \
  'Match=all' 'PasswordAuthentication=yes'

# h. The deprecated alias still flips kbdinteractiveauthentication in sshd.
run_reenable_case 'h: ChallengeResponseAuthentication alias' "$hostile_file" \
  'Match Address 198.51.100.0/24' 'ChallengeResponseAuthentication yes'

printf 'ssh-hardening-match-reenable: OK (all Match bypass forms fail --verify loudly; clean tree passes before and after every case)\n'
