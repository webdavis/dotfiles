#!/usr/bin/env bash
# ssh-hardening-match-reenable.sh -- `--verify` must catch a criteria-based Match
# block that re-enables a protected directive (R2-1). The audit found the old
# `--verify` parsed only `sshd -G` (WITHOUT -C), which dumps the global/pre-Match
# config, so a hostile sibling drop-in with `Match Address 0.0.0.0/0` (or `Match
# User *`, `Match all`, or a specific `Match User <name>`) re-enabling
# PasswordAuthentication or PermitRootLogin PASSED --verify ("all five in force")
# while `sshd -G -T -C user=root,...` showed password auth + root login back ON --
# the most idiomatic sshd bypass.
#
# The fix makes --verify assert three ways, all read-only and host-key-free:
#   1. the global (pre-Match) effective config via `sshd -G`;
#   2. a RAW scan of every included file for a Match block that weakens a protected
#      directive (PasswordAuthentication / KbdInteractiveAuthentication /
#      PubkeyAuthentication / PermitRootLogin) -- this names the offending file and
#      catches even a specific-user Match the connection-spec sampling would miss;
#   3. an authoritative per-connection resolution via `sshd -G -T -C` for a root
#      spec and a normal-user spec.
#
# This reconstructs an Include tree in a SANDBOX (NEVER touches live /etc/ssh, NEVER
# reloads sshd), drops a hostile sibling per case, and asserts --verify FAILS loudly
# (rc=1, a warning naming the file) on every re-enable while a clean tree PASSES.
# SKIPs cleanly where sshd (or the `sshd -G -T -C` seam) is absent.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_ssh-hardening.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -f $SCRIPT ]] || fail "missing script: $SCRIPT"
SSHD="$(command -v sshd 2>/dev/null || true)"
[[ -n $SSHD ]] || {
  printf 'SKIP: sshd not on PATH; cannot exercise Match resolution with the real parser\n'
  exit 0
}
if ! "$SSHD" -G -f /dev/null >/dev/null 2>&1; then
  printf 'SKIP: sshd -G is not usable in this environment (older OpenSSH or sandbox)\n'
  exit 0
fi
if ! "$SSHD" -G -T -C user=root,addr=1.2.3.4,host=h -f /dev/null >/dev/null 2>&1; then
  printf 'SKIP: sshd -G -T -C (host-key-free Match resolution) is unavailable here\n'
  exit 0
fi

work="$(cd "$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$work"' EXIT

confd="$work/sshd_config.d"
mkdir -p "$confd"
main="$work/sshd_config"
printf 'Include %s/*\n' "$confd" >"$main"

# The winning managed drop-in (globally hardened; sorts first).
bash "$SCRIPT" --print-config >"$confd/000-ssh-hardening.conf"

failures=0
report() {
  if [[ $1 == ok ]]; then printf '  ok   %s\n' "$2"; else
    printf '  FAIL %s\n' "$2"
    failures=$((failures + 1))
  fi
}

# verify_run -> populates RC, ERR by running --verify against the sandbox tree.
verify_run() {
  RC=0
  SSH_HARDENING_SUDO="" SSHD_BIN="$SSHD" SSHD_MAIN_CONFIG="$main" \
    bash "$SCRIPT" --verify >"$work/vout" 2>"$work/verr" || RC=$?
  ERR="$(cat "$work/verr")"
}

# hostile_case <name> <sibling-basename> <sibling-content> [name_file] -- drop a
# hostile sibling, assert --verify FAILS (rc!=0) and warns, then remove it. When
# name_file is "yes" (the default), also assert the warning names the sibling file
# (the Match-scan does this); a GLOBAL re-enable is caught by the pre-Match `sshd -G`
# check, which fails+warns but does not attribute a file, so name_file="no" there.
hostile_case() {
  local name="$1" fname="$2" content="$3" name_file="${4:-yes}"
  printf '%s' "$content" >"$confd/$fname"
  verify_run
  if [[ $RC -ne 0 ]]; then report ok "$name: --verify FAILS (rc=$RC)"; else
    report bad "$name: --verify must FAIL on the re-enable (got rc=0; err: $ERR)"
  fi
  if grep -qi 'WARNING' <<<"$ERR"; then report ok "$name: warns loudly"; else
    report bad "$name: must warn on stderr (err: $ERR)"
  fi
  if [[ $name_file == yes ]]; then
    if grep -qF "$fname" <<<"$ERR"; then report ok "$name: names the offending file"; else
      report bad "$name: warning must name '$fname' (err: $ERR)"
    fi
  fi
  rm -f "$confd/$fname"
}

printf 'ssh-hardening --verify Match-re-enable cases:\n'

# 1. clean tree (only the winning 000-) -> --verify PASSES.
verify_run
if [[ $RC -eq 0 ]]; then report ok "clean: --verify PASSES (rc=0)"; else
  report bad "clean: --verify must PASS on a hardened tree (rc=$RC; err: $ERR)"
fi
if grep -qi 'verified' "$work/vout"; then report ok "clean: reports verified"; else
  report bad "clean: must report verified (out: $(cat "$work/vout"))"
fi

# 2. Match Address 0.0.0.0/0 re-enabling password auth (the idiomatic bypass).
hostile_case match-address 900-hostile-addr.conf \
  $'Match Address 0.0.0.0/0\n    PasswordAuthentication yes\n'

# 3. Match User * re-enabling root login.
hostile_case match-user-star 900-hostile-userstar.conf \
  $'Match User *\n    PermitRootLogin yes\n'

# 4. Match all re-enabling keyboard-interactive (PAM password) auth.
hostile_case match-all 900-hostile-all.conf \
  $'Match all\n    KbdInteractiveAuthentication yes\n'

# 5. A SPECIFIC-user Match the connection-spec sampling (root + a normal user) does
#    NOT cover -- only the raw file scan catches it. Proves the scan is not merely a
#    duplicate of the -C sampling.
hostile_case match-specific-user 900-hostile-backdoor.conf \
  $'Match User zzbackdoor\n    PasswordAuthentication yes\n    PubkeyAuthentication no\n'

# 6. Regression: a GLOBAL (top-level) re-enable in a sibling that sorts BEFORE 000-
#    still fails via the pre-Match global check (first-value-wins shadowing, R1-1).
#    The global check attributes no file, so do not assert file-naming here.
hostile_case global-shadow 000-aaa-hostile.conf \
  $'PasswordAuthentication yes\nPermitRootLogin yes\n' no

# 7. After all hostiles are removed, the clean tree PASSES again (no leakage).
verify_run
if [[ $RC -eq 0 ]]; then report ok "clean-again: --verify PASSES after cleanup"; else
  report bad "clean-again: --verify must PASS once hostiles are removed (rc=$RC; err: $ERR)"
fi

if [[ $failures -gt 0 ]]; then
  printf 'ssh-hardening-match-reenable: %d assertion(s) FAILED\n' "$failures" >&2
  exit 1
fi
printf 'ssh-hardening-match-reenable: OK (Match re-enables caught + named; global shadow caught; clean passes)\n'
