# Hue bridge certificate and identity pinning

Status: design, written 2026-09-14 by an agent with the operator asleep. NOT approved, NOT built. No
code was written or changed by the task that produced this document. Every choice made in the
operator's place is listed under "Assumptions made in the operator's place" with the alternative, and
the decisions that are genuinely theirs are at the end under "Open questions".

## Why this exists

The work ledger carries this under "pns validation follow-ups":

> Preserve the pns refactor plan's explicitly carried-forward behavior work (section 7). B1 needs a
> reviewed Hue bridge certificate/identity-pinning design; `pns/crates/pns-adapters/src/hue/bridge.rs`
> still disables certificate verification. Define enrollment, changed-certificate handling and recovery
> before changing that behavior.

The refactor plan's own entry for B1 (section 7 of
`docs/superpowers/plans/2026-09-05-pns-refactor-plan.md`) says the same thing from the other side: "a
decision the hue adapter PR 10.1 records; the code change is new behavior after the ladder unless the
operator rules otherwise." Step 10 of that plan moved the transport and pinned its current shape in
tests, and recorded that "certificate verification remains disabled as before. The separate B1
certificate-pinning behavior remains outside this move."

So the code is where it was, the tests currently pin the unverified behavior, and what is missing is a
decision.

Three standing rulings shape the answer more than anything technical does.

- **Optimal over cheap** (operator, 2026-09-05). The criterion is result quality, never how much code a
  design costs. "More code and harder" is not a mark against a design. This matters here because the
  cheapest honest option is to write down the risk and keep the current behavior, and the rule says
  that option may not win on effort.
- **No manual intervention designs** (operator, 2026-07-11). A design that needs the operator to
  remember a recurring step is unfinished. One-time, operator-owned actions are allowed when they are
  delivered as an exact command at the moment they are needed. Enrollment is exactly that shape;
  silence on a mismatch is not allowed at all, because "alerts replace silence, never replace
  automation".
- **Rust tools are shippable products** and **no workspace may depend on another**. pns and lights are
  two separate products that both talk to this one bridge. Any shared verifier code is DUPLICATED
  deliberately, the way `uu-adapters` carries its own signed-POST client.

## What is already built

Three call sites in this repository disable TLS (transport layer security) certificate verification:

- `pns/crates/pns-adapters/src/hue/bridge.rs:151`, in `UreqBridge::agent()`. The comment there says
  verification is disabled "exactly as openhue does it; there is no CA that could vouch for a Hue
  bridge". The measurements below show the second half of that sentence is false: a certificate
  authority does vouch for it. What is true is that the certificate cannot be verified by NAME.
- `lights/crates/lights-adapters/src/hue.rs:62`, the same bridge, the same key, a different product.
  Its comment is already honest: "Approved bridge-specific certificate exception. This disables server
  authentication; it does not establish that certificate verification is impossible for Hue."
- `pns/crates/pns-adapters/src/unifi/client.rs:45`, the UniFi router client, which quotes the hue
  comment as its precedent. Out of scope here, filed as a follow-up below.

pns's hue transport today:

- `UreqBridge { base, key, deadline }` is built as a bare struct literal in SEVEN places:
  `pns/crates/pns/src/lamp_pulse.rs` (twice), `command_presence.rs`, `lamp_event_lease.rs`,
  `lights_tick_runtime.rs`, plus `pns-adapters/src/hue/inventory.rs` and
  `pns-adapters/src/doctor/bridge.rs`. Each one formats `https://{bridge}/clip/v2/resource` itself.
- The `Bridge` trait is deliberately lossy: `get` returns `Option<String>`, `put` returns nothing at
  all, because "a pulse that did not land is not worth failing, reporting or retrying on a notification
  path".
- Two deadlines exist: `BRIDGE_DEADLINE` (10 seconds, unattended) and `TYPED_COMMAND_DEADLINE` (1
  second, a human waiting at a terminal).
- There is already a real TLS fixture: `hue/transport_tests/server.rs` serves a committed
  `cert.der`/`key.der` pair (subject `CN=pns-private-fixture.invalid`, DNS SAN
  `pns-private-fixture.invalid`) over rustls with the ring provider, and the four transport tests
  connect to it at `https://127.0.0.1:<port>`. rustls 0.23.43 is already a dev-dependency of
  `pns-adapters` for that fixture, and a transitive dependency of ureq in the normal build.
- Config: `[plugins.hue]` carries `enabled`, `bridge`, `key`, `rooms`, `quiet_hours`. The key roster is
  `pns/crates/pns-adapters/src/config/schema.rs:114`, the rendered layout is `PLUGINS_HUE` in
  `pns/crates/pns-adapters/src/config/render/layout/destinations.rs:92`, the first-run wizard writes the
  table in `pns/crates/pns-adapters/src/config/setup.rs`, and the committed input is
  `dot_config/pns/config-values.toml`, where `bridge` and `key` are KeePassXC references to the entry
  "OpenHue :: API Key (hue-bridge-pro)".
- `quiet_hours` sets the house grammar for a bad setting: a value that is not a window is a REFUSAL
  naming the key and what it cost, not a silent fallback.

## What the bridge actually serves, measured

Measured on dresden on 2026-09-13 against the live bridge at 192.168.4.37, which is the address the
deployed `~/.config/openhue/config.yaml` holds.

Unauthenticated, from `https://192.168.4.37/api/config`:

```
name "Hue Bridge Pro", bridgeid "C42996FFFECB6DCC", modelid "BSB003",
apiversion "1.78.0", swversion "2071476020", factorynew false
```

The certificate it presents, captured with Python's `ssl` module and parsed with `openssl x509`:

```
Subject:  C=NL, O=Philips Hue, CN=C42996FFFECB6DCC, OU=BSB003
Issuer:   C=NL, O=Philips Hue, CN=root-bridge
Validity: Not Before 2025-09-02, Not After 2038-01-19
Key:      id-ecPublicKey, prime256v1 (P-256), signature ecdsa-with-SHA256
X509v3:   Basic Constraints critical CA:FALSE, Key Usage critical Digital Signature,
          Extended Key Usage TLS Web Server Authentication,
          Authority Key Identifier keyid 67:63:8D:4C:...:71:C5:1A:64
NO subjectAltName extension at all
SHA-256 of the certificate (DER): 90ab633b3a5607129abe4fa4c94811ee0f0d05c5ad758837890f0fd61162120f
SHA-256 of the public key (SubjectPublicKeyInfo, base64): whXloExDqOwKXASEgjiM0ylbKVqHbZcTVkAyqrojMo4=
```

Five facts follow, and each one moves the design.

1. **The certificate's only identity is its common name, and that name is the bridge id.** The
   certificate CN is `C42996FFFECB6DCC` and `/api/config` reports `bridgeid` as the same string. The
   model appears twice as well, as `OU=BSB003` and as `modelid`.
2. **There is no subjectAltName.** Every modern verifier, rustls and webpki included, ignores the common
   name entirely and requires a subjectAltName to match a host. So NAME verification against this
   certificate is impossible, whatever trust anchor is configured. A web search corroborates that this
   is the long-standing state of the Hue local application programming interface and that library
   authors hit it repeatedly.
3. **Chain verification, by contrast, works today.** The published Philips Hue root certificate
   (self-signed, `C=NL, O=Philips Hue, CN=root-bridge`, valid 2017-01-01 to 2038-01-19, SHA-256
   `F0:BD:8E:65:09:E8:2F:77:4D:63:BC:00:9D:53:88:C9:69:FE:3D:CF:7D:6D:54:1D:63:51:B7:2B:89:8D:8A:CF`)
   verifies this bridge's leaf: `openssl verify -CAfile hue-root.pem <leaf>` answered OK, and the
   leaf's authority key identifier equals the root's subject key identifier. The copy used for that
   check came from a third-party GitHub mirror, not from Signify, which is a provenance question that
   only matters if the operator picks approach B below.
4. **The bridge sends the leaf alone**, no intermediate, so any chain-building trust anchor has to come
   from our side.
5. **The certificate is effectively permanent.** Not After is 2038-01-19, which is the 32-bit time
   limit, so nothing here expires within the life of the hardware. From training, not verified: a Hue
   bridge certificate is provisioned once per device and is not rotated in the field, and a factory
   reset does not regenerate it. The recovery path below is designed so that being wrong about this
   costs one refusal and one enrollment command, not a broken system.

## What blocks the obvious fix

The obvious fix is "trust the Hue root and keep verifying", which ureq can almost express:
`TlsConfig::builder().root_certs(RootCerts::Specific(vec![hue_root]))`. It does not work, and the
reason is worth recording so nobody proposes it again.

- ureq 3.4.0 (and 3.4.2, read to confirm nothing was added) exposes exactly five TLS knobs: `provider`,
  `client_cert`, `root_certs`, `use_sni`, `disable_verification`, plus an unstable rustls crypto
  provider. There is NO hook for a custom `ServerCertVerifier`, and `disable_verification` is all or
  nothing.
- With the Hue root as the only trust anchor, rustls would still verify the server name taken from the
  request URI's authority, which is an internet protocol address literal, against a certificate with no
  subjectAltName. That fails.
- Pointing the URI at the bridge id as a hostname and mapping it to the address with a custom resolver
  does not help either, for the same reason: webpki does not read the common name.
- ureq's native-tls path would have the right primitive, since native-tls separates
  `danger_accept_invalid_certs` from `danger_accept_invalid_hostnames`, but ureq sets BOTH from its one
  `disable_verification` flag (`src/tls/native_tls.rs:128`), so the separation is not reachable.

Therefore any form of verification here needs a custom rustls `ServerCertVerifier`, and reaching one
means supplying ureq a custom connector through `Agent::with_parts`, which is the seam
`lights-adapters` already uses for its transport tests.

### The mechanism, prototyped and measured

A throwaway prototype (scratchpad only, not in the repository) built the connector out of ureq's public
`ureq::unversioned::transport` items and rustls 0.23.43, and it works:

- Everything needed is public: `Connector`, `Transport`, `Either`, `TcpConnector`, `Connector::chain`,
  `TransportAdapter::{new, set_timeout, get_mut}`, `LazyBuffers::new`, `Config::input_buffer_size`,
  `Config::output_buffer_size`, and `Agent::with_parts`. The whole custom piece is one verifier, one
  connector and one transport, and the transport is a 30-line copy of ureq's own.
- Against a loopback TLS fixture with the matching pin, `GET /` returned HTTP 200.
- With one bit flipped in the pin, the handshake was refused and OUR OWN message came back out through
  ureq intact: `pin mismatch: b8cea16f...`. The refusal text reaching the caller verbatim is what makes
  an operator-facing message possible.
- The handshake must be completed inside `connect()` (`conn.complete_io(&mut sock)` after
  `sock.set_timeout(details.timeout)`), which is what ureq 3.4.1 changed its own connector to do. With
  a lazy handshake the pin failure surfaces mid-request instead of at connect.
- Two caveats on the prototype. It hashed the whole certificate rather than the public key, which is
  the pin shape recommended below. And it could not reach the live bridge: the agent's shell sandbox
  blocks this binary's sockets, and the control case (stock ureq with verification disabled, exactly
  what pns ships) failed the same way, so nothing about the live LAN path was measured here. A live
  handshake against 192.168.4.37 is an acceptance gate, not a settled fact.
- `ureq::unversioned` is documented by ureq as "not covered by the promises of semver (yet)", and the
  verifier is written against rustls 0.23 types that ureq re-exports only implicitly. A ureq or rustls
  bump can therefore break this code at compile time. That is a real cost and it is priced below.

## Three approaches

### A. Pin the bridge's own certificate (recommended)

Record, at enrollment, the SHA-256 of the certificate the bridge presents. A custom verifier compares
the presented leaf against that pin and accepts nothing else. No trust anchor, no chain math, no name
check, no parser.

- Authenticates exactly one physical device. An attacker holding any other Hue-signed certificate, from
  their own bridge, is refused.
- Independent of Signify's certificate authority: a compromised or coerced Hue root cannot mint a
  certificate this verifier accepts.
- Smallest verifier (roughly 40 lines including the two signature callbacks, which delegate to the
  crypto provider), no new crate in the dependency graph beyond promoting rustls from dev-dependency to
  dependency.
- Cost: one enrolled value per bridge, and a legitimate certificate change (hardware replacement, or a
  field re-key if Hue ever does one) is a refusal until the operator re-enrolls. A hardware replacement
  already forces re-enrollment of the application key, so the two land together.

### B. Trust the Hue root and check the identity

Ship the Philips Hue root certificate, verify the chain with rustls-webpki, then compare the leaf's
subject to the enrolled identity. The comparison can be exact bytes: rustls-webpki's
`EndEntityCert::subject()` hands back the subject distinguished name, so no common-name parser is
needed.

- Survives a re-issue of this bridge's certificate under a new key, which A does not.
- Costs a direct dependency on `rustls-webpki` 0.103 (pre-1.0, already in the tree transitively), more
  verifier code, and a trust anchor whose copy came from a third-party mirror. The anchor can be
  cross-checked: this bridge's real certificate verifies against it, which proves it is the genuine
  issuer of the certificate we hold.
- Weaker in the case that matters most on a local network. Dropping the identity check (chain only)
  would need no enrollment at all, and would be defeated by anyone who owns a Hue bridge of their own
  and can answer for the bridge's address, which is a fifty-dollar attack.
- Adds Signify's certificate authority to this machine's trust surface for no gain that A does not
  already give.

### C. Keep the current behavior and record the risk

Close B1 as a documented acceptance: keep `disable_verification(true)`, write the threat model into a
runbook, and spend nothing. What is at risk is the Hue application key (control of the lamps) and the
truth of the room and presence readings pns takes from the bridge. The attack needs a position on the
home network and address-resolution spoofing or equivalent.

This is the option the "optimal over cheap" ruling exists to stop from winning on effort. It is listed
because the operator may still judge the exposure as acceptable for a notification decoration, and
because a fourth option that looks like security is worse than this one: comparing `bridgeid` from
`/api/config` after connecting proves nothing against an active attacker, who can simply relay the real
bridge's answer. If the operator picks C, the code comment in `bridge.rs` still has to be corrected,
because it currently claims no authority can vouch for the bridge, and that claim is false.

**Recommendation: A.** It is the strongest authentication of the three, the smallest verifier, and the
only one that needs no third-party trust anchor. B's one advantage covers an event that does not happen
to this device class, and it pays for it with a pre-1.0 dependency and someone else's certificate
authority.

## The recommended design in detail

### The pin as a value

A new pure type in `pns-domain`, beside `parse_window`, because the config layer must be able to refuse
a malformed one before any transport exists:

- Wire form: `"sha256:<64 lowercase hex characters>"`. The algorithm prefix is there so a future
  algorithm is a new prefix rather than an ambiguous length check.
- `parse` returns `Option<CertificatePin>` (or a `Result` carrying the refusal text, matching whichever
  shape `parse_window`'s caller uses at the time of writing).
- `Display` writes the wire form back, so enrollment output and config are literally the same string.
- Comparison is over the 32 raw bytes, never over strings, so case and whitespace cannot make two pins
  differ.

### Where the pin lives

`[plugins.hue] certificate = "sha256:..."`, one new key, added to the roster in `config/schema.rs`, to
`PLUGINS_HUE` in the render layout with its own prose, to the wizard in `config/setup.rs`, and to
`dot_config/pns/config-values.toml` as a literal (not a KeePassXC reference: a certificate fingerprint
is a public hash, not a secret, and the five secret-bearing keys the render generator guards stay
five). The shipped template is then regenerated with `just pns-config-render`, which the byte-equality
test in `just test-rust` enforces.

When `[plugins.hue] enabled = true` and `certificate` is missing or empty, config load REFUSES by name,
in the `quiet_hours` grammar, and the refusal carries the enrollment command:

```
pns: config error (plugins.hue.certificate is unset); run `pns lights enroll` and paste the line it
prints; no pulse
```

That is a fail-closed choice and it is deliberate: a pin that can be omitted is a pin that will be
omitted, and the refusal happens at config parse, which is the visible place, not at the moment a lamp
fails to flash.

### One constructor for the transport

`UreqBridge` gains a `pin: CertificatePin` field, and the six struct literals collapse into one
constructor, `UreqBridge::new(&HueSettings, Duration)`, which formats the base URL as well. Six places
currently repeat `format!("https://{}/clip/v2/resource", hue.bridge)`; after this change exactly one
does. `HueSettings` gains the pin beside `bridge` and `key`, so `hue_settings` is the single place that
decides whether a bridge is configured at all, as it already does for the other two.

### The pinned transport

A new module, `pns/crates/pns-adapters/src/hue/pinned_tls.rs`:

- `PinnedVerifier { pin, provider, observer }` implements `rustls::client::danger::ServerCertVerifier`.
  `verify_server_cert` hashes the presented leaf and compares; the two signature methods delegate to
  `rustls::crypto::verify_tls12_signature` and `verify_tls13_signature` with the provider's algorithms,
  so the handshake's own cryptography is still checked by rustls rather than waved through.
  `supported_verify_schemes` comes from the provider.
- On mismatch it returns `rustls::Error::General` with a message naming the bridge address, the expected
  pin and the presented one, and it records the same into the observer (see reporting below).
- `PinnedTlsConnector` implements `Connector<In>`, completes the handshake in `connect`, and returns a
  `PinnedTlsTransport` that is a copy of ureq's `RustlsTransport` (ureq's own type has private fields
  and no constructor, so it cannot be reused).
- The agent is assembled once per `UreqBridge::agent()` call as today:
  `TcpConnector::default().chain(PinnedTlsConnector { .. })` handed to `Agent::with_parts` with
  `DefaultResolver::default()` and a `Config` carrying the existing `timeout_global` and
  `max_redirects(0)`.
- Server name indication is set to a fixed placeholder and the name is never verified. The prototype
  suggests sending no indication at all matches what curl does against an address literal; whether the
  bridge cares is an acceptance check, not a design question.

### Enrollment

`pns lights enroll` (the `lights` word already carries `tick` and `quiet`, and the bridge is the lamps'
bridge). It takes no pin and needs none, because it is the command that produces one:

1. Read `https://<bridge>/api/config` and keep `bridgeid` and `modelid`. This path needs no application
   key.
2. Open a TLS connection with a capture-only verifier that accepts anything and records the leaf.
3. Print, in one block: the address, the certificate's common name, the bridge id from step 1, whether
   the two agree, the model, the validity dates, and the line to paste:
   `certificate = "sha256:<hex>"`.
4. Exit non-zero, printing a refusal rather than a pastable line, when the common name and the bridge id
   disagree. That disagreement is the signature of an impostor answering for the address, and it is the
   one moment in the whole design where an attacker can win, so it is the one moment that gets a hard
   refusal rather than a warning.
5. Print nothing that a `pns doctor` run would not also print, so the two stay one implementation.

Enrollment writes NOTHING. The operator pastes the line into `dot_config/pns/config-values.toml` (or the
deployed config on a machine that is not chezmoi-managed) and applies. A command that edited the
deployed `~/.config/pns/config.toml` would be overwritten by the next chezmoi apply, which is exactly
the trap `dot_config/backpass/config.json` exists to avoid for the global rules.

`pns doctor` reports the pin state as a line of its own: enrolled and matching, enrolled and
MISMATCHED with both fingerprints, or not enrolled with the enroll command. The doctor already builds a
bridge through `doctor_bridge`, so this is one more reading beside the ones it takes.

### A changed certificate, and recovery

A mismatch is not a transport failure and must not look like one.

- Every call through the pinned agent fails at the handshake. `get` answers `None` and `put` stays fire
  and forget, so the `Bridge` trait keeps its shape and no caller learns to handle a new error. That
  preserves the existing contract and it is the reason the observer below exists.
- The verifier records the mismatch (expected pin, presented pin, address, wall-clock time) into a
  process-scoped observer that the caller reads after the call. The FIRST mismatch a process sees is
  reported once through the delivery-failure reporting path designed on 2026-09-08
  (`docs/superpowers/specs/2026-09-08-pns-delivery-failure-reporting-design.md`), as a permanent
  failure, because a wrong certificate does not heal by retrying. Later mismatches in the same process
  are counted and not re-reported.
- The failure's fix line names the exact recovery: run `pns lights enroll`, compare the printed bridge
  id and common name to the ones in the report, and paste the new line only if they are the bridge you
  expect. Recovery is therefore one command plus one paste plus an apply, and it is the same path
  enrollment took the first time.
- A bridge that is simply unreachable is unchanged: no pin is presented, no mismatch is recorded, and the
  existing "bridge answered nothing" behavior stands (`bridge_inventory` still falls back to the
  declared names, `clear_held` still writes off the held paths).
- Nothing auto-accepts a new certificate, ever. Trust on first use with no confirmation would make the
  pin decorative.

### Failure modes

- `enabled = true` and `certificate` unset or empty: config refusal by name at parse, naming the
  enrollment command.
- `certificate` malformed (bad prefix, wrong length, non-hex): config refusal by name at parse, quoting
  what was written.
- Pin matches: unchanged behavior, one handshake per agent, the same two deadlines.
- Pin does not match: handshake refused at connect, `get` returns `None`, `put` writes nothing, ONE
  permanent failure report naming both fingerprints, and `pns doctor` shows MISMATCHED.
- Bridge unreachable or slow: unchanged. The deadline expires, no mismatch is recorded, no report is
  raised.
- Bridge replaced: a mismatch, as above, then `pns lights enroll` and one paste.
- Enrollment against an impostor: the common name and the `/api/config` bridge id disagree, so
  enrollment refuses and prints no pastable line.
- `[plugins.hue]` absent or disabled: nothing changes, no pin is read, no transport is built.

### What this defeats, and what it does not

Defeated: any party answering for the bridge's address who cannot present the bridge's exact
certificate. That covers address-resolution spoofing, a rogue device that took the address by dynamic
host configuration protocol lease, and an attacker with a certificate signed by the Hue root for a
different bridge.

Not defeated: an attacker who is already in position AT ENROLLMENT TIME and controls both the TLS
connection and the `/api/config` answer. Step 4 raises the cost (they must fake a consistent bridge id
in two places) but does not close it. Closing it would need an out-of-band identity, for example
reading the bridge id off the device's own label and passing it to enrollment as an argument. That is a
cheap addition (`pns lights enroll --bridge-id C42996FFFECB6DCC`) and it is left as an open question
rather than assumed, because it adds a step the operator must not get wrong.

Also not defeated, and not in scope: anything reachable through the lamps themselves, and the fact that
the Hue application key remains a bearer credential in a header. Pinning protects it in transit; it
does not change what it grants.

### Behaviors to pin test-first

Ten, each one a sentence a test name can carry, in the repository's behavior-not-task decomposition.
The existing fixture server and its committed `cert.der` cover the first three with no new fixture:

1. A GET through a bridge pinned to the fixture's certificate returns the fixture's body.
2. A GET through a bridge pinned to a DIFFERENT fingerprint returns nothing, and the fixture records
   that no request ever arrived (the refusal is at the handshake, not after the request).
3. A PUT through a mismatched pin writes nothing and does not fail its caller.
4. A mismatch records the presented fingerprint and the expected one, once, whatever the number of
   calls.
5. A pin parses from its wire form, and round-trips through `Display` to the same bytes.
6. A pin that is not `sha256:` plus 64 hex characters is refused by name, quoting the offending value.
7. `[plugins.hue] enabled = true` with no `certificate` is a config refusal naming the key and the
   enroll command.
8. The key roster accepts `certificate` inside `plugins.hue` and still refuses an unknown sibling.
9. `pns lights enroll` prints the pastable line when the certificate common name equals the bridge id
   the same host reports, and refuses with a non-zero exit when it does not (both over the fixture, with
   the `/api/config` answer scripted).
10. The rendered config template still matches `just pns-config-render`'s output byte for byte after the
    new key is added.

The suite's standing rules apply: every test under a second, no test of ureq's or rustls's own
behavior, and the fixture budgets stay generous (the machine runs 700 other tests, and a TLS handshake
has lost a 150 ms race here before).

### Module boundaries and sizes

- `pns-domain`: the `CertificatePin` value type, parse, display, compare. Pure, no rustls, no input or
  output. Roughly 60 lines with tests.
- `pns-adapters/src/hue/pinned_tls.rs`: verifier, connector, transport. Roughly 140 lines, which is
  under the 300-line ideal and well under the 500-line cap. It is the only file in the repository that
  touches rustls in a non-test build.
- `pns-adapters/src/hue/bridge.rs`: `UreqBridge` gains one field and one constructor, and its `agent()`
  swaps `disable_verification(true)` for the pinned connector. The file is already near its comfortable
  size, so the connector does NOT go in it.
- `pns-adapters/src/hue/enroll.rs`: the capture-only verifier and the enrollment report. The capture
  verifier is a second `ServerCertVerifier` and it is NOT the pinned one with a flag: a verifier that
  can be told to accept anything is one boolean away from disabling the whole feature.
- `pns-adapters/Cargo.toml`: rustls moves from `[dev-dependencies]` to `[dependencies]` at the same
  version, features unchanged (`ring`, `std`, `tls12`), so the dependency graph gains nothing new.
- `pns/crates/pns/src/*`: five of the seven call sites are here; they lose their struct literals and
  call the constructor. The other two are `hue/inventory.rs` and `doctor/bridge.rs`.

### The pull-request shape

Four, in this order, each independently green:

1. `CertificatePin` in `pns-domain`, plus the config roster, refusal, layout, wizard and regenerated
   template. No transport change, so nothing about verification moves and the ledger's "no verification
   behavior has changed" still holds after it merges.
2. `pns lights enroll` and the doctor line, over the fixture. Still no verification change; this is what
   produces the value the operator needs BEFORE the transport starts demanding it.
3. The pinned transport, the one constructor, and the four transport-behavior tests. This is the
   behavior change, and it is the one the operator's apply has to follow immediately, because a pns
   binary that pins and a config with no pin refuse to pulse.
4. The mismatch report through the delivery-failure path.

Splitting 1 and 2 ahead of 3 is what keeps the operator from having to enroll and upgrade in the same
breath: after 2 they can enroll at leisure, and 3 turns the pin on.

## Out of scope

- **lights.** `lights/crates/lights-adapters/src/hue.rs` disables verification against the same bridge
  with the same key. It should get the same treatment, as its own design-free port of this one (the
  ruling is that each product carries its own copy), in its own pull request, after this lands and is
  accepted in practice. Its `with_transport` seam makes it the easier of the two.
- **The UniFi router client.** `pns/crates/pns-adapters/src/unifi/client.rs` disables verification
  against `https://192.168.1.1` while sending a router API key, which is a more valuable credential
  than the lamp key. It is a different device with a different certificate story and it needs its own
  measurement and design. Filed, not solved here.
- The Hue event stream, which this repository does not use.
- Bridge discovery (multicast domain name system, or Signify's discovery endpoint), replacing the
  configured address with a name, and anything that would make the address itself self-verifying.
- Automatic certificate rotation, certificate revocation checking, and online certificate status
  protocol stapling. None of them apply to a device certificate that expires in 2038 and has no
  revocation distribution point.
- Any change to the `Bridge` trait's lossiness. The fire-and-forget `put` and `Option`-returning `get`
  stay exactly as they are.

## Assumptions made in the operator's place

Each of these is a choice this document made so it could be concrete. Each says what the alternative
was.

1. **Approach A, a certificate fingerprint pin, over B, root plus identity.** Alternative: B, which
   survives a same-device re-key at the cost of a pre-1.0 `rustls-webpki` dependency and trusting
   Signify's certificate authority. If the operator prefers B, the enrollment, recovery, config and
   reporting design above is unchanged; only the verifier's body and the enrolled value (a bridge id
   rather than a fingerprint) change.
2. **The pin is the SHA-256 of the whole leaf certificate.** Alternative: the SHA-256 of the public key
   (SubjectPublicKeyInfo), which survives a re-issue that keeps the key and is the conventional pin
   shape, but needs `rustls-webpki` as a direct dependency to extract the field. The value for this
   bridge is already measured both ways, above.
3. **The pin lives in the committed `config-values.toml`, not in KeePassXC.** A fingerprint is a public
   hash of a public certificate, so the vault buys nothing, and keeping it out preserves the render
   generator's five-secret guard. Alternative: a custom attribute on the "OpenHue :: API Key
   (hue-bridge-pro)" entry, which puts the pin next to the credential it authenticates at the cost of
   another vault read at apply time and a sixth case in that guard.
4. **Fail closed: hue enabled with no pin is a config refusal.** Alternative: one release that warns and
   runs unverified, which is gentler on a fresh machine and leaves the door open for exactly as long as
   nobody notices.
5. **Enrollment prints, and never writes.** Alternative: `pns lights enroll --write`, which would edit
   the deployed config and be reverted by the next chezmoi apply, or write a separate state file under
   `~/.local/state/pns`, which would make a second source of truth beside config.
6. **A mismatch is reported through the delivery-failure path, once per process.** Alternative: a
   posture security page, which is the louder channel and the one the operator already associates with
   tampering, at the cost of coupling pns's lamp transport to posture's producer path.
7. **This change is pns only; lights follows separately.** Alternative: one design, two pull requests in
   the same wave, which halves the window in which the same key is exposed by the other product.
8. **One bridge.** The config holds one address, so the pin is one value. Alternative: a table of
   bridges keyed by address, which nothing on this machine needs.
9. **Enrollment takes no out-of-band bridge id.** Alternative: `--bridge-id <id>` read off the device's
   label, which closes the enrollment-time impostor window and adds a step the operator can get wrong.
10. **The verifier is written against ureq's `unversioned` API.** Alternative: wait for ureq to expose a
    verifier hook (nothing in its changelog through 3.4.2 suggests one is coming), or drop ureq for the
    hue transport and drive rustls directly, which is more code and a second HTTP client in one product.

## Open questions

All seven answered by the operator on 2026-09-15; recorded again in
`docs/decisions/2026-09-15-pns-behavior-backlog-brief.md` so the two documents agree.

1. Approve approach A (pin the bridge's certificate) over B (Hue root plus bridge identity) and C (keep
   the current behavior, record the risk)? **Answered: A.** The bridge certificate carries no
   subjectAltName, so B could only answer "is this some Hue bridge" rather than "is this my bridge", and
   B would also require trusting a Philips root certificate copied from a third-party mirror. A pins one
   specific device and is the more secure choice, not merely the simpler one.
2. Fail closed on a missing pin, or one release of warn-then-refuse? **Answered: fail closed, at config
   parse.** No warn-then-refuse release.
3. The pin in the committed `config-values.toml`, or as a KeePassXC attribute beside the bridge address
   and key? **Answered: the vault**, reversing this design's own recommendation. This repository is
   public (verified 2026-09-15 through the GitHub API), and its own convention already treats an id as a
   vault reference rather than a committed literal, stated in `dot_config/pns/config-values.toml`'s own
   words about channel ids. The cost is nothing, since the OpenHue entry already exists and the pin
   becomes one more attribute on it; the fingerprint is a hash and publishing it would be low harm
   regardless, but consistency with the repository's own rule and the public repository are what decided
   it.
4. Should `pns lights enroll` accept the bridge id out of band, off the device label, so an
   enrollment-time impostor is refused rather than merely made harder? **Answered: optional, with a
   warning when skipped, not required.** Without it, enrollment trusts whatever answers first, so an
   impostor present at enrollment time gets pinned and everything afterward looks correct.
5. Does lights get the same change in this wave, and is the UniFi router client's unverified TLS worth
   its own design task now (its credential is a router API key)? **Answered: yes to both.** `lights` gets
   the identical pinning change in this wave; the UniFi router client becomes its own design task.
6. Is the command name `pns lights enroll` right, or should the bridge's identity live under
   `pns doctor` and a flag? **Answered: stays `pns lights enroll`.** Doctor reports state, enrolling
   performs an action and hands back a value to save, and a diagnostic that sometimes changes things
   becomes one nobody runs.
7. If approach B is chosen after all: is a Hue root certificate copied from a third-party mirror
   acceptable as a trust anchor, given that this bridge's real certificate verifies against it?
   **Moot.** Approach A was chosen, so B's trust-anchor question does not arise.

## Verification, when it is built

- `just test-rust` and `just ship`, the usual gates, plus the ten behaviors above.
- One live handshake against 192.168.4.37 from outside the agent sandbox, with the enrolled pin, showing
  a real inventory read. The prototype could not do this and the design does not claim it.
- One live handshake with a deliberately wrong pin, showing the refusal text, the `None` from `get`, and
  exactly one failure report.
- One run with the bridge unplugged, showing the unchanged unreachable behavior and NO mismatch report.
- `pns doctor` in all three pin states.
- `just pns-config-render` clean, and `just d` quiet on a no-op apply after the operator applies.
