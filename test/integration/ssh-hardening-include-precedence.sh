#!/usr/bin/env bash
# ssh-hardening-include-precedence.sh -- prove the managed sshd drop-in WINS sshd's
# first-value-wins Include precedence, and that install migrates the superseded
# name away (R1-1). The audit found the old drop-in was `50-no-password-auth.conf`,
# which sorts lexically AFTER Apple's `100-macos.conf` under `Include
# sshd_config.d/*` ("1" < "5"), so a hostile or updated 100- file would SHADOW the
# hardening (password auth back on, root login back on). The fix renames it to
# `000-ssh-hardening.conf` (sorts before any Apple 0*/1* file) and, on install,
# removes a pre-existing 50- file.
#
# This reconstructs an Include tree in a SANDBOX (it NEVER touches live /etc/ssh and
# NEVER reloads sshd) with a deliberately HOSTILE 100-macos.conf that reopens every
# hole, then:
#   1. runs the installer against the sandbox (the SSHD_CONFIG_D + SSH_HARDENING_SUDO
#      seams) and asserts it writes 000-, REMOVES the seeded 50-, and reports the
#      effective config fully hardened (exit 0);
#   2. independently confirms via `sshd -G` that the 000- name wins all five values
#      while the 50- name is DEFEATED -- the regression the rename closes;
#   3. asserts `--verify` FAILS loudly on a tree where the hardening is shadowed --
#      the defense-in-depth assertion actually catches a bad config, and PASSES once
#      the winning 000- drop-in is restored.
#
# A failing `sudo` stub on PATH guarantees the UNFIXED script (bare `sudo tee
# /etc/ssh/...`) cannot touch live /etc/ssh: it fails the write instead (a clean
# RED). SKIPs cleanly where sshd is absent (de-homebrewed / Linux CI).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_ssh-hardening.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -f $SCRIPT ]] || fail "missing script: $SCRIPT"
command -v sshd >/dev/null 2>&1 || {
  printf 'SKIP: sshd not on PATH; cannot exercise Include precedence with the real parser\n'
  exit 0
}
if ! sshd -G -f /dev/null >/dev/null 2>&1; then
  printf 'SKIP: sshd -G is not usable in this environment (older OpenSSH or sandbox)\n'
  exit 0
fi

# Canonicalize the /var -> /private/var symlink so absolute Include paths match.
work="$(cd "$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$work"' EXIT

confd="$work/sshd_config.d"
mkdir -p "$confd"
main="$work/sshd_config"
printf 'Include %s/*\n' "$confd" >"$main"

# HOSTILE Apple file: reopens every closed path. Sorts as 100- (before 50-, after
# 000-). If the hardening does not sort FIRST, these values win.
cat >"$confd/100-macos.conf" <<'EOF'
PasswordAuthentication yes
KbdInteractiveAuthentication yes
UsePAM yes
PubkeyAuthentication no
PermitRootLogin yes
EOF

# A pre-existing LEGACY 50- drop-in the migration must remove.
cat >"$confd/50-no-password-auth.conf" <<'EOF'
# Superseded managed drop-in (must be migrated away).
PasswordAuthentication no
EOF

# A failing `sudo` stub: blocks the UNFIXED script from ever touching live /etc/ssh
# (its bare `sudo tee /etc/ssh/...` fails here) and is never called by the fixed
# script (SSH_HARDENING_SUDO="" drops the prefix entirely).
stub="$work/bin"
mkdir -p "$stub"
cat >"$stub/sudo" <<'EOF'
#!/bin/bash
printf 'STUB sudo invoked (blocked to protect live /etc/ssh): %s\n' "$*" >&2
exit 1
EOF
chmod +x "$stub/sudo"

# ---- 1. install against the sandbox: write 000-, migrate 50-, verify hardened ---
out="$work/out"
err="$work/err"
rc=0
SSH_HARDENING_SUDO="" SSHD_CONFIG_D="$confd" SSHD_MAIN_CONFIG="$main" \
  PATH="$stub:$PATH" bash "$SCRIPT" >"$out" 2>"$err" || rc=$?

[[ $rc -eq 0 ]] ||
  fail "installer must exit 0 on a tree where the drop-in wins (got rc=$rc; err: $(cat "$err"))"
[[ -f "$confd/000-ssh-hardening.conf" ]] ||
  fail "installer did not write the 000- managed drop-in"
grep -qxF 'PasswordAuthentication no' "$confd/000-ssh-hardening.conf" ||
  fail "000- drop-in is missing the hardening content"
[[ ! -e "$confd/50-no-password-auth.conf" ]] ||
  fail "installer did NOT migrate away the superseded 50- drop-in (R1-1 migration)"
grep -qi 'verified' "$out" ||
  fail "installer must report the effective config verified when the drop-in wins (out: $(cat "$out"))"

# ---- 2. independent proof: 000- wins all five effective values ------------------
eff="$(sshd -G -f "$main" 2>/dev/null)" || fail "sshd -G failed on the reconstructed tree"
declare -a want=(
  'passwordauthentication no'
  'kbdinteractiveauthentication no'
  'usepam yes'
  'pubkeyauthentication yes'
  'permitrootlogin no'
)
for pair in "${want[@]}"; do
  grep -qxiF "$pair" <<<"$eff" ||
    fail "000- did not win '$pair' (effective: $(grep -iE '^(passwordauthentication|kbdinteractiveauthentication|usepam|pubkeyauthentication|permitrootlogin) ' <<<"$eff" | tr '\n' ';'))"
done

# ---- 3a. regression guard: the OLD 50- name is DEFEATED by the hostile 100- ------
# (documents WHY the rename is required: hostile 100- sorts before 50-.)
rm -f "$confd/000-ssh-hardening.conf"
bash "$SCRIPT" --print-config >"$confd/50-no-password-auth.conf"
eff_legacy="$(sshd -G -f "$main" 2>/dev/null)" || fail "sshd -G failed on the legacy-name tree"
grep -qxiF 'passwordauthentication yes' <<<"$eff_legacy" ||
  fail "regression guard: the 50- name should be DEFEATED (hostile 100- wins passwordauthentication) -- this is the shadowing the rename closes"

# ---- 3b. --verify FAILS loudly on the shadowed tree -----------------------------
vrc=0
SSH_HARDENING_SUDO="" SSHD_MAIN_CONFIG="$main" bash "$SCRIPT" --verify \
  >"$work/vout" 2>"$work/verr" || vrc=$?
[[ $vrc -ne 0 ]] ||
  fail "--verify must FAIL (nonzero) when the hardening is shadowed (defense in depth)"
grep -qi 'WARNING' "$work/verr" ||
  fail "--verify must warn loudly on a shadowed tree (stderr: $(cat "$work/verr"))"

# ---- 3c. --verify PASSES once the winning 000- drop-in is restored ---------------
bash "$SCRIPT" --print-config >"$confd/000-ssh-hardening.conf"
vrc=0
SSH_HARDENING_SUDO="" SSHD_MAIN_CONFIG="$main" bash "$SCRIPT" --verify \
  >"$work/vout2" 2>"$work/verr2" || vrc=$?
[[ $vrc -eq 0 ]] ||
  fail "--verify must PASS when 000- wins (got rc=$vrc; err: $(cat "$work/verr2"))"

printf 'ssh-hardening-include-precedence: OK (000- wins; 50- migrated + defeated; --verify catches shadowing)\n'
