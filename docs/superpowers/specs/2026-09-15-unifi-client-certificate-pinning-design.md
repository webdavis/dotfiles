# UniFi router client certificate pinning

Status: design, written 2026-09-15 by an agent. NOT approved, NOT built. No code was written or
changed by the task that produced this document, and nothing in it was measured against the live
router: the agent was barred from touching the operator's router, so every certificate fact below is
either read out of this repository or sourced from Ubiquiti's own documentation and community
records. Choices made in the operator's place are listed under "Assumptions made in the operator's
place"; the decisions that are genuinely theirs are under "Open questions".

## Why this exists

Ledger task 89, filed 2026-09-15:

> Design certificate pinning for the UniFi router client. Out of scope for the Hue bridge pinning
> design: `pns/crates/pns-adapters/src/unifi/client.rs` disables verification against
> `https://192.168.1.1` while sending a router API key, a more valuable credential than the lamp key,
> and it is a different device with a different certificate story that needs its own measurement and
> design. Follows the same shape as the Hue design once written: measure the live certificate, choose
> a pinning approach, decide where the pin lives (the vault convention that decided the Hue pin
> applies here too).

The bullet holds up. Verification is off, the key really is sent over that connection, and the vault
convention it points at is real: the operator answered the Hue design's open question 3 on 2026-09-15
by putting the Hue pin in KeePassXC rather than in the committed values file, because this repository
is public and its own convention already treats an id as a vault reference.

One part of the bullet cannot be honored here. "Measure the live certificate" is an operator step, not
an agent step: this task was explicitly barred from reaching the router, so the measurement is a gate
on building the design rather than an input to writing it. Section "What the router serves, unmeasured"
says exactly what is known without it and what changes if the measurement surprises us.

## Current state, from source

1. **The crate and the caller.** `pns-adapters` owns the client: `UniFiRouter` is declared at
   `pns/crates/pns-adapters/src/unifi/client.rs:13` and exported at
   `pns/crates/pns-adapters/src/lib.rs:131`. Exactly one place builds it in production,
   `pns/crates/pns/src/command_home.rs:69`, which is the body of `pns home`. The only other
   construction is the scripted-transport test at
   `pns/crates/pns-adapters/src/unifi/tests/transport.rs:172`, through the injection seam
   `UniFiRouter::with_agent` (`client.rs:55`). The presence narrowing's away gate would be a second
   reader, and `pns/crates/pns/src/presence_runtime.rs:154` records why it is dormant: two router
   calls at five seconds each is too much for a lamp path.
2. **The stack and the TLS (transport layer security) configuration.** ureq 3.4.0 with the rustls
   feature, `pns/crates/pns-adapters/Cargo.toml:30`. The agent is built in `UniFiRouter::new` at
   `client.rs:39`, and its TLS configuration is this, verbatim from `client.rs:42`:

   ```rust
   .tls_config(
       ureq::tls::TlsConfig::builder()
           .disable_verification(true)
           .build(),
   )
   ```

3. **Whether the certificate is verified at all: it is not.** `disable_verification(true)` is ureq's
   all-or-nothing switch, and with the rustls backend it installs a verifier that accepts any chain and
   any name. The client's own doc comment at `client.rs:9` states the reason and names its precedent:
   "TLS verification is disabled the way the hue bridge's is, and for the same reason: the router serves
   a self-signed certificate for its own LAN address, and no CA vouches for it." So the finding is not
   "pinning is missing", it is "nothing is checked", and the API key rides that connection in a header
   on every call (`client.rs:70`, `.header("X-API-KEY", &self.key)`).
4. **The trust story today: nothing.** No pin, no private certificate authority, no trust anchor of any
   kind. `[plugins.router]` carries eight keys and none of them is about identity
   (`pns/crates/pns-adapters/src/config/schema.rs:158`): `api_key`, `device_hostname`, `device_ipv4`,
   `device_mac`, `enabled`, `router_url`, `stale_alert_channel`, `type`. The committed values file
   points `router_url` at `https://192.168.1.1` and reads the key from the vault entry
   "UniFi :: API Key (dresden-udr)" (`dot_config/pns/config-values.toml:82`).

## The threat this addresses

One threat, stated at its real size. Anything already on the home network that can answer for
192.168.1.1, by address-resolution spoofing, by taking the address through a dynamic host configuration
protocol lease, or by sitting between the laptop and the router, gets a full TLS handshake from `pns
home` and is handed the router API key in the first request header. There is no name check, no chain
check and no pin, so the impostor needs no certificate of any provenance: a self-signed certificate it
generates on the spot is accepted.

What that key buys an attacker is the reason this is worth more than the Hue task. It reads the router's
site and client listings, which is a roster of every device in the house and where each one is, and on a
UniFi console the same key reaches the rest of the Network integration application programming
interface, not only the two paths this client calls.

Pinning closes exactly that and nothing else. It does not protect the key at rest, it does not change
what the key grants, it does not help if the router itself is compromised, and it does nothing about an
attacker who is already in position at the moment the pin is first taken. The residual case is listed
under "What this defeats, and what it does not".

## What the router serves, unmeasured

Not measured. What follows is from Ubiquiti's documentation and long-standing community record, and
every item is an acceptance check before the code is written rather than a settled fact.

- A UniFi OS console serves a self-signed certificate it generates for itself, stored on a
  UniFi Dream Router or Dream Machine at `/mnt/data/unifi-os/unifi-core/config/unifi-core.crt` with its
  key beside it. Deleting the pair and restarting mints a new one, and a UniFi OS major-version upgrade
  or a service-restart procedure can regenerate it as well.
- That certificate is issued to a name (`unifi.local` in the common reports) and carries no
  subjectAltName entry for the console's own address. Name verification of `https://192.168.1.1`
  against it therefore fails even if the certificate is installed as a trust anchor first, which is the
  same wall the Hue design hit for a different reason.
- UniFi OS 4.1 and later accepts an operator-supplied certificate and key through
  Settings, Control Plane, Console, Certificates. That is what makes approach C below possible at all,
  and it is the one material difference from the Hue bridge, which has no such feature.
- The integration application programming interface this client uses is reached through the console's
  own reverse proxy under `/proxy/network/integration/v1/`, so it is the console's certificate that is
  presented, not a separate one.

If the measurement contradicts any of this, the shape of the design does not move. A certificate with a
usable subjectAltName for the address would make ordinary verification against a pinned anchor possible
and would promote approach C; a certificate that turns out to rotate on its own schedule would turn the
rotation cost below from "rare" into "recurring", which would be a reason to reopen the choice.

## Three approaches

### A. Pin the certificate's SHA-256 fingerprint (recommended)

Record, once, the SHA-256 of the certificate the console presents. A custom rustls verifier hashes the
presented leaf on every handshake and accepts nothing else. No trust anchor, no chain math, no name
check, no certificate parser.

- Authenticates one physical console. Any other certificate, however signed, is refused.
- The verifier this needs is the one the Hue design already specifies for the same crate
  (`pns/crates/pns-adapters/src/hue/pinned_tls.rs`), so in this repository it is a reuse rather than a
  new mechanism. It is also the reason this design is cheap to build and expensive to build FIRST: see
  "Sequencing" below.
- Rotation cost for the operator: a console firmware upgrade or a certificate regeneration refuses
  every call until they run one command and paste one line. That is the honest cost and it is priced in
  "What this costs" below.

### B. Pin the public key (SubjectPublicKeyInfo SHA-256)

The conventional pin shape. It survives a certificate re-issue that keeps the same key pair.

- Costs `rustls-webpki` as a direct dependency (pre-1.0) to extract the SubjectPublicKeyInfo field,
  and a slightly larger verifier.
- Buys nothing here that A does not, because the UniFi regeneration path mints a fresh key with the
  fresh certificate. The event B is built to survive is the event this device class does not have.

### C. Install a certificate from a private certificate authority and verify normally

Generate a local certificate authority, issue a certificate for the console with a name and a matching
subjectAltName, upload it through Settings, Control Plane, Console, Certificates, point `router_url` at
that name, and configure ureq with `RootCerts::Specific(vec![our_ca])`.

- The only approach that stays inside ureq's five public TLS knobs. No `ureq::unversioned` API, no
  custom verifier, nothing that a ureq or rustls bump can break at compile time. The Hue design's own
  cost item 10 disappears.
- Rotation becomes renewal rather than re-pinning, and a renewal under the same authority needs no
  change in pns at all. This is the approach that survives a certificate change without paging anyone.
- Costs a private certificate authority on this machine that nothing else here needs, a name for the
  console that resolves reliably (this repository already pins MagicDNS names into `/etc/hosts` through
  `.chezmoidata/macos_system_setup.yaml`, so the mechanism exists), and a certificate upload that a
  UniFi OS major-version upgrade can silently revert, which lands the operator back at a refusing
  client with a longer recovery path than "paste a line".
- It also widens trust: anything signed by that authority is accepted, so the authority's key becomes a
  credential worth as much as the router key it protects.

**Recommendation: A.** It pins one device, it needs no new trust anchor and no new authority key to
protect, and the verifier is already being written for the same crate in the Hue wave, so the marginal
build is one config key, one constructor and one enrollment path. C is the better answer on rotation
alone and it is the one to revisit if the measurement shows the console's certificate rotates on its
own, or if the operator wants a real name for the console for other reasons. B is not recommended in
any case: it pays a dependency for resilience against an event that does not occur here.

## The recommended design in detail

### The pin as a value

Reuse `CertificatePin` from `pns-domain`, the value type the Hue design specifies: wire form
`"sha256:<64 lowercase hex characters>"`, comparison over the 32 raw bytes, `Display` writing the wire
form back so an enrollment line and a config line are the same string. One type, two devices, no second
parser. If the Hue work has not landed when this is built, this design carries the type instead and the
Hue work reuses it; whichever lands first owns it.

### Where the pin lives

A certificate fingerprint is public data. It is a hash of a certificate the console hands to anyone who
connects, so its confidentiality is worth nothing and its INTEGRITY is worth everything: an attacker who
can change the stored pin owns the connection, and an attacker who merely reads it learns nothing they
could not have learned by connecting. Treated by its own nature, it belongs in version control beside
`router_url`.

It goes in the vault anyway, as a custom attribute on the existing "UniFi :: API Key (dresden-udr)"
entry, read by name in `dot_config/pns/config-values.toml` the way `api_key` already is. Two reasons,
neither of them confidentiality. The operator ruled on exactly this question for the Hue pin on
2026-09-15 and chose the vault, for consistency with this repository's own convention about committed
ids in a public repository; and the entry already exists, so the pin costs one attribute rather than a
new vault read.

The config key is `[plugins.router] certificate`, which needs the same five edits every other key in
that table needed: the roster in `pns/crates/pns-adapters/src/config/schema.rs:158`, the rendered
layout `PLUGINS_ROUTER` in
`pns/crates/pns-adapters/src/config/render/layout/sensors.rs:75`, the settings read in
`pns/crates/pns-adapters/src/config/router.rs:32`, the first-run wizard, and the committed values file,
after which `just pns-config-render` regenerates the shipped template and the byte-equality test in
`just test-rust` holds the two together.

When `[plugins.router]` is enabled and `certificate` is missing, empty or malformed, the config load
REFUSES by name in the grammar `stale_alert_channel` already set (`config/router.rs:151`), and the
refusal carries the command that produces the value:

```
pns: config error (plugins.router.certificate is unset); run `pns home enroll` and paste the line it
prints; no home reading
```

Fail closed, at parse, for the reason the Hue design gives: a pin that may be omitted is a pin that
will be omitted, and the refusal belongs where the operator is looking at config rather than in the
middle of a reading that came back Unknown.

### Enrollment and rotation

`pns home enroll`, the sibling of `pns home` the way `pns lights enroll` sits beside the lamp commands.
It takes no pin, because it is the command that produces one. It writes nothing.

1. Open a TLS connection to `router_url` with a capture-only verifier that accepts anything and records
   the leaf certificate. This is a SEPARATE verifier type from the pinning one, not the pinning one
   with a flag, so no boolean exists that could turn verification off in production.
2. Print the address, the certificate subject and issuer, the validity dates, the fingerprint, and the
   line to paste: `certificate = "sha256:<hex>"` (example only, and obviously fake:
   `sha256:0000000000000000000000000000000000000000000000000000000000000000`).
3. Print, beside it, the one thing the operator has to judge: this is trust on first use, so the value
   is only as good as the network at the moment it was taken. The report says so in one line rather
   than implying the command verified anything.
4. Exit non-zero and print no pastable line when the connection cannot be made at all, so a typo in
   `router_url` cannot be enrolled as "no certificate".

There is no equivalent of the Hue design's bridge-id cross-check. The Hue bridge names itself in an
unauthenticated endpoint whose answer can be compared against the certificate's common name; the UniFi
console's certificate is issued to a generic name and its console identity endpoints sit behind the
credential we are trying to protect, so there is nothing independent to compare. The out-of-band option
is real but manual: the operator can read the fingerprint off the console's own web interface in a
browser and compare it to the printed one. The design recommends doing that once, at enrollment, and
says why in the printed output.

Rotation is the same three steps as enrollment: run `pns home enroll`, compare, paste, apply. The
mismatch report (below) names those steps, so the path out of a refusal is in the message that
announces it rather than in a runbook.

### What happens when the pin does not match

Loud, permanent, and never a fallback. There is no configuration value, environment variable or flag
that accepts a mismatched certificate: recovery is re-enrollment, always.

- The handshake is refused inside `connect`, before any request is written, so the API key is never
  sent to the impostor. Completing the handshake inside `connect` is what makes this true and it is the
  one mechanical requirement the Hue design also records.
- `Router::clients` answers `None`, which the existing reading path already renders as Unknown. The
  `Router` trait (`pns-application/src/ports/devices.rs`, exported at
  `pns/crates/pns-application/src/lib.rs:44`) keeps its shape, so no caller learns a new error type.
- `pns home` prints the mismatch as its own cause line, beside the unconfigured causes it already
  enumerates: the address, the expected fingerprint, the presented fingerprint, and the enroll command.
  That command is a diagnostic whose whole job is to answer "why did the probe not read", so a wrong
  certificate has to be one of the answers it gives.
- The first mismatch in a process is reported once through the delivery-failure path designed on
  2026-09-08 as a PERMANENT failure, because a wrong certificate does not heal on retry. Later
  mismatches in the same process are counted and not re-reported.
- `pns doctor` reports the pin state as its own line: enrolled and matching, enrolled and MISMATCHED
  with both fingerprints, or not enrolled with the enroll command.
- An unreachable router is unchanged. No certificate is presented, no mismatch is recorded, no report
  is raised, and the reading is Unknown exactly as it is today.

### What this tool is, and where the pin may not live

`pns` owns this client, and everything this design adds stays inside the pns workspace: the value type
in `pns-domain`, the verifier and the client in `pns-adapters`, the command in `pns`. Nothing is read
from `.chezmoidata`, nothing assumes this repository exists, and no path outside the pns workspace is
named in the source. Somebody who runs `cargo install --git https://github.com/webdavis/dotfiles pns`
gets the pinning, the refusal and the enroll command, and supplies their own pin through their own
config file.

Two houses the pin may NOT have, for that reason. It may not live in a chezmoi data file that the code
reads, because a tool that reads this repository's data is a tool only this repository can run. And it
may not live in a crate shared with `posture`, `uu` or `lights`, because no workspace may depend on
another (operator ruling 2026-09-10) and `uu` and `posture` may not name an engine at all (2026-09-14).
The KeePassXC reference is not a counterexample: the vault read happens in a chezmoi template at apply
time, and what reaches the binary is a plain string in a config file, which is the same thing
`api_key` already does.

`lights` is unaffected. It talks to the Hue bridge and has no router client, so there is nothing to
duplicate here.

### Behaviors to pin test-first

Behavior sentences, in the repository's behavior-not-task decomposition. The existing loopback
`RouterStub` (`pns/crates/pns/tests/support/router.rs`) serves plain HTTP, so the transport cases need
the TLS fixture the hue transport tests already own (`hue/transport_tests/server.rs`, with its
committed certificate and key pair) pointed at a router listing instead.

1. A clients read through a router pinned to the fixture's certificate returns the fixture's clients.
2. A clients read through a router pinned to a DIFFERENT fingerprint returns nothing, and the fixture
   records that no request ever arrived, which is what proves the key was never sent.
3. A mismatch records the expected and the presented fingerprint once, whatever the number of calls.
4. `[plugins.router] enabled = true` with no `certificate` is a config refusal naming the key and the
   enroll command.
5. A `certificate` value that is not `sha256:` plus 64 hexadecimal characters is refused by name,
   quoting what was written.
6. The key roster accepts `certificate` inside `plugins.router` and still refuses an unknown sibling.
7. `pns home` on a mismatch prints the mismatch cause line with both fingerprints and the enroll
   command, and prints no API key.
8. `pns home enroll` prints the pastable line for the certificate the fixture presents, and exits
   non-zero with no pastable line when nothing answers.
9. An unreachable router still reads Unknown and raises no mismatch report.
10. The rendered config template still matches `just pns-config-render` byte for byte after the new key
    exists.

Case 2 is the negative case the whole design rests on, and its assertion is the fixture's request log
rather than the client's return value: "returned nothing" is also what a timeout looks like.

The suite's standing rules apply. Every test under a second, nothing that tests ureq's or rustls's own
behavior, and generous fixture budgets, because a TLS handshake has lost a 150 ms race on this machine
before.

## What this costs

Stated plainly, because the operator's standing rule judges a design on robustness rather than on how
little work it is.

- **A firmware upgrade can break `pns home` until the operator re-pins.** A UniFi OS upgrade that
  regenerates the console certificate makes every call refuse. `pns home` reads Unknown, the presence
  narrowing's away gate stays where it already is (dormant, answering Unknown), and the stale-identifier
  alert that reading can raise does not fire. Recovery is `pns home enroll`, one paste, one apply. The
  refusal says so, and `pns doctor` says so, so the failure is visible on the next run of either rather
  than at 3am; the cost is real and it is bounded by the operator noticing.
- **A fresh machine cannot read the home probe until it is enrolled.** Fail-closed means a config with
  `[plugins.router]` enabled and no pin refuses at parse. The refusal names the command.
- **One more value the operator maintains**, in the vault, for the life of the console.
- **The verifier is written against ureq's `unversioned` API**, which ureq documents as outside its
  compatibility promises, so a ureq or rustls bump can break it at compile time. Shared with the Hue
  work rather than added by this one, and the reason approach C is worth revisiting if that breakage
  turns out to be frequent.

## Sequencing

This design should be built AFTER the Hue pinning work, not before. Three of its parts are the Hue
work's parts: the `CertificatePin` value type, the pinned rustls verifier, and the connector that
completes the handshake inside `connect`. Building them here first would mean the Hue pull request
ladder either reuses code written for a device it was not measured against, or writes its own second
copy inside the same crate.

The pull request shape once those exist, two of them, each independently green:

1. The config key, its refusal, the layout, the wizard, the regenerated template, and
   `pns home enroll`. No verification change, so the operator can enroll at leisure.
2. The pinned agent in `UniFiRouter::new`, the mismatch cause line in `pns home`, the doctor line, and
   the single permanent failure report. This is the behavior change, and the operator's apply has to
   follow it promptly, because a binary that pins and a config with no pin refuse to read.

## Out of scope

- The Hue bridge and `lights`, both covered by the 2026-09-14 design.
- Any other UniFi endpoint. This client calls two paths and no design here widens that.
- Making the router's address self-verifying, console discovery, and replacing the address with a name.
  Approach C would need the name; approach A does not.
- Certificate revocation checking and online certificate status protocol stapling, which do not apply
  to a self-signed device certificate with no distribution point.
- Any change to the `Router` port's shape or to the reading path's Unknown semantics.
- Rotating the router API key itself, which is a separate credential question.

## Assumptions made in the operator's place

1. **Approach A, a certificate fingerprint pin.** Alternative: C, a private certificate authority plus
   a real name for the console, which survives a certificate renewal with no change in pns and stays
   inside ureq's public API, at the cost of an authority key to protect and an upload a major firmware
   version can revert.
2. **The pin is the SHA-256 of the whole leaf certificate**, matching the Hue design. Alternative: the
   public key hash, which needs `rustls-webpki` and buys nothing against UniFi's regeneration behavior.
3. **The pin lives in the vault, as an attribute on the existing UniFi entry.** This follows the
   operator's 2026-09-15 ruling for the Hue pin rather than this document's own reading of what a
   fingerprint is. Alternative: a literal in `dot_config/pns/config-values.toml`, which is what its
   public-data nature would otherwise argue for.
4. **Fail closed at config parse on a missing pin.** Alternative: one release that warns and reads
   unverified.
5. **`pns home enroll` is the command name**, mirroring `pns lights enroll`. Alternative: a flag on
   `pns home`, which makes a diagnostic sometimes change things.
6. **Enrollment prints and never writes.** Alternative: writing the deployed config, which the next
   chezmoi apply would revert.
7. **A mismatch goes through the 2026-09-08 delivery-failure path, once per process**, plus a cause
   line in `pns home`. Alternative: a posture security page, which is louder and couples pns's sensor
   path to posture's producer path.
8. **No out-of-band identity check at enrollment.** There is nothing independent to compare against, so
   the design prints the trust-on-first-use caveat and recommends a one-time browser comparison instead.
   Alternative: requiring the operator to pass a fingerprint they read off the console's web interface,
   which closes the enrollment window and adds a step they can get wrong.
9. **This is built after the Hue wave** and reuses its value type and verifier. Alternative: build it
   first and have the Hue work reuse this one.
10. **One router.** The config holds one address, so the pin is one value.

## Open questions

1. Approve approach A, or C? C is the only one that survives a certificate renewal without an operator
   step, and the console supports the upload that makes it possible, which the Hue bridge did not. It
   costs a private certificate authority on this machine and a name for the console.
2. The pin in the vault as an attribute on "UniFi :: API Key (dresden-udr)", confirming the Hue
   ruling applies here, or a committed literal on the grounds that a fingerprint is public data?
3. Is a one-time browser comparison of the fingerprint at enrollment acceptable as the answer to trust
   on first use, or should enrollment require the fingerprint as an argument?
4. Does a mismatch warrant more than one permanent delivery-failure report and a cause line, given that
   a wrong certificate on the home network is a security event rather than a lamp that did not flash?
5. Should the presence narrowing's away gate stay dormant after this lands? It is dormant for cost
   reasons today (`presence_runtime.rs:154`), and pinning does not change that cost, but a verified
   router reading is worth more than an unverified one and the question has not been asked since.

## Verification, when it is built

- `just lint-check`, `just test-rust` and `just ship`, plus the ten behaviors above.
- The measurement this design could not take: the live certificate's subject, issuer, validity,
  subjectAltName entries and fingerprint, read by the operator from outside any agent sandbox, before
  the transport change merges. It decides nothing about the shape and everything about whether approach
  C becomes available.
- One live read against the console with the enrolled pin, showing a real clients listing.
- One live read with a deliberately wrong pin, showing the refusal, the Unknown reading, the cause line
  and exactly one failure report.
- One run with the router unplugged, showing the unchanged unreachable behavior and no mismatch report.
- `pns doctor` in all three pin states.
- `just pns-config-render` clean, and a quiet no-op diff after the operator applies.
