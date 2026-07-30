#!/usr/bin/env bash
# ssh-hardening-include-following.sh -- --verify reads the configuration sshd
# reads, not the two directories it was pointed at.
#
# The scan used to glob $SSHD_MAIN_CONFIG plus $SSHD_CONFIG_D/* and stop
# there. sshd does not stop there: Include pulls in arbitrary paths, from
# anywhere, and the Match state in force at the Include point carries INTO the
# included file. Both shapes below were demonstrated against OpenSSH 10.0p2 as
# working bypasses, and every case here proves itself the same way -- the test
# resolves the tree with its own sshd and requires the unsafe value before it
# asks whether --verify catches it.
#
#   a. Include reaching OUTSIDE the scanned set: a drop-in pulls in a file in
#      another directory that opens a Match block and re-enables two
#      directives. Nothing in the old scanned set contained a single hostile
#      byte.
#   b. Include INSIDE a Match block: the drop-in opens
#      `Match Address *,!127.0.0.1` and includes a file holding nothing but
#      `PasswordAuthentication yes`. sshd applies that directive inside the
#      surrounding Match -- off-loopback resolves yes while loopback stays no,
#      which is what makes it invisible to both connection samples.
#   c. The state does NOT bleed back: a Match opened inside an included file
#      must not leak into the including file after the Include returns. This
#      one asserts a PASS, because the failure mode is a false positive, and a
#      verifier that cries wolf is a verifier somebody turns off.
#   d. An Include CYCLE fails closed, names itself as a cycle, and TERMINATES.
#   e. A chain deeper than sshd itself accepts fails closed.
#   f. A RELATIVE Include is resolved against sshd's configuration directory.
#   g. Several paths on one Include line are all followed.
#   h. A glob Include is expanded.
#   i. A quoted Include path containing spaces survives tokenization.
#
# Case d is bounded by a wall clock rather than asserted after the fact: an
# unguarded cycle does not fail, it never returns, and a verifier that hangs
# reports nothing at all.
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

run_ssh_hardening
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "install on a clean sandbox must succeed (stderr: $SSH_RUN_ERR)"
run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "positive control: --verify must PASS on the clean hardened tree (stderr: $SSH_RUN_ERR)"

off_sample_spec='user=offsample,host=elsewhere.example,addr=198.51.100.23'
loopback_spec='user=root,host=localhost,addr=127.0.0.1'
outside_dir="$SSH_SANDBOX/outside"
mkdir -p "$outside_dir"

# assert_resolves <label> <spec> <key> <expected>: the test's own sshd, so a
# fixture that sshd ignores cannot masquerade as a bypass.
assert_resolves() {
  local label="$1" spec="$2" key="$3" expected="$4" got
  got="$(effective_spec_value "$spec" "$key")" ||
    fail "$label: the test's own sshd could not resolve '$key' for '$spec'"
  [[ $got == "$expected" ]] ||
    fail "$label: real sshd must resolve '$key' as '$expected' for '$spec', got '$got'"
}

# assert_verify_fails_naming <label> <needle>...
assert_verify_fails_naming() {
  local label="$1" needle
  shift
  run_ssh_hardening --verify
  [[ $SSH_RUN_STATUS -ne 0 ]] ||
    fail "$label: --verify must exit nonzero (stdout: $SSH_RUN_OUT)"
  for needle in "$@"; do
    grep -qF -- "$needle" <<<"$SSH_RUN_ERR" ||
      fail "$label: stderr must contain '$needle' (stderr: $SSH_RUN_ERR)"
  done
}

assert_verify_passes_again() {
  run_ssh_hardening --verify
  [[ $SSH_RUN_STATUS -eq 0 ]] ||
    fail "$1: --verify must PASS again once the fixture is removed; state leaked (stderr: $SSH_RUN_ERR)"
}

# --- a: Include reaching outside the scanned set -----------------------------

included_evil="$outside_dir/evil-included"
printf 'Match Address 198.51.100.0/24\n    PasswordAuthentication yes\n    PermitRootLogin yes\n' \
  >"$included_evil"
printf 'Include %s\n' "$included_evil" >"$SSHD_CONFIG_D/999-pull.conf"

assert_resolves 'a: include outside the scanned set' \
  "$off_sample_spec" passwordauthentication yes
assert_resolves 'a: include outside the scanned set' \
  "$off_sample_spec" permitrootlogin yes
assert_verify_fails_naming 'a: include outside the scanned set' \
  "match scan: '$included_evil' sets 'passwordauthentication yes'" \
  "match scan: '$included_evil' sets 'permitrootlogin yes'"
rm -f "$SSHD_CONFIG_D/999-pull.conf" "$included_evil"
assert_verify_passes_again 'a'

# --- b: Include inside a Match block ------------------------------------------
# The included file holds no Match line of its own. It is hostile only because
# of where it is pulled in, which is exactly why a per-file scan cannot see it.

reopen_inc="$SSH_SANDBOX/reopen.inc"
printf 'PasswordAuthentication yes\n' >"$reopen_inc"
printf 'Match Address *,!127.0.0.1\nInclude %s\n' "$reopen_inc" \
  >"$SSHD_CONFIG_D/500-wrapper.conf"

assert_resolves 'b: include inside a Match block' \
  "$off_sample_spec" passwordauthentication yes
# The other half of the demonstration: loopback stays hardened, so neither
# connection sample --verify probes can see this at all.
assert_resolves 'b: include inside a Match block (loopback stays hardened)' \
  "$loopback_spec" passwordauthentication no
assert_verify_fails_naming 'b: include inside a Match block' \
  "match scan: '$reopen_inc' sets 'passwordauthentication yes'"
rm -f "$SSHD_CONFIG_D/500-wrapper.conf" "$reopen_inc"
assert_verify_passes_again 'b'

# --- c: a Match inside an included file does not bleed back -------------------
# sshd restores the including file's Match state when the Include returns
# (verified: a directive after the Include is applied even though the included
# file ended inside a Match that did not match). A scan that kept the included
# file's state would read the parent's remaining GLOBAL directives as
# Match-scoped and report a tree sshd resolves as hardened.

harmless_inc="$SSH_SANDBOX/harmless.inc"
printf 'Match User no-such-user-for-this-test\nPasswordAuthentication no\n' >"$harmless_inc"
printf 'Include %s\nPasswordAuthentication yes\n' "$harmless_inc" \
  >"$SSHD_CONFIG_D/500-parent.conf"

# The parent's global `PasswordAuthentication yes` loses to the 000- drop-in
# under first-value-wins, so the tree really is hardened and a failure here
# could only be the scan's own false positive.
for spec in "$off_sample_spec" "$loopback_spec"; do
  assert_resolves 'c: no bleed-back' "$spec" passwordauthentication no
done
run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "c: --verify must PASS: the parent's global directive is not inside a Match, and sshd resolves the tree hardened (stderr: $SSH_RUN_ERR)"
rm -f "$SSHD_CONFIG_D/500-parent.conf" "$harmless_inc"
assert_verify_passes_again 'c'

# --- d: an Include cycle fails closed, and terminates -------------------------

cycle_b="$SSH_SANDBOX/cycle-b.inc"
printf 'Include %s\n' "$cycle_b" >"$SSHD_CONFIG_D/700-cycle-a.conf"
printf 'Include %s/700-cycle-a.conf\n' "$SSHD_CONFIG_D" >"$cycle_b"

run_ssh_hardening_bounded 60 --verify
[[ $SSH_RUN_TIMED_OUT -eq 0 ]] ||
  fail 'd: --verify must TERMINATE on an Include cycle; it was still running after 60s'
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail "d: an Include cycle must fail --verify (stdout: $SSH_RUN_OUT)"
grep -qF 'Include cycle' <<<"$SSH_RUN_ERR" ||
  fail "d: the failure must name the cycle rather than any other refusal (stderr: $SSH_RUN_ERR)"
rm -f "$SSHD_CONFIG_D/700-cycle-a.conf" "$cycle_b"
assert_verify_passes_again 'd'

# --- e: a chain deeper than sshd accepts fails closed -------------------------
# sshd refuses at 16 files below the main config ("Too many recursive
# configuration includes"). The scan refuses at the same depth instead of
# returning quietly with part of the tree unread.

deep_dir="$SSH_SANDBOX/deep"
mkdir -p "$deep_dir"
printf 'PasswordAuthentication no\n' >"$deep_dir/deep_end"
previous="$deep_dir/deep_end"
for level in $(seq 1 20); do
  printf 'Include %s\n' "$previous" >"$deep_dir/deep_$level"
  previous="$deep_dir/deep_$level"
done
printf 'Include %s\n' "$previous" >"$SSHD_CONFIG_D/700-deep.conf"

run_ssh_hardening_bounded 60 --verify
[[ $SSH_RUN_TIMED_OUT -eq 0 ]] ||
  fail 'e: --verify must TERMINATE on an over-deep include chain'
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail "e: an over-deep include chain must fail --verify (stdout: $SSH_RUN_OUT)"
grep -qF 'Include levels deep' <<<"$SSH_RUN_ERR" ||
  fail "e: the failure must name the depth refusal (stderr: $SSH_RUN_ERR)"
rm -rf "$deep_dir" "$SSHD_CONFIG_D/700-deep.conf"
assert_verify_passes_again 'e'

# --- f: a relative Include resolves against sshd's configuration directory ----
# HONESTY NOTE: sshd resolves a relative Include against its COMPILED-IN
# /etc/ssh, whatever -f points at (verified two ways: a matching file in the
# working directory is ignored, and `Include sshd_config.d/*` from a sandbox
# main config reproduces the live /etc/ssh resolution byte for byte). This test
# must never write to /etc/ssh, so it cannot make real sshd read the fixture
# below, and this case alone is not proven end to end. It pins the resolution
# RULE the scan applies -- relative to the directory holding the main config,
# which IS /etc/ssh in production -- and the scan is deliberately conservative
# here: reporting a file sshd would not read costs a false alarm, while
# skipping relative includes would be a hole in the live tree.

relative_evil="$SSH_SANDBOX/relative-evil.inc"
printf 'Match Address 198.51.100.0/24\nPermitRootLogin yes\n' >"$relative_evil"
printf 'Include relative-evil.inc\n' >"$SSHD_CONFIG_D/600-relative.conf"
assert_verify_fails_naming 'f: relative include' \
  "match scan: '$relative_evil' sets 'permitrootlogin yes'"
rm -f "$SSHD_CONFIG_D/600-relative.conf" "$relative_evil"
assert_verify_passes_again 'f'

# --- g: several paths on one Include line -------------------------------------

first_inc="$SSH_SANDBOX/multi-1.inc"
second_inc="$SSH_SANDBOX/multi-2.inc"
printf 'Match Address 198.51.100.0/24\nKbdInteractiveAuthentication yes\n' >"$first_inc"
printf 'Match Address 198.51.100.0/24\nPermitRootLogin yes\n' >"$second_inc"
printf 'Include %s %s\n' "$first_inc" "$second_inc" >"$SSHD_CONFIG_D/600-multi.conf"

assert_resolves 'g: multiple include paths' \
  "$off_sample_spec" kbdinteractiveauthentication yes
assert_resolves 'g: multiple include paths' \
  "$off_sample_spec" permitrootlogin yes
# Both needles: a scan that follows only the first path passes the first
# assertion and fails the second.
assert_verify_fails_naming 'g: multiple include paths' \
  "match scan: '$first_inc' sets 'kbdinteractiveauthentication yes'" \
  "match scan: '$second_inc' sets 'permitrootlogin yes'"
rm -f "$SSHD_CONFIG_D/600-multi.conf" "$first_inc" "$second_inc"
assert_verify_passes_again 'g'

# --- h: a glob Include is expanded --------------------------------------------

glob_dir="$SSH_SANDBOX/globdir"
mkdir -p "$glob_dir"
printf 'Match Address 198.51.100.0/24\nKbdInteractiveAuthentication yes\n' >"$glob_dir/hostile.inc"
printf 'Include %s/*.inc\n' "$glob_dir" >"$SSHD_CONFIG_D/600-glob.conf"

assert_resolves 'h: glob include' "$off_sample_spec" kbdinteractiveauthentication yes
assert_verify_fails_naming 'h: glob include' \
  "match scan: '$glob_dir/hostile.inc' sets 'kbdinteractiveauthentication yes'"
rm -rf "$glob_dir" "$SSHD_CONFIG_D/600-glob.conf"
assert_verify_passes_again 'h'

# --- i: a quoted Include path containing spaces -------------------------------
# sshd's tokenizer keeps a double-quoted token whole, so this path really is
# read. A scan that splits the Include arguments on whitespace looks for two
# directories that do not exist and reports the tree clean.

spaced_dir="$SSH_SANDBOX/dir with spaces"
mkdir -p "$spaced_dir"
printf 'Match Address 198.51.100.0/24\nPermitRootLogin yes\n' >"$spaced_dir/evil.inc"
printf 'Include "%s/evil.inc"\n' "$spaced_dir" >"$SSHD_CONFIG_D/600-quoted.conf"

assert_resolves 'i: quoted include path with spaces' \
  "$off_sample_spec" permitrootlogin yes
assert_verify_fails_naming 'i: quoted include path with spaces' \
  "match scan: '$spaced_dir/evil.inc' sets 'permitrootlogin yes'"
rm -rf "$spaced_dir" "$SSHD_CONFIG_D/600-quoted.conf"
assert_verify_passes_again 'i'

printf 'ssh-hardening-include-following: OK (includes followed outside the scanned set, Match state carried across the boundary and not leaked back, cycles and over-deep chains fail closed and terminate, relative/multiple/glob/quoted paths all resolved)\n'
