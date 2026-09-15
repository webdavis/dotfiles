# Security alert metadata and route reconciliation

Status: design, written 2026-09-14 for the ledger task "Define the required alert/evidence metadata
through the existing delivery path". Not approved, not built. No code was written or changed for this
document.

Scope: the data one security alert carries across the delivery path, and the route names the producers
and the checkers must agree on. The investigator itself, its sandbox, its prompt and its advisory limits
belong to the neighboring ledger tasks and are out of scope here.

## Why this exists

The ledger asks for two things. First, the delivery path carries `agent`, `state`, `project`, `detail`
and `request_id` and nothing else, so a downstream reader cannot tell a critical security finding from a
daily digest except by reading rendered prose. Second, the route names disagree: native posture names
`posture`, while the tracked checker covers `pns` and `unattended-upgrades`.

The second half turned out to be live and losing pages. Measured on this machine on 2026-09-14:

```
$ pns failures
◆ Not arriving ── 11 delivery legs
    id  when              status        route      sent by
  · 2421  2026-09-14 00:00Z  HTTP 404   posture    posture
  · 2117  2026-09-13 15:00Z  HTTP 404   posture    posture
  · 1547  2026-09-13 09:39Z  HTTP 404   pns-recap  pns
  ... (8 posture legs, 2 pns-recap legs, 1 no-response)
```

Every one of the eight `posture` legs is dead-lettered, `deadletter_reason = permanent`,
`http_status = 404`, confirmed by reading `ledger_legs` in `~/.local/state/pns/pns.db`. Their local
banner legs all show `acknowledged = 1`. So since the posture cutovers merged, the operator has seen a
banner for each of these events and the durable Discord record was never written for any of them.

The cause is not subtle. The hermes gateway serves three webhook routes. Both the deployed
`~/.hermes/config.yaml` and the age-encrypted source under `private_dot_hermes/` declare exactly
`priority`, `pns` and `unattended-upgrades` (decrypted and read on 2026-09-14 with the age identity
already on disk at `~/.config/chezmoi/key.txt`; no vault unlock was needed, which settles the operator
half of this ledger task). An unsigned probe against the running gateway answers 401 for a route that
exists and 404 for one that does not:

```
pns                    401
priority               401
unattended-upgrades    401
posture                404
```

Native posture hard-codes the route name `posture` at six call sites under
`posture/crates/posture/src/`: `funnel.rs:39`, `watchdog.rs:52`, `alert.rs:67`, `poll.rs:121`,
`heartbeat.rs:42` and `digest.rs:56`, each spelled
`String::from("posture").expect("the fixed posture route is valid")`. Nothing ever declared that route,
so every hermes leg posture raises 404s.

### Why this blocks the alert cutover, not just the digest

Five of the seven osquery LaunchAgents already run the `posture` binary (`digest`, `poll`, `heartbeat`,
`funnel`, `watchdog`). The two that still run Bash are `osquery-results-alerter` and
`osquery-alert-drainer`, and both source `alert-dispatch.sh`, which posts to the `priority` route. So
the CRITICAL security page still arrives today, over Bash, on a route that exists.

The ledger records `feat/posture-alert-cutover` as finished and unmerged (four commits, stopped at its
first ship gate). The moment that branch lands and the operator applies, the CRITICAL page moves to
`posture alert`, which posts to the `posture` route, which 404s. The eight lost digests become lost
security pages. **Fix the route before that cutover merges, or the cutover silently removes the durable
half of the security alert.**

## Constraints

From `CLAUDE.md`, the recorded operator rulings and the ledger:

- **posture and pns are shippable products.** No workspace may depend on another; posture keeps its own
  copy of the wire envelope in `posture-pns-wire`, held honest by golden fixtures rather than a shared
  type. Any protocol field added here is added twice, by hand, in both copies.
- **Nothing in a workspace may assume this repository exists.** A route name compiled into posture is a
  product default, not a dotfiles fact.
- **We test the behavior of tools we wrote, and nothing else** (2026-08-05). A test asserting that the
  checker's route list matches the producers' route names is declaration-consistency checking and is
  deleted on sight. The reconciliation therefore cannot be held by a gate; it has to be held by having
  one place, or by runtime detection.
- **Optimal over cheap** (2026-09-05): never choose a design because it is less work.
- **A page that does not arrive is the worst outcome in the system** (the 2026-09-08 delivery-failure
  design's own framing).
- **Defaults visible in config**; opt-in defaults; concise comments.
- **Rust files target 300 lines and never exceed 500**, tests included.
- **The operator runs applies.** Agents propose; the encrypted hermes config is operator-edited.
- Trigger decision 2026-09-12: investigate Critical alerts only. Deliver the original alert immediately,
  publish the investigation separately. Daily digests do not trigger the workflow.
- Supported-interface review 2026-09-13: a named route selects one destination rather than broadcasting
  to two; transport deduplication needs distinct delivery-attempt identifiers under one alert
  correlation key.

## What already exists

Verified by reading the code and the installed hermes revision
`a4091e49f10ddceaac1a902848aabfb1b9aae210` (pinned in `.chezmoidata/hermes.yaml`).

### The request envelope already has room

`pns.request/1` (`pns/crates/pns-protocol/src/request.rs`, mirrored in
`posture/crates/posture-pns-wire/src/request.rs`) defines fifteen top-level fields, including:

- `class: Option<Name>`, "an operator-configured delivery class, independent of producer and route",
  serialized with `skip_serializing_if = "Option::is_none"`.
- `extensions: Map<String, Value>`, "producer-specific data, carried verbatim and never read here".

An unknown top-level key is **ignored and named, never refused**: `decode` collects unknown keys and the
result carries them as an `ignored_fields` diagnostic. Additive fields are therefore safe against an
older pns.

The wire caps (`pns/crates/pns-protocol/src/bounds.rs`) are 65,536 bytes per envelope, 64 fields per
object at every level, 8,000 characters per string, 64 items per array, and 8 nested containers. A
`Name` is at most 64 characters with no control characters; a `RequestId` is at most 128 visible ASCII
characters.

### The whole request already reaches every destination

`DeliveryRequest` (`pns/crates/pns-application/src/destinations.rs`) carries
`producer_request: Option<&'a str>`, the full encoded request, alongside the rendered event. The banner
destination already re-decodes it to read `class` and `signal` and picks the `Sosumi` sound for a
security page (`pns/crates/pns-adapters/src/destinations/banner.rs:148`). The retry path supplies the
same string from the ledger (`submission_delivery.rs:130`), and `ledger_events.producer_request` stores
it durably; a stored example read back on this machine carries `"extensions":{}` and `"route":"posture"`
verbatim.

So the metadata does not need a new transport. It needs a producer that fills a field and a body builder
that mirrors it.

### Where the metadata is dropped today

`pns/crates/pns/src/event_flow/submit/mapping.rs` maps a decoded request to `EventArgs`, keeping
producer, signal, project, branch, pane, detail, route, scope and an elapsed-seconds threshold. It drops
`class`, `extensions`, `occurred_at` and `session`. The rendered `Event` is then what
`pns/crates/pns-adapters/src/destinations/hermes.rs:body_with_id` turns into the posted body:

```json
{"agent": ..., "state": ..., "project": ..., "detail": ..., "request_id": ...}
```

### posture already computes the metadata and throws it away

`posture-domain` has a three-value `Severity` (`Critical`, `Notice`, `Info`) and
`PageFinding { query, severity, enrichment_path, columns, act, signing, triage }`. `render_page` filters
to `Severity::Critical`, renders at most `BLOCK_LIMIT` (8) blocks, caps the body at `BODY_LIMIT` (1,900)
characters with a truncation notice pointing at `results.log`, and returns `Page { count, body }` where
`count` is every critical finding including the ones the caps left out.

`Alert` (`posture-application`) is then `{ occurrence_id, event, signal, occurred_at, title, detail }`.
The detector name, the resolved artifact path and the severity are all present one layer earlier and
none of them survives into the alert. `PnsProducer::encode` sets `detail = "{title}\n{detail}"` and
`class = Some("security")` when the signal is `NeedsAttention`.

This is exactly the "inferring them from rendered prose" the ledger warns about, and it is lossy in a
way prose cannot recover: with twelve critical findings the page shows eight and says so, and the other
four exist only in a log the reader has no pointer into.

### What the hermes route does with a body

`gateway/platforms/webhook.py` at the installed revision:

- One route name selects one route (`self._routes.get(route_name)`). There is no fan-out to two routes,
  which retires the original 2026-06-03 design's "two routes fire on one incoming webhook" assumption.
- `deliver_only: true` skips the agent: the rendered template **is** the delivered message. All three
  live routes set it.
- The `prompt` is a template over the posted body. Substitution is `re.sub(r"\{([a-zA-Z0-9_.]+)\}", ...)`
  with dot-notation into nested objects. **A key the body does not carry renders as the literal text
  `{key}`.** A value that is an object or array renders as indented JSON (JavaScript Object Notation)
  cut to 2,000 characters. `{__raw__}` dumps the whole body cut to 4,000 characters. Because the
  replacement comes from a callable, a substituted value is not rescanned, so a value containing
  `{agent}` is inserted literally.
- Extra body keys are simply unused. Nothing rejects them.
- The event-type filter reads `X-GitHub-Event`, `X-GitLab-Event`, then `payload["event_type"]`, then
  `payload["type"]`. A route with an `events` list answers **HTTP 200 with `{"status":"ignored"}`** for
  an event outside it. No live route sets `events`, so nothing filters today.
- Authentication is `X-Webhook-Signature`, a lowercase hex hash-based message authentication code over
  the raw body, which is what pns sends. Checked before the rate limit and before parsing.
- Deduplication keys on `X-GitHub-Delivery`, then `svix-id`, then `X-Request-ID`, else a millisecond
  timestamp. The cache is one in-memory dictionary shared by every route, entries live 3,600 seconds.
  **`Idempotency-Key`, which is the header pns sends, is not read at this revision.** So pns retries are
  not deduplicated by the gateway, and two posts about one alert would be deduplicated into one if they
  ever shared a delivery identifier.
- Defaults with no override in the live config: 30 requests per minute per route, 1,048,576-byte body
  cap, bind `127.0.0.1:8644`.
- `deliver_extra` string values are template-rendered against the body, and `thread_id` /
  `message_thread_id` are passed through as delivery metadata. The neighboring advisory task can use
  that; this document does not.

### The route landscape, reconciled

Five route names are in use on this machine. For each: whether hermes declares it, who names it, what
body shape reaches it, and which of the two checkers covers it.

- **`priority`.** Declared, `deliver_only`. Named by `alert-dispatch.sh`, which both live Bash jobs
  source, and GET-probed by `posture watchdog` for gateway health. Body
  `{event_type, host, tier, ts, alert:{title, detail}}`. Covered by neither checker.
- **`pns`.** Declared, `deliver_only`. The pns default route. Body
  `{agent, state, project, detail, request_id}`. Covered by both.
- **`pns-recap`.** **Not declared, 404 live.** Named by `post_return_recap.rs:RECAP_ROUTE`. Same body as
  `pns`. Covered by `pns doctor` only, which reports it failing.
- **`posture`.** **Not declared, 404 live.** Named by posture at six hard-coded sites. Same body as
  `pns`. Covered by `pns doctor` only, which reports it failing.
- **`unattended-upgrades`.** Declared, `deliver_only`. Named by uu's own signed post
  (`uu-protocol::record_body`). Body `{agent, state, project, detail}`. Covered by the apply-time
  checker only, since no pns leg ever posts there.

Four separate facts fall out of that list.

1. **`priority` carries the only CRITICAL security page on this machine today and nothing checks it.**
   That is the worst omission in the current checker list, and it is not the one the ledger names.
2. **The apply-time checker is the only surface that can see a route pns does not post to.**
   `.chezmoiscripts/run_after_68-hermes-log-route-status.sh.tmpl` reads the decrypted config, checks
   membership and `deliver_only`, and posts an unsigned body to catch a route in the file that the
   running gateway has not loaded. Its list is `expected_routes=(pns unattended-upgrades)`.
3. **`pns doctor`'s route check is observation-driven and after the fact.** It derives its list from
   `SELECT DISTINCT route FROM ledger_legs WHERE destination = 'hermes'`, so a route is only checked
   once a page has already been lost to it, and a route no pns leg uses is never checked at all.
4. **posture's own gateway health check cannot detect a missing route.**
   `posture-domain/src/watchdog.rs:route_problem` treats HTTP 405 and any 2xx as healthy, and a GET
   answers 405 for every path on this gateway, real or not (measured: `priority` 405, `posture` 405). The
   check proves the gateway is listening and nothing more, while its message names the `#priority` route
   specifically.

### One more verified surface defect

`RouteVerdict::Missing` maps to `Mark::Warn` (`pns-domain/src/doctor/routes.rs:115`), and only
`Mark::Bad` rows reach `pns doctor`'s closing list (`pns/src/doctor_style.rs:close`). So the run captured
above printed two "THE GATEWAY HAS NO SUCH ROUTE; a page sent here is lost" warnings and then closed
with `✓ nothing to act on`. A route that drops every page is not a warning.

## Approaches: the metadata

### M1. Carry it under `extensions`, unchanged protocol

The producer writes `extensions.security = {...}`; the hermes body mirrors a named subset of
`extensions`.

- Cheapest: no field added, no golden fixture moved, nothing to keep in step across two wire copies.
- Breaks the documented meaning of `extensions` ("carried verbatim and never read here"). A delivery
  path that reads it makes an informal schema load-bearing without giving it a boundary to be validated
  at, and two producers can spell the same concept differently with nothing to notice.

### M2. Two additive top-level fields, `severity` and `evidence`

- Validated at the wire boundary, one spelling for every producer, additive so an older pns names them
  as ignored and still accepts the request.
- Two fields to add by hand in two copies. Puts a security vocabulary in a general notification protocol
  where every producer sees it, including a shell notifier that has no business setting a severity.

### M3. One additive top-level `security` object (recommended)

```json
"security": {
  "severity": "critical",
  "finding_count": 12,
  "evidence": [
    {"kind": "file", "locator": "/Users/x/Library/LaunchAgents/com.bad.plist",
     "detector": "persistence_launchd"},
    {"kind": "launchd_label", "locator": "com.bad", "detector": "persistence_launchd"}
  ]
}
```

- One field in two copies rather than two. With `skip_serializing_if = "Option::is_none"` the golden
  fixtures in `pns/crates/pns-protocol/src/request/tests/classes.rs` and
  `posture/crates/posture-pns-wire/src/request/tests/classes.rs` keep their current byte-exact strings,
  because an absent field is not serialized at all. `class` already works this way, so there is a
  precedent to copy rather than a pattern to invent.
- Groups the vocabulary, so a producer with no security story never fills it and never sees it.
- Reachable from a hermes route prompt as `{security.severity}` through the dot-notation the renderer
  already supports.
- Depth 4 at the deepest (envelope, `security`, `evidence`, item) against a cap of 8. `evidence` holds up
  to 64 items, which is why `finding_count` is separate: it is the true count including whatever the caps
  left out, the same reasoning `Page.count` already carries.
- Cost: the name sits next to `class`, whose value is already the literal string `security`, so the two
  need their difference stated in the field's own doc comment. `finding` was the alternative name
  considered.

### The cheap alternative, named because it is genuinely available

`class = "security"` and `signal = needs_attention` together already mean "at least one CRITICAL finding"
today, because `render_page` filters to `Severity::Critical` and `PnsProducer` sets the class only for
`NeedsAttention`. A trigger could read exactly that pair with **zero** protocol change.

It is rejected because it is a coincidence of two unrelated fields rather than a statement. It breaks the
first time posture pages on a Notice (`gate.rs` already escalates severity under conditions), or the
first time another producer sets `class = "security"` for something that is not critical, and it carries
no evidence reference at all, which is the half of the task that cannot be reconstructed later.

## Approaches: the routes

### R1. Declare the `posture` route and extend the checker list

The operator adds a `posture` route to the encrypted config with its own secret and channel; the checker
list grows to every route any producer on this machine names.

- Keeps one route per producer, which is how `pns` and `unattended-upgrades` already work, and keeps a
  per-producer Discord channel possible.
- Needs an operator edit of an encrypted file and a decision about which channel security pages land in.

### R2. Repoint posture at the existing `priority` route

- Reuses the watched channel and an existing secret, no new declaration.
- Does not work during the overlap. `priority`'s prompt is `{alert.title}\n\n{alert.detail}` and the pns
  body has no `alert` object, so a posture page would render the literal text `{alert.title}` in Discord.
  The reverse is equally true: rewriting the prompt to `{detail}` breaks the Bash alerter that still
  posts there. Verified from the renderer: a missing key becomes its own literal. **No single prompt
  serves both body shapes**, so R2 is only available after the Bash alerter is gone, and it is precisely
  the alert cutover that removes it. Sequencing this way puts the riskiest step in the middle of a
  security cutover.

### R3. One route constant with an operator override, defaulting to `posture`

Replace six hard-coded `String::from("posture").expect(...)` sites with one place, and let the operator
point it elsewhere without a rebuild, the way every other posture path is already an environment
override with a visible default.

- Removes a six-way duplication inside one workspace and satisfies the no-hardcoding review standard.
- Complementary to R1 rather than an alternative: R3 supplies the knob, R1 is what the operator does with
  it today.

### R4. Fall back loud-ward when a route permanently refuses

pns already does this twice. `hermes_url_for` posts to the default route and says so when a route name is
unusable, "because a misrouted notification on the loud route beats a silently dropped one". And
`post_return_recap` posts the recap to `pns-recap`, and on a refusal re-posts it to the default route
with the line "(the pns-recap route did not take this, so it landed on the default route instead)". That
is why the two `pns-recap` 404s in the ledger are a degraded delivery while the eight `posture` 404s are
total loss: the recap has the fallback and the security page does not.

- Turns a missing route from silent total loss into a delivered page in the wrong channel with a line
  saying why, for every producer, permanently.
- All three live routes are `deliver_only` and deliver to Discord channels the operator owns, so the
  fallback does not cross a trust boundary; it crosses a channel boundary.
- The 2026-09-08 design deferred "reporting a failure over a route that still works" as the piece most
  likely to misfire. This is a different thing (the page itself, not a report about it) with an existing
  precedent, but the deferral is close enough that it is recorded as an assumption below rather than
  assumed settled.

## Recommended design

M3 for the metadata, R1 plus R3 plus R4 for the routes, and R2 explicitly not now. Stated as behaviors,
so each can be pinned by one test first.

### B1. The envelope carries an optional `security` object

`pns.request/1` gains one top-level optional field in both wire copies, serialized only when present.

- `severity`: a `Name`, not a closed enum at the wire boundary. The boundary enforces bounds and nothing
  else, exactly as `class: Option<Name>` already does; meaning is a policy-layer comparison against the
  three words `critical`, `notice`, `info`.
- `finding_count`: an unsigned integer, the true count of findings at this severity including any the
  render caps left out.
- `evidence`: an array of objects, each `{kind, locator, detector}`, all three bounded strings. `kind` is
  a closed vocabulary compared at the policy layer: `file` (an absolute path on this host whose contents
  are untrusted), `launchd_label` (a launchd label, not a path), `record` (a line in a local record file,
  for the digest spool). `locator`'s meaning is defined per kind. `detector` is the producer's own
  detector or query name.

Red tests: an absent `security` field leaves the encoded bytes byte-identical to today's golden fixture,
in both copies. A present one round-trips. An oversized or control-bearing member is refused with the
existing bounds diagnostic. A `severity` word outside the three is accepted by the envelope and named at
the policy layer.

### B2. Malformed metadata never costs a delivery, only an investigation

Delivery must not depend on the metadata parsing. An unrecognized `severity`, an unrecognized `kind`, an
absent `security` object: the page is rendered and delivered exactly as today, and the anomaly is
reported as a result diagnostic. The investigator trigger reads the other way: anything it cannot
positively read as `critical` is **not** investigated. Fail open for delivery, fail closed for the
trigger. A suppressed alert is never an acceptable outcome of a metadata bug.

### B3. The hermes body mirrors the metadata additively

`body_with_id` keeps its five current keys with their current names and adds `security` when the
producer request carried one, read off `DeliveryRequest.producer_request`, which the first attempt and
every retry both supply from the same stored string. The signed document is therefore stable across
attempts under one signature scheme.

Two prohibitions, both load-bearing:

- **The body must never gain a key named `type` or `event_type`.** The gateway derives its event-type
  filter from those two names, and a route that ever sets an `events` list would then answer HTTP 200
  with `{"status":"ignored"}`, which pns records as a successful delivery of a page nobody received.
- **No existing key is renamed, retyped or removed.** The three live route prompts are in an
  age-encrypted file the operator edits by hand; a renamed key renders as its own literal text in
  Discord.

The executable-channel contract (`event_json`) is deliberately left alone. It is a test seam and an
escape hatch, and nothing reads security metadata through it.

### B4. posture fills the field from data it already has

`render_page` already holds every `PageFinding`. The change is to stop discarding the structured half at
the `Alert` boundary: the page's evidence travels beside its prose, `Alert` gains an optional security
member, and `PnsProducer::encode` sets it. `class = "security"` stays exactly as it is; it is the
delivery-policy word that `bypass_silence_classes` matches, and it is not overloaded with severity.
Overloading it would be a silent regression: the config lists the literal `"security"`, so a value of
`security-critical` would stop bypassing the mute.

Watch the file-size rule here. `page.rs` and its submodules are the likely site and the 300-line target
applies with tests included.

### B5. Structured locators are raw, and rendering is the renderer's job

`posture-domain/src/sanitize.rs` strips backticks, folds line-breaking whitespace and cuts at 240
characters, because every rendered value fans out into Discord markdown. That treatment is correct for
prose and **wrong for a locator**: a path with a backtick stripped and a path cut at 240 characters are
both paths that do not exist, and an investigator handed one would resolve the wrong artifact or none.

So the structured locator is carried raw, bounded only by the envelope's 8,000-character text cap, and
every consumer that renders it is responsible for escaping it at the point of render. The concrete
consequence for the route prompt: the `posture` route may render `{security.severity}`, which is a word
from a closed vocabulary, and must **not** render `{security.evidence}`, which is attacker-influenced
text that would arrive in Discord as unescaped indented JSON. The human-readable version of the same
information is already in `{detail}`, sanitized.

### B6. A permanent route refusal falls back to the default route, once, with a line

On a permanent refusal (the 2026-09-08 design's classification, already shipped: `deadletter_reason`
`permanent` with the status on the record), the page is re-posted to the default route carrying one line
naming the route that refused it and the status. Exactly once: if the default route also refuses, the
leg dead-letters as it does today and no third attempt is made. The banner leg is unaffected; it already
delivers.

This is `post_return_recap`'s behavior applied to a security page, and it is the behavior that would have
turned eight lost pages into eight pages in the wrong channel.

### B7. A missing route is an issue, not a warning

`RouteVerdict::Missing` becomes `Mark::Bad` so it reaches `pns doctor`'s closing list. A route that drops
every page cannot coexist with `✓ nothing to act on`.

### B8. One route name in posture, with an override

One constant in one place, `posture` as the visible default, overridable by an environment variable the
way every other posture path already is. Six `expect` call sites collapse to one.

### B9. The checker lists every route any producer on this machine names

`expected_routes=(posture priority pns pns-recap unattended-upgrades)`, keeping the script's existing
habit of saying per route how it fails, because they fail differently:

- `priority`: the CRITICAL page from the Bash alerter, until that cutover lands. Unchecked today.
- `pns`: the default route, and now also the fallback target for B6, so its absence is two failures.
- `pns-recap`: the threaded recap. Degrades to the default route today, so its absence is visible but
  cheap.
- `posture`: every native posture page and digest. Total loss today.
- `unattended-upgrades`: the weekly record, whose absence looks exactly like a healthy quiet week.

No test pins this list, by the 2026-08-05 ruling. The comment block is the enforcement, and B6 is what
makes a missed entry survivable.

### B10. The delivery-attempt identifier

pns sends `X-Request-ID: <request_id>:<route>` alongside the headers it already sends. Stable across
retries of one leg, distinct across routes, so one alert posted to two routes is two deliveries while a
retry of one leg is the same delivery. `request_id` stays the correlation key in the body, which is what
a reader joins on. `Idempotency-Key` is left in place: it is harmless and a later hermes may read it.

The gateway's deduplication is best-effort and must not be depended on for correctness: the cache is in
memory, shared across routes, and entries live one hour, so a gateway restart forgets everything.

### Failure modes, stated

- **`security` absent.** Page delivers as today. No investigation.
- **`severity` unreadable.** Page delivers. Diagnostic recorded. No investigation.
- **`evidence` empty or capped.** Page delivers, and `finding_count` still states the true number. An
  investigator with no locator investigates nothing rather than guessing from prose.
- **Route missing.** Page falls back to the default route carrying a line (B6). The banner has already
  delivered. `pns doctor` names it as an issue (B7).
- **Default route also missing.** Leg dead-letters, as today. `pns failures` carries the status and the
  route.
- **Route not `deliver_only`.** The gateway hands an untrusted security page to an agent. The apply-time
  checker already catches this per route and would now catch it for all five.
- **Route gains an `events` list.** Silent HTTP 200 drop, which is why B3 forbids `type` and
  `event_type` in the body.
- **Body over one mebibyte.** HTTP 413. The envelope's own 64 kibibyte cap makes this unreachable from
  pns.

### Security

- Every `locator` and `detector` value is attacker-influenced input. No reader may treat one as a path to
  execute, a command to run, or a template to expand. Today no agent reads them, because all live routes
  are `deliver_only`; the day an agent route does, the untrusted-evidence handling in the neighboring
  task applies and this contract is what tells it which fields are untrusted.
- Adding structured metadata to the body does not widen exposure to Discord: the same paths are already
  in the sanitized prose. It does add a machine-readable copy, which is the point.
- The signature covers the exact body bytes, so metadata added to the body is authenticated by the same
  key as the rest of it. No new secret and no new trust boundary.
- No secret, no key material and no host credential belongs in `security`. `evidence` names locations,
  never contents.

## Out of scope

- The investigator: its sandbox, network boundary, model-provider disclosure, prompt, advisory limits and
  result validator. Those are the neighboring ledger tasks and remain blocked on the operator's security
  boundary decision.
- Copying evidence anywhere. This document defines a reference; nothing collects, uploads or mounts a
  referenced artifact.
- Threading the advisory beneath the alert. The gateway's template-rendered `deliver_extra.thread_id` is
  noted above as a supported mechanism and is the advisory task's to use.
- Migrating the Bash `priority` body shape to the pns shape, and retiring the `priority` route. That
  belongs to the alert cutover.
- The executable-channel JSON contract.
- The moshi and lights legs. A phone card is character-budgeted prose and a lamp is a color; neither has
  anywhere to put an evidence reference.
- `pns-recap`'s 404. It is the same class of defect, it already degrades gracefully, and it is named here
  only so the checker list is complete.

## Assumptions made in the operator's place

1. **One `security` object rather than flat `severity` and `evidence` fields.** Alternative: two
   top-level fields, which read slightly more directly in a route prompt (`{severity}`) at the cost of a
   second field to keep in step across two wire copies and a security word visible to every producer.
2. **`severity` is validated at the policy layer, not the wire boundary.** Alternative: a closed serde
   enum, which refuses a misspelled severity at decode. Rejected because a refused request is a lost
   security page, and B2's split (fail open for delivery, fail closed for the trigger) is the safer
   asymmetry.
3. **The evidence vocabulary starts at three kinds** (`file`, `launchd_label`, `record`). Alternative: a
   single `path` string, which is what the original 2026-06-03 design assumed. Rejected because a launchd
   label is not a path and a reader that cannot tell them apart will try to open one.
4. **`evidence` locators are raw and unsanitized.** Alternative: run them through `sanitize::code` for
   consistency with the prose. Rejected in B5: a sanitized path is a wrong path. The cost is that every
   renderer downstream now owes its own escaping, which is recorded rather than hidden.
5. **A permanent route refusal falls back to the default route (B6).** Alternative: leave the route
   dependency hard, which is today's behavior and which lost eight pages. A third option is to refuse at
   startup, which nothing in this architecture can do because the route name is per-request. The fallback
   puts security content in the general `pns` channel when the security route is broken; both channels
   are `deliver_only` Discord channels the operator owns, so the trust level is identical and only the
   audience is different. **If the operator would rather a security page never appear in the general
   channel, B6 must be dropped and B7 plus B9 become the whole safety net.**
6. **posture keeps `posture` as its default route name and gains an override (R3), rather than moving to
   `priority` (R2).** Alternative stated in full under R2: it requires the Bash alerter to be gone first,
   which makes it a step inside the alert cutover rather than before it.
7. **The delivery-attempt identifier is per leg, not per retry** (B10). Reading "distinct
   delivery-attempt identifiers under one alert correlation key" as "one identifier per delivery, stable
   across that delivery's retries". Alternative: a per-retry identifier, which guarantees a retry is
   never swallowed by the gateway's cache at the cost of a possible duplicate security page in Discord
   when a first attempt delivered but pns did not see the response. That trade is duplicate-over-lost,
   which this repository usually prefers, so it is a genuine coin-flip and the operator should call it.
8. **The checker list includes `priority` and `pns-recap`** even though the ledger task names only the
   `posture` mismatch, because `priority` carries today's only CRITICAL page and is unchecked by
   anything.
9. **`class` is not overloaded with severity.** Alternative: `class = "security-critical"`. Rejected
   because `bypass_silence_classes` matches the literal `"security"`, so that change would silently stop
   security pages bypassing the mute.

## Open questions

1. Which Discord channel do security pages land in: a new one for the `posture` route, or `#priority`
   reused with its own secret and a prompt that serves the pns body shape? This decides whether R1's new
   route gets a new channel identifier.
2. Assumption 5: is a security page in the general `pns` channel better than no security page, or must a
   security page appear only in its own channel?
3. Assumption 7: per-leg or per-retry delivery-attempt identifier, that is, lost-page risk or
   duplicate-page risk?
4. Should the `posture` route be declared and the checker extended **before** the branch
   `feat/posture-alert-cutover` merges? This design says yes and treats it as a blocker; the operator
   owns the merge order.
5. Is `security` the right field name next to an existing `class` whose value is `"security"`, or is
   `finding` clearer?
6. Does the evidence vocabulary need a fourth kind now for the file-integrity path (a manifest entry),
   or does `file` plus `detector` cover it until that reader exists?
7. `pns doctor` closing with `✓ nothing to act on` over two route warnings (B7) is a one-line severity
   change in an unrelated file. Fold it into this work, or file it separately?

## Verification, when this is built

- The golden fixtures in both wire copies still assert byte-identical encodings for a request with no
  `security` field. This is the test that proves the addition is additive.
- A request carrying `security` reaches the hermes body: assert the posted body parses and carries
  `security.severity` while the five existing keys are unchanged, character for character.
- A request with an unreadable `severity` still delivers, and the result carries a diagnostic naming it.
- The permanent-refusal fallback: a page to a route the stub gateway answers 404 for arrives on the
  default route, once, carrying the line that names the refusing route, and the leg is dead-lettered
  exactly once.
- The route-name change in posture: one place, and every command that submits reads it.
- Live acceptance, operator-run after an apply: `pns doctor` reports the `posture` route served, one real
  digest arrives in the chosen Discord channel, and `pns failures` stops growing on route `posture`.
