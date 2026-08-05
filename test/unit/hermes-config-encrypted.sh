#!/usr/bin/env bash
# hermes-config-encrypted: source-tree secret-leak guard for the age-encrypted ~/.hermes/config.yaml.
# Runs everywhere (incl. CI) -- inspects only the repo source, never decrypts, never touches ~/.hermes.
# FAILS (does not skip) until the encrypted config is captured, so the full-track can't be half-committed.
# Re-camped into test/unit/ (source inspection, single component, fast); REPO_ROOT is two levels up.
set -uo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root" || exit 1
fail() {
  echo "hermes-config-encrypted: FAIL -- $1" >&2
  exit 1
}
enc="private_dot_hermes/encrypted_private_config.yaml.age"

if [[ ! -f $enc ]]; then
  # Pre-migration (the modify_ template still owns the relay route) is a valid interim -- skip. But a removed
  # modify_ with no captured encrypted config is a half-migrated state (relay route untracked) -- hard-fail.
  [[ -f private_dot_hermes/modify_private_config.yaml.tmpl ]] && {
    echo "hermes-config-encrypted: skipped (config not yet on the encrypted track)"
    exit 0
  }
  fail "modify_ removed but $enc not captured -- half-migrated state; do not commit"
fi
head -3 "$enc" | grep -qE 'AGE ENCRYPTED FILE|age-encryption\.org/v1' || fail "$enc is not an age blob (plaintext leak risk)"
grep -qE '(_config_version|deliver_only|basic_auth):' "$enc" && fail "plaintext config markers found inside $enc"
# Route-level leak canaries, CI-safe (this test still never decrypts). A webhook
# route is a NAME (public: it is the URL path segment, and the log lib and
# run_after_68 both name it in the clear), a Discord chat_id, and an HMAC key.
# The chat_ids exist ONLY inside the encrypted config, so finding one anywhere in
# the source tree means a decrypted copy or a debug dump got committed. Checked
# against the whole tree, not just the blob. Each literal is split across
# adjacent quoted strings -- the same idiom the age-key pattern above uses -- so
# this file's own bytes can never satisfy its own search.
for chat_id in '15333878568''21747803' '15192121325''18989915' '15103791806''78975638' '15111558445''59933543'; do
  leaked="$(grep -rl -- "$chat_id" . --exclude-dir=.git 2>/dev/null || true)"
  [[ -z $leaked ]] || fail "a hermes route chat_id is readable in the source tree: $leaked"
done
[[ ! -e private_dot_hermes/private_config.yaml && ! -e private_dot_hermes/config.yaml ]] || fail "a plaintext config sibling exists in private_dot_hermes/"
[[ ! -e private_dot_hermes/modify_private_config.yaml.tmpl && ! -e private/relay-hermes-route.yq ]] || fail "old modify_ mechanism still present"
[[ ! -e private_dot_hermes/private_dot_env ]] || fail "a rendered plaintext .env (private_dot_env) is present -- it must stay a .tmpl"
# Match only REAL keys: the marker followed by a long bech32 tail. The optional
# (PQ-) segment covers post-quantum identities (age-keygen -pq emits
# AGE-SECRET-KEY-PQ-1...) as well as the classic AGE-SECRET-KEY-1... form. Prose
# that merely mentions the marker (specs, this file) never has the tail, so docs
# stay covered against actual leaks without tripping on documentation. The pattern
# is additionally split across adjacent quoted strings so this line's own bytes can
# never match it.
grep -rlqE 'AGE-SECRET-KEY-''(PQ-)?1[A-Z0-9]{40,}' . --exclude-dir=.git 2>/dev/null && fail "an age PRIVATE key is in the source tree"
for p in private_dot_hermes/config.yaml.bak.test private_dot_hermes/key.txt private_dot_hermes/backups/pre-migration-x.zip; do
  git check-ignore -q "$p" || fail ".gitignore failsafe is not covering $p"
done
toml="$(CI=1 chezmoi execute-template --no-tty <.chezmoi.toml.tmpl 2>/dev/null || true)"
grep -q 'secrets = "error"' <<<"$toml" || fail '.chezmoi.toml.tmpl missing add.secrets = "error"'
grep -q 'encryption = "age"' <<<"$toml" || fail '.chezmoi.toml.tmpl missing encryption = "age"'
grep -qE 'recipient = "age1' <<<"$toml" || fail '.chezmoi.toml.tmpl missing an age recipient'
echo "hermes-config-encrypted: OK (the committed config is an age blob with no plaintext markers, no route chat_id is readable anywhere in the tree, no plaintext sibling or retired modify_ mechanism remains)"
