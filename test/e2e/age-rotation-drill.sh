#!/usr/bin/env bash
# age-rotation-drill: disaster-recovery rehearsal for the age key rotation documented in
# docs/runbooks/age-key.md. Proves the rotation OUTCOME end-to-end in a fully throwaway sandbox:
# a fresh keypair encrypts a dummy secret, a SECOND fresh keypair re-encrypts it, and afterwards the
# ciphertext decrypts ONLY with the new key while the plaintext survives byte-for-byte.
#
# It rehearses the cryptographic effect of the runbook's `chezmoi re-add --re-encrypt` step using the
# age primitives directly (decrypt-with-old | encrypt-to-new), so the drill is independent of the local
# chezmoi version (nix ships 2.62.3, which predates the --re-encrypt flag) and touches no chezmoi source
# tree, no live key, and no real secret. The operator runs the actual `chezmoi re-add --re-encrypt`
# command per the runbook; this test guards the invariant that command must uphold.
#
# CI-safe: needs only the age binary + age-keygen (no host identity). Skips cleanly when either is absent.
set -uo pipefail

fail() {
  echo "age-rotation-drill: FAIL -- $1" >&2
  exit 1
}

command -v age >/dev/null 2>&1 || {
  echo "age-rotation-drill: skipped (no age binary)"
  exit 0
}
command -v age-keygen >/dev/null 2>&1 || {
  echo "age-rotation-drill: skipped (no age-keygen)"
  exit 0
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
umask 077

# Throwaway identities. age-keygen writes the identity (incl. the AGE-SECRET-KEY line) to the file and
# records the public recipient on its "# public key:" comment line (age-keygen prints it to stderr, so
# read it back from the file rather than from stdout).
age-keygen -o "$tmp/old.key" >/dev/null 2>&1 || fail "age-keygen (old) failed"
age-keygen -o "$tmp/new.key" >/dev/null 2>&1 || fail "age-keygen (new) failed"
recip_old="$(sed -n 's/^# public key: //p' "$tmp/old.key")"
recip_new="$(sed -n 's/^# public key: //p' "$tmp/new.key")"
[[ -n $recip_old && -n $recip_new ]] || fail "age-keygen did not yield recipients"
[[ $recip_old != "$recip_new" ]] || fail "two keygen runs produced the same recipient"

# Dummy secret (never a real secret; a random token is enough to prove round-trip integrity).
printf 'dr-drill-secret-%s\n' "$RANDOM$RANDOM" >"$tmp/secret.txt"
want="$(sha256sum "$tmp/secret.txt" | awk '{print $1}')"

# 1. Encrypt with the OLD recipient (models the initial `chezmoi add --encrypt`).
age -R <(printf '%s\n' "$recip_old") -o "$tmp/blob.age" "$tmp/secret.txt" 2>/dev/null || fail "initial encrypt failed"
head -1 "$tmp/blob.age" | grep -qE 'AGE ENCRYPTED FILE|age-encryption\.org/v1' || fail "blob is not an age file"

# Old key decrypts; new key must NOT (yet).
got_old="$(age -d -i "$tmp/old.key" <"$tmp/blob.age" 2>/dev/null | sha256sum | awk '{print $1}')"
[[ $got_old == "$want" ]] || fail "old key did not round-trip the secret before rotation"
age -d -i "$tmp/new.key" <"$tmp/blob.age" >/dev/null 2>&1 && fail "new key decrypted the pre-rotation blob (should not)"

# 2. ROTATE: re-encrypt to the NEW recipient. This is the cryptographic effect of
#    `chezmoi re-add --re-encrypt` after swapping the [age] recipient/identity in the config.
age -d -i "$tmp/old.key" <"$tmp/blob.age" 2>/dev/null | age -R <(printf '%s\n' "$recip_new") -o "$tmp/blob.rotated.age" 2>/dev/null || fail "re-encrypt failed"
head -1 "$tmp/blob.rotated.age" | grep -qE 'AGE ENCRYPTED FILE|age-encryption\.org/v1' || fail "rotated blob is not an age file"

# 3. After rotation: NEW key decrypts to the original plaintext; OLD key is locked out.
got_new="$(age -d -i "$tmp/new.key" <"$tmp/blob.rotated.age" 2>/dev/null | sha256sum | awk '{print $1}')"
[[ $got_new == "$want" ]] || fail "new key did not round-trip the secret after rotation"
age -d -i "$tmp/old.key" <"$tmp/blob.rotated.age" >/dev/null 2>&1 && fail "old key still decrypted after rotation (rotation ineffective)"

echo "age-rotation-drill: OK (encrypt -> rotate -> only new key decrypts; plaintext preserved)"
