# vpp's optional Open Notebook handoff

Status: design, written 2026-09-14 while the operator was asleep. Not approved, not built. No code was
written or changed. Nothing was sent to any service, and no Open Notebook instance exists to send to:
every claim about Open Notebook below comes from its documentation and its source, read today through
its public repository. Every choice made in the operator's place is listed under "Assumptions made in
the operator's place" with its alternative, and the questions that need an answer are at the end.

Scope: `docs/remaining-work.md`, the `Homelab plan coordination` section, second bullet.

> Coordinate vpp's optional Open Notebook handoff with L6. Preserve one capture/transcription pipeline
> and canonical originals; decide the handoff format during integration design. vpp and Bob must not
> require Open Notebook merely to read or produce ordinary notes.

Its first bullet is the section's premise and binds this one:

> Keep the requested server deployments in `webdavis/homelab`: Infisical (A6), NetBird (F4), Dozzle (F5)
> and Open Notebook (L6) are recorded in `docs/plans/PLAN-v12.md` and its service matrix. Dotfiles owns
> their needed laptop configuration and managed client updates. Record actual cross-project dependencies
> without importing the full homelab deployment backlog into this modernization.

The other half of the same coordination lives in homelab `PLAN-v12.md`, L6, read today rather than
remembered. Its third bullet is the one this document answers:

> Define an optional supported handoff from vpp (Voice Processing Pipeline), the planned Rust tool.
> Open Notebook must not create a second automatic Voice Memos capture/transcription workflow.

The done-means for this item, from the triage record: "A recorded handoff format that leaves vpp and Bob
fully working with Open Notebook absent." Both halves are mechanical, so this document's job is to make
them properties of the design rather than promises, and to name the one that cannot be mechanical.

## What this builds on

This is the seventh document in the vpp chain and it assumes the six before it.

**The reconciliation** established that no existing plan specifies a watcher for Apple Voice Memos, and
that the `agent-processing-pipeline/` layout in the vault is a filing convention rather than a running
pipeline.

**The boundaries design** put vpp's application code in its own project, macOS installation and service
configuration in dotfiles, the operator's content in a configured output directory, and server-side
deployments in homelab. Its four-homes table already names this item's subject: homelab owns "any
server-side deployment vpp may optionally use: Open Notebook (L6), a remote transcription host,
credential brokering" and never holds "anything vpp needs in order to run on the laptop". This document
is that row made concrete.

**The discovery design** produced the recording identity, `2026-08-24T144736-4f3ab19c02de`, and the rule
that Apple's originals are read-only to vpp.

**The redundant transcription design** produced the transcript of record, the review record with its
flag classes, the in-line `[unverified]` convention, `known-terms.txt`, and `vpp verify-note`.

**The tags, schema and filing design** produced the three-layer model: the record at
`~/.local/state/vpp/recordings/<id>.json` is authoritative, the note is a rendering, the index is a cache
rebuilt in 0.245 seconds. It also produced the two output profiles and `vpp path`.

**The meeting-briefs design** produced the occasion identity, the `vpp.brief/1` machine form in which
every item carries `certainty` and `sources` with no default, and the integration shape the whole chain
uses: "A consumer runs a command and gets a document. There is no daemon, no socket, no registration and
no push." This document extends that sentence to a consumer that happens to be a server.

**The redacted-draft design** produced `vpp share draft|review|approve|release`, the rule that vpp never
transmits anything, the destination refusals, and the byte-bound approval. Its central rule is the one
most at risk here, and it is restated as a constraint below.

Two chain disagreements remain open and are untouched here: the audio's home, and vpp's shipping name
(`VPP` collides with FD.io's Vector Packet Processing).

## Constraints this design is bound by

From the two ledger statements and L6:

1. The handoff is **optional**. Its absence is the shipped posture, not a degraded one.
1. **One capture and transcription pipeline.** Open Notebook must not become a second place where a
   Voice Memo is ingested and transcribed.
1. **Canonical originals are preserved.** Whatever reaches Open Notebook is a copy, and the copy is
   never the thing anything else reads back.
1. **vpp and Bob must not require Open Notebook to read or produce ordinary notes.** This is the
   done-means, and it is an absence property: remove the service and nothing changes.
1. The **format** is the deliverable. Deployment, ports, backup and provider selection are L6's, and
   importing them here is what the section's first bullet forbids.

From the repository's standing rules, carried with the labels the earlier documents used:

- **R1.** No workspace may depend on another, and a tool never assumes this checkout exists. Here it
  extends: a tool never assumes a server exists either.
- **R2.** Rust files target 300 lines and never exceed 500, unit tests included.
- **R3.** Never patch, fork or modify a third-party tool. Open Notebook is third party: it is deployed
  and called through its published interface, never modified.
- **R4.** Tests cover the behavior of tools we wrote and nothing else. No test here asserts anything
  about Open Notebook's behavior.
- **R5.** The operator runs applies. An agent proposes.
- **R6.** This repository builds no removal mechanisms, and the chain reads that as: vpp deletes
  nothing.

And two rules the chain set for itself that this document must not quietly break:

- **C1.** vpp never transmits. The redacted-draft design made the share gate meaningful by keeping every
  egress channel out of the binary. A built-in push to a server would be exactly such a channel.
- **C2.** vpp never runs git, and refuses to write where git would find its output.

One constraint of this document's own: `gh-axi repo view` reports `visibility: public` for
`webdavis/dotfiles`, so no vault content, no tailnet name and no private identifier appears below. Every
example is invented.

## What was verified, and how

### Open Notebook, from its own source and documentation

Nothing was installed and nothing was called. These come from the upstream repository, read today.

| Fact | Where it comes from |
| --- | --- |
| Self-hosted research workspace, "an open source, privacy-focused alternative" to a hosted notebook product | repository README |
| Web interface on port 8502, its programming interface on 5055, SurrealDB on 8000 | README deployment section |
| `POST /api/sources` (multipart form) and `POST /api/sources/json` create a source | `api/routers/sources.py` |
| A source `type` is one of `link`, `upload` or `text`; `content` is required for `text` | `api/routers/sources.py` |
| A source create carries `type`, `notebook_id`, `notebooks`, `url`, `content`, `title`, `file_path`, `transformations`, `embed`, `delete_source`, `async_processing`, `file` | `api/routers/sources.py` |
| Notes have their own create, read, update and delete endpoints under `/api/notes` | `docs/7-DEVELOPMENT/api-reference.md` |
| Authentication is one shared password sent as `Authorization: Bearer <password>`, from `OPEN_NOTEBOOK_PASSWORD` | `docs/5-CONFIGURATION/security.md` |
| "Always use HTTPS", because password transmission is plain text | `docs/5-CONFIGURATION/security.md` |
| No rate limiting, no audit logging, no session timeout beyond closing the browser; "basic access control, not enterprise-grade security" | `docs/5-CONFIGURATION/security.md` |
| With `CORS_ORIGINS` unrestricted, "any website the user visits can issue authenticated cross-origin requests to your API" | `docs/5-CONFIGURATION/security.md` |
| `OPEN_NOTEBOOK_ENCRYPTION_KEY` has no default and credential storage fails without it | `docs/5-CONFIGURATION/security.md` |
| Transcription is available through nine hosted providers, and separately through a local speech-to-text configuration | README provider matrix, `docs/5-CONFIGURATION/local-stt.md` |
| A Model Context Protocol server exists, `uvx open-notebook-mcp`, configured with `OPEN_NOTEBOOK_URL` and `OPEN_NOTEBOOK_PASSWORD`, and it can create sources and notes | `docs/5-CONFIGURATION/mcp-integration.md` |

The sentence that decides the largest question in this document is in the user guide, on adding sources:

> Audio/video is transcribed to text automatically. This requires enabling speech-to-text in settings.

Supported audio there is MP3, WAV, M4A, OGG and FLAC, and M4A is the format Apple Voice Memos writes.
So handing Open Notebook a recording is not a neutral act of filing: it starts a transcription, on a
second engine, outside vpp's review record, producing a second transcript with no flags, no
alternatives and no `known-terms.txt`. That is the precise thing L6's own bullet forbids, and it is
reachable by one drag onto a web page.

The other end of the same guide is the cheap half: a text source is "processed immediately", with "no
wait time", and `.md` is a supported document format. So the entire handoff this item asks for is
already a supported operation with a request body vpp can produce and no new upstream capability.

### This machine

- Open Notebook is not deployed anywhere reachable from here, and L6 is queued behind F1 and F2, so
  nothing below was exercised against a live instance. That is stated rather than glossed, because a
  design validated only on paper is a different artifact from one validated against a service.
- `curl` 8.22.0 is installed and its `-H, --header <header/@file>` form reads a header from a file, so
  the shared password never has to appear in a process argument list. That matters because this ledger
  already carries a finding against exactly that pattern elsewhere.
- The four-line transport recipe below was run end to end against a throwaway local listener on
  127.0.0.1, with an invented password: the listener received `Authorization: Bearer …`,
  `Content-Type: application/json` and a body whose `type` was `text`, and the password appeared in no
  process argument. The `jq` mapping was checked against a sample `vpp.handoff/1` document and the
  recipe passes `shellcheck` clean. Nothing left the machine.
- `jq` and `uvx` are installed, so both of the transports discussed below exist today with nothing new
  to add.
- `tailscale` is installed and is the ledger's current private path, with a NetBird cutover (F4) named
  and not approved.

## Approaches

Three, and the recommendation is one of them with a second named as a transport rather than a rival.

### A. vpp pushes: a built-in Open Notebook client

vpp gains a configuration block with a base address, a notebook identifier and a credential pulled from
KeePassXC, and `vpp handoff` performs the request itself.

It is the most convenient shape and it is what most tools do. It is rejected on three counts, any one of
which is sufficient.

It breaks **C1**. The redacted-draft design's gate works because vpp cannot send: the worst an unattended
job can do is write a file. Put an authenticated write channel to a content service inside the same
binary and the gate is no longer a boundary, it is a policy check in front of an egress that exists.

It breaks the spirit of **R1**. A `[handoff.open_notebook]` block, a `notebook_id` field and a
`5055` default are Open Notebook concepts compiled into a tool that ships to other people, and the
done-means asks for the opposite property.

And it puts a credential on the laptop for a service the laptop does not otherwise need, at a moment
when the credential story for agent-reachable secrets (A6, Infisical) is itself an open ledger item.

### B. vpp emits a document, and the push is a separate act

`vpp handoff <id>` prints one self-contained document on standard output and does nothing else. Whoever
pushes it, a three-line command the operator runs, an agent, or a homelab-side ingester, consumes that
document. The format is recorded and tested; the transport is not vpp's code.

This is the chain's own integration shape applied unchanged. Bob reads `vpp brief --json`; an external
workspace reads `vpp handoff`. Neither is registered, neither is pushed to, and both work by running a
command.

Its weakness is honest: the last step is somebody else's, so vpp cannot report that the handoff landed,
cannot learn the remote identifier and therefore cannot update a source it already sent. That ceiling is
named in the design rather than engineered around.

### C. No vpp command at all

The handoff is "open the note and upload it", or the Model Context Protocol server above with an agent
doing the filing, or a homelab job that reads the vault out of its GitHub remote.

Cheapest, and it fails the ledger on the one word that matters: the bullet asks for a **recorded
format**. An agent uploading a note freehand will sometimes paraphrase it, sometimes drop the frontmatter
and sometimes upload the wrong stage, and none of that is reviewable because there is nothing to review
it against. The homelab-pull variant is worse for a different reason: it would make the handoff depend on
the vault being a git repository with a remote, and the chain has spent five documents making the vault
optional.

### Recommendation

**B, with C's Model Context Protocol server named as one transport for B's document.** vpp produces the
bytes; the operator chooses who carries them. That keeps the format under test, keeps every Open Notebook
identifier out of vpp, and leaves the credential on whichever side already has a reason to hold one.

The honest sentence, stated once: **nothing in this design prevents the operator from dragging a
recording into Open Notebook's web page.** What it does is make the supported path the text path, make
the audio path a refusal inside vpp, and write the reason down where L6 will read it. A handoff design
cannot police a browser.

## The recommended design

### The boundary

One verb, and it is a pure function.

```
vpp handoff <id> [--stage transcript|analysis|brief|draft]
```

It reads the record, reads the artifact, and prints one JSON (JavaScript Object Notation) document on
standard output. It opens no socket, resolves no name, reads no credential and writes no file. `--stage`
is required when the identifier resolves to more than one artifact, and is inferred when it resolves to
exactly one, matching `vpp path`'s existing behavior.

`draft` means a **released** draft from the redacted-draft design, addressed by its draft identifier.
Making a released draft a handoff source costs one branch and answers, per handoff rather than once and
forever, the question of whether private text may cross.

The internal seam is the chain's: a domain function from (record, artifact bytes, stage) to the document,
with no input or output of its own, and one adapter that reads the two files. Under **R2** this is one
small module, not a crate.

### The handoff document

```json
{
  "schema": "vpp.handoff/1",
  "kind": "transcript",
  "identity": {
    "recording": "2026-08-24T144736-4f3ab19c02de",
    "note": "transcripts/2026-08-24-invoice-call-4f3ab19c.md",
    "content_sha256": "9f2c…"
  },
  "title": "vpp transcript 2026-08-24-invoice-call-4f3ab19c",
  "content": "# vpp transcript, 2026-08-24-invoice-call-4f3ab19c\n\n…",
  "provenance": {
    "produced_by": "vpp",
    "captured_at": "2026-08-24T14:47:36-06:00",
    "engines": ["whisply:large-v3-turbo", "elevenlabs:scribe_v2"],
    "open_flags": 6,
    "reviewed": false,
    "redacted": false
  },
  "generated_at": "2026-09-14T02:40:00-06:00"
}
```

Five rules govern it, and they are the ledger's requirements made mechanical.

1. **`content` is text, always, and the schema has no field that can hold a path to audio or a binary
   body.** There is no `file`, no `audio_path`, no `attachment`. A handoff cannot carry a recording
   because the document has nowhere to put one. This is how "no second capture and transcription
   workflow" becomes structural rather than a warning in a runbook.
1. **`kind` has four values**, `transcript`, `analysis`, `brief` and `draft`, and vpp refuses any
   identifier that resolves to a recording rather than to one of those renderings.
1. **`identity` names the canonical thing and the copy's digest.** The `recording` (or `occasion`, for a
   brief) is the authority; `note` is where it lives on the operator's machine; `content_sha256` is over
   the emitted `content` bytes, so a copy in a notebook can be compared against the current note without
   reading the note's own file.
1. **`provenance` carries uncertainty, never a score.** `open_flags` is the count from the record,
   `reviewed` is the review record's state, `redacted` says whether the bytes went through the share
   pipeline. Same discipline as `vpp.brief/1`: two-valued or counted, never a confidence number a
   consumer has to interpret.
1. **The function is pure.** The same record, the same artifact bytes and the same stage produce the
   same document, byte for byte, except `generated_at`. That is what makes the digest meaningful, makes
   a lost notebook recoverable by re-running one command, and makes the whole thing one test.

### The mapping to Open Notebook, which lives here and not in the code

| `vpp.handoff/1` | `POST /api/sources/json` | Supplied by |
| --- | --- | --- |
| `content` | `content` | vpp |
| `title` | `title` | vpp |
| (none) | `type`, fixed at `"text"` | the pusher |
| (none) | `notebook_id` | the pusher |
| (none) | `embed` | the pusher |
| `identity`, `provenance`, `schema` | no field exists | carried inside `content`, see below |

The whole Open Notebook vocabulary sits in that table's middle column, in this document and in the
recipe below, and nowhere in vpp. The mapping for the Model Context Protocol transport is the same three
values handed to its source-creation tool instead of to the endpoint.

The recipe, for the record, with the password read from a file rather than an argument:

```bash
vpp handoff "$id" |
  jq --arg nb "$notebook_id" \
    '{type: "text", notebook_id: $nb, title: .title, content: .content, embed: true}' |
  curl -sS --fail-with-body -H @"$header_file" -H 'Content-Type: application/json' \
    --data-binary @- "https://$open_notebook_host/api/sources/json"
```

That is the entire transport. It is four lines, it holds every Open Notebook detail, and it belongs to
whoever runs it. If it ever earns a home in dotfiles it is one `libexec` script under the naming rules,
proposed separately; it is deliberately not part of this item.

### What rides inside `content`, and why

Open Notebook's source model has no field for arbitrary metadata, so anything that must survive the
crossing has to be in the text. `content` is therefore the artifact's body with a fixed header prepended
and the note's YAML frontmatter dropped.

```markdown
# vpp transcript, 2026-08-24-invoice-call-4f3ab19c

Produced by vpp on the operator's machine and copied here. The authoritative version is the record
2026-08-24T144736-4f3ab19c02de and the note it renders; this copy is derived and may be stale.
Six spans are unverified and are marked [unverified] below. A summary that drops those markers is not
supported by this source.

## Transcript
…
```

Three decisions are inside that block.

**The frontmatter does not cross.** `vppRecording`, `vppCapturedAt`, `vppDurationSecs`, `vppEngines`,
`vppSchema`, `tags`, `vppSuggestedTags` and the vault's own eight keys are vpp's machine keys and the
vault's conventions; they mean nothing in a notebook, they would be embedded and searchable, and
`vppRecording` is a capture timestamp to the second wearing an identifier's clothes. The two that matter
for a human reading the copy are in the header in words.

**Uncertainty is marked in place, not in a footer.** Same finding as the briefs design, and it bites
harder here: Open Notebook's whole purpose is to put a model between the source and the reader, and a
model summarizing a source drops a footer and keeps the sentence. In-line `[unverified]` markers travel
with the sentence they qualify, and the header states the count so a reader who never scrolls still
knows.

**The header is declarative and contains no instruction.** Not "do not summarize without the markers",
which is an imperative sentence in a document a model will read, and a source that instructs a model is
indistinguishable from one that was written to. A notebook the operator also fills with web pages is
exactly the place not to teach that a source may give orders.

### Identity, re-handoff, and the named ceiling

Open Notebook creates a source on `POST` and updates one by its own identifier on `PUT`. vpp never
performs the request, so it never learns that identifier, so a second handoff of a corrected note
creates a second source rather than replacing the first.

That is a real cost and the answer is deliberately cheap: the `title` is deterministic and carries the
vpp identity, so a duplicate is findable by a title search, and `content_sha256` tells the operator
whether the copy they are looking at is current.

```
ponytail: no remote identifier is recorded, so a re-handoff duplicates rather than replaces. The upgrade
path is `vpp handoff record <id> <external-id>`, one field in the record, added when duplicates actually
hurt.
```

Building that now would mean designing a remote-identifier field, a write-back verb and a reconciliation
rule for a service that is not deployed, against an interface nobody on this machine has called. The
alternative is one search in a web page.

### Configuration: none

This feature adds no configuration key, no section and no dotfiles target. A command that reads two local
files and prints a document needs none, and a configuration section naming an external service is exactly
the coupling the done-means forbids. When L6 exists, the shared password belongs in KeePassXC and reaches
the recipe on the pushing side; that is a one-line addition to the credential story A6 already owns.

The alternative, a `[handoff]` block with a default title prefix, is named in the assumptions and is
reversible in an afternoon if the fixed prefix turns out to be wrong.

### Failure modes

| Condition | What vpp does | Notification |
| --- | --- | --- |
| Identifier resolves to nothing | refuse, name it | none, exit 2 |
| Identifier resolves to a recording rather than a rendering | refuse, name the stages that exist | none, exit 2 |
| `--stage` omitted and several stages exist | refuse, list them | none, exit 2 |
| The named stage has no note yet | refuse, say which stage is missing | none, exit 2 |
| Stage is `draft` and the draft has no release | refuse, print the `vpp share` command | none, exit 2 |
| The note's path in the record does not exist | refuse, name both the record and the path | page |
| Managed `vpp:` markers missing, doubled or unbalanced | refuse to read the note, name the path | page |
| The record says `open_flags > 0` | emit, with `reviewed: false` and the count in the header | none, log |
| `known-terms.txt`, the index or any share state is absent | irrelevant, none is read | none |

Two refusals page, and they are the chain's usual pair: the record and the filesystem disagree, or a
managed region is not intact. Everything else is a typo at the command line.

An unreviewed artifact is emitted rather than refused, which is a choice. Refusing it would send the
operator to copy and paste, and a pasted copy carries no header, no count and no markers, which is
strictly worse than a labelled one.

### Security and privacy

**A handoff is an egress decision, and the document says so in its own fields.** Whatever crosses is read
by Open Notebook's configured model provider when it is embedded, chatted with or transformed. L6's
second bullet already owns that choice ("record what content leaves the homelab when a hosted model
processes it"); this design's contribution is that `redacted` and `open_flags` are in the document, so
what left is recorded on the producing side too.

**A recording never crosses.** Verified above: an audio source is transcribed automatically, by a second
engine, with no flags and no alternatives. The schema has nowhere to put audio and the command refuses a
recording identifier, so the supported path cannot produce that outcome.

**The upstream access control is weak, by its own documentation.** One shared password, sent in plain
text, no rate limiting, no audit log, and a session that ends when the browser closes. Two consequences
belong in L6's hands rather than vpp's, and both are worth writing down because a notebook holding
transcripts is a different risk class from a notebook holding public articles:

- Transport must be encrypted end to end. Over the tailnet today (WireGuard carries it), or behind the
  reverse proxy upstream documents. Plain port 5055 across a local network is a password and a transcript
  in the clear.
- `CORS_ORIGINS` must be set. Unrestricted, by upstream's own sentence, any website the operator visits
  can issue authenticated cross-origin requests to the interface, and the browser holding the session is
  the same browser that reads the rest of the internet.

**The shared password never appears in a process argument list.** `curl -H @file` is available on the
installed 8.22.0 and the recipe above uses it. This ledger already carries a finding against a token
passed in argv elsewhere; repeating it here would be a choice, not an accident.

**The emitted document is the full private note on standard output.** vpp has no destination to refuse,
so the redacted-draft design's destination checks do not apply and are not pretended to. The rule is in
the recipe instead, which pipes and never saves, and the residual risk is named rather than mechanized.

**Nothing about a handoff rides a pns request.** Not a title, not a path, not a count. Same rule the
chain set for flagged spans, tags and drafts.

**vpp still never runs git and still never writes into a git working tree.** This feature writes nothing
at all, so **C2** holds trivially.

### The independence proof, which is this item's done-means

Four absence checks, in the boundaries design's style, because "works without Open Notebook" is an
absence property and mocking a service that does not exist proves nothing.

1. **Open Notebook absent.** Every vpp command behaves identically, `vpp handoff` included: it prints its
   document, because it performs no request. Nothing is retried, nothing is queued, nothing warns.
1. **No configuration.** With no `[handoff]` section, no environment variable and no credential anywhere
   on the machine, every command including `vpp handoff` succeeds. There is no key to be missing.
1. **No Open Notebook vocabulary in the source.** A grep over vpp's tree for `open.notebook`,
   `notebook_id`, `5055`, `8502` and `surreal` finds nothing. This is a test, it runs in milliseconds,
   and it is the single strongest guard against the coupling creeping back in during implementation.
1. **Bob absent, and Open Notebook absent from Bob.** `vpp brief --json` and `vpp.brief/1` are unchanged
   by this document. Bob reads vpp; Bob does not read Open Notebook, and nothing in vpp's output tells it
   to.

The reciprocal claim, that Open Notebook works without vpp, is trivially true and is L6's own: it ingests
web pages, documents and text with no knowledge that vpp exists.

### The behaviors to drive the implementation, test-first

Each is one failing test before one piece of code. None needs a network, a server, a recording, an agent
or a vault: each takes a record, a note file and a temporary directory.

1. The same record, note bytes and stage produce the same document twice, byte for byte apart from
   `generated_at`, and `content_sha256` matches the emitted `content`.
1. An identifier that resolves to a recording rather than a rendering is refused, and the message lists
   the stages that exist.
1. The document has no field that can carry audio: given a record whose `audio_path` is set, the emitted
   document contains neither the path nor the word.
1. `--stage draft` on a draft with no release is refused and the message names the `vpp share` command.
1. `kind` is one of exactly four values, and a fifth is a compile-time impossibility rather than a
   runtime check.
1. `provenance.open_flags` equals the record's count, and `reviewed` is false when the review record has
   unresolved flags.
1. An artifact with six flagged spans emits all six `[unverified]` markers inside `content` and states
   the count in the header.
1. `content` contains no YAML frontmatter, and none of `vppRecording`, `vppCapturedAt`,
   `vppDurationSecs`, `vppEngines`, `vppSchema`, `tags` or `vppSuggestedTags` appears anywhere in it.
1. The header is present exactly once, names the record identity, and contains no imperative sentence.
1. `title` is deterministic for a given identity and stage, and two different stages of one recording
   produce two different titles.
1. The command writes no file and creates no directory: the temporary directory is byte-identical before
   and after, and the source note is unchanged.
1. The command makes no network call: run with every name resolution failing, it still succeeds.
1. With no configuration file at all, the command succeeds.
1. A grep over the source tree finds none of `open.notebook`, `notebook_id`, `5055`, `8502`, `surreal`.
1. A note whose managed markers are unbalanced is refused, and the message names the path.
1. A record whose note path does not exist is refused, and the message names both.

Behaviors 3 and 5 pin "one capture and transcription pipeline", 1 and 11 pin "canonical originals", and
12 through 14 pin the done-means itself. Behavior 14 is the one a later change will erode first, because
adding a convenience push is the most natural thing in the world for somebody who has just watched the
recipe work.

## Out of scope

Deliberately not designed here, and not to be smuggled in during implementation.

- **vpp performing any request, holding any credential, or knowing any address.** This is the whole
  boundary. A `--push` flag is the failure mode this document exists to prevent.
- **Any import from Open Notebook.** A note authored there that should become durable is exported by the
  operator into the vault as an ordinary note, which is L6's own bullet ("keep original documents and
  exported durable notes in their canonical locations, including Obsidian"). vpp has no importer, no
  reconciler and no awareness of a remote note.
- **The boundary with Hindsight's agent memory.** Named in the same L6 bullet and owned there.
- **Open Notebook's deployment, ports, reverse proxy, SurrealDB backup, encryption key custody, provider
  selection and cost.** All L6, and the ledger's first bullet in this section says not to import them.
- **Podcasts, transformations, insights and embeddings.** Open Notebook's own features, applied to a
  source after it arrives.
- **Recording a remote source identifier, updating a source in place, or deleting one.** The named
  ceiling above.
- **Speaker labels and dated digests.** Unapproved candidates in the ledger, and unchanged here.
- **Sending audio, under any circumstance.**
- **A dotfiles-side push script, justfile recipe or LaunchAgent.** If one is ever wanted it is proposed
  on its own, under the `libexec` naming rules, and a scheduled one would need a separate argument
  against "no second automatic workflow".
- **Any change to the discovery, transcription, tagging, brief or share designs.** This reads their
  artifacts and adds one verb.

## Assumptions made in the operator's place

Each was a choice this document had to make to be written at all. None is a decision, each names its
alternative, and reversing any changes one section rather than the design's shape.

1. **vpp emits a document and never pushes.** Alternative: a built-in client, which is one command
   instead of three and is what most tools do. Taken because it preserves the chain's own rule that vpp
   cannot transmit, which is what makes the share gate a boundary rather than a policy check.
1. **The handoff is one command per artifact, never automatic.** Alternative: a hook on note write, so
   every transcript arrives in the notebook without being asked for. Rejected because that is the second
   automatic workflow L6 forbids, arriving through the front door instead of the audio one.
1. **A released redacted draft is a handoff source alongside the three private stages.** Alternative:
   allow only private stages (simpler) or only released drafts (safer). Taken because it costs one branch
   and turns "may private text cross?" into a per-handoff choice the document records, rather than a
   policy nobody revisits.
1. **Private notes may cross by default.** Alternative: require a released draft, always. Taken because
   a research workspace over redacted material answers redacted questions, and the likely result is the
   operator pasting the private note into a hosted notebook instead. It is the assumption most worth
   contesting and it is the first open question.
1. **The note's YAML frontmatter does not cross; a fixed header carries identity, the unresolved count
   and the derived-copy sentence.** Alternative: send the file verbatim, which is simpler and preserves
   everything. Taken because the frontmatter is vpp's machine keys and the vault's conventions, and
   because `vppRecording` is a to-the-second capture timestamp.
1. **The header is declarative, never imperative.** Alternative: instruct the model to preserve the
   markers, which would probably work more often. Rejected because a source that gives orders is the
   shape of an injected one, in a notebook that also holds web pages.
1. **No remote identifier is recorded, so a re-handoff duplicates.** Alternative: a write-back verb and a
   reconciliation rule. Deferred with its upgrade path written in the source, because it would be
   designed against an interface nobody here has called.
1. **No configuration at all.** Alternative: a `[handoff]` block with a title prefix and a default stage.
   Taken because every key would be speculative and one of them would name the service.
1. **The transport recipe is documented here and owned by whoever runs it.** Alternative: ship it as a
   dotfiles `libexec` script now. Deferred until L6 is actually deployed, because a script for a service
   that does not exist cannot be tested and will be wrong by the time it is.
1. **`vpp.handoff/1` is a new document rather than a reuse of `vpp.brief/1`.** Alternative: extend the
   brief shape to cover every artifact. Rejected because the brief's `items` array with per-item
   certainty is the wrong shape for a whole transcript, and widening it would weaken the contract Bob
   depends on.
1. **The Model Context Protocol server is named as a transport, not designed as one.** Alternative:
   specify the agent-side flow. Left alone because handing an agent a write credential to the notebook is
   an A6-shaped decision, not a format decision.
1. **Nothing was sent, installed or deployed while writing this.** No instance exists, no request was
   made, and every upstream claim is cited to a file in the public repository.

## Operator steps

**1. Answer the private-versus-redacted question**, because it decides what the feature is for. Today's
proposal is that private notes may cross and the document records that they did. The alternative is that
only released drafts cross, which makes the notebook safe and much less useful.

**2. Confirm the no-push rule.** Three lines of `jq` and `curl` at the moment of handoff, forever, versus
one `vpp handoff --push`. If the three lines are too heavy for how this will actually be used, the time
to say so is before the rule is written into tests, not after a convenience flag is added around it.

**3. Decide who holds the shared password when L6 exists.** The laptop through KeePassXC, an agent
through the Model Context Protocol server, or nobody, with the handoff performed by hand in the web
interface. This is the smallest concrete instance of the A6 credential question already in this ledger.

**4. Hand L6 the two configuration findings**, which are upstream's own words and are not vpp's to fix:
encrypted transport is mandatory because the password is sent in plain text, and `CORS_ORIGINS` must be
restricted or any site the operator's browser visits can reach the interface with the operator's session.

**5. Decide who owns the return path** for a note authored in Open Notebook that should become durable.
L6's bullet says canonical locations including Obsidian; this design says vpp has no importer. That
leaves the act unassigned, which is fine as long as it is assigned deliberately.

Nothing here needs an apply, a package, a grant or a deployment. The whole item is one command in a tool
that does not exist yet plus a mapping table.

## Open questions for the operator

1. **May a private note cross into Open Notebook, or only a released redacted draft?** The design
   supports both and defaults to allowing private notes, on the reasoning that a redacted research
   workspace answers redacted questions. The counter-argument is that once a hosted model provider is
   configured, a private transcript in a notebook has left the machine as surely as an email would.
1. **Is the no-push rule right?** It is the load-bearing assumption, it costs three lines at every
   handoff, and it is the reason the share gate in the previous document still means something.
1. **Which artifacts may be handed off at all?** Transcripts, analysis notes, briefs, released drafts, or
   a subset. The brief is the same caution the share design raised: it names the people who will be in a
   room.
1. **Should an unreviewed artifact be refusable rather than labelled?** Today it is emitted with the
   count and the markers. A refusal would be stricter and would probably be routed around by hand.
1. **Who holds the shared password, and does an agent get write access to the notebook?** The Model
   Context Protocol server makes agent-driven filing available today, with `uvx` already installed.
1. **Does the handoff need to say it came from vpp?** The proposed header names vpp and the record
   identity, which is good provenance and also tells anyone with notebook access that a recording exists.
1. **What happens to a copy in the notebook when the note it came from is corrected?** Today, nothing:
   a second handoff makes a second source. The upgrade path is named; whether it is needed depends on how
   often a transcript is corrected after filing.
1. **Where does the transport recipe eventually live?** Nowhere today. A dotfiles `libexec` script, a
   `just` recipe, a homelab-side ingester and "the operator types it" are four different answers with
   four different maintenance costs.
1. **Where does vpp's code live, and what is it called?** Carried forward unresolved from the boundaries
   design, because the chain should not stay in disagreement with itself.
