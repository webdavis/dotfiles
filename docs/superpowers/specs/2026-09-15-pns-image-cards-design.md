# Opt-in image cards and uu's weekly run bar chart

Status: design, written 2026-09-15. Nothing built. Every choice made in the operator's place is under
"Assumptions", and "Open questions" holds only what the operator must answer. Every pns, uu or posture
claim carries the file it was read in, in this checkout; every Moshi API claim is carried forward from
`docs/research/2026-09-moshi-image-cards.md`, dated 2026-09-14, rather than re-fetched here.

## What the operator ruled, 2026-09-15

These are implemented rather than revisited:

1. **The capability is approved and covers every card type.** What the operator declined was an image on
   their own recap card, a setting in their own configuration, not a limit on the product. Another
   operator might want exactly that. So the capability is built, shipped off for every card type, and left
   off in this operator's own configuration. No card type is written off in the product itself.
1. **It is per card type and opt-in, because of a real cost.** A Moshi card's `data` object carries one
   `type` (`pns/crates/pns-adapters/src/destinations/moshi.rs:82-88`), so a card that carries an image gives
   up the deep link that focuses the originating herdr pane when tapped
   (`pns/crates/pns-adapters/src/destinations/moshi.rs:29-49`). Turning images on for a card type loses
   tap-to-focus for that card type.
1. **The tradeoff must be stated where the operator opts in**, not only in this document. Section 2 below
   works out what that means in this repository's config habits.
1. **Images must be judicious, not decorative.** An image earns its place only when it carries something
   the text cannot.

### The analysis that produced the first use

pns sends six lamp behaviours (`BEHAVIOUR_WORDS` in
`pns/crates/pns-domain/src/lamps/config/behaviour.rs:22-28`: done, failed, blocked, unread, loop, github)
plus the recap, uu's weekly run, and posture pages. Sorting by whether the operator taps the card:

- **Always tapped, so an image would actively hurt:** blocked, loop, unread. These exist to send the
  operator somewhere; an image that cost the deep link would remove the card's one job.
- **Usually tapped:** failed, done, a GitHub failure.
- **Read and closed, so an image costs nothing:** the recap, uu's weekly run, posture's digest, a GitHub
  pass.

Of the last group, only two are cases where a picture beats the text: uu's weekly run rendered as one bar
per lane, green or red, so a broken lane is visible without reading every line; and posture's digest as a
sparkline over the week, because text cannot show a trend. **The first use is uu's weekly run bar chart.**
Ship the capability plus that one use, and nothing else turned on. If it does not earn its place in a
month, nothing else will; that is this design's success criterion, on the operator's own framing.

## What is built today

**The card and its one `data` slot.** `webhook_body` in `moshi.rs:56-90` builds
`{"token", "title", "message"}` and, only when a deep link exists, one `data` object holding either
`{"type": "url", "url": ...}` or nothing; the module's own comment states the constraint plainly: "ONE
`data` object carrying ONE `type`, which is what makes a url action and an image action mutually
exclusive: a structural limit of the field, not a rule Moshi states" (`moshi.rs:82-84`). The POST itself
runs through `HttpPost::post_json`, a 10 second deadline, no redirects followed
(`pns/crates/pns-adapters/src/destinations/moshi/http.rs:1-39`).

**The upload interface exists and is reachable with the token pns already holds.** Per the 2026-09-14
research, `POST https://api.getmoshi.app/api/v1/images/upload` takes the same `[plugins.mobile] token`
in an `Authorization: Bearer` header, answers with an eight-character `code`, and the resulting
`https://i.getmoshi.app/<code>` link is what a webhook's `data.url` then names for an image action. Moshi
hosted uploads cap at 10 MB and 10 successful uploads per hour, links expire after one day, and the whole
round trip was proved live (a bogus bearer answered `401`, a control path answered `404`), with no card
sent and no real token used.

**The recap card's ownership is not yet where an image render would need it to be.**
`ReplayMissedNotifications::run` calls `RecapPublisher::publish`
(`pns/crates/pns-application/src/replay_missed.rs:91`), which resolves to `spawn_recap`, a DETACHED child
(`pns/crates/pns/src/return_replay.rs:85`: `impl RecapPublisher for CatchUp { fn publish(...) { spawn_recap(...) } }`),
and then the calling process posts the card a few instructions later
(`replay_missed.rs:102`, `missed::recap_card`). So the card is dispatched before any render inside the
detached child could exist. The child's own deadline is comfortable for more work:
`RECAP_DEADLINE_SECS = 30` (`pns/crates/pns-adapters/src/recap_child.rs:67`), against the moshi post's
10 seconds per request.

**Producer data already crosses into pns through `extensions`, and a destination can already read it back
out of the raw envelope.** `pns-protocol`'s `Request::extensions: Map<String, Value>` is documented as
"Producer-specific data, carried verbatim and never read here"
(`pns/crates/pns-protocol/src/request.rs:129-131`), additive by the schema's own promise: "a key not in
this list is ignored and named, never refused" (`request.rs:23-25`). One extension is already read: the
GitHub source design's `extensions.github` is decoded at the submit boundary by
`pns_adapters::github_event`, which returns `Ok(None)` for an absent key and a named refusal for a
malformed one, never dropping the event over it
(`pns/crates/pns-adapters/src/github.rs:20-39`, `pns/crates/pns/src/event_flow/submit.rs:63-68`). That
decoded value does not ride the internal `EventArgs` (`pns/crates/pns-domain/src/notification.rs:39-65`
has no field for it); it rides beside it as `ProducerRequest.github`, consumed only by the lamp-flash
decision in `execution.rs:294-296` and `pns-application/src/submit_notification.rs:161`. A DIFFERENT,
more general seam already reaches every destination: `DeliveryRequest.producer_request: Option<&str>`
(`pns/crates/pns-application/src/destinations.rs:32-39`) carries the WHOLE encoded `pns.request` envelope,
populated in `execution.rs:223` from `producer.encoded`. One destination already reads it back: the
banner destination decodes `producer_request` with `pns_protocol::decode_request` to inspect the original
`class` field for a sound choice (`pns/crates/pns-adapters/src/destinations/banner.rs:147-157`). A recap
card, by contrast, is never producer-submitted and carries `producer_request: None`
(`pns/crates/pns/src/recap_delivery_runtime.rs:70`), so this seam reaches uu's bar chart but not the
recap.

**uu's weekly record bypasses pns entirely today, and carries no structured chart data.** `record.rs`
states it plainly: "one entry per run, posted straight to the hermes gateway"
(`uu/crates/uu-adapters/src/record.rs:1-7`). `record_detail` composes one free-text blob: a header, the
gap line, and per-lane verdict lines built from `LaneReport` (`record.rs:38-71`), posted through uu's own
`signed_post.rs`, which is deliberately NOT a dependency on `pns-hermes`, "uu and pns ship as separate
projects, installed independently by people who do not have this repository"
(`uu/crates/uu-adapters/src/signed_post.rs:1-15`). uu's only existing path INTO pns at all is the alert
leg, argv flags spawning the deployed `pns` binary for a single failed lane
(`uu/crates/uu-adapters/src/alert.rs:1-10`), never the JSON producer protocol, and never the weekly
record.

**The producer-command mode this design needs for uu already exists, built for posture.**
`posture-adapters/src/producer.rs:1-8` states the contract in one sentence: "A JSON request goes in on
standard input, a JSON result plus an exit code comes back, and the command and its arguments are both
config. posture therefore names no engine." `[delivery] mode = "producer"` plus
`[delivery.producer] command = ...` and `arguments = [...]` is posture's config shape
(`posture/crates/posture-adapters/src/delivery/tests.rs:12-13`), read against posture's own copy of the
wire contract, `posture_producer_wire` (`producer.rs:16`). uu has no equivalent mode; its weekly record
has exactly one destination today, hermes, chosen at compile time rather than by config.

**Renderers are already declared and already fast.** The 2026-09-14 research measured the recap's own
render (never shipped) at 8.5 KB quantized, 22 KB from a pure-standard-library PNG writer, and 50 KB from
a full `rsvg-convert` render, all under a millisecond; `rsvg-convert` and `magick` are both declared in
the brew bundle already (`.chezmoidata/system_packages_autoinstall.yaml`).

## Decisions

### 1. The capability is one config-driven fork inside the mobile destination, off everywhere by default

`MoshiChannel::deliver` (`moshi.rs:129-149`) gains a second body shape beside `webhook_body`: when the
event's card type is turned on in `[plugins.mobile.image_cards]` AND a renderable attachment exists for
this event, the `data` object is built as `{"type": "image", "url": ...}` instead of `{"type": "url",
...}`. The deep link is not deleted; it is a fact this design records as a structural cost, per ruling 2.
When the toggle is off, or no attachment exists, delivery is byte-for-byte what it is today. No event ever
carries both, because Moshi's field cannot.

### 2. The config surface: `[plugins.mobile.image_cards]`, an open table, the tradeoff in its own comment

A new opt-in child table, mirroring the shape `PLUGINS_DISCORD_CHANNELS` already establishes for an open,
operator-named key set (`pns/crates/pns-adapters/src/config/render/layout/destinations.rs:44-79` in the
2026-09-15 Discord design). Keys are not an enumerated roster: pns compiles in no list of card types, the
same reasoning `routes.rs` already states for route names ("NO COMPILED ROSTER ... pns and every producer
that posts through it are separate tools that learn about each other when somebody configures them
together", `pns/crates/pns-domain/src/routes.rs:1-15`). A key is either one of pns's own six behaviour
words or `recap`, or, for a producer-submitted event, `<producer>.<event>` taken verbatim from the wire
request's own `producer` and `event` `Name` fields (`pns-protocol/src/request.rs:104-105`). The first
entry this design ships is `"uu.run"`, and pns's source never spells the word `uu` anywhere but a
committed config value, the same way the Discord map's `dotfiles` and `pns` rows are config rather than
compiled constants.

Because `dot_config/pns/config-values.toml` is the committed input and
`dot_config/pns/private_config.toml.tmpl` is `just pns-config-render`'s regenerated output
(`dot_config/pns/config-values.toml:1-13`), the tradeoff lives in the LAYOUT comment for this table, not
hand-typed into the template: a `Table` entry in `destinations.rs` carries `prose` the way
`mobile_watch_card`'s does today (`destinations.rs:26-32`), something in the shape of:

```
# Which card types carry an image instead of the herdr pane deep link.
# The two never ride together: Moshi's `data` holds one `type`, so turning
# a card type on here trades tap-to-focus for that card type away. Off for
# every card type until named here; a render or upload failure always
# falls back to today's text card rather than losing the event.
```

The table itself renders commented out, on the same "ABSENCE IS NOT NEUTRAL" rule the values file already
states for an opt-in feature nobody has armed yet (`config-values.toml:8-10`): a table this file never
mentions renders as a commented block, an armed one is a key added under it. Shipped state: the table is
present, commented, with `# "uu.run" = true` as the one documented example line, and every real key
absent, so the operator's own configuration matches ruling 1 exactly, capability built, everything off.

### 3. Who renders, and how it crosses without either tool naming the other

**pns renders.** uu computes ONLY the structured pass/fail facts it already has as `LaneReport`
(`record.rs:38-71` already walks this list to build the text lines), and hands them across the producer
protocol as a new, generic, versioned extension, not an image: `extensions.chart = {"kind":
"bar-pass-fail", "bars": [{"label": "...", "ok": true}, ...]}`. This is a wire-contract addition to
`pns-protocol` in the exact spirit `extensions` already promises ("additive fields from a newer producer
must not break an older pns", `request.rs:23-25`) and the exact shape `extensions.github` already proves
out end to end (`github.rs:20-39`): a new `pns_adapters::chart_event(&extensions)` function, same
optional-and-malformed-is-refused-not-dropped contract.

The reasons rendering happens on pns's side and not uu's:

- **Only pns's process ever holds the moshi token, the upload client, the delivery ledger, retry and
  dead-letter machinery, and the presence gate.** Duplicating all of that in uu so uu could render, upload
  and post its own card would be a second implementation of exactly what `moshi.rs` already is, for the
  same destination, the same token, the same phone. That fails the "does this tool still build with pns
  absent from the filesystem" test the workspace-independence rule states
  (`uu/crates/uu-adapters/Cargo.toml:1-11`) in the other direction: it would make uu need to know Moshi
  exists at all, a destination-specific concept that belongs to whichever tool actually talks to Moshi.
- **uu naming nothing about images or Moshi is what keeps "uu names no engine" literally true.** uu's
  contribution is a generic `bars: [{label, ok}]` shape any producer could populate for any bar-shaped
  card; nothing in it says pns, moshi, or an image.

**How it reaches the destination without a new field on `EventArgs`.** `EventArgs` stays exactly as
narrow as it is today (`notification.rs:39-65`); no chart data rides it, the same way `github` never
does. `DeliveryRequest.producer_request` already carries the whole encoded envelope to every destination
(`destinations.rs:32-39`), and `banner.rs:147-157` already proves the read-it-back-out pattern live: decode
`producer_request` with `pns_protocol::decode_request`, read a field the caller never threaded through.
`MoshiChannel::deliver` does the same thing banner already does, reading `request.producer_request`,
decoding it, and calling `chart_event` on `.request.extensions` when the resolved card-type key names a
`true` entry in `[plugins.mobile.image_cards]`.

**Where the pixels get made.** A small render seam beside the existing `HttpPost` trait
(`moshi/http.rs:1-11`), so a fake can record in tests: `bars: &[Bar]` in, PNG bytes out. The
zero-new-dependency route the 2026-09-14 research already measured is the pure-standard-library writer
(roughly 150 lines, sub-millisecond, ~22 KB for the recap's larger table; a six-lane bar row is smaller
still); `rsvg-convert`, already declared in the brew bundle, is the fallback if the standard-library
writer proves too limited for a labeled bar chart specifically, a build-time call this design leaves open
rather than pre-deciding without a rendered sample in hand.

### 4. The upload leg, and the token question this design does not answer for the operator

The multipart POST to `https://api.getmoshi.app/api/v1/images/upload` carries the token in an
`Authorization: Bearer` header, per the documented interface. `moshi.rs`'s own module documentation states
the rule as written today: the token "is read from the config's `[plugins.mobile]` table, placed in the
request BODY, and never touches argv, the environment of a child, or an error string"
(`moshi.rs:6-9`). A header is in-process and not world-readable, so the rule's actual purpose survives,
but the sentence as written says body and nowhere else. **This design does not decide whether that
sentence is amended**, per the 2026-09-14 research's own open question 3, carried forward unresolved
below rather than settled here without the operator's ruling on a deliberately strict security rule.

`ureq` 3.4.0 ships a `multipart` feature, but it lives in the crate's own `unversioned` module, whose
stated policy is that breaking changes there "will NOT be reflected in a major version bump." The
alternative, a hand-built multipart body over the existing `send()`, is roughly twenty lines and needs no
unstable surface; which one the build actually uses is left for the implementing pull request to answer
with a working body in hand, not decided here on paper.

### 5. Failure never loses the event; it falls back to today's text card

Three points can fail: the render, the multipart upload, and the webhook post itself. All three take the
SAME path: build and send today's text card, with its deep link intact, instead. This is not a new idea;
it is the same trade `channel_url` already makes for an unusable route name in the Discord design's own
words, "a message in the wrong place beats a message nowhere"
(`pns-adapters/src/destinations/hermes.rs:82-90`, quoted in
`docs/superpowers/specs/2026-09-15-pns-discord-destination-design.md:253-255`). Concretely: a render
failure or a non-2xx upload response is caught before the webhook body is built, and the ordinary
`webhook_body` runs instead; the event is never dropped and the outcome pns reports is `Delivered` or
`Failed` exactly as it is today, with no third state for "sent, but as text."

### 6. An image card is never chosen automatically

Every fork in decision 1 is gated on the config table from decision 2. There is no heuristic, no
size-of-payload threshold, no "this looked chart-shaped" inference anywhere in this design. A card carries
an image because its card-type key is `true` in `[plugins.mobile.image_cards]`, full stop; the table
render's absent-by-default state means a fresh install, and this operator's own configuration, carry no
image cards until a key is added by hand.

### 7. The recap's card-ownership refactor is real, and it is not a prerequisite for the first use

The refactor `replay_missed.rs:91` and `return_replay.rs:85` names is required BEFORE `recap` may ever be a
`true` key in `[plugins.mobile.image_cards]`, because today's card is posted in the calling process while
the render (if any) would happen in a detached child that has not finished. It is orthogonal to uu's bar
chart, which needs no such refactor: a producer submission's delivery is already synchronous end to end,
one process, one deadline, no detached child in the way. This design records the refactor's shape so a
future recap-image pull request does not re-derive it, but does not schedule it in the build ladder below,
because the first use does not need it and no card type is written off by leaving it undone. The honest
shape, unchanged from the research: the detached child renders, uploads, and posts the one recap card, and
the hook process posts nothing for that moment, pinned by a test that the hook process posts nothing and
the child posts exactly one.

## Assumptions

1. **A card-type key is `<producer>.<event>` for a producer-submitted event, or the bare behaviour/recap
   word for pns's own.** *Alternative:* a single flat namespace requiring every producer to coordinate
   unique key names by hand, which the `owner/name` versus bare-name split in the Discord design's channel
   map already shows is the wrong default when a collision is possible.
1. **pns renders and uploads; uu only supplies structured facts.** *Alternative:* uu renders and uploads
   itself, which needs uu to gain a second, independent implementation of everything `moshi.rs` already is
   for the one destination that would ever show the image.
1. **The crossing is a new `extensions.chart` shape, read at the destination via the already-live
   `producer_request` seam, exactly the pattern `banner.rs` already uses for `class`.** *Alternative:* add
   a new field to `EventArgs` itself, which would carry chart data through paths (recap, retries with no
   original request) that structurally cannot have any.
1. **uu gains posture's exact `[delivery] mode = "producer"` shape for its weekly record**, as an
   alternative to today's direct-to-hermes post, not a replacement for it. *Alternative:* a uu-specific
   config shape, which duplicates a contract posture already proved rather than reusing it.
1. **The renderer is the pure-standard-library PNG writer first, `rsvg-convert` as fallback**, decided by
   the implementing pull request once a labeled bar chart is actually rendered and looked at.
   *Alternative:* commit to `rsvg-convert` now, on the strength of the recap's larger, unlabeled table
   render measuring fine, which is not proof a small labeled chart needs the same tool.
1. **The `moshi.rs` "body and nowhere else" token rule is left open rather than amended here.** *Alternative:*
   amend it in this document, which would decide a deliberately strict security rule without the operator
   in the room, exactly what the 2026-09-14 research already declined to do.
1. **Any of render, upload, or webhook failing falls back to today's text card rather than dead-lettering.**
   *Alternative:* dead-letter the event on an image-specific failure, which loses an event the text-only
   path would have delivered fine.

## Open questions

1. **May the Moshi token ride an `Authorization: Bearer` header on the upload leg?** Carried forward
   unresolved from the 2026-09-14 research. A "no" here does not block this design: it only removes the
   image leg's ability to upload at all, at which point every image-card delivery falls back to text by
   decision 5, and the capability ships with no card type able to arm past the config layer until the rule
   is amended.
1. **Is the pure-standard-library PNG writer legible for a labeled, six-lane bar chart**, or does it need
   `rsvg-convert`'s text layout? Answered by rendering one and looking at it, not by this document.
1. **Does the operator want `"uu.run"` specifically, or a different first card-type key** if uu's config
   surface changes shape before this ships (for instance, a config key spelled differently from `run`)?

## Build ladder

Four pull requests. Tests are behavior of tools we wrote, each under a second, and none of them talks to
Moshi, Discord, or a real chart file: the render and the HTTP client are both behind seams a fake can
record.

**PR 1: the wire contract and its decode.** `pns-protocol`'s `extensions.chart` shape documented beside
`extensions.github`'s; `pns_adapters::chart_event`, mirroring `github_event`'s optional-and-refused-not-
dropped contract. Behaviors: an absent `extensions.chart` decodes to `None`; a well-formed one decodes to
its bars; a malformed one is refused by name and the event still takes the ordinary path.

**PR 2: the image-card capability in the mobile destination, shipped with every key off.** The render
seam and its fake; the multipart upload seam and its fake; the `data: {"type": "image", ...}` body
variant; the `[plugins.mobile.image_cards]` schema row and its `destinations.rs` layout entry carrying the
tradeoff comment; the fallback-to-text path on any of the three failures. Behaviors: a card-type key that
is absent or `false` posts exactly today's body; a key that is `true` with a decodable chart posts an
image body and no deep link; a render, upload, or webhook failure of an image-eligible card posts today's
text body instead, and the event is never dropped; the token never appears in a rendered failure line.

**PR 3: uu's producer-command delivery mode for its weekly record.** `[delivery] mode = "producer"` on
posture's exact shape; `record.rs` populates `extensions.chart` from `LaneReport` when a producer command
is configured. Behaviors: the default (`mode` absent, or `"hermes"`) posts exactly today's text-only
record with no `extensions.chart`; `mode = "producer"` hands the encoded request to the configured
command and reads its result the way `posture-adapters/src/producer.rs` already does; the bars in
`extensions.chart` match `LaneReport`'s own verdict-per-lane facts one for one.

**PR 4: wiring `"uu.run"` live.** `dot_config/pns/config-values.toml` gains the commented
`[plugins.mobile.image_cards]` table with its documented example line; the operator's own configuration
stays with every key off, per ruling 1; the runbook gains the capability's paragraph naming the one armed
key nowhere by default and the fallback behavior. No operator config change ships armed; arming `"uu.run"`
is a config edit the operator makes by hand once the render has been looked at.

## Deliberately out

**The recap's card-ownership refactor**, recorded in decision 7 for the pull request that eventually
wants it, not scheduled here.

**Posture's sparkline.** Named in the analysis as the other case where an image beats text, and nothing
more: no wire shape, no config key, no pull request. It waits for its own design once the first use has
run long enough to answer whether the capability earns its keep at all.

**Any removal mechanism.** Nothing in this design proposes deleting uu's direct-to-hermes path for its
weekly record; it stays the default, and the producer-command mode is an addition beside it, exactly the
posture precedent it borrows.
