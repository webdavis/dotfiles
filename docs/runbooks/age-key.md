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
complete age identity on its own; the comment lines in a key file are optional). age also supports
post-quantum identities (`AGE-SECRET-KEY-PQ-1…`); everything below applies to either form.

### Every managed encrypted target

Each `encrypted_` source file below decrypts to the listed target on `chezmoi apply`. Rotation and
recovery both cover all of them. There are five files.

| Source file (in this repo)                                                | Decrypts to                                |
| ------------------------------------------------------------------------- | ------------------------------------------ |
| `dot_hermes/encrypted_private_config.yaml.age`                            | `~/.hermes/config.yaml` (root, Bob)        |
| `dot_hermes/profiles/private_butters/encrypted_private_config.yaml.age`   | `~/.hermes/profiles/butters/config.yaml`   |
| `dot_hermes/profiles/private_concerned/encrypted_private_config.yaml.age` | `~/.hermes/profiles/concerned/config.yaml` |
| `dot_hermes/profiles/private_elaine/encrypted_private_config.yaml.age`    | `~/.hermes/profiles/elaine/config.yaml`    |
| `dot_hermes/profiles/private_nicodemus/encrypted_private_config.yaml.age` | `~/.hermes/profiles/nicodemus/config.yaml` |

The Codegraph Model Context Protocol enablement is a section inside the root `~/.hermes/config.yaml`, so
it rides the root capture above rather than a separate encrypted file. There is no standalone Codegraph
secret to track; when counting targets it is part of the root config, not a sixth file.

## Disaster recovery: restore the key on a fresh or wiped machine

The bootstrap already unlocks KeePassXC for every other templated secret, so restoring the age key adds
no new manual step beyond that unlock.

1. Make sure the KeePassXC database is reachable (iCloud Drive or an offline backup) at the path in
   `.chezmoi.toml.tmpl`.
1. Run `chezmoi init --apply` once. The `run_before_05-restore-age-key` script (darwin-only, idempotent)
   reads the private key from the KeePassXC entry `chezmoi :: Private Key :: age`, streams it into
   `~/.config/chezmoi/key.txt` (mode 600) at execution time, validates it, and only then decrypts any
   encrypted file. The secret is never written into the script body.
1. If you need to restore the key by hand (the script did not run, or a non-macOS host), copy the
   Password field of `chezmoi :: Private Key :: age` into `~/.config/chezmoi/key.txt`, then
   `chmod 600 ~/.config/chezmoi/key.txt`.
1. Confirm recovery without printing any plaintext: `chezmoi cat ~/.hermes/config.yaml | shasum -a 256`.
   A hash (not an error) means every encrypted target will decrypt.

The restore script self-heals: if `key.txt` is later deleted it is restored on the next apply; if the
file present derives the wrong recipient, or is a symlink, the script warns loudly, points back here, and
refuses to overwrite it rather than bricking the apply.

## Rotation: replace the key

Rotate when the private key may have been exposed, or on a scheduled cadence. The safe ordering keeps the
OLD identity able to decrypt until the new one has provably taken over, so a half-finished rotation never
locks you out. An age identity file may hold several identities at once, and age tries every one when
decrypting, which is what makes the overlap window possible.

`chezmoi re-add --re-encrypt` requires chezmoi >= 2.68.0 (the flag was added there). The operator host
runs 2.70.5, so it is available; confirm with `chezmoi --version` before starting.

1. **Generate a new keypair.** `age-keygen -o ~/.config/chezmoi/key.new.txt`. It prints the new public
   recipient (an `age1…` string) to stderr and writes the identity to the file. Note the recipient.

   - Rollback: delete `~/.config/chezmoi/key.new.txt`. Nothing else has changed yet.

1. **Append the new identity to the live key file, keeping the old one.** Add the new `AGE-SECRET-KEY-1…`
   line to `~/.config/chezmoi/key.txt` as an additional line (do not replace the old line). Both
   identities are now active; either decrypts existing captures.

   - Rollback: remove the appended line from `key.txt`; you are back to the old-only identity.

1. **Update the recipient in config and KeePassXC.** Set the new `age1…` recipient in
   `.chezmoi.toml.tmpl` under `[age]`. In the KeePassXC entry `chezmoi :: Private Key :: age`, put the
   new `AGE-SECRET-KEY-1…` line in the Password field, and keep the previous key in a dated attribute or
   history entry until rotation is verified.

   - Rollback: revert the `.chezmoi.toml.tmpl` recipient and the KeePassXC Password to the old values.

1. **Regenerate the rendered config.** Run `chezmoi init` (or `chezmoi apply --init`) so the recipient
   change in `.chezmoi.toml.tmpl` is written into `~/.config/chezmoi/chezmoi.toml`. A plain
   `chezmoi apply` does NOT regenerate that config file, so the new recipient would otherwise not take
   effect.

   - Rollback: after reverting `.chezmoi.toml.tmpl` (previous step), run `chezmoi init` again to restore
     the old rendered recipient.

1. **Re-encrypt each managed target to the new recipient, listed explicitly by path.** Scope the command
   to the five encrypted files so it cannot ingest an unrelated modified file:

   ```
   chezmoi re-add --re-encrypt \
     ~/.hermes/config.yaml \
     ~/.hermes/profiles/butters/config.yaml \
     ~/.hermes/profiles/concerned/config.yaml \
     ~/.hermes/profiles/elaine/config.yaml \
     ~/.hermes/profiles/nicodemus/config.yaml
   ```

   chezmoi reads each decrypted target, re-encrypts it to the new recipient, and rewrites the
   `encrypted_` source file, preserving the encrypted attribute.

   - Rollback: `git -C ~/.local/share/chezmoi restore -- <the encrypted_ paths>` (or your worktree) to
     bring back the old-recipient ciphertext; the old identity still decrypts it.

1. **Verify every capture decrypts with the NEW identity alone.** Temporarily point the identity at a
   file holding ONLY the new `AGE-SECRET-KEY-1…` line and hash each target (no plaintext printed):
   `for t in ~/.hermes/config.yaml ~/.hermes/profiles/*/config.yaml; do chezmoi cat "$t" | shasum -a 256; done`.
   Each must produce a hash, not an error. `test/integration/hermes-age-captures.sh` also decrypts every
   committed capture when the identity is present.

   - Rollback: if any target fails to decrypt under the new-only identity, restore the old identity line
     in `key.txt` (both keys active again) and investigate before proceeding; do not remove the old key.

1. **Retire the old identity.** Only after every target verifies under the new-only identity: remove the
   old `AGE-SECRET-KEY-1…` line from `~/.config/chezmoi/key.txt` (leaving the new line), delete
   `~/.config/chezmoi/key.new.txt` if you used it, and remove the retained previous key from KeePassXC.

   - Rollback: none needed past this point; if a problem surfaces later, the old ciphertext is still in
     git history, but only the compromise-response steps below, not re-adding the old key, are the
     correct response to exposure.

1. **Commit.** Commit the rotated `encrypted_` files and the `.chezmoi.toml.tmpl` recipient change.

## Compromise response: a leaked key is not fixed by rotation alone

Rotating the key changes which identity decrypts FUTURE ciphertext. It does nothing for the past: every
`encrypted_` blob ever committed sits in git history, and a leaked age private key decrypts all of it,
forever, for anyone who has both the key and a clone. Treat a suspected key leak as a leak of every
secret those five configs have ever held.

1. Rotate the age key using the procedure above, so new captures are encrypted to a recipient the
   attacker does not hold.
1. Rotate every underlying credential carried in the five configs AT ITS SOURCE: revoke and reissue each
   token, webhook secret, HMAC (hash-based message authentication code) key, and password, so the values
   the attacker can still read from history are dead.
1. Re-encrypt the captures only AFTER those underlying values have changed, so the new ciphertext holds
   the new secrets. Re-encrypting unchanged secrets to a new key protects nothing.
1. Assume the historical ciphertext stays readable by whoever holds the leaked key. Rewriting git history
   does not reach clones or forks that already exist, so the credential rotation in step 2, not history
   surgery, is what actually contains the exposure.

## Rehearsing this runbook

`test/e2e/age-rotation-drill.sh` rehearses the CRYPTOGRAPHIC EFFECT of the rotation in a throwaway
sandbox: two fresh keypairs, a dummy secret, and an encrypt-then-re-encrypt cycle proving that after
rotation only the new key decrypts and the plaintext survives byte for byte. It touches no live key or
source file and skips cleanly where age is absent (continuous integration).

The drill does NOT invoke the real `chezmoi re-add --re-encrypt` command. That flag needs chezmoi

> = 2.68.0, but the flake pins chezmoi 2.62.3, and continuous integration must never depend on the
> operator's host chezmoi. So the drill proves the invariant the command must uphold (old key locked out,
> new key decrypts, plaintext preserved), not the command itself. Bumping the nixpkgs chezmoi to >=
> 2.68.0 so the drill can exercise the real flow is tracked as follow-up. Run the drill before a real
> rotation to confirm the age tools behave as this runbook expects, then run the actual chezmoi command
> per the steps above.
