# age Encryption Key: Rotation and Disaster Recovery

How to restore the chezmoi age key on a new or wiped machine, and how to rotate it. Every command here is
run by the operator, interactively, with the KeePassXC database unlocked. Automated agents must never run
`chezmoi add`, `chezmoi re-add`, `chezmoi apply`, or `chezmoi edit` against this repo, because chezmoi's
configured source directory is the primary checkout, so those commands would write outside a worktree and
could touch live secret state.

## What the key protects

chezmoi encrypts a handful of source files with [age](https://age-encryption.org). The public recipient
lives in `.chezmoi.toml.tmpl` under `[age]`; the private identity lives at `~/.config/chezmoi/key.txt`
(mode 600) and is never committed. The single source of truth for the private key is the KeePassXC entry
**`chezmoi :: Private Key :: age`**, whose Password field holds the one `AGE-SECRET-KEY-1…` line (a
complete age identity on its own; the comment lines in a key file are optional).

### Every managed encrypted target

Each `encrypted_` source file below decrypts to the listed target on `chezmoi apply`. Rotation and
recovery both cover all of them.

| Source file (in this repo)                                                | Decrypts to                                |
| ------------------------------------------------------------------------- | ------------------------------------------ |
| `dot_hermes/encrypted_private_config.yaml.age`                            | `~/.hermes/config.yaml` (root, Bob)        |
| `dot_hermes/profiles/private_butters/encrypted_private_config.yaml.age`   | `~/.hermes/profiles/butters/config.yaml`   |
| `dot_hermes/profiles/private_concerned/encrypted_private_config.yaml.age` | `~/.hermes/profiles/concerned/config.yaml` |
| `dot_hermes/profiles/private_elaine/encrypted_private_config.yaml.age`    | `~/.hermes/profiles/elaine/config.yaml`    |
| `dot_hermes/profiles/private_nicodemus/encrypted_private_config.yaml.age` | `~/.hermes/profiles/nicodemus/config.yaml` |

The Codegraph Model Context Protocol enablement is a section inside the root `~/.hermes/config.yaml`, so
it rides the root capture above rather than a separate encrypted file. There is no standalone Codegraph
secret to track.

## Disaster recovery: restore the key on a fresh or wiped machine

The bootstrap already unlocks KeePassXC for every other templated secret, so restoring the age key adds
no new manual step beyond that unlock.

1. Make sure the KeePassXC database is reachable (iCloud Drive or an offline backup) at the path in
   `.chezmoi.toml.tmpl`.
1. Run `chezmoi init` (or `chezmoi apply`) once. On macOS the `run_once_before_05-restore-age-key` script
   writes the private key from the KeePassXC entry `chezmoi :: Private Key :: age` to
   `~/.config/chezmoi/key.txt` (mode 600) before any encrypted file is read. That script is darwin-only.
1. If you need to restore the key by hand (the script did not run, or a non-macOS host), copy the
   Password field of `chezmoi :: Private Key :: age` into `~/.config/chezmoi/key.txt`, then
   `chmod 600 ~/.config/chezmoi/key.txt`.
1. Confirm recovery without printing any plaintext: `chezmoi cat ~/.hermes/config.yaml | shasum -a 256`.
   A hash (not an error) means every encrypted target will decrypt.

## Rotation: replace the key

Rotate when the private key may have been exposed, or on a scheduled cadence. Rotation re-encrypts every
managed target to a new recipient in place. **Use `chezmoi re-add --re-encrypt`. Never use a
`chezmoi forget` then `chezmoi add` sequence**, which drops the file from the source state and can lose
history or the encrypted attribute.

1. Generate a new keypair: `age-keygen -o ~/.config/chezmoi/key.txt.new`. It prints the new public
   recipient (an `age1…` string) to stderr and writes the identity to the file.
1. Update the KeePassXC entry `chezmoi :: Private Key :: age`: put the new `AGE-SECRET-KEY-1…` line in
   the Password field. Keep the previous key in a dated attribute or history entry until rotation is
   verified, so a half-finished rotation is recoverable.
1. Swap the identity into place: `mv ~/.config/chezmoi/key.txt.new ~/.config/chezmoi/key.txt` and
   `chmod 600 ~/.config/chezmoi/key.txt`.
1. Update the recipient in `.chezmoi.toml.tmpl` under `[age]` to the new `age1…` recipient, and
   `chezmoi apply` the config so the rendered `~/.config/chezmoi/chezmoi.toml` carries it.
1. Re-encrypt every managed target to the new recipient: `chezmoi re-add --re-encrypt`. chezmoi reads
   each decrypted target, re-encrypts it to the new recipient, and rewrites the `encrypted_` source file,
   preserving the encrypted attribute.
1. Verify each target still decrypts under the new key (hashes only, no plaintext):
   `chezmoi cat ~/.hermes/config.yaml | shasum -a 256` and the same for each profile config. The
   `test/integration/hermes-age-captures.sh` check also decrypts every committed capture when the
   identity is present.
1. Commit the rotated `encrypted_` files and the `.chezmoi.toml.tmpl` recipient change. Once verified,
   remove the retained previous key from KeePassXC.

## Rehearsing this runbook

`test/e2e/age-rotation-drill.sh` rehearses the rotation cryptographically in a throwaway sandbox: two
fresh keypairs, a dummy secret, and an encrypt-then-re-encrypt cycle proving that after rotation only the
new key decrypts and the plaintext survives byte for byte. It needs only `age` and `age-keygen`, touches
no live key or source file, and skips cleanly where age is absent (continuous integration). Run it before
a real rotation to confirm the tools behave as this runbook expects.
