# Research: sops-nix vs HashiCorp Vault, Operational Risk and Rotation

Audience: senior power user, single-user nix-darwin workstation + homelab. Failure modes, not enthusiasm.

## Findings

### F1. sops rotation is a four-command, manual loop

From the upstream sops README (https://github.com/getsops/sops):

- `sops rotate example.yaml`: "generates a new data encryption key and reencrypt[s] all values with the
  new key." Use `-i` to write in place.
- `sops updatekeys test.enc.yaml`: "Uses the `.sops.yaml` configuration file to update (add or remove)
  the corresponding secrets in the encrypted file." Does *not* rotate the data key. "Use `updatekeys` if
  you want to add a key without rotating the data key."
- `sops edit example.yaml`: opens `$EDITOR` on the decrypted plaintext and re-encrypts on save.

The two commands answer different questions. `updatekeys` is for changing recipients (added a new host,
removed a compromised key). `rotate` is for changing the data encryption key itself. When *removing* a
recipient, the upstream README is explicit: "When removing keys, it is recommended to rotate the data key
using `-r`, otherwise, owners of the removed key may have add access to the data key in the past." The
previous holder of the removed age key still possesses the old DEK from prior git history; without
`rotate`, old commits remain readable to them.

**Concrete rotation workflow for a credential** (the (a) to (e) you outlined):

```bash
sops edit secrets/foo.yaml         # decrypt, edit value, re-encrypt on save
git add secrets/foo.yaml && git commit -m "rotate foo credential"
darwin-rebuild switch --flake .
```

Per credential: ~30 to 90 seconds for the edit, plus whatever `darwin-rebuild switch` takes on your
system (typically 10 to 60s for a no-op closure rebuild when only `/run/secrets.d/` content changed). The
rebuild is required because sops-nix decrypts at NixOS activation time, not evaluation: "Secrets are
decrypted from sops files during activation time." No service-level reload primitive exists in sops-nix
itself; downstream services reload via systemd `restartTriggers` if you wired them.

### F2. sops-nix README is silent on rotation strategy

From https://github.com/Mic92/sops-nix README: only `sops updatekeys secrets/example.yaml` is mentioned,
and only in the context of adding a new host: "If you add a new host to your `.sops.yaml` file, you will
need to update the keys for all secrets that are used by the new host." There is no documented procedure
for routine credential rotation, no reminder mechanism, no expiry tracking. Rotation is a discipline you
impose on yourself.

### F3. Vault dynamic secrets cover most major databases but require Vault to broker every connection

From https://developer.hashicorp.com/vault/docs/secrets/databases, supported plugins: PostgreSQL,
MySQL/MariaDB, MongoDB, MongoDB Atlas, Oracle, Cassandra, Couchbase, Elasticsearch, HanaDB, InfluxDB,
MSSQL, Redis, Redshift, Snowflake, plus custom plugins. Two distinct models:

- **Dynamic secrets**: per-request unique credentials with a lease. "Services no longer need to hardcode
  credentials: they can request them from Vault" with automatic expiration. Each request produces a fresh
  `username/password` with a TTL; on lease expiry, Vault revokes the database user.
- **Static role rotation**: Vault owns an existing database user and rotates its password on a schedule.
  "A 1-to-1 mapping of Vault roles to usernames in a database" where "Vault stores and automatically
  rotates passwords for the associated database user based on a configurable period." Default 24h.

The two are orthogonal. Dynamic = ephemeral identity. Static = persistent identity, rotating credential.
For a homelab Postgres, dynamic mode is the strongest case for Vault. sops-nix simply can't replicate
this.

### F4. Vault cannot rotate arbitrary static API tokens (GitHub PAT, OAuth client secrets)

The Key Management secrets engine (https://developer.hashicorp.com/vault/docs/secrets/key-management)
handles **cryptographic key material** distributed to KMS providers (AWS, Azure, GCP, OCI). It does not
rotate GitHub PATs, OAuth client secrets, Slack webhook URLs, or arbitrary third-party tokens. The KV v2
engine versions secrets so you can roll back, but Vault doesn't *generate* a new GitHub PAT for you. You
still go to github.com, create a new token, and write it to KV. There are vendor-specific dynamic-secret
engines (GitHub App, AWS, Azure AD, Consul, RabbitMQ, etc.), but classic GitHub user PATs and most
OAuth-app client secrets are outside that scope. Practical answer: **for the bulk of personal SaaS
credentials, Vault is a versioned KV store with policies, not an automatic rotator.**

### F5. Vault sealing model: every restart needs an unseal

From https://developer.hashicorp.com/vault/docs/concepts/seal: Shamir-sealed Vault uses "an algorithm
known as Shamir's Secret Sharing to split the key into shares." A quorum of shares must be supplied after
every restart before Vault is operational. The docs call out the operational pain: "Unsealing nodes can
make automating a Vault installation difficult." HashiCorp's recommendation: "For most users, auto unseal
provides a better experience."

Implication for a homelab: every reboot of the Vault host (kernel update, power blip, scheduled
maintenance) is a manual unseal ceremony unless you configure auto-unseal, which means another keystore.

### F6. Auto-unseal options and their homelab fit

From https://developer.hashicorp.com/vault/docs/configuration/seal, seal types: Transit, PKCS11 (HSM),
AWS KMS, Azure Key Vault, GCP Cloud KMS, AliCloud KMS, OCI KMS. (TPM is not in the documented list as of
the page rendering; it is not a first-class seal type in OSS Vault.)

For homelab self-hosting, the realistic options are:

- **Transit seal** (https://developer.hashicorp.com/vault/docs/configuration/seal/transit): "Configures
  Vault to use Vault's Transit Secret Engine as the autoseal mechanism." Requires `address` and `token`
  to a *second* Vault cluster. Caveats: token must be "an orphan token, otherwise when the parent token
  expires or gets revoked the seal will break"; should also be periodic. This creates a circular
  dependency: the unsealing Vault must itself stay unsealed (by Shamir, typically), which means you've
  moved the manual-unseal problem to the second box rather than eliminating it. For a single-user homelab
  this is overkill.
- **PKCS11 / HSM**: enterprise, hardware investment, not homelab.
- **Cloud KMS** (AWS/Azure/GCP): defeats the "self-hosted" goal. Pragmatically the best UX, since a
  $1/month KMS key auto-unseals reliably, but it ties homelab availability to a SaaS dependency.

There is no clean homelab-default in 2026. The community-realistic answers are (a) accept manual Shamir
unseal on every reboot, (b) use cloud KMS and accept the dependency, or (c) Transit seal with a second
always-on Vault. None of these match the simplicity of `sops-nix`'s "the age key is on disk, decrypt at
activation."

### F7. Vault Agent during outage: retry, not cached fallback

From https://developer.hashicorp.com/vault/docs/agent-and-proxy/agent/template: when Vault is
unreachable, "On failure, it will back off for a short while (including some randomness to help prevent
thundering herd scenarios) and retry." The `exit_on_retry_failure` flag in `template_config` controls
termination, default is `false` (retry indefinitely). With `exit_on_retry_failure = true` plus
`error_on_missing_key = true`, Agent exits on failure.

Persistent cache: from https://developer.hashicorp.com/vault/docs/agent-and-proxy/agent/caching, Agent
supports a persistent cache, but the documented `type` is `kubernetes` only. The docs are explicit:
"Agent performs all operations in memory and does not persist anything to storage. This means that when
the agent is shut down, all the renewal operations are immediately terminated." A previous Agent
process's cache can be restored on startup, but renewals are not resumed.

GitHub issue #28305 confirms a real gap: "Vault Proxy with persistent static secrets cache & auto-auth
enabled is unable to start up offline without trying to connect to Vault Server to renew its token." So
the "fail-cached" mental model does not hold reliably: a restart while Vault is unreachable can leave
Agent unable to serve cached secrets.

**Verdict for the offline-laptop scenario:** if you `darwin-rebuild switch` while Vault is unreachable,
behavior depends on how secrets are wired. With Vault Agent templates feeding files, the Agent retries
and the file may be stale, missing, or empty depending on whether Agent is fresh or already running.
Services started fresh will likely block or fail. There is no documented "use last-known-good plaintext"
guarantee.

### F8. sops key loss: nothing is recoverable

Neither the sops-nix README nor the upstream sops README documents a recovery mechanism for losing the
sole age key. The upstream README's only relevant statement is the inverse: "a copy of the
encryption/decryption key is stored securely in each KMS and PGP block. As long as one of the KMS or PGP
method is still usable, you will be able to access your data." If your `.sops.yaml` lists only one age
recipient and that age private key is gone, the encrypted files in the git repo are ciphertext forever.
**There is no backdoor, no master key, no recovery.** This is by design, and the mitigation is
operational: list multiple recipients (a backup age key stored offline, a YubiKey-bound age identity, a
KeePassXC-stored age key), or escrow the age key in your password manager.

### F9. Vault token TTL while offline

From https://developer.hashicorp.com/vault/docs/concepts/tokens: system max TTL defaults to 32 days,
configurable per mount. "After the current TTL is up, the token will no longer function: it, and its
associated leases, are revoked." The docs do *not* address offline behavior specifically. Practical
inference: if your laptop is offline longer than the token TTL and Agent cannot reach Vault to renew, the
token expires server-side. On reconnect, Agent must re-auth (which for AppRole means a usable secret_id).
For OIDC auth, default token TTLs are typically 1 to 24h depending on role config, not directly stated on
the OIDC providers index page. Periodic tokens are the recommended pattern for long-running services
because they "reset their TTL to the configured period on each successful renewal."

### F10. AppRole secret_id: bootstrap SPOF, prevention-only mitigations

From https://developer.hashicorp.com/vault/docs/auth/approle: "SecretID is a credential that is required
by default for any login (via `secret_id`) and is intended to always be secret." The docs consistently
push response wrapping: "instead of the app having knowledge of the secret ID directly, we have a trusted
orchestrator give the app access to a short-lived response-wrapping token."

Mitigation parameters: `secret_id_ttl` (e.g., `10m`), `secret_id_num_uses` (e.g., `40`),
`secret_id_bound_cidrs` ("only allow logins coming from IP addresses belonging to configured CIDR
blocks").

What the docs *don't* explicitly say but follows from the model: a leaked unwrapped secret_id with a long
TTL and unbound CIDR is full Vault access at the role's policy. Lost secret_id has no recovery: you
generate a new one via `vault write auth/approle/role/<name>/secret-id`. The bootstrapping problem is
real: *something* on disk has to authenticate to Vault, and that something is itself an unrotated
long-lived credential. The chain has to terminate somewhere.

### F11. Vault audit logs: bring your own rotation

From https://developer.hashicorp.com/vault/docs/audit/file: "The device does not currently assist with
any log rotation. There are very stable and feature-filled log rotation tools already, so we recommend
using existing tools." Send `SIGHUP` to Vault after rotation: "configure your log rotation software to
send the `vault` process a signal hang up / `SIGHUP` after each rotation of the log file."

On macOS, this means newsyslog or a launchd-driven script. If you ignore this, the audit log grows
unbounded, and Vault refuses operations if all configured audit devices fail to write. That last detail
is the operational footgun: a full disk because of an unrotated audit log will halt Vault.

### F12. Real-world incident search: thin

HN/Reddit searches for sops lockout and Vault homelab cascade failures returned only general discussion
threads, no concrete "I lost my key and lost everything" post-mortems with verifiable detail. The closest
documented operational-pain artifact is the Vault Proxy offline-startup bug
(https://github.com/hashicorp/vault/issues/28305), filed Sept 2024, confirming the
cache-doesn't-survive-offline-restart problem in practice. Absence of post-mortems is not evidence of
safety: it's likely selection bias (people who lose their age key don't post about it; sysadmins who
endured a Vault unseal cascade tend to write internal docs, not HN threads). A separate signal: the
August 2025 Vault zero-day cluster (HN 44821434) speaks to attack surface, not personal-scale operational
risk, and isn't directly relevant.

### F13. Activation-time vs runtime decryption: structural difference

sops-nix decrypts once, at NixOS/nix-darwin activation, and writes plaintext to `/run/secrets.d/<n>/`.
After activation, secrets are static files on tmpfs until the next rebuild. Vault Agent decrypts
continuously: leases renew, templates re-render, files change without a rebuild. This has two
consequences:

- **sops-nix:** rotation requires rebuild. Forgotten rotations are silent; nothing nags you.
- **Vault:** rotation is the default state. But every reboot of the secrets infrastructure is a potential
  incident.

### F14. The bootstrap credential question is asymmetric

sops-nix's bootstrap credential is the age key on disk, typically in `~/.config/sops/age/keys.txt` or
`/var/lib/sops-nix/key.txt`. Loss = total loss (F8). Theft = full repo decryption. There is no second
factor.

Vault's bootstrap credential is the AppRole secret_id (or root token, or AWS IAM identity). Loss =
re-issue. Theft = role-policy access until revocation. Better blast-radius story, worse
single-point-of-failure story for the *server's* unseal keys (F5-F6).

Both systems terminate in "something on disk that can decrypt." The difference is what you can do *after*
compromise. sops-nix offers no revocation: you rotate every secret in the repo, push, rebuild every host.
Vault offers `vault token revoke` and can invalidate the AppRole.

### F15. Operational footprint at single-user scale

sops-nix runtime cost: zero processes, zero ports, one binary at activation. No daemons, no audit log, no
quorum, no unseal.

Vault runtime cost: one always-on server (sealed-on-restart), Vault Agent on each consumer, audit log
file rotation, periodic upgrade (Vault releases monthly; recent CVEs in auth/identity per Aug 2025
disclosures), backup of the storage backend (raft snapshots), and unseal-key custody. On a homelab with
one user, this is a non-trivial fraction of total ops time.

## Synthesis (250 words)

Both systems push their failure modes onto different parts of the lifecycle. **sops-nix concentrates risk
at key-loss and rotation discipline.** The age private key is a single point of failure with zero
recovery: lose it, lose everything in the repo. The mitigation is multi-recipient encryption (a backup
age identity in your password manager, a YubiKey-bound identity), and that mitigation is your
responsibility because nothing in the toolchain enforces it. Rotation is a manual `sops edit` + commit +
`darwin-rebuild switch` loop, ~1 to 2 minutes per credential, with no expiry tracking. The system never
nags. Forgotten rotations stay forgotten until something else surfaces them. Trade-off: zero runtime
infrastructure, no daemon to keep alive, full offline operation, deterministic activation.

**Vault concentrates risk at availability and bootstrap.** A reboot needs an unseal: manual Shamir or
auto-unseal that adds a second-keystore dependency (Transit seal recurses, cloud KMS introduces SaaS
coupling). Vault Agent's persistent cache does not reliably survive a restart while Vault is unreachable
(issue #28305), so an offline `darwin-rebuild switch` can fail-closed. The AppRole secret_id is a
long-lived bootstrap credential with no documented disk-storage warning. Audit logs don't rotate
themselves. The compensating wins are real but narrow: dynamic database credentials, automatic
static-role rotation, revocation primitives, lease-bound access. For a single-user workstation with
mostly third-party API tokens and one homelab Postgres, you pay continuous availability tax for
capabilities you'd exercise rarely. **sops-nix is honest about its single failure mode; Vault distributes
risk across more surfaces with more recovery options at the cost of perpetual operational overhead.**

## Sources

- https://github.com/getsops/sops
- https://github.com/Mic92/sops-nix
- https://developer.hashicorp.com/vault/docs/secrets/databases
- https://developer.hashicorp.com/vault/docs/secrets/key-management
- https://developer.hashicorp.com/vault/docs/concepts/seal
- https://developer.hashicorp.com/vault/docs/configuration/seal
- https://developer.hashicorp.com/vault/docs/configuration/seal/transit
- https://developer.hashicorp.com/vault/docs/agent-and-proxy/agent/template
- https://developer.hashicorp.com/vault/docs/agent-and-proxy/agent/caching
- https://developer.hashicorp.com/vault/docs/auth/approle
- https://developer.hashicorp.com/vault/tutorials/auth-methods/approle-best-practices
- https://developer.hashicorp.com/vault/docs/audit/file
- https://developer.hashicorp.com/vault/docs/concepts/tokens
- https://github.com/hashicorp/vault/issues/28305
