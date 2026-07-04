#!/usr/bin/env bash
# hermes-config-encrypted: source-tree secret-leak guard for the age-encrypted ~/.hermes/config.yaml.
# Runs everywhere (incl. CI) -- inspects only the repo source, never decrypts, never touches ~/.hermes.
# FAILS (does not skip) until the encrypted config is captured, so the full-track can't be half-committed.
set -uo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root" || exit 1
fail() {
  echo "hermes-config-encrypted: FAIL -- $1" >&2
  exit 1
}
enc="dot_hermes/encrypted_private_config.yaml.age"

if [[ ! -f $enc ]]; then
  # Pre-migration (the modify_ template still owns the relay route) is a valid interim -- skip. But a removed
  # modify_ with no captured encrypted config is a half-migrated state (relay route untracked) -- hard-fail.
  [[ -f dot_hermes/modify_private_config.yaml.tmpl ]] && {
    echo "hermes-config-encrypted: skipped (config not yet on the encrypted track)"
    exit 0
  }
  fail "modify_ removed but $enc not captured -- half-migrated state; do not commit"
fi
head -3 "$enc" | grep -qE 'AGE ENCRYPTED FILE|age-encryption\.org/v1' || fail "$enc is not an age blob (plaintext leak risk)"
grep -qE '(_config_version|deliver_only|basic_auth):' "$enc" && fail "plaintext config markers found inside $enc"
[[ ! -e dot_hermes/private_config.yaml && ! -e dot_hermes/config.yaml ]] || fail "a plaintext config sibling exists in dot_hermes/"
[[ ! -e dot_hermes/modify_private_config.yaml.tmpl && ! -e private/relay-hermes-route.yq ]] || fail "old modify_ mechanism still present"
[[ ! -e dot_hermes/private_dot_env ]] || fail "a rendered plaintext .env (private_dot_env) is present -- it must stay a .tmpl"
# Match only REAL keys: the marker followed by a long bech32 tail. Prose that
# merely mentions the marker (specs, this file) never has the tail, so docs stay
# covered against actual leaks without tripping on documentation. The pattern is
# additionally split across adjacent quoted strings so this line's own bytes can
# never match it.
grep -rlqE 'AGE-SECRET-KEY-''1[A-Z0-9]{40,}' . --exclude-dir=.git 2>/dev/null && fail "an age PRIVATE key is in the source tree"
for p in dot_hermes/config.yaml.bak.test dot_hermes/key.txt dot_hermes/backups/pre-migration-x.zip; do
  git check-ignore -q "$p" || fail ".gitignore failsafe is not covering $p"
done
toml="$(CI=1 chezmoi execute-template --no-tty <.chezmoi.toml.tmpl 2>/dev/null || true)"
grep -q 'secrets = "error"' <<<"$toml" || fail '.chezmoi.toml.tmpl missing add.secrets = "error"'
grep -q 'encryption = "age"' <<<"$toml" || fail '.chezmoi.toml.tmpl missing encryption = "age"'
grep -qE 'recipient = "age1' <<<"$toml" || fail '.chezmoi.toml.tmpl missing an age recipient'
echo "hermes-config-encrypted: OK"
