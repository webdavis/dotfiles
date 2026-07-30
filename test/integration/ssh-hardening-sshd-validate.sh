#!/usr/bin/env bash
# ssh-hardening-sshd-validate.sh -- default install against a hostile tree
# (slice 7, acceptance criterion 3), judged by an INDEPENDENT sshd -G.
#
# The properties pinned:
#   1. Against a sandbox tree containing a hostile 100-macos.conf that reopens
#      every hole, install writes the 000- drop-in, REMOVES a seeded legacy
#      50-no-password-auth.conf, and reports the effective configuration
#      verified.
#   2. The judge is not the script: the test's own `/usr/sbin/sshd -G` on the
#      same tree confirms all five effective values, one by one.
#   3. Install REFUSES to claim success when verification cannot pass: with a
#      hostile `Match all` re-enable present it exits nonzero and does not
#      print its success line, because the effective configuration is not fully
#      hardened. The tree is rolled back to what that run found, which here is
#      the drop-in the previous run left; ssh-hardening-install-safety.sh is
#      where the rollback itself is pinned.
set -euo pipefail

# Scrubbed at SCRIPT scope. Git exports GIT_DIR to every hook it runs and this
# suite can still inherit one from its caller.
unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

# shellcheck source=../fixtures/ssh-hardening-lib.bash
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../fixtures" && pwd)/ssh-hardening-lib.bash"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

refute_contains() { # <haystack> <fixed-string> <message>
  if grep -qiF -- "$2" <<<"$1"; then
    fail "$3"
  fi
}

[[ -x /usr/sbin/sshd ]] || {
  printf 'SKIP: /usr/sbin/sshd not present; cannot resolve effective configuration\n'
  exit 0
}

ssh_sandbox_setup
trap 'ssh_sandbox_teardown' EXIT

# --- 1 + 2: hostile tree, install, independent judge -------------------------

write_hostile_apple_conf
legacy="$SSHD_CONFIG_D/50-no-password-auth.conf"
printf 'PasswordAuthentication no\n' >"$legacy"
chmod 600 "$legacy"

run_ssh_hardening
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "install must succeed on the hostile tree, got exit $SSH_RUN_STATUS (stderr: $SSH_RUN_ERR)"

dropin="$SSHD_CONFIG_D/000-ssh-hardening.conf"
[[ -f $dropin ]] ||
  fail "install must write $dropin"
[[ ! -e $legacy ]] ||
  fail "install must REMOVE the seeded legacy $legacy (a 0600 root-owned legacy file breaks unprivileged verification for the whole tree)"
grep -qi 'verified' <<<"$SSH_RUN_OUT" ||
  fail "install must report the effective configuration verified (stdout: $SSH_RUN_OUT)"

# The independent judge: the test's own sshd -G, not the script's verifier.
for pair in "${SSH_HARDENED_PAIRS[@]}"; do
  key="${pair%% *}"
  want="${pair##* }"
  got="$(effective_global_value "$key")" ||
    fail "independent sshd -G failed on the installed tree"
  [[ $got == "$want" ]] ||
    fail "independent sshd -G: '$key' is '$got', want '$want' (the hostile 100-macos.conf must lose to the 000- drop-in)"
done

# --- 3: install refuses success when verify cannot pass ----------------------

cat >"$SSHD_CONFIG_D/700-hostile-match.conf" <<'EOF'
Match all
PasswordAuthentication yes
EOF

run_ssh_hardening
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail 'with a Match-block re-enable present, install must exit nonzero'
refute_contains "$SSH_RUN_OUT" 'install complete' \
  'a failed verification must suppress the install success line'
grep -qi 'not' <<<"$SSH_RUN_ERR" ||
  fail "the refusal must say the configuration is not fully hardened (stderr: $SSH_RUN_ERR)"

printf 'ssh-hardening-sshd-validate: OK (hostile tree installed and independently verified; legacy removed; install refuses success when verification fails)\n'
