#!/bin/bash
# ssh-hardening.sh -- generate, install, and verify a public-key-only sshd
# drop-in. Everything here is inert for the RUNNING daemon: sshd re-reads its
# configuration only on restart, so writing the drop-in changes nothing until
# Remote Login (re)starts sshd. The reload is a deliberately separate,
# disruptive step and is NOT provided by this script.
#
# Modes:
#   --print-config  print the drop-in content (pure: no privilege, no writes)
#   --print-path    print the drop-in target path (pure)
#   --verify        read-only three-way check that the EFFECTIVE sshd
#                   configuration is fully hardened (see the verify section)
#   (no argument)   install: write the drop-in, pin mode 0644, migrate the
#                   legacy 50-no-password-auth.conf away, then run the verify
#                   and refuse to claim success unless it passes
#
# The drop-in file IS the lock; leave it in place permanently. Without it,
# sshd reverts to its defaults at the next restart.
#
# Seams (environment; defaults are the live values):
#   SSHD_CONFIG_D       drop-in directory      (default /etc/ssh/sshd_config.d)
#   SSHD_MAIN_CONFIG    main sshd config       (default /etc/ssh/sshd_config)
#   SSHD_BIN            sshd binary, ABSOLUTE  (default /usr/sbin/sshd) so a
#                       stripped PATH cannot turn the verifier into a no-op
#   SSH_HARDENING_SUDO  privilege wrapper for writes; set EMPTY to run
#                       unprivileged against a sandbox tree (default sudo)
#   SSH_HARDENING_ALLOW_MISSING_SSHD
#                       explicit test seam: when set AND $SSHD_BIN cannot run,
#                       --verify skips (exit 0) WITHOUT a verified claim.
#                       Never set in the default path; absent it, an
#                       unrunnable verifier fails closed.
set -euo pipefail

SSHD_CONFIG_D="${SSHD_CONFIG_D:-/etc/ssh/sshd_config.d}"
SSHD_MAIN_CONFIG="${SSHD_MAIN_CONFIG:-/etc/ssh/sshd_config}"
SSHD_BIN="${SSHD_BIN:-/usr/sbin/sshd}"
# `-` not `:-`: set-but-empty means "no wrapper, run the commands directly",
# which is how tests write into a user-owned sandbox without privilege.
SSH_HARDENING_SUDO="${SSH_HARDENING_SUDO-sudo}"

DROPIN_NAME="000-ssh-hardening.conf"
LEGACY_DROPIN_NAME="50-no-password-auth.conf"

# The five protected directives and their required values, lowercase exactly
# as `sshd -G` prints them. Parallel arrays because the deployed interpreter
# is the system bash 3.2, which has no associative arrays; every test drives
# this script through /bin/bash so a newer-bash-ism fails there.
PROTECTED_KEYS=(passwordauthentication kbdinteractiveauthentication usepam
  pubkeyauthentication permitrootlogin)
PROTECTED_VALUES=(no no yes yes no)

die() {
  printf '[ssh-hardening] ERROR: %s\n' "$*" >&2
  exit 1
}

run_privileged() {
  if [[ -n $SSH_HARDENING_SUDO ]]; then
    "$SSH_HARDENING_SUDO" "$@"
  else
    "$@"
  fi
}

dropin_path() {
  printf '%s/%s\n' "$SSHD_CONFIG_D" "$DROPIN_NAME"
}

print_config() {
  cat <<'EOF'
# 000-ssh-hardening.conf - public-key-only sshd policy, written by
# ssh-hardening.sh. This file IS the lock: remove it and sshd reverts to its
# defaults at the next restart.
#
# The 000- prefix sorts (LC_ALL=C) before Apple's 100-macos.conf. sshd's
# Include is lexical and first-value-wins, so sorting first keeps these values
# authoritative even if a future macOS release adds a competing directive to
# 100-macos.conf. Today's Apple file sets none of these, so the prefix is
# insurance, not the repair of a live conflict.
#
# PasswordAuthentication and KbdInteractiveAuthentication together close BOTH
# interactive password channels; either alone leaves one open. UsePAM yes is
# required on macOS for account and session management, and is safe here
# precisely because no password path remains for PAM to authenticate.
# PermitRootLogin no is strictly tighter than the without-password default,
# which still allows root login BY KEY.
PasswordAuthentication no
KbdInteractiveAuthentication no
UsePAM yes
PubkeyAuthentication yes
PermitRootLogin no
EOF
}

# --- verify ------------------------------------------------------------------
# Three independent, read-only, host-key-free checks. All of them run and
# EVERY failure is reported, so one broken layer cannot mask another:
#
#   1. check_global: the pre-Match effective configuration via `sshd -G`.
#      Catches a global re-enable anywhere in the include chain, including a
#      sibling that sorts before the drop-in (first-value-wins). It cannot
#      see Match blocks at all, which is why the next two exist.
#   2. check_match_scan: a raw scan of the main config and every drop-in for
#      a Match block that re-enables a protected directive. The completeness
#      net: it flags ANY Match-scoped re-enable, including forms on subnets
#      or users the connection samples below never probe.
#   3. check_connection_specs: per-connection resolution via
#      `sshd -G -T -C`. Proves Match blocks RESOLVE hardened for concrete
#      connections (root and the invoking user); samples only, by nature.
#
# Any check that cannot run FAILS the verify. A skip exists only behind the
# SSH_HARDENING_ALLOW_MISSING_SSHD test seam and never claims verified.

VERIFY_FAILURES=()
VERIFY_SKIPPED=0

add_failure() {
  VERIFY_FAILURES+=("$1")
}

# required_value <lowercase-key>: print the required value, or return 1 for a
# key that is not protected.
required_value() {
  local i
  for i in "${!PROTECTED_KEYS[@]}"; do
    if [[ $1 == "${PROTECTED_KEYS[$i]}" ]]; then
      printf '%s\n' "${PROTECTED_VALUES[$i]}"
      return 0
    fi
  done
  return 1
}

# assert_output_hardened <check-label> <sshd -G output>: every protected
# directive must be present with its required value. Three outcomes per key,
# each named: correct, wrong value, absent. All five are asserted
# individually; completeness beats counting.
assert_output_hardened() {
  local label="$1" output="$2" i key want got
  for i in "${!PROTECTED_KEYS[@]}"; do
    key="${PROTECTED_KEYS[$i]}"
    want="${PROTECTED_VALUES[$i]}"
    got="$(printf '%s\n' "$output" | awk -v k="$key" '$1 == k { print $2; exit }')"
    if [[ -z $got ]]; then
      add_failure "$label: '$key' is absent from the effective configuration"
    elif [[ $got != "$want" ]]; then
      add_failure "$label: '$key' is '$got', want '$want'"
    fi
  done
}

check_global() {
  local output status=0
  # Capture first, inspect after: the exit status of a piped sshd would be
  # lost to the pipeline's last element.
  output="$("$SSHD_BIN" -G -f "$SSHD_MAIN_CONFIG" 2>&1)" || status=$?
  if [[ $status -ne 0 ]]; then
    add_failure "global check: '$SSHD_BIN -G' exited $status; failing closed rather than assuming the tree is safe (output: $output)"
    return 0
  fi
  assert_output_hardened 'global check' "$output"
}

# scan_file_for_match_reenable <file>: flag every protected directive set to a
# non-required value inside a Match block. Normalization mirrors sshd's parser
# (all verified against OpenSSH 10.0p2): keywords are case-insensitive, one
# '=' may replace the separating whitespace, arguments may be double-quoted,
# and the deprecated ChallengeResponseAuthentication alias still flips
# kbdinteractiveauthentication. A Match block does NOT extend into the next
# included file (verified empirically), so the in-Match state is per file.
scan_file_for_match_reenable() {
  local file="$1" in_match=0 key value _ want content status=0
  content="$(tr '=\t' '  ' <"$file" | tr '[:upper:]' '[:lower:]')" || status=$?
  if [[ $status -ne 0 ]]; then
    add_failure "match scan: cannot read '$file'; failing closed rather than treating it as clean"
    return 0
  fi
  while read -r key value _; do
    [[ -n $key ]] || continue
    case $key in '#'*) continue ;; esac
    if [[ $key == match ]]; then
      in_match=1
      continue
    fi
    [[ $in_match -eq 1 ]] || continue
    value="${value#\"}"
    value="${value%\"}"
    if [[ $key == challengeresponseauthentication ]]; then
      key=kbdinteractiveauthentication
    fi
    if want="$(required_value "$key")" && [[ $value != "$want" ]]; then
      add_failure "match scan: '$file' sets '$key $value' inside a Match block (want '$want'); a Match-scoped re-enable bypasses the global check"
    fi
  done <<<"$content"
}

check_match_scan() {
  local file
  # The include order is lexical byte order; LC_ALL=C sort mirrors it. A
  # config file name containing a newline would break this listing; sshd's
  # own glob handling shares the no-newline assumption.
  while IFS= read -r file; do
    [[ -n $file && -f $file ]] || continue
    scan_file_for_match_reenable "$file"
  done < <(printf '%s\n' "$SSHD_MAIN_CONFIG" "$SSHD_CONFIG_D"/* | LC_ALL=C sort -u)
}

check_connection_specs() {
  local invoking_user spec output status
  invoking_user="$(id -un)"
  # Two samples: root (the account PermitRootLogin exists to keep out) and
  # the invoking user (so a 'Match User <name>' aimed at the operator's own
  # account fails RESOLUTION, not only the raw scan). Samples cannot be
  # exhaustive; check_match_scan is the completeness net behind them.
  for spec in \
    'user=root,host=localhost,addr=127.0.0.1' \
    "user=$invoking_user,host=localhost,addr=127.0.0.1"; do
    status=0
    # -G -T -C: on OpenSSH 10.0p2, -C without -T is rejected and -T alone
    # demands host keys; the three together resolve Match blocks for the spec
    # with no privilege and no host keys (verified empirically).
    output="$("$SSHD_BIN" -G -T -C "$spec" -f "$SSHD_MAIN_CONFIG" 2>&1)" || status=$?
    if [[ $status -ne 0 ]]; then
      add_failure "connection check ($spec): '$SSHD_BIN -G -T -C' exited $status; failing closed (output: $output)"
      continue
    fi
    assert_output_hardened "connection check ($spec)" "$output"
  done
}

verify() {
  VERIFY_FAILURES=()
  VERIFY_SKIPPED=0
  if [[ ! -x $SSHD_BIN ]]; then
    if [[ -n ${SSH_HARDENING_ALLOW_MISSING_SSHD:-} ]]; then
      VERIFY_SKIPPED=1
      printf '[ssh-hardening] verify SKIPPED: %s is not executable and the SSH_HARDENING_ALLOW_MISSING_SSHD test seam is set. The configuration was NOT checked.\n' "$SSHD_BIN"
      return 0
    fi
    printf '[ssh-hardening] verify: FAILING CLOSED: %s is not executable, so the effective configuration cannot be checked. Refusing to guess.\n' "$SSHD_BIN" >&2
    return 1
  fi
  check_global
  check_match_scan
  check_connection_specs
  if [[ ${#VERIFY_FAILURES[@]} -gt 0 ]]; then
    printf '[ssh-hardening] verify FAILED, %d problem(s):\n' "${#VERIFY_FAILURES[@]}" >&2
    printf '  - %s\n' "${VERIFY_FAILURES[@]}" >&2
    return 1
  fi
  printf '[ssh-hardening] verify: PASS: all five directives hold globally, no Match block re-enables any of them, and both sampled connections resolve hardened.\n'
}

# --- install -----------------------------------------------------------------

install_dropin() {
  local target legacy
  target="$(dropin_path)"
  legacy="$SSHD_CONFIG_D/$LEGACY_DROPIN_NAME"
  [[ -d $SSHD_CONFIG_D ]] ||
    die "drop-in directory '$SSHD_CONFIG_D' does not exist"
  if ! print_config | run_privileged tee -- "$target" >/dev/null; then
    die "could not write '$target'"
  fi
  # Explicit mode, never the ambient umask: under e.g. umask 0077 tee lands
  # the file 0600, and a root-owned 0600 drop-in makes UNPRIVILEGED `sshd -G`
  # fail outright, so the whole verification would need elevation. 0644 is
  # safe: the file holds no credential and sshd must be able to read it.
  # `--` BEFORE the mode: BSD chmod treats a `--` after the mode as a file
  # operand and fails.
  if ! run_privileged chmod -- 0644 "$target"; then
    die "could not set mode 0644 on '$target'"
  fi
  printf '[ssh-hardening] wrote %s (mode 0644)\n' "$target"
  # Migrate the legacy drop-in away. Two reasons: one lock in one file (two
  # files declaring the same policy is drift waiting to happen), and the
  # legacy file was created 0600 under the umask, which breaks unprivileged
  # verification for the entire tree (see the chmod comment above).
  if [[ -e $legacy || -L $legacy ]]; then
    if ! run_privileged rm -f -- "$legacy"; then
      die "could not remove the legacy drop-in '$legacy'"
    fi
    printf '[ssh-hardening] removed legacy drop-in %s\n' "$legacy"
  fi
  if ! verify; then
    die "wrote '$target' but the effective configuration did NOT verify as fully hardened; refusing to claim success"
  fi
  if [[ $VERIFY_SKIPPED -eq 1 ]]; then
    printf '[ssh-hardening] wrote %s, but verification was SKIPPED via the test seam; the effective configuration is NOT verified.\n' "$target"
    return 0
  fi
  printf '[ssh-hardening] install complete: %s is in place and the effective configuration verified fully hardened.\n' "$target"
}

usage() {
  cat <<'EOF'
usage: ssh-hardening.sh [--print-config | --print-path | --verify]

  --print-config  print the generated drop-in content and exit
  --print-path    print the drop-in target path and exit
  --verify        read-only check that the effective sshd configuration is
                  fully hardened; never writes, never escalates
  (no argument)   install the drop-in and verify

Reloading a running sshd is deliberately not provided here; the drop-in
takes effect when sshd next starts.
EOF
}

main() {
  if [[ $# -gt 1 ]]; then
    usage >&2
    exit 2
  fi
  case "${1-}" in
    --print-config) print_config ;;
    --print-path) dropin_path ;;
    --verify) verify ;;
    '') install_dropin ;;
    --help | -h) usage ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
}

main "$@"
