#!/usr/bin/env bash
# age-tripwire-pq: the source-tree private-key tripwire must catch post-quantum identities too.
# Installed age (1.3.1) emits classic `AGE-SECRET-KEY-1...` AND, with `age-keygen -pq`, post-quantum
# hybrid `AGE-SECRET-KEY-PQ-1...` identities. The tripwire in test/unit/hermes-config-encrypted.sh must
# trip on BOTH, or a leaked PQ key would slip past the guard.
#
# Secret-safe: the probe identities are assembled at RUN TIME from a non-key tail, split across adjacent
# string literals, so this file never contains a contiguous key-shaped byte sequence for another scanner
# to flag. No real key is ever generated or written.
set -uo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root" || exit 1
fail() {
  echo "age-tripwire-pq: FAIL -- $1" >&2
  exit 1
}

# A 40+ char uppercase-alnum tail starting with 1 (the shape a bech32 age secret tail has). Not a real key.
tail_body='1ABCDEFGHIJKLMNOPQRSTUVWXYZ234567ABCDEFGHIJKLMNOPQR'
classic_probe="AGE-SECRET-KEY-""$tail_body"
pq_probe="AGE-SECRET-KEY-""PQ-$tail_body"

# The pre-fix pattern (classic only) and the required extended pattern (optional PQ- segment). Each is
# split across adjacent single-quoted strings so the pattern text itself is not a scannable key.
old_pattern='AGE-SECRET-KEY-''1[A-Z0-9]{40,}'
new_pattern='AGE-SECRET-KEY-''(PQ-)?1[A-Z0-9]{40,}'

# The extended pattern must catch both identity forms.
grep -qE "$new_pattern" <<<"$pq_probe" || fail "extended pattern does not match a post-quantum identity"
grep -qE "$new_pattern" <<<"$classic_probe" || fail "extended pattern does not match a classic identity"

# The old pattern must MISS the PQ form (that is exactly the gap being closed) and still catch the classic.
grep -qE "$old_pattern" <<<"$pq_probe" && fail "old pattern already matched PQ -- the fix would be a no-op"
grep -qE "$old_pattern" <<<"$classic_probe" || fail "old pattern should still match a classic identity"

# Tie the assertion to the real tripwire: the guard script must carry the extended (PQ-) form.
guard="test/unit/hermes-config-encrypted.sh"
grep -qF '(PQ-)?1[A-Z0-9]{40,}' "$guard" || fail "$guard tripwire is not extended for the PQ- segment"

echo "age-tripwire-pq: OK (classic + post-quantum identities both trip the extended tripwire)"
