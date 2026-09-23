# The HTTP tap, an opt-in alternative to the SSH tap

Status: design, written 2026-09-14. NOT approved and NOT built. Task 74 in `docs/remaining-work.md`,
which records the operator's 2026-09-09 ruling that the SSH (secure shell) shape ships first and this is
offered as an upgrade the operator chooses. Task 76's Shortcuts research says in its own words that its
finding "does not authorize task 74".

The recommendation at the end of this document is to record the design and NOT build it yet, with the
conditions that would change that answer written down. Everything between here and there is the design as
it would be built if the operator says yes, in enough detail to implement test-first.

## Why this exists

Today one tap transport is shipped. An iOS Shortcut opens an SSH connection to the Mac, sshd runs a
forced command on a dedicated key, and that command is `pns tap`, which updates the mtime of one marker
file. The presence probe reads that mtime and decides whether notifications go to the desk, the phone, or
away.

The HTTP (hypertext transfer protocol) tap would replace that transport with a request. The Shortcut
would use "Get Contents of URL" instead of "Run Script Over SSH", post to a small endpoint pns serves,
and pns would record the tap itself. The claim in the task text is that pns then owns both ends, so there
is no `authorized_keys` line, no forced command, and no second system holding a copy of anything.

### What is already fixed, and what the HTTP tap would actually buy

Task 71 shipped in PR #537 and changed the premise of this task. The forced command is now
`command="<binary> tap",restrict` and names no marker path, so the marker path is written down once, in
pns, and the silent mismatch the section intro describes (two systems holding one path, nothing checking
they agree) is gone. `pns tap --install` prints the line, pns never reads or writes
`~/.ssh/authorized_keys`, and `pns doctor` carries a Pairing row reporting the marker's own freshness
(`crates/pns/src/presence_runtime.rs:phone_tap_status`).

So the decoupling argument is mostly spent. What is left for the HTTP tap to buy, stated honestly:

1. No `authorized_keys` paste. One manual step, once per machine, ever.
1. No Remote Login. Task 71a made sshd accepting connections a prerequisite of the whole feature, and an
   operator who does not want Remote Login on at all currently cannot have a tap.
1. A phone-readable result without changing the Mac. Over SSH the JSON (JavaScript object notation) form
   arrives only if the forced command is respelled `pns tap --json`, which is a Mac-side edit; over HTTP
   the response body is whatever the endpoint returns, so the Shortcut can branch on it with no second
   machine visit.
1. One less thing outside chezmoi. The `authorized_keys` line is hand-managed on this machine.

## Constraints this design starts from

**It assumes nothing about the operator's network** (operator ruling 2026-09-09, correcting an earlier
draft of the task that said "on the tailnet"). No Tailscale, no virtual private network, no local network
shape, nothing detected and nothing guessed. Task 75 states the same boundary from the other side:
restricting this Mac's SSH exposure "belongs to dotfiles; pns remains network-independent".

**The listener is off unless configured, and its bind address has no default.** Loopback is safe and a
phone cannot reach it; every other address is a guess about somebody's network. The operator writes the
address.

**Authentication is a config secret**, the way `[plugins.hermes] key` already is: a value in
`~/.config/pns/config.toml`, which on this machine is a chezmoi-rendered target whose secrets come out of
KeePassXC at apply time (`dot_config/pns/config-values.toml`).

**pns is a product other people install** with `cargo install --git`. Nothing here may assume this
repository exists, and the security posture has to be defensible to somebody who has never seen it.

**The config file is generated.** `dot_config/pns/private_config.toml.tmpl` is the renderer's output over
the committed values file, so a new table means a row in `TABLE_KEYS`
(`crates/pns-adapters/src/config/schema.rs`), a `Table` in `LAYOUT`
(`crates/pns-adapters/src/config/render/layout.rs`), a parse arm, and a regenerated template checked by
the resolved-config snapshot test.

**Every word pns prints goes through the `humanizer` skill before it ships** (operator ruling 2026-09-09,
standing, covering every pns command).

## The existing state, verified in the code

Every claim below was read in the code today, and the path is where.

- `pns failures serve` is a real listener: single threaded, GET only, read only, everything escaped into
  a `<pre>` block (`crates/pns/src/failures_page.rs`).
- It binds `127.0.0.1` and nothing else, and its module comment says the per-session SSH forward is the
  whole of the access control (same file).
- It waits and retries on a taken port, saying so once, and never gives up (same file, `bind`).
- It sets NO socket read or write timeout (same file, `answer`).
- The daemon starts it as a long-lived child, re-reads the config every tick so a switch needs no daemon
  bounce, and serves nothing on an unreadable config (`crates/pns/src/daemon_runtime.rs:start_page`).
- `[failures]` is two defaulted keys, with a port floor of 1024 and a refusal rather than a clamp
  (`crates/pns-adapters/src/config/failures.rs`).
- The hermes leg signs its body with HMAC-SHA256 (hash-based message authentication code over SHA-256)
  under the config key, sent as `X-Webhook-Signature` (`crates/pns-hermes/src/post.rs`).
- `pns tap` writes the marker with `utimensat` and `AT_SYMLINK_NOFOLLOW`, creating 0700 parents and a
  0600 file (`crates/pns-adapters/src/phone_marker.rs`).
- The `pns.tap/1` result already permits a null `marker` and a null `surface`, documented as the state of
  a run that failed before reading them (`crates/pns-protocol/src/tap.rs`,
  `pns/docs/pns-tap-apple-shortcut.md`).
- Five outbound destinations are inventoried in `pns/docs/specs/privacy-and-hostile-input.md` section 18.
  No inbound network listener is inventoried at all.

The last line is the important one. `127.0.0.1` is the only address pns binds anywhere in the tree, so an
HTTP tap would be its first listener reachable from a network, its first endpoint that authenticates a
caller, and its first place where hostile input arrives from something other than a harness hook or a
config file.

### What the phone can and cannot do

Verified against Apple's Shortcuts user guide,
[Request your first API](https://support.apple.com/guide/shortcuts/request-your-first-api-apd58d46713f/ios):
the "Get Contents of URL" action has a method selector carrying GET, POST, PUT, PATCH and DELETE, and
POST, PUT and PATCH reveal a Request Body parameter accepting JSON, a Form or a File. Headers are a key
and value list on the same action. So a POST with a header and a body is available with no third-party
app.

What is NOT available: a keyed signature. Shortcuts ships a Generate Hash action with the common digests
and no key parameter, so there is no native way to compute an HMAC on the phone. Every route to one is a
third-party App Intents helper, a scripting app carrying its own crypto library, or a Run Script action,
which exists on macOS and not on iOS. That single fact decides the authentication design below, and it is
why this endpoint cannot reuse the hermes signing shape.

## Approaches

### A. A daemon-child listener, a bearer secret, an operator-written bind

`pns tap serve` is a foreground listener, the same shape as `pns failures serve`. The daemon starts it
when `[tap.http] bind` is configured. The Shortcut posts to it with the secret in a header. The response
is the `pns.tap/1` object the SSH route already returns under `--json`.

Costs: the tap dies with the daemon, the secret crosses the network in cleartext unless the operator's
transport is confidential, and a replayed request taps forever.

### B. A loopback listener reached over an SSH local forward

Bind `127.0.0.1` and let the phone reach it through a forward, which is exactly the failures page's trust
model, so the transport is encrypted and no secret is needed on the wire.

Rejected. A local forward needs a real SSH login rather than a `restrict` key running one forced command,
which is strictly more access than the shipped tap grants, and it needs Remote Login anyway. It gives up
the SSH tap's security properties to avoid the SSH tap.

### C. A phone-computed keyed digest instead of a bearer secret

Have the Shortcut hash the secret together with a timestamp using Generate Hash, send the digest and the
timestamp, and have pns recompute and compare inside a clock window with a seen-nonce cache.

Rejected on the verified finding above. Generate Hash takes no key, so this would be a home-grown
secret-prefix construction rather than an HMAC, it needs clock-skew handling and replay state on the Mac,
and it buys protection out of proportion to what the endpoint can do. The capability granted by a
successful tap is one file's mtime.

### D. Do not build it

Keep the SSH tap as the only transport and spend the effort on task 71b, which is the one piece of this
feature with no substitute: a verified Shortcut and a real device test, including an unavailable Mac and
a write failure. The four gains listed above are each one manual step or one Mac-side edit, and the cost
is pns's first authenticated network listener.

This is the recommendation. The rest of the document specifies approach A so that a yes needs no second
design round.

## The recommended design, if it is built

### Configuration

```toml
[tap.http]
# bind = "192.0.2.10:8647"
# key = ""
```

Both keys render commented, because `[tap.http]` is an opt-in table and neither key has a default a
machine could use. `Sample::Example` is the layout's existing mechanism for a key whose working setting
is unset (`crates/pns-adapters/src/config/render/layout.rs`).

`bind` is an internet protocol address and a port, parsed as a socket address so a name is refused rather
than resolved. Resolution would pick an interface pns has no business choosing. A port below 1024 is
refused by name, reusing `[failures]`'s reasoning: the daemon runs as the operator, so a privileged port
is a config that cannot do what it says.

`key` is the shared secret. A key shorter than 32 characters is refused by name, because the entropy of
the secret is the entire defense and there is no lockout behind it.

There is no `enabled` key. Naming no bind and switching the listener off are one statement, which is the
rule `[focus] silence` and `[nag] after_secs` already follow (`crates/pns-adapters/src/config/model.rs`).

`bind` written with no `key`, or with a key under the floor, REFUSES TO BIND. The listener prints one
line naming which key is missing and exits. An unauthenticated tap endpoint on a network address is the
one state this must never reach by accident, so it is a refusal rather than a warning.

What the new table touches, so nothing is half-declared: `TOP_LEVEL` gains `tap`, `TABLE_KEYS` gains
`("tap", &["http"])` and `("tap.http", &["bind", "key"])` the way `lights` lists its own sub-tables,
`LAYOUT` gains a dotted `Table` named `tap.http`, `Config` gains a field, the parse dispatch gains an
arm, and the resolved-config snapshot fixture changes. A key declared in the roster with no arm to read
it is refused by that arm and caught by the schema module's own walk, so both halves of a half-finished
addition are red rather than quiet.

On this machine the secret reaches the file through KeePassXC:

```toml
[tap.http]
bind = "<operator-chosen address>:8647"
key = { keepassxc = "pns :: Tap Key", field = "Password" }
```

### The endpoint

One method and one path. `POST /tap` taps; everything else answers and writes nothing.

A GET must never tap. `moshi-hook` probes local listeners and remembers the ones that answer with an HTTP
header, which is how the failures page is discovered, so a port pns opens will be requested by software
the operator did not aim at it. A GET that tapped would record taps from a port scan.

| Request                                 | Answer                            | Writes |
| --------------------------------------- | --------------------------------- | ------ |
| `POST /tap` with a matching key         | 200, the `pns.tap/1` object       | yes    |
| `POST /tap` with a wrong or missing key | 401, a minimal `pns.tap/1` object | no     |
| `GET /tap`, or any other method on it   | 405                               | no     |
| Any other path                          | 404                               | no     |
| A request line past its cap             | 414, nothing read further         | no     |
| Headers past their cap                  | 431, nothing read further         | no     |

The key travels in `X-Pns-Tap-Key`. It is compared in constant time, against length as well as bytes, and
it is never printed, logged or echoed in any response.

### The response

The 200 body is the `pns.tap/1` object, byte for byte what `pns tap --json` prints today, with
`Content-Type: application/json`. One report type, two transports, and no second formatter free to drift
from the first. This is the same rule the failures page follows for the terminal view.

The 401 body is the same schema with `ok: false`, `write_status: "not_requested"`, null `marker`, null
`surface`, and `error.code = "unauthorized"`. Null `marker` and null `surface` are already documented
states of this schema for a run that fails before reading them, so no schema change is needed and the
Shortcut's existing parser keeps working. Nulling the marker is also what keeps the Mac's filesystem
layout out of an unauthenticated response.

The Shortcut can then tell three cases apart that the SSH route collapses into one SSH error: the tap
landed and the surface is `mobile`, the key is wrong, and nothing answered.

### Reading a request from a network

The failures page reads a request line with no timeout and no cap, which is defensible on loopback with
one operator. It is not defensible on an address a phone can reach. The tap listener bounds every read:

- A 5 second socket read timeout, so a client that connects and sends nothing cannot hold the listener,
  and a 5 second write timeout for the other direction.
- A request line capped at 8 KiB and headers capped at 16 KiB in total and 64 in count. Past either cap
  the request is refused unread.
- A body capped at 64 KiB, read and discarded. The tap carries no input, and an unread body hangs some
  clients.

Single threaded, like the failures page, and for the same reason: one operator with one phone taps once
at a time. The read timeout is what makes that safe, because it bounds how long one slow client delays
the next to five seconds rather than forever. A thread per connection is the upgrade path if that ever
matters, and it does not yet.

### Rejected requests are nearly silent

The first rejection since the listener started prints one line naming the reason class and nothing else.
Every later one is silent. A line per rejected request on a port that gets scanned is a log-flooding
vector aimed at the file the rotation reads, and the `bind` retry loop already uses this exact
say-it-once pattern. The peer address is not printed, the presented key never is.

### Rotation

The listener re-reads the config on every request and takes the key from that read, so a rotated secret
is live on the next tap with no restart. A tap is a human action a few times a day, so the cost is one
small TOML parse per tap.

The bind address is fixed for the listener's life, because changing it means a new socket. Changing
`bind` therefore needs the daemon restarted (or the child stopped, whichever the machine's supervision
does), and that is stated at the config key.

Rotation on this machine, end to end:

1. Change the value in the KeePassXC entry.
1. `chezmoi apply`, which re-renders `~/.config/pns/config.toml`.
1. Update the key field in the Shortcut on the phone.
1. Tap. A stale phone gets a 401 with a body that says the key did not match, which is the whole
   diagnosis.

Steps 2 and 3 are not atomic, so taps between them fail with a 401 rather than silently doing nothing.
That is the intended failure, and it is why the 401 carries a readable message.

### Where the listener runs, and the cost the task names

`pns tap serve` is a plain foreground listener. The daemon starts it exactly the way it starts the
failures page: a long-lived child in a set built for short ones, `running` preventing a second listener,
the config re-read every tick so switching the feature on or off needs no daemon bounce, and an
unreadable config serving nothing.

**THE SSH TAP SURVIVES A DEAD PNS DAEMON AND THIS DOES NOT.** That is the one real cost, and it is why
this is opt-in rather than the default. On the SSH route, sshd accepts the connection and execs `pns tap`
as a one-shot process; pns's daemon is not in the path, and a machine with `[daemon] enabled = false`
taps correctly. On the HTTP route the listener is a daemon child, so a daemon that is wedged, crashed or
switched off means nothing is bound and the phone gets a refused connection.

The asymmetry is worse than it first looks, and the reason is that a dead daemon does not take the rest
of pns with it. The harness hooks and the shell notifier are one-shot processes that deliver
synchronously, and each of them reads the marker to pick a surface. So with the daemon down, pns still
notifies and still needs to know where the operator is, which is exactly when the operator most wants
their phone to be able to say "I am here". The HTTP tap is unavailable in that state.

Two mitigations, neither of them pns's:

- A separate LaunchAgent supervising `pns tap serve`, with `KeepAlive`, so launchd restarts it
  independently of the daemon. That is a dotfiles change, and it is available because `pns tap serve` is
  a foreground process that does not care who started it.
- Keep the SSH key and the `authorized_keys` line in place as a fallback. Both transports write the same
  marker through the same code, so having both costs nothing but the line that was already there.

### The doctor and `--info`

`pns doctor` gains one row in the Pairing section beside the existing `phone tap:` row, and only when
`[tap.http] bind` is configured. It says whether the address is bound, names the address, and says
whether a key is set. It never prints the key. A configured bind with nothing listening is a Warn,
because that is the daemon-down case above and it is the state an operator needs named.

`pns tap --info` gains the same three facts, plus the two sentences pns must say out loud:

- the secret travels in a request header, so on a transport that is not confidential it is readable by
  anything on the path, and pns does not know what the operator's transport is;
- an address that is not loopback is reachable by everything that can reach this machine on that address,
  and `0.0.0.0` means every interface.

Setup prose stays off `--info`, per the task 71 ruling. `--info` closes with the pointer to
`pns tap --install` that it already has.

### `--install`

`pns tap --install` prints the HTTP recipe as step 1 when `[tap.http] bind` is configured, and the
`authorized_keys` recipe otherwise, naming the other route in one line either way. The guide then
describes the machine the operator is standing at rather than a route they did not choose.

The HTTP step 1 carries the URL built from the configured bind, the method, the header name, and where
the key comes from. Step 2's fields change from the SSH action's Host, Port, User, Auth and Script to Get
Contents of URL's URL, Method, Headers and Request Body. Step 3, the trigger methods, is unchanged.

This changes one documented behavior: `pns tap --install` currently reports "its guide and nothing about
this machine's state". It would now read the config. `marker` and `surface` stay null under `--install`,
so the `pns.tap/1` contract is untouched, and the change is listed as an open question below because it
is a contract the phone's parser reads.

### Security summary

The threat model is anyone who can reach the bind address.

What a successful request can do: update one file's mtime. Nothing else. No command runs, no file is read
back, and the response carries no text the caller did not already have except the marker path and the
surface. The handler does exactly what `pns tap` does, with nothing configurable about it, and that is
deliberate: a listener that grows verbs grows blast radius.

What holds: a shared secret with a 32-character floor, compared in constant time; a refusal to bind
without one; one method and one path; bounded reads with timeouts; nearly silent rejections; no marker
path or config path in an unauthenticated response; the key never printed anywhere.

What does NOT hold, stated rather than mitigated: there is no transport security. pns does not terminate
TLS (transport layer security), will not ship a certificate story, and cannot get a certificate for an
address it knows nothing about, so the secret is cleartext on the wire unless the operator's transport
encrypts it. A captured request replays forever, and the endpoint's whole capability is one mtime, which
is what makes that acceptable. An operator whose bind address sits on an untrusted network is exposing a
denial-of-notification to anyone who watches one tap.

The new inbound listener joins `pns/docs/specs/privacy-and-hostile-input.md` as its own numbered
behavior, beside section 18's outbound inventory, because that document's job is to be the one place
every trust boundary is written down.

### Behaviors to pin, fail-first

Each line is one test, and each fails before the code exists.

1. No `[tap.http]` table: nothing binds, and the daemon starts no child.
1. `bind` set, `key` absent: the listener refuses to bind, names the missing key, and exits non-zero.
1. `bind` set, `key` 31 characters: the same refusal, naming the floor.
1. `bind` that is not an address and a port: the config is refused by name at parse time.
1. `bind` port 1023: refused by name, naming the range.
1. `POST /tap` with the right key: the marker's mtime advances, the response is 200, and the body parses
   as `pns.tap/1` with `write_status: "recorded"`.
1. `POST /tap` with a wrong key: the marker's mtime does NOT move, the response is 401, and the body
   carries null `marker` and null `surface`.
1. `POST /tap` with no key header: the same as a wrong key.
1. `GET /tap`: 405, and the mtime does not move.
1. `GET /`: 404, and the mtime does not move.
1. A connection that sends nothing: closed within the read timeout, and a second connection is served
   afterwards.
1. A request line past 8 KiB: refused, and the mtime does not move.
1. The key rotated in the config between two requests: the old key gets a 401 and the new key gets a 200,
   with no restart.
1. Two rejections in a row: exactly one line of output.
1. The response body of a 200 is byte-identical to `pns tap --json` for the same state.
1. `pns doctor` with a configured bind and nothing listening: a Warn row naming the address.
1. Every response, on every path: the configured key does not appear in the body or in any output.

Tests bind an operating-system-assigned port, the way `failures_page::serve_on` is split from `bind` so a
test never races another suite for one fixed number.

## Out of scope

- **Replacing or deprecating the SSH tap.** Both transports write the same marker through the same code,
  and the operator ruling is explicit that this arrives as an alternative.
- **TLS, certificates, or any certificate automation.**
- **Detecting the network.** No Tailscale check, no interface enumeration, no reachability probe, no
  suggested address. Task 75 owns this machine's SSH exposure and pns stays network-independent.
- **Waking the Mac.** Task 76 records that network wake is conditional, and this design promises nothing
  about a sleeping machine that the SSH route does not already promise.
- **A second endpoint.** No presence read, no quiet toggle, no failure listing. The failures page is
  already its own loopback listener and stays separate.
- **Reading or writing `~/.ssh/authorized_keys`,** settled under task 71.
- **`--delete-marker` and `--for <duration>`,** settled under task 71.
- **The phone artifact itself.** Task 71b owns the Shortcut, its verbatim record, and the device
  verification. This design says what the endpoint accepts and returns; it does not publish a Shortcut.
- **A per-request rate limit or lockout.** The key's entropy is the defense and the capability is one
  mtime.

## Assumptions made in the operator's place

Each is a choice this document made because the operator is asleep. Each names the alternative.

1. **Recommend not building it.** Alternative: build it now. The four gains are real and small; the cost
   is pns's first authenticated network listener and the loss of daemon-independence.
1. **Table named `[tap.http]`.** The transport is in the table name, which answers the 2026-08-31 naming
   ruling ("one word selects a backend everywhere: `type`") without inventing a `type` key whose only
   possible value is `"http"`, since SSH needs no pns-side configuration at all. Alternatives: `[tap]`
   with `bind` and `key`, or `[tap] type = "http"`.
1. **No `enabled` key; an absent `bind` is off.** Alternative: an explicit bool, at the cost of two
   switches that can disagree.
1. **`bind` with no `key` refuses to bind.** Alternative: bind anyway and warn, which is how a machine
   ends up serving an unauthenticated tap.
1. **A 32-character floor on the key.** Alternative: accept any non-empty string, which is what
   `[plugins.hermes] key` does, at the cost of an operator typing something guessable.
1. **`0.0.0.0` is accepted and named plainly in `--info` and the doctor row.** Alternative: refuse the
   wildcard, which breaks a machine whose address moves and substitutes pns's judgment for the
   operator's.
1. **Header `X-Pns-Tap-Key`.** Alternative: `Authorization: Bearer <secret>`, which is more conventional
   and more likely to be logged by anything in the path.
1. **`POST /tap`, and a GET never taps.** Alternative: `POST /` for a shorter URL in the Shortcut, at the
   cost of colliding with the convention that `/` is a listing.
1. **The 200 body is the full `pns.tap/1` object.** Alternative: a three-field body, which is fewer bytes
   and a second formatter.
1. **The key is re-read per request, so rotation needs no restart. The bind is not.** Alternative: read
   both once at start, and document a restart for a rotation.
1. **The listener is a daemon child.** Alternative: a dotfiles LaunchAgent supervising it, which restores
   independence from the daemon at the cost of a second supervised process on this machine and a
   mechanism pns cannot ship for anyone else.
1. **One log line for the first rejection, then silence.** Alternatives: never log a rejection, or log
   every one and accept the flood.
1. **`--install` becomes config-aware.** Alternative: a fourth `operation` value, `install_http`, which
   keeps `--install` free of config reads but widens an enum the phone's parser reads.
1. **Single threaded, with the read timeout as the protection.** Alternative: a thread per connection.

## Open questions

1. Build the HTTP tap at all, or record task 74 as declined? Task 76's research explicitly does not
   authorize it, and this document recommends declining for now.
1. If it is built, what bind address, and what makes that address confidential? pns must not detect the
   answer, so the operator states it and accepts it.
1. Is losing daemon-independence acceptable, or should dotfiles supervise `pns tap serve` under its own
   LaunchAgent from the start?
1. `--install` config-aware, or a fourth `operation` value? The second choice widens the `pns.tap/1`
   vocabulary the Shortcut's JSON parsing reads, which is task 71b's code.
1. Where does the key live on the phone, given that the shipped Shortcut sets Hostname, SSH Port and
   Username through a mechanism recorded as "Get all global variables"
   (`pns/docs/pns-tap-apple-shortcut.md`)? This design assumes the key field uses whatever that mechanism
   is and introduces no new one. Whether a Shortcut exported after setup carries the key with it is
   unverified and matters, because the SSH variant is safely shareable: its private key lives in the
   phone's own key store rather than in the Shortcut.
1. Does the HTTP tap change the answer to task 75? Narrowing SSH exposure to the tailnet would narrow the
   SSH tap to the tailnet, and an HTTP tap bound to a tailnet address is reachable from exactly the same
   places, so the two look equivalent from here. Confirm that before either is treated as a reason for
   the other.

## Verification, if it is built

- The refusal to bind without a key: measured by starting the listener with `bind` set and `key` removed,
  and confirming nothing is listening on the address.
- The capability boundary: a 401 request and a 405 request each leave the marker's mtime unchanged,
  measured with `stat` before and after.
- Rotation: two requests either side of a config edit, the first key rejected and the second accepted,
  with no process restarted in between.
- The daemon-death cost, measured rather than argued: stop the daemon, tap from the phone, and record
  what the Shortcut shows. Then do the same with the SSH transport and record that it still works.
- The device half, which task 71b owns: a real tap from a locked phone, from a sleeping Mac, and from a
  network where the Mac is unreachable.
