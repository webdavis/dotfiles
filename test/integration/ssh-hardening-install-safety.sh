#!/usr/bin/env bash
# ssh-hardening-install-safety.sh -- a failed install leaves the machine no
# worse than it found it.
#
# Refusing to CLAIM success is not the same as refusing to CAUSE harm, and the
# install path did the first while failing the second. Two demonstrations, both
# reproduced before this guard existed:
#
#   a. `tee` truncates its target the instant it opens it. With `cat` missing
#      from PATH the pipeline feeding it fails -- after the truncation. A valid
#      1478-byte drop-in came out of that run as 0 bytes, and the run exited
#      nonzero having destroyed the policy it was there to install.
#   b. On a main config that includes only `50-*`, install wrote an
#      unreferenced `000-` file, deleted the legacy `50-` file that was the
#      ONLY effective policy, failed its verification, and stopped. The machine
#      went from passwordauthentication no to passwordauthentication yes across
#      a run that reported failure.
#
# So the property is not "install reports honestly" but "install is a
# transaction": stage, publish, and on any failure put back exactly what was
# there. Every case below judges the tree with the test's own sshd, not with
# the script's report of itself.
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

refute_contains() { # <haystack> <fixed-string> <message>
  if grep -qiF -- "$2" <<<"$1"; then
    fail "$3"
  fi
}

[[ -x /usr/sbin/sshd ]] || {
  printf 'SKIP: /usr/sbin/sshd not present; cannot resolve effective configuration\n'
  exit 0
}

file_fingerprint() { # <path> -> size and checksum, or the word absent
  if [[ -e $1 ]]; then
    cksum <"$1"
  else
    printf 'absent\n'
  fi
}

ssh_sandbox_setup
trap 'ssh_sandbox_teardown' EXIT

dropin="$SSHD_CONFIG_D/000-ssh-hardening.conf"
legacy="$SSHD_CONFIG_D/50-no-password-auth.conf"

# --- a: a staging failure must not touch the drop-in already in place ---------

run_ssh_hardening
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "setup: the first install must succeed (stderr: $SSH_RUN_ERR)"
before_fingerprint="$(file_fingerprint "$dropin")"
[[ $before_fingerprint != 'absent' ]] ||
  fail 'setup: the first install must leave a drop-in behind'

# `cat` is what print_config uses to emit the policy. Losing it fails the
# pipeline that feeds tee -- which is exactly the ordering that used to empty
# the target.
run_ssh_hardening_without cat
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail 'a: install must fail when it cannot generate the policy'
after_fingerprint="$(file_fingerprint "$dropin")"
[[ $after_fingerprint == "$before_fingerprint" ]] ||
  fail "a: the drop-in already in place must be untouched by a failed staging (before: $before_fingerprint; after: $after_fingerprint)"
refute_contains "$SSH_RUN_OUT" 'install complete' \
  'a: a failed staging must not claim success'

run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "a: the tree must still verify after the failed install (stderr: $SSH_RUN_ERR)"

# --- b: a failed verification restores the tree it found ----------------------
# The main config here includes ONLY 50-*, so the drop-in install writes is
# inert and the legacy file is the whole policy. Install therefore cannot
# succeed, and the question is what it leaves behind.

rm -f "$SSHD_CONFIG_D"/* "$SSHD_CONFIG_D"/.[!.]* 2>/dev/null || true
printf 'Include %s/50-*\n' "$SSHD_CONFIG_D" >"$SSHD_MAIN_CONFIG"
/bin/bash "$SSH_HARDENING_SCRIPT" --print-config >"$legacy"
legacy_fingerprint="$(file_fingerprint "$legacy")"

for pair in "${SSH_HARDENED_PAIRS[@]}"; do
  key="${pair%% *}"
  want="${pair##* }"
  got="$(effective_global_value "$key")" || fail 'b: sshd -G failed before the install'
  [[ $got == "$want" ]] ||
    fail "b: the tree must be hardened BEFORE the install, '$key' is '$got'; otherwise this case cannot show install making it worse"
done

run_ssh_hardening
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail 'b: install cannot succeed when the drop-in it writes is not included'
refute_contains "$SSH_RUN_OUT" 'install complete' \
  'b: a failed verification must not claim success'

# The whole point. Every directive must still resolve hardened.
for pair in "${SSH_HARDENED_PAIRS[@]}"; do
  key="${pair%% *}"
  want="${pair##* }"
  got="$(effective_global_value "$key")" || fail 'b: sshd -G failed after the install'
  [[ $got == "$want" ]] ||
    fail "b: after a FAILED install the tree must be exactly as hardened as before, but '$key' is now '$got' (want '$want'); the install made the machine worse"
done

[[ "$(file_fingerprint "$legacy")" == "$legacy_fingerprint" ]] ||
  fail 'b: the legacy drop-in was the only effective policy and must be back, byte for byte'
[[ ! -e $dropin ]] ||
  fail 'b: the drop-in this run created must be removed again; there was none before it'

# --- c: no working files are left behind, on either outcome -------------------
# The staging and rollback files are dot-prefixed so sshd's Include glob cannot
# match them (glob(3) does not match a leading dot), but leaving them lying
# around is still litter in a directory that is supposed to hold policy only.

leftovers="$(ls -A "$SSHD_CONFIG_D" | grep -E '\.(staging|saved)$' || true)"
[[ -z $leftovers ]] ||
  fail "c: a failed install left working files behind: $leftovers"

printf 'Include %s/*\n' "$SSHD_CONFIG_D" >"$SSHD_MAIN_CONFIG"
rm -f "$SSHD_CONFIG_D"/* "$SSHD_CONFIG_D"/.[!.]* 2>/dev/null || true
run_ssh_hardening
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "c: install on a clean tree must succeed (stderr: $SSH_RUN_ERR)"
leftovers="$(ls -A "$SSHD_CONFIG_D" | grep -E '\.(staging|saved)$' || true)"
[[ -z $leftovers ]] ||
  fail "c: a successful install left working files behind: $leftovers"

# --- d: the legacy file survives a failed run, and only a failed run ----------
# On success the legacy file really must go: two files declaring the same
# policy is drift waiting to happen, and the legacy one is 0600, which breaks
# unprivileged verification for the whole tree.

printf 'PasswordAuthentication no\n' >"$legacy"
run_ssh_hardening
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "d: install must succeed on this tree (stderr: $SSH_RUN_ERR)"
[[ ! -e $legacy ]] ||
  fail 'd: a SUCCESSFUL install must still retire the legacy drop-in'

# --- e: the rollback copies are invisible to sshd -----------------------------
# The legacy file here carries a Match-scoped re-enable, so retiring it is the
# whole reason the tree can verify at all. install keeps a copy of it until the
# replacement is proven good -- and if that copy were visible to sshd's Include
# glob, the re-enable would still be in the effective configuration and the
# install would fail. glob(3) does not match a leading dot, which is exactly
# why the copy is dot-prefixed and not, say, suffixed.

rm -f "$SSHD_CONFIG_D"/* "$SSHD_CONFIG_D"/.[!.]* 2>/dev/null || true
printf 'Include %s/*\n' "$SSHD_CONFIG_D" >"$SSHD_MAIN_CONFIG"
printf 'Match Address 198.51.100.0/24\nPasswordAuthentication yes\n' >"$legacy"

off_sample_spec='user=offsample,host=elsewhere.example,addr=198.51.100.23'
got="$(effective_spec_value "$off_sample_spec" passwordauthentication)" ||
  fail 'e: sshd -G -T -C failed before the install'
[[ $got == 'yes' ]] ||
  fail "e: the legacy file must really re-enable password authentication off-loopback (got '$got'); otherwise retiring it proves nothing"

run_ssh_hardening
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "e: retiring a legacy file that carried the re-enable must let the install SUCCEED; a rollback copy still visible to sshd's Include glob would keep the re-enable in the effective configuration (stderr: $SSH_RUN_ERR)"

got="$(effective_spec_value "$off_sample_spec" passwordauthentication)" ||
  fail 'e: sshd -G -T -C failed after the install'
[[ $got == 'no' ]] ||
  fail "e: after the install the re-enable must be gone from the effective configuration, got '$got'"

printf 'ssh-hardening-install-safety: OK (a failed staging leaves the existing drop-in byte-identical, a failed verification restores the tree sshd resolves as hardened along with the legacy file, no working files survive either outcome, and a successful install still retires the legacy file)\n'
