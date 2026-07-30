#!/usr/bin/env bash
# ssh-hardening-include-precedence.sh -- the REGRESSION GUARD that documents
# why the drop-in was renamed 50-no-password-auth.conf -> 000-ssh-hardening.conf
# (slice 7, acceptance criterion 4). It PROVES the shadowing with a real sshd
# rather than asserting it in prose.
#
# Honesty note: the file Apple ships today sets none of the protected
# directives, so nothing is shadowed on a real machine right now; the rename
# is insurance against a future 100-macos.conf that competes. The guard
# therefore runs against a HOSTILE 100-macos.conf that reopens every hole:
#
#   Phase A: the OLD name (50-) carrying the full hardened content IS defeated
#            by the hostile 100- file, because '1' < '5' in LC_ALL=C order and
#            sshd's Include is lexical, first-value-wins.
#   Phase B: the NEW name, taken from the script's own --print-path, wins
#            against the same hostile file on the same tree.
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

[[ -x /usr/sbin/sshd ]] || {
  printf 'SKIP: /usr/sbin/sshd not present; cannot resolve effective configuration\n'
  exit 0
}

ssh_sandbox_setup
trap 'ssh_sandbox_teardown' EXIT

write_hostile_apple_conf

# --- Phase A: the old name loses ---------------------------------------------

run_ssh_hardening --print-config
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "--print-config must succeed to seed the phases (stderr: $SSH_RUN_ERR)"
hardened_content="$SSH_RUN_OUT"

old_name_file="$SSHD_CONFIG_D/50-no-password-auth.conf"
printf '%s\n' "$hardened_content" >"$old_name_file"

for pair in "${SSH_HOSTILE_PAIRS[@]}"; do
  key="${pair%% *}"
  hostile_value="${pair##* }"
  got="$(effective_global_value "$key")" || fail 'sshd -G failed in phase A'
  [[ $got == "$hostile_value" ]] ||
    fail "phase A: with the OLD 50- name, the hostile value for '$key' must win (got '$got'); if this now fails, the shadowing premise itself has changed"
done
rm -f "$old_name_file"

# --- Phase B: the script's name wins -----------------------------------------

run_ssh_hardening --print-path
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "--print-path must succeed (stderr: $SSH_RUN_ERR)"
new_name_file="$SSH_RUN_OUT"
[[ $new_name_file == "$SSHD_CONFIG_D"/* ]] ||
  fail "--print-path must resolve inside the sandbox tree, got '$new_name_file'"
printf '%s\n' "$hardened_content" >"$new_name_file"

for pair in "${SSH_HARDENED_PAIRS[@]}"; do
  key="${pair%% *}"
  want="${pair##* }"
  got="$(effective_global_value "$key")" || fail 'sshd -G failed in phase B'
  [[ $got == "$want" ]] ||
    fail "phase B: with the script's name '$(basename "$new_name_file")', the hardened value for '$key' must win (got '$got')"
done

printf 'ssh-hardening-include-precedence: OK (old 50- name provably defeated by a hostile 100-macos.conf; the 000- name wins on the same tree)\n'
