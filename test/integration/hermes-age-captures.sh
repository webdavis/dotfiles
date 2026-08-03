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

# ── Layer 3: the WEBHOOK ROUTE contract of the root config capture. Same gate as
# layer 2 (age + identity), because it needs the same plaintext. Every assertion
# is a yq PREDICATE evaluated inside the decrypted file: a route secret is never
# printed, never assigned to a shell variable, never placed on an argv, and never
# put in the environment (an audit hook records every Bash command line verbatim).
#
# What this pins, and why each matters:
#   - both routes exist. `relay` carries alerts to #relay/#priority;
#     `unattended-upgrades` carries the weekly RECORD to its own channel.
#   - they SHARE one secret. relay.sh signs with the single hermes_secret in
#     ~/.config/relay/auth.json and no other key, so a route with a different
#     secret answers 401 to every entry, and relay.sh's log path would report
#     `post FAILED HTTP 401` forever while the channel stayed empty.
#   - the shared secret is long. Without this, two EMPTY secrets would satisfy
#     the equality above -- the "stable field keeps its shape while losing its
#     contents" failure.
#   - deliver_only stays true. A route without it feeds the entry to an agent
#     instead of posting it verbatim. NOTE that this layer cannot enforce that in
#     CI: it needs the age identity, CI has none, and this file exits 0 printing
#     "structural only" there. Making it fail without a key would red every pull
#     request. The enforceable copy of this one assertion lives in
#     .chezmoiscripts/run_after_68-hermes-relay-route-status.sh.tmpl, which reads
#     the DECRYPTED config at apply time and is covered by
#     test/integration/hermes-route-status.sh; the guard and the risk then arrive
#     together. Keep both: this layer is what the operator's own machine checks
#     before anything is applied.
#   - the prompt spends every field relay.sh actually sends.
root_capture="dot_hermes/encrypted_private_config.yaml.age"
[[ -f $root_capture ]] || fail "missing root capture $root_capture"
plain_dir="$(mktemp -d)"
trap 'rm -rf "$plain_dir"' EXIT
chmod 700 "$plain_dir"
plain="$plain_dir/config.yaml"
(umask 077 && age -d -i "$identity" <"$root_capture" >"$plain") || fail "root capture did not decrypt"
[[ -s $plain ]] || fail "root capture decrypted to an empty file"

# Guard the guard: if the routes map itself did not parse, every predicate below
# would evaluate against null and quietly agree with everything.
route_count="$(yq -r '.platforms.webhook.extra.routes | length' "$plain" 2>/dev/null || echo "")"
[[ $route_count =~ ^[0-9]+$ ]] || fail "the decrypted config has no parseable webhook routes map"
[[ $route_count -ge 4 ]] || fail "expected at least 4 webhook routes, found $route_count"

assert_route_predicate() { # <yq-boolean-expression> <failure message>
  local expression="$1" message="$2" verdict
  verdict="$(yq -r "$expression" "$plain" 2>/dev/null || echo "ERROR")"
  [[ $verdict == "true" ]] || fail "$message (yq answered: $verdict)"
}

assert_route_predicate '.platforms.webhook.extra.routes | has("relay")' \
  "the alert route 'relay' is missing from the committed config"
assert_route_predicate '.platforms.webhook.extra.routes | has("unattended-upgrades")' \
  "the log route 'unattended-upgrades' is missing from the committed config"
assert_route_predicate \
  '.platforms.webhook.extra.routes."unattended-upgrades".secret == .platforms.webhook.extra.routes.relay.secret' \
  "the log route does not share the relay route's secret; relay.sh signs with one key, so every entry would 401"
assert_route_predicate \
  '(.platforms.webhook.extra.routes."unattended-upgrades".secret | length) >= 32' \
  "the log route's secret is shorter than 32 characters (two empty secrets would satisfy the equality above)"
assert_route_predicate '.platforms.webhook.extra.routes."unattended-upgrades".deliver == "discord"' \
  "the log route does not deliver to discord"
assert_route_predicate '.platforms.webhook.extra.routes."unattended-upgrades".deliver_only == true' \
  "the log route is not deliver_only; the entry would be fed to an agent instead of posted verbatim"
assert_route_predicate \
  '.platforms.webhook.extra.routes."unattended-upgrades".deliver_extra.chat_id == "15333878568" + "21747803"' \
  "the log route points at the wrong Discord channel"
assert_route_predicate \
  '.platforms.webhook.extra.routes."unattended-upgrades".deliver_extra.chat_id != .platforms.webhook.extra.routes.relay.deliver_extra.chat_id' \
  "the log route posts into the ALERT channel; the whole point is a separate channel"
# relay.sh posts exactly {agent, state, project, detail}; a template that drops
# one silently truncates every entry.
for field in agent state project detail; do
  assert_route_predicate \
    ".platforms.webhook.extra.routes.\"unattended-upgrades\".prompt | test(\"[{]${field}[}]\")" \
    "the log route's prompt template never spends the {$field} field relay.sh sends"
done
rm -f "$plain"
echo "hermes-age-captures: OK (relay + unattended-upgrades routes share one secret; log route is deliver_only discord on its own channel)"
