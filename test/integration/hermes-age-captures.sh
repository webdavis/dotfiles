#!/usr/bin/env bash
# hermes-age-captures: structural + round-trip validation of every committed hermes .age capture
# (the root ~/.hermes/config.yaml capture plus the four profile captures). Composes the committed
# ciphertext files, the age binary, and the host age identity referenced by the live chezmoi config.
#
# Two layers:
#   1. STRUCTURAL (always, incl. CI): each capture is a real age blob (armored or binary header),
#      nonzero, and carries no plaintext config markers. Never decrypts here.
#   2. ROUND-TRIP (only when BOTH the age binary and the referenced identity exist -- e.g. the
#      operator's machine): decrypt each capture and assert it yields nonzero plaintext. Plaintext is
#      piped straight into `wc -c`; it is NEVER written, printed, or otherwise emitted. When age or the
#      identity is absent (CI, a fresh machine, the de-homebrewed run) the round-trip layer SKIPS
#      cleanly -- it never fails on their absence.
set -uo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root" || exit 1
fail() {
  echo "hermes-age-captures: FAIL -- $1" >&2
  exit 1
}

# Collect every committed .age capture under dot_hermes. `find` (not a globstar glob) so the test does not
# depend on bash 4+ -- macOS /bin/bash is 3.2 and lacks globstar.
captures=()
while IFS= read -r -d '' f; do
  captures+=("$f")
done < <(find dot_hermes -type f -name '*.age' -print0)
[[ ${#captures[@]} -gt 0 ]] || fail "no dot_hermes/**/*.age captures found -- nothing to validate"

# Layer 1: structure only.
for f in "${captures[@]}"; do
  [[ -s $f ]] || fail "$f is empty (zero bytes)"
  head -3 "$f" | grep -qE 'AGE ENCRYPTED FILE|age-encryption\.org/v1' || fail "$f is not an age blob (plaintext leak risk)"
  grep -qE '(_config_version|deliver_only|basic_auth):' "$f" && fail "plaintext config markers found inside $f"
done
echo "hermes-age-captures: ${#captures[@]} captures pass structural validation"

# Layer 2: optional decrypt round-trip. Resolve the identity from the live chezmoi config (rail: use the
# identity the live [age] section references; never hardcode or copy it).
cfg="${CHEZMOI_CONFIG:-$HOME/.config/chezmoi/chezmoi.toml}"
identity=""
if [[ -f $cfg ]]; then
  identity="$(sed -n 's/^[[:space:]]*identity[[:space:]]*=[[:space:]]*"\(.*\)"[[:space:]]*$/\1/p' "$cfg" | head -1)"
fi

if ! command -v age >/dev/null 2>&1; then
  echo "hermes-age-captures: round-trip SKIPPED (age binary not on PATH)"
  echo "hermes-age-captures: OK (structural only)"
  exit 0
fi
if [[ -z $identity || ! -f $identity ]]; then
  echo "hermes-age-captures: round-trip SKIPPED (no age identity available)"
  echo "hermes-age-captures: OK (structural only)"
  exit 0
fi

for f in "${captures[@]}"; do
  # Decrypt into wc -c only; pipefail makes a decrypt failure fail the pipeline. Plaintext is never emitted.
  if bytes="$(age -d -i "$identity" <"$f" 2>/dev/null | wc -c | tr -d ' ')"; then
    [[ ${bytes:-0} -gt 0 ]] || fail "$f decrypted to zero bytes"
  else
    fail "$f did not decrypt with the referenced identity"
  fi
done
echo "hermes-age-captures: OK (${#captures[@]} captures decrypt to nonzero plaintext)"
