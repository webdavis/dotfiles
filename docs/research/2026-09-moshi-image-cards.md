# Moshi image recap cards, transport re-check, 2026-09-14

## The question

`docs/remaining-work.md` holds Moshi image recap cards as "blocked on transport, not ready to build",
with three recorded reopening conditions: a homelab HTTPS (hypertext transfer protocol secure) image
host, an upstream upload interface, or a documented data-URL (uniform resource locator) path. The task
asks one thing of this pass: does any of the three now hold, recorded as a dated verdict, with no
renderer built. The source design is `~/.claude/pipeline/slices/design-moshi-image-cards.md`, dated
2026-09-01, and there is no predecessor research document.

Measured overnight 2026-09-13 into 2026-09-14 on dresden. Nothing was built, no pns source was edited, no
card was sent, and the operator's Moshi token was never read or used.

## Verdict

**Condition 2 holds. Reopen the transport gate; keep the build blocked, now on value rather than on
transport.** Moshi's own notification documentation carries a working upload interface,
`POST https://api.getmoshi.app/api/v1/images/upload`, authenticated with the same token pns already holds
in `[plugins.mobile] token`. The route is live: an unauthenticated post answers `401` and a bogus bearer
answers `401 Invalid token`, while a deliberately wrong sibling path answers `404`. So the "nothing can
deliver the bytes" half of the 2026-09-01 design is no longer true.

The other two conditions still fail, and one of them now fails for a stated reason rather than for lack
of documentation:

- **Condition 1, a homelab HTTPS image host: does not hold**, and would not help if it did. The
  documentation says Moshi "passes it to Expo as a rich-content attachment", so the fetch happens on
  Moshi's side, not on the phone. A tailnet-only address cannot serve that fetch even though the iOS
  device `mister` is on this tailnet, and this node still has no Tailscale Funnel capability.
- **Condition 3, a documented data-URL path: does not hold**, and the Expo sentence above explains why it
  never will in this shape. A server-side rich-attachment fetch of `data:` is not an HTTPS fetch.

**The recommendation is still do not build, for the reason the design already gave and this pass did not
re-litigate: the best producer duplicates the Discord recap.** What changed is which sentence the ledger
should carry. "Blocked on transport" is now false and must be rewritten as "transport available, unbuilt
by value judgment, one operator decision away". That is a decision this document does not make for the
operator, which is why this task is not ticked.

**The probe the task authorizes has changed shape and got cheaper.** The recorded probe was a single card
carrying a `1x1` data-URL image. That probe is now pointless: the answer is documented. The probe worth
running is Moshi's own built-in one. The documentation states that "the app's notification settings also
include an image test action for checking rich-notification delivery on a physical device". That is zero
code, zero token handling, and one tap on the phone, and it answers the only question a probe was ever
for, namely whether a rich image notification actually displays on this device. Run that before any curl.

## What was checked, and how

Local tooling, all on dresden:

| Check                   | Command                     | Result                      |
| ----------------------- | --------------------------- | --------------------------- |
| moshi-hook version      | `brew info moshi-hook`      | 0.3.16 here, 0.3.22 in tap  |
| moshi-hook commands     | `moshi-hook --help`         | no `upload` subcommand      |
| moshi-hook settings     | `moshi-hook set --help`     | no image or upload knob     |
| Funnel capability       | `tailscale status --json`   | no funnel cap listed        |
| serve and funnel config | `tailscale funnel status`   | "No serve config"           |
| iOS on the tailnet      | `tailscale status`          | `mister`, iOS, idle         |
| renderers installed     | `which rsvg-convert magick` | both present, plus qlmanage |
| renderers declared      | the brew bundle data file   | `imagemagick`, `librsvg`    |
| curl config on stdin    | `curl --config -`           | accepted, curl 8.22.0       |

The tap's release notes for 0.3.17 through 0.3.22 were read at
`https://github.com/rjyo/homebrew-moshi/releases`. None of the six releases mentions images, uploads,
attachments or the webhook API (application programming interface). The entries cover Pi agent detection,
Herdr sidebar controls, Codex session resets, Browser Preview port ranges, per-host terminal session
limits, a GPT-6 Astra model row, a Git init fix, and embedded web app bumps. So the moshi-hook CLI
(command-line interface) side of condition 2 is unchanged since the design; what moved, or what the
design missed, is the web API side.

Upstream documentation, fetched and then read from the raw pages rather than only through a summarizer:
`https://getmoshi.app/docs/notifications`, `/docs/files`, `/docs/image-paste`.

Route existence, the one network probe run here, carrying no real credential:

```
POST https://api.getmoshi.app/api/v1/images/upload                      -> 401
POST same, Authorization: Bearer definitely-not-a-real-token            -> 401  body "Invalid token"
POST https://api.getmoshi.app/api/v1/images/upload-nonexistent-control   -> 404
```

Code read in this checkout, at these lines:

- `pns/crates/pns-adapters/src/destinations/moshi.rs`, the whole 182 lines: the token rule, the webhook
  body builder, `herdr_link`, and the delivery seam.
- `pns/crates/pns-adapters/src/destinations/moshi/http.rs`: `UreqPost`, a 10 second whole-request
  deadline, `max_redirects(0)`, and `DELIVERED_STATUS = 200..300`.
- `pns/crates/pns-application/src/replay_missed.rs:91` and `:102`: `RecapPublisher::publish` then
  `missed::recap_card`.
- `pns/crates/pns/src/return_replay.rs:85`: `publish` is `spawn_recap`, so the digest child is detached
  and the card is delivered in the calling process immediately after.
- `pns/crates/pns-adapters/src/recap_child.rs:67`: the child's `RECAP_DEADLINE_SECS = 30`.
- `pns/crates/pns-domain/src/render.rs:8`: `PREVIEW_MAX_CHARS = 260`.
- `dot_config/pns/private_config.toml.tmpl:184` and `:199`: `replay_card = true`, `min_events = 8`.
- `dot_config/pns/config-values.toml:16`: the token's provenance,
  `{ keepassxc = "moshi-hook :: Device Token", field = "Password" }`.
- `pns/crates/pns-adapters/Cargo.toml:30`:
  `ureq = { version = "3.4.0", default-features = false, features = ["rustls"] }`, confirmed at that
  version in `pns/Cargo.lock`.

`ureq` 3.4.0's own documentation was checked for a multipart body helper: one exists, behind a
`multipart` feature, in the `unversioned` module. That module states its own policy plainly: "Breaking
changes to anything under the module `unversioned` ... will NOT be reflected in a major version bump of
the `ureq` crate."

Homelab was checked in its own checkout at `~/workspaces/Ivy/webdavis/homelab`. Every HTTPS ingress in
its plans is internal: Traefik fronting `*.uriel.webdavis.io`, and the self-hosted ntfy server is
described as one the "phone subscribes via Tailscale". These are plans in `docs/plans/`, not deployed
services, and none of them is a public host.

## Findings

### 1. The upload interface is documented, live, and reachable with the token pns already holds

The notifications page carries the whole round trip. Quoted from the page:

> The image URL must be reachable without authentication when the notification is delivered. Moshi passes
> it to Expo as a rich-content attachment and also includes it in the notification data.

> If the image is local, upload it to Moshi first with the same API token:

```
curl -X POST https://api.getmoshi.app/api/v1/images/upload \
  -H "Authorization: Bearer YOUR_API_TOKEN" \
  -F "file=@/path/to/screenshot.png"
```

The response carries an eight-character `code`:

```json
{
  "id": "abcde",
  "code": "abcde1xy",
  "expires_at": "2026-07-23T12:00:00.000Z"
}
```

The public address is then `https://i.getmoshi.app/<code>`, sent as
`"data": {"type": "image", "url": "https://i.getmoshi.app/abcde1xy"}` on the ordinary webhook. The page
closes the section with the caps: "Moshi-hosted uploads are limited to 10 MB and 10 successful uploads
per hour. Their public links expire after one day."

The files page independently confirms the credential: "Uploads go over HTTPS to Moshi's web service.
Authentication reuses your push-notification token, so push notifications must be enabled". The token in
`[plugins.mobile]` is exactly that push token, sourced from the KeePassXC entry
`moshi-hook :: Device Token`.

**Whether this is new or was missed on 2026-09-01 could not be settled.** The notifications page's
structured data reports `dateModified` as 2026-08-26, which predates the design, and the Internet Archive
is serving a "Temporarily Offline" page, so no historical snapshot is available. The design read
`/docs/files`, which documents the phone-side Files screen and no endpoint, and concluded "no HTTP API is
documented"; the endpoint lives on `/docs/notifications` instead. Either the page changed without moving
its `dateModified`, or the endpoint was on the wrong page for the way the design searched. The practical
consequence is identical and this document does not need the answer.

### 2. The three caps are not constraints for this producer

The recap card's render measured 8.5 KB quantized, 22 KB from the pure-standard-library PNG (portable
network graphics) writer, and 50 KB from the full `rsvg-convert` render of an SVG (scalable vector
graphics). Against a 10 MB ceiling all three are noise. The 10-uploads-per-hour ceiling is not a
constraint either: the card fires only where `fires` is true in `replay_missed`, which needs `digest`, a
durable route, a window, and `counted.len() >= min_events`, and the shipped `min_events` is 8. A one-day
link expiry is correct for a recap, since the card is read on return or not at all, and a dead image on a
day-old card costs nothing.

### 3. The card-ownership refactor the design named is still required, and the upload leg makes it worse

Confirmed against current source. `replay_missed` calls `RecapPublisher::publish` (which is
`spawn_recap`, a detached child) and then delivers the card in the calling process a few instructions
later. So the card is dispatched before any render could exist. With the upload interface in play there
are now two network round trips between "render finished" and "card posted", the multipart upload and
then the webhook, which makes delaying the card inside the hook process worse than the design assumed.
The honest shape is unchanged and now better justified: the detached child renders, uploads, and posts
the one recap card, and the hook process posts nothing for that moment. That is a real refactor of
`replay_missed` and its return-moment claim, which is why the design split this into two pull requests
with the behavior-neutral ownership move first.

The child's 30 second deadline is comfortable for a render plus two posts; the existing post deadline is
10 seconds per request.

### 4. The token's path needs an explicit amendment, or the upload leg must be refused

`moshi.rs`'s module documentation states the rule in its own words: the token "is read from the config's
`[plugins.mobile]` table, placed in the request BODY, and never touches argv, the environment of a child,
or an error string". The documented upload interface takes the token in an `Authorization: Bearer` header
instead. A header is in-process, is not world-readable, and is not argv, so the rule's actual purpose
survives, but the rule as written says "the request body and nowhere else" and would have to be amended
to say body or Authorization header. This is a wording decision on a deliberately strict rule, so it
belongs to the operator rather than to a pull request that quietly widens it.

### 5. The renderer question is unchanged and is not the blocker

Every renderer the design measured is still present: `rsvg-convert` and `magick` from the declared brew
bundle, `qlmanage` from macOS. The zero-new-dependency route is still the pure-standard-library 1-bit PNG
writer, roughly 150 lines including an embedded glyph table, which the design measured at under a
millisecond and about 22 KB. Nothing here needs re-deciding; no renderer is the reason this is unbuilt.

### 6. The value objection stands, and this pass did not attempt to overturn it

The design's ranking of five candidate producers put the recap table first and judged the other four
worthless on the phone. Its objection to the winner is that the full recap already lands in Discord
through hermes and the card already says "recap in #pns", so the image saves the operator one tap. The
phone card's text is capped at 260 characters, which fits two or three urgent titles plus counts, and the
image would show the whole table. Whether one saved tap is worth two pull requests and a refactor of the
return-moment path is a value call, not a technical one, and it is the operator's.

One structural cost to weigh in that call: `data` holds one object with one `type`, so an image card
cannot also carry the herdr deep link. For the recap specifically that is acceptable, because the window
spans many panes and the link only resumes a card Moshi already holds, but it is a real loss on the one
card that would gain the image.

## What the build would cost, if the operator says yes

Recorded so the value call is made against a real number, not to start work.

Two pull requests, each under 300 lines including tests, per the design's split:

1. Move recap card ownership into the detached `pns recap` child, text card unchanged, behavior-neutral,
   pinned by tests that the hook process posts nothing for that moment and the child posts exactly one.
1. The renderer, the upload leg, the image body, and the knob.

New surface in (2): a render seam beside the existing `HttpPost` so a fake can record; a multipart upload
call; `[plugins.mobile] image_cards = false`, shipped commented out per the config hygiene ruling; and a
fallback that is pinned by test, where a render failure, a non-2xx upload, or a non-2xx webhook all fall
back to today's text card with its deep link and the event is never dropped.

The one new implementation choice the design could not have anticipated is how to build the multipart
body. `ureq` 3.4.0 ships a `multipart` feature, but it lives in the `unversioned` module whose stated
policy is that breaking changes there will not produce a major version bump. The alternative is roughly
twenty lines of hand-built body through the existing `send()`, which needs no feature change and no
unstable surface.

## Assumptions made in the operator's place

Each of these is a choice this document made overnight rather than waking the operator, with the
alternative stated so it can be overturned.

1. **A documented web API endpoint satisfies "an upstream upload interface".** The recorded condition
   does not say which upstream surface counts, and moshi-hook itself still has no `upload` subcommand.
   Read as "moshi-hook exposes an upload", condition 2 fails and the task stays blocked exactly as
   written. This document takes the broader reading, because pns does not use moshi-hook for the card at
   all: it posts to `api.getmoshi.app/api/webhook` itself, so a web endpoint is the surface it would
   actually call.
1. **No probe was run that uses the operator's token or sends a card.** The route-existence probe used a
   bogus bearer and a control path. The alternative was to read the token out of
   `~/.config/pns/config.toml` and settle actual image display tonight, which would have spent one of the
   ten hourly uploads and pushed a real notification to a sleeping phone. The task reserves the probe for
   the operator, so it stays reserved.
1. **The probe is re-specified as the phone's own image test action first, curl second.** The recorded
   probe was a data-URL card, which the documentation now answers without a probe. The alternative is to
   run the recorded probe anyway for the record; that spends a notification to confirm a documented
   negative.
1. **A hand-built multipart body is preferred to `ureq`'s `multipart` feature.** The alternative is to
   enable the feature and accept the `unversioned` module's no-major-bump policy on a tool other people
   install with `cargo install`.
1. **The token may ride an `Authorization` header on the upload leg, subject to the operator's ruling.**
   The alternative is to treat `moshi.rs`'s "body and nowhere else" sentence as binding as written, which
   refuses the upload leg and leaves the task blocked on the same transport it just got.
1. **The design's value judgment was carried forward, not re-opened.** This pass verified that the
   design's code claims still hold and did not re-rank the five producers or re-argue whether one saved
   tap is worth the work. The alternative is a fresh value pass, which would need the operator's own
   sense of how often they read a recap card on the phone, which no document can measure for them.
1. **The pull request sizing was not re-costed.** The design's two-pull-request, sub-300-line estimate is
   carried forward on the strength of its call sites still existing, not on a fresh line count.

## What would change the verdict

- **Moshi's image test action fails on this device.** Then rich image notifications do not display here
  at all and the whole line closes, transport or not.
- **The operator rules that one saved tap is worth it.** Then nothing technical stands in the way and the
  two pull requests are writable as specified, under the header amendment in finding 4.
- **Moshi ships a card carrying both an image and a deep link.** The one-`type` limit is what forces the
  recap card to trade its pane link for the image. If that trade disappears, the value objection gets
  materially weaker, because the image would stop costing anything.
- **A producer with no Discord twin appears.** The design's condition (c), a failed-tests table from a
  future editor-side producer, is the one candidate whose value does not duplicate the recap. Design an
  image card with that producer if it ever exists, not before.
- **The upload endpoint stops answering, or its terms change.** The route probe above is the cheap
  re-check; it needs no credential.
- **A Funnel capability grant.** This would revive condition 1 and make a self-hosted image address
  possible, but it is an operator posture decision about exposing the workstation to the internet, and it
  buys nothing that the Moshi-hosted upload does not already provide more cheaply.

## Open questions for the operator

Question 2 was answered by the operator on 2026-09-15, and question 5 follows from that answer; both are
recorded again in `docs/decisions/2026-09-15-pns-behavior-backlog-brief.md` so the two documents agree.
Question 3 is left open for the build to answer with evidence. Questions 1, 4 and the unrelated
moshi-hook item were not part of that ruling.

1. **Does the phone's own image test action display a rich image notification on `mister`?** Settings,
   notifications, the image test action. Zero code, no token handling, one tap. Everything below is moot
   if this fails.
1. **Is one saved tap into Discord worth two pull requests and moving recap card ownership into the
   detached child?** **Answered: the capability is approved**, and it covers every card type, the recap
   included. What the operator declined is an image on their own recap card, which is a setting in their
   own config, not a limit on the capability; another user might want exactly that for their recap. The
   pattern is the one this repository already uses everywhere: build the capability, ship it off, and
   leave it off in the operator's own configuration. It is opt-in per card type because of a real
   tradeoff: a Moshi card's `data` carries one `type`, so turning images on for a card type gives up the
   deep link that focuses the originating herdr pane when that card is tapped. The operator's own
   configuration keeps the recap card's images off and keeps its deep link.
1. **May the Moshi token ride an `Authorization: Bearer` header on the upload leg?** The documented
   interface requires it, and `moshi.rs`'s rule currently says body and nowhere else. **Left open**: now
   that image cards are approved, this question is live again, and it is left for the build to answer
   with evidence rather than re-decided here.
1. **Do you want the real round trip proved with your token, and if so may it happen while you are
   awake?** The two commands are below. They spend one of ten hourly uploads and send one real card.
1. **Does the ledger entry get rewritten, or does the task get closed outright?** Rewritten: the entry
   now reads transport-available, capability-approved-as-opt-in, not-yet-started, tracked as ledger task
   88\.
1. **Unrelated to this task, and noticed while checking it: moshi-hook is six releases behind** (0.3.16
   installed, 0.3.22 in the tap). Worth a separate `brew upgrade moshi-hook` decision, since 0.3.20
   through 0.3.22 carry Pi agent detection, Codex named-session reset fixes and Herdr sidebar controls
   that touch this machine's daily path.

### The operator-run probe, if question 4 is yes

Run only after question 1 passes. The token stays out of argv by riding a curl config on standard input,
which was verified working on the installed curl 8.22.0.

```bash
# A small legible PNG, rendered by the librsvg already in the brew bundle.
# Run here while writing this: 360x120, 4,287 bytes, no network touched.
image="${TMPDIR:-/tmp}/moshi-image-probe.png"
printf '%s' '<svg xmlns="http://www.w3.org/2000/svg" width="360" height="120"><rect
width="360" height="120" fill="white"/><text x="16" y="70" font-family="Menlo"
font-size="32">image probe</text></svg>' | rsvg-convert -o "$image"

# Matches exactly one line of the deployed config; verified without printing the value.
token=$(sed -n 's/^token = "\(.*\)"$/\1/p' "$HOME/.config/pns/config.toml")

# 1. Upload. Expect a JSON body with id, code and expires_at.
code=$(printf 'header = "Authorization: Bearer %s"\n' "$token" |
  curl --config - -sS -X POST \
    -F "file=@$image" \
    https://api.getmoshi.app/api/v1/images/upload | jq -r .code)
printf 'code: %s\n' "$code"

# 2. One real card carrying that image. Expect 2xx and an image on the phone.
jq -n --arg token "$token" --arg url "https://i.getmoshi.app/$code" \
  '{token: $token, title: "image probe", message: "one card, one image",
    data: {type: "image", url: $url}}' |
  curl -sS -X POST -H "Content-Type: application/json" --data-binary @- \
    -w '\nstatus=%{http_code}\n' https://api.getmoshi.app/api/webhook
```

A 2xx on both with an image visible on the phone card means finding 3's refactor is the only thing
between here and a working image recap card, and question 2 is the only remaining gate. Anything else
closes this line for good, and the ledger entry becomes "closed, image cards do not display".

Note on the token in step 2: the webhook takes the token in the body, which is the path `moshi.rs`
already uses, so that half needs no amendment. Only the upload leg's header does.
