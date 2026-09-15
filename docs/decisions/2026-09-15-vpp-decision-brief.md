# vpp decision brief

vpp has seven design documents and zero lines of code. Each document ends with its own open questions,
and nobody has asked the operator all of them in one sitting, so nothing can start. This page is that
ask: every decision the seven documents left open, in the order the documents were written, each as a
question with concrete options and a recommendation. The documents live under `docs/superpowers/specs/`,
one path per section below, and the `vpp (Voice Processing Pipeline)` and `Homelab plan coordination`
sections of `docs/remaining-work.md` are the ledger items behind them.

Four questions repeat across several documents because later documents depend on an earlier one's answer,
or because two documents quietly disagree. They carry a short code (B1, B2, B5, F1) the first time they
appear and are referenced by that code everywhere else, so the size of the whole ask stays visible
without the same paragraph repeating seven times.

## What's being asked

| #   | Decision                                                                      | Section       | Recommendation                                     |
| --- | ----------------------------------------------------------------------------- | ------------- | -------------------------------------------------- |
| B1  | vpp's own repository, or a fifth cargo workspace in dotfiles                  | Boundaries    | own repository                                     |
| B2  | Keep the name `vpp`, or rename before the repository exists                   | Boundaries    | rename now                                         |
| B3  | Notes share `transcripts/`/`analysis/`, or get their own subtree              | Boundaries    | share the existing directories                     |
| B4  | Vault conventions and folder-note upkeep for machine-written notes            | Boundaries    | apply them; operator owns folder notes             |
| B5  | Is `minutes` kept, replaced, or run alongside vpp                             | Boundaries    | replace, once vpp's ingestion ships                |
| B6  | Does vpp's binary/config/LaunchAgent join posture's watch list                | Boundaries    | not yet, opt in later                              |
| D1  | Is reading Apple's undocumented Voice Memos store acceptable at all           | Discovery     | yes, read-only                                     |
| D2  | Can a LaunchAgent read the group container with no session attached           | Discovery     | run the logout probe first                         |
| D3  | Clone the audio, or reference it in place                                     | Discovery     | clone                                              |
| D4  | Sweep interval, and is `WatchPaths` worth it                                  | Discovery     | 15 minutes, sweep only                             |
| D5  | Notice a recording deleted from Voice Memos after cloning, or stay silent     | Discovery     | stay silent                                        |
| D6  | Emit `vpp ingest`'s record on stdout as JSON, or sidecar only                 | Discovery     | keep both                                          |
| T1  | Which transcription engine pairing                                            | Transcription | whisply MLX plus ElevenLabs Scribe v2              |
| T2  | Is an Apple SpeechAnalyzer Swift helper worth building                        | Transcription | not yet                                            |
| T3  | May transcripts be committed to the vault and synced to a phone               | Transcription | yes                                                |
| T4  | Should a confirmed correction rewrite future transcripts                      | Transcription | no, record only                                    |
| T5  | How loud should `agreed-unverified` be in the pns notification                | Transcription | counts only, detail stays in the file              |
| T6  | One pns notification per recording, or one per run                            | Transcription | per recording, aggregate past three                |
| T7  | Does `verify-note` belong in vpp, or in whatever writes the note              | Transcription | in vpp                                             |
| F1  | Where the audio archive copy lives: inside the vault or outside it            | Filing        | outside the vault, symlinked in                    |
| F2  | New tags held as suggestions, or accepted automatically                       | Filing        | held                                               |
| F3  | New multi-word tag shape: kebab-case or camelCase                             | Filing        | kebab-case                                         |
| F4  | May vpp write a backlink into a note it did not create                        | Filing        | no, transcript side only                           |
| F5  | Which mobile sync actually carries the vault                                  | Filing        | confirm before trusting T3                         |
| F6  | Does `vpp path` stay the filing contract, or does vpp write notes itself      | Filing        | `vpp path` stays the contract                      |
| R1  | Does vpp read the calendar and Todoist, or does Bob supply them               | Briefs        | Bob supplies them, later                           |
| R2  | Requested briefs, or scheduled with a lead time                               | Briefs        | requested only                                     |
| R3  | Which calendars and Todoist projects a brief may read                         | Briefs        | name them before enabling collectors               |
| R4  | Can a read-only Todoist credential coexist with the read-write one            | Briefs        | test it; expect no                                 |
| R5  | May a brief be committed to the vault, given who it names                     | Briefs        | keep briefs outside the vault                      |
| R6  | Does "brief" collide with Forzare's morning brief                             | Briefs        | rename to `vpp prep`                               |
| R7  | Is the local Apple calendar representative of the operator's meetings         | Briefs        | confirm Google is where meetings live              |
| S1  | Is a shared draft an extract, or generated prose                              | Sharing       | extract only, for now                              |
| S2  | Retention of drafts and released copies                                       | Sharing       | operator prunes by hand, on a schedule             |
| S3  | Stable pseudonyms across drafts, or per-draft numbering                       | Sharing       | per-draft numbering                                |
| S4  | May a brief be drafted from                                                   | Sharing       | not yet                                            |
| S5  | Should an approval expire                                                     | Sharing       | yes, a short time-to-live                          |
| S6  | Does the released file say it came from vpp                                   | Sharing       | no, keep it generic                                |
| S7  | Is a PDF release path in scope                                                | Sharing       | not yet                                            |
| H1  | May a private note cross into Open Notebook, or only a released draft         | Handoff       | only a released draft                              |
| H2  | Is the no-push rule (three lines of `jq` and `curl`, forever) right           | Handoff       | yes, keep it                                       |
| H3  | Which artifacts may be handed off                                             | Handoff       | transcripts and analysis notes only, for now       |
| H4  | Should an unreviewed artifact be refusable rather than labelled               | Handoff       | stay labelled                                      |
| H5  | Who holds Open Notebook's shared password, and does an agent get write access | Handoff       | KeePassXC on the laptop, no agent write access     |
| H6  | Does the handoff header name vpp                                              | Handoff       | yes, keep the current header                       |
| H7  | What happens to a notebook copy when its source note is corrected             | Handoff       | accept duplication for now                         |
| H8  | Where does the transport recipe eventually live                               | Handoff       | nowhere yet, decide once Open Notebook is deployed |

Three items appear in every one of the seven documents and are not repeated in the table above: B1 (where
vpp's code lives), B2 (its name) and B5 (the `minutes` disposition). Every section below notes where its
own design depended on one of them.

## 1. Project boundaries

`docs/superpowers/specs/2026-09-14-vpp-project-boundaries-design.md`

This document decides where vpp's code, its Mac installation, and its content each live. Today none of
the four candidate homes (`webdavis/dotfiles`, `webdavis/homelab`, the Ivy vault, a `webdavis/vpp` that
does not exist) has a vpp line in it beyond this design chain itself, and the vault's
`agent-processing-pipeline/raw/audio/`, `transcripts/` and `analysis/` directories are empty except for a
`.gitkeep` in each, unchanged since 2026-07-22.

**B1. Own repository, or a fifth cargo workspace in dotfiles?** Recommendation: **own repository**,
`webdavis/vpp`, because it matches the two most recent naming and packaging rulings (own repository for
the custom Neovim plugins, tools as products others install) and copies a mechanism already running on
this machine for scalebar. This is the one answer the rest of the chain hangs from: the discovery
design's own assumptions section chose the opposite (a fifth workspace), so the chain currently disagrees
with itself and the recording-discovery section below repeats the same question rather than silently
picking a side.

**B2. Keep the name `vpp`, or rename before the repository exists?** Recommendation: **rename now**.
`VPP` is FD.io's Vector Packet Processing, which is the first search result anyone will find, and the
repository's own rules avoid introducing an uncommon acronym rather than spelling it out. Renaming after
install instructions circulate is a breaking change, which makes this the cheapest moment to decide.

**B3. Notes share `transcripts/`/`analysis/`, or get their own subtree?** Recommendation: **share the
existing directories**, because the ledger already asks for "the existing vault layout" and a per-note
provenance field tells the two kinds of note apart without a new folder.

**B4. Do the vault's folder-note and frontmatter conventions apply to machine-written notes, and who
maintains the folder note for a directory a tool writes into?** Recommendation: **apply them, and the
operator writes the folder notes**, which is what the filing design (Section 4 below) already settled:
vpp writes content, never vault furniture.

**B5. Is `minutes` kept, replaced, or run alongside vpp?** Recommendation: **replace it**, once vpp's
ingestion and transcription ship. `minutes` already covers six of vpp's seven feature bullets and already
owns a directory inside the vault, so keeping both means two tools filing notes about the same
recordings. Every later document in this chain treats this as still open and works either way; answering
it now removes a standing "and if `minutes` stays" clause from four more documents.

**B6. Should vpp's binary, configuration and LaunchAgent join posture's user-configured watch list?**
Recommendation: **not yet**. They sit outside the osquery known-good manifests by default, so this is an
addition the operator opts into later rather than a consequence of shipping vpp.

**Unblocks:** answering B1 and B2 lets `webdavis/vpp` be created; answering B5 removes the same open
clause from the transcription, filing, briefs and sharing designs.

## 2. Recording discovery

`docs/superpowers/specs/2026-09-14-vpp-recording-discovery-design.md`

This document designs `vpp ingest`, the one piece of vpp that is new work regardless of the `minutes`
ruling. Measured on dresden: Apple's Voice Memos ships no scripting dictionary and no export action, so
the only path to the audio is a read-only read of
`~/Library/Group Containers/group.com.apple.VoiceMemos.shared/`, and the capture timestamp is already
inside each `.m4a` file's own `mvhd` box, matching the private database to the second.

**D1. Is reading Apple's undocumented Voice Memos store acceptable at all?** Recommendation: **yes,
read-only**, because every supported programmatic path (AppleScript, Shortcuts, Spotlight) was measured
to yield no audio at all, and the design confines the undocumented dependency to the human-assigned
title, which degrades to "untitled" rather than to lost audio.

**D2. Can a LaunchAgent that launchd starts at login read the group container?** Unresolved; every read
in the measuring session inherited the terminal's own Full Disk Access grant, so it proves nothing about
a job with no session attached. Recommendation: **run the logout probe** the design specifies (a plist
that reads one byte of the recordings directory and one byte of the TCC database, loaded after a full
log-out and log-in) before relying on an unattended sweep.

**D3. Clone the audio, or reference it in place?** Recommendation: **clone**, using `clonefile(2)`.
Measured: cloning a 187.9 MB recording took 0.00 seconds and consumed 16 KB, so the clone gives an
independent, durable original at effectively the storage cost of a reference.

**D4. Sweep interval, and is `WatchPaths` worth taking despite its own manual page discouraging it?**
Recommendation: **15 minutes, sweep only**, with `WatchPaths` left as a one-key plist change if the
latency proves annoying in use. Nothing measured argues for a different number.

**D5. What happens to a recording deleted in Voice Memos after vpp has already cloned it?**
Recommendation: **the clone survives silently**. L-R5 is explicit that the evaluation is "not permission
to delete recordings or notes," and noticing the disappearance adds a check for a case that costs nothing
to leave alone.

**D6. Should `vpp ingest` emit its per-recording record on stdout as JSON, or write only the sidecar?**
Recommendation: **keep both**. The transcription stage is the next command in the chain and needs a
stream to consume; the sidecar alone would make every run re-scan the state directory to find what
changed.

**Unblocks:** D1 and D2 decide whether vpp is an automatic watcher or a manual filing tool; the rest are
one configuration value each and do not block implementation.

## 3. Redundant transcription and disagreement review

`docs/superpowers/specs/2026-09-14-vpp-redundant-transcription-design.md`

This document prices the four engine pairings the ledger asks the operator to choose between and designs
the disagreement, confidence and risk-class review around whichever pairing wins. Measured against a
synthesized clip with known ground truth: two runs of the same Whisper model on two different runtimes
produced byte-identical transcripts, so pairing `whisply` with `openai-whisper` looks redundant and
detects nothing, and the one error both engines shared was invisible to confidence scoring too.

**T1. Which engine pairing?** Recommendation: **whisply on Apple's MLX framework, paired with ElevenLabs
Scribe v2**, at a measured cost of about five minutes of laptop compute plus $0.037 per ten-minute
recording, different model families, confidence reported on both sides. The whole existing 28-recording
back catalogue costs $1.01 once. Apple's SpeechAnalyzer is the free different-family alternative and is
confirmed available on this Mac, at the cost of a Swift helper inside a Rust project (T2).

**T2. Is an Apple SpeechAnalyzer Swift helper worth building?** Recommendation: **not yet**. It is the
only free different-family option, but its confidence reporting is unknown and unprobed, and it puts a
second language into vpp's build for a saving of about four cents per recording.

**T3. May transcripts be committed to the vault and therefore synced to a phone?** Recommendation:
**yes**, because that is what the vault's `transcripts/` directory is for and reading notes on a phone is
plausibly the point. Confirm this after answering F5 below, since which sync path is actually live
changes who else receives that transcript.

**T4. Should a confirmed correction rewrite future transcripts?** Recommendation: **no, record only**.
Recording that a name should read differently is cheap; applying it automatically changes the transcript
of record with no human reading the result, which is a different and riskier feature.

**T5. How loud should the `agreed-unverified` class be in the pns notification?** Recommendation:
**counts only in the notification, detail stays in the file**. This class is the largest and it is also
the one that catches the shared-error case the design measured; putting its content in a phone
notification would be the exact kind of sensitive-fragment leak the chain refuses everywhere else.

**T6. One pns notification per recording, or one per run?** Recommendation: **per recording, aggregating
past three in one run**, which is what the design already ships as the default and which avoids both a
buried urgent recording and a 28-notification backlog stampede.

**T7. Does `verify-note` belong in vpp, or in whatever writes the note?** Recommendation: **in vpp**,
since the note generator is still undecided and vpp already owns the review record `verify-note` checks
against.

**Unblocks:** T1 turns Open Question 8 from a priced menu into a configuration value and lets the
transcription stage be built; the rest refine its notification and review behavior.

## 4. Metadata schema, tags, relationships and filing rules

`docs/superpowers/specs/2026-09-14-vpp-metadata-schema-and-filing-design.md`

This document defines vpp's own frontmatter keys, the tag-suggestion gate, four closed relationship
kinds, and deterministic filing, keeping the vault's own documented schema and `minutes`' vocabulary
where they already named a concept. Measured in the vault while writing: 738 notes carry frontmatter, 122
distinct tags are in use with no single naming convention, and a full name-and-alias scan over 753 files
costs 0.245 seconds, which is why the design rebuilds its index every run instead of caching it.

**F1. Where does the audio archive copy live: inside the vault's `raw/audio/`, or outside it with an
optional symlink?** This is a direct disagreement between two earlier documents in this chain: the
discovery design clones into the vault (Section 2 above), and the boundaries design prefers an archive
directory outside the vault, symlinked in, the way `minutes` already does with `~/meetings` (Section 1,
B3's neighbor). This filing design works with either, because the audio path is one configured value
separate from the notes' output root. Recommendation: **outside the vault, symlinked in**, matching the
boundaries design and the existing `minutes` precedent, because it keeps a large binary asset out of a
git-backed directory even though it is gitignored there, and because "is vpp's copy the backup" is a real
question that an archive directory makes explicit rather than accidental.

**F2. Are new tags held as suggestions, or accepted automatically?** Recommendation: **held**. The
vault's 122-tag vocabulary is small and deliberate, git makes a mistake permanent within about ten
minutes, and an agent tagging unsupervised would roughly double that vocabulary within a year.

**F3. New multi-word tag shape: kebab-case or camelCase?** Recommendation: **kebab-case**, the design's
default, though the corpus itself splits 10 kebab to 23 camelCase, so there is no majority to derive this
from and it is the weakest recommendation in this brief.

**F4. May vpp write a backlink into a note it did not create?** Recommendation: **no, transcript side
only**. Writing into the operator's own contact and project notes is a different class of edit than
writing vpp's own output, and Obsidian's backlinks pane already makes the reverse direction visible
inside Obsidian.

**F5. Which mobile sync actually carries the vault, Obsidian's core Sync plugin or `obsidian-git`?**
Unresolved; both are configured and enabled, and they send the same content to two different third
parties. Recommendation: **confirm which is actually subscribed and active** before relying on T3's "yes,
transcripts may be committed" answer, since the honest privacy picture depends on knowing who receives
the push.

**F6. Does `vpp path` stay the contract for the later note generator, or does vpp write the analysis note
itself?** Recommendation: **`vpp path` stays the contract**. Exposing the filing rules as a command lets
any future generator, an agent, `minutes`, or vpp itself, file correctly without a second copy of the
rules to drift.

**Unblocks:** F1 resolves the chain's one structural disagreement about where audio lives; F2 and F3 are
needed before the tag-suggestion gate can ship with real defaults.

## 5. Meeting briefs, with optional calendar and Todoist context

`docs/superpowers/specs/2026-09-14-vpp-meeting-briefs-design.md`

This document verifies the permission boundaries the ledger asked to be checked and designs `vpp brief`
around whichever answers the operator gives to who holds the calendar credential and when a brief runs.
Measured: Google Calendar and Todoist each offer a read-only scope but no per-calendar or per-project
scope, and Apple's EventKit has had no read-only level for events since macOS 14, full access or
write-only only, so Google through the already-installed `gog` is the only provider that can honestly
call itself read-only.

**R1. Does vpp read the calendar and Todoist directly, or does Bob supply them?** Recommendation: **Bob
supplies them, later**, and vpp ships with its collectors disabled by default. This keeps vpp useful
before Bob exists, which the ledger requires, and avoids putting a second calendar credential on the
laptop for work Forzare's own design already assigns to Bob's `calendar-read` skill.

**R2. Requested briefs, or scheduled with a lead time?** Recommendation: **requested only**. Measured on
this machine: not one non-recurring forward calendar item is timed, and only 8 items in the last 90 days
carried a participant at all, so a scheduler would wake daily and find nothing.

**R3. Which calendars and Todoist projects may a brief read?** No provider can enforce a narrower scope
than "read everything," so this list is the only scope that exists. Recommendation: **name the list
explicitly before either collector is enabled**; an empty list refuses by design rather than defaulting
to "everything."

**R4. Can a read-only Todoist credential coexist with the operator's existing read-write one?**
Recommendation: **test it, and expect the answer to be no**. `td` stores one credential per account,
keyed by identity rather than scope, so re-authorizing read-only would most likely replace the operator's
daily credential. If it cannot coexist, either a second Todoist account or Bob supplying task context
(R1) is the way out.

**R5. May a brief be committed to the vault, given that it names the people who will be in a room?**
Recommendation: **keep briefs outside the vault**, or at minimum gate their entry into it. A brief
concentrates several recordings' worth of participant names into one document; the transcription design
already flagged transcripts as sensitive enough to weigh (T3), and a brief raises the same question at
higher stakes because it is a list of who was in the room rather than one incidental mention.

**R6. Does "brief" collide with Forzare's own morning brief?** Recommendation: **rename this artifact**,
to `vpp prep` or similar. Bob's morning brief is a day plan delivered on a schedule; this is a
per-occasion evidence pack produced on request, and one word naming two different things is a confusion
waiting for a future session.

**R7. Is the local Apple Calendar store representative of the operator's actual meetings?**
Recommendation: **confirm before relying on the Google-only recommendation above**. The measurement came
from Calendar.app's own aggregated store, 16 calendars, and a calendar that lives only there and not in
Google would be invisible to this design.

**Unblocks:** R1 through R4 decide whether the calendar half of this feature ships at all before Bob
exists; R5 and R6 are naming and placement decisions that do not block the notes-only default the design
already ships with.

## 6. Redacted drafts for sharing, with a review gate before release

`docs/superpowers/specs/2026-09-14-vpp-redacted-sharing-design.md`

This document designs `vpp share draft|review|approve|release` so that a shared draft is structurally a
different file from the private original, and so release is impossible without a human reading the exact
bytes being released. Measured: the vault auto-commits within about ten minutes and pushes within
fifteen, so any draft built inside the vault publishes before anyone reviews it, which is why the draft
tree lives entirely outside the vault and outside git until release.

**S1. Is a shared draft an extract of the source, or generated prose?** Recommendation: **extract only,
for now**. The design supports prose, generated by whatever the operator chooses and checked by
`vpp verify-note`, but states plainly that for prose the human read is the only real protection, since a
paraphrase can reintroduce a redacted fact in words the removal pass never saw. Ship the mode where the
residue scan is a real mechanical check.

**S2. What is the retention of drafts and of released copies?** vpp deletes nothing, so this is entirely
the operator's decision to make and enforce by hand. Recommendation: **a short retention window for the
draft tree**, since `draft.json` holds the map from every placeholder back to the real value it replaced,
making it more sensitive than the transcript it came from.

**S3. Should pseudonyms be stable across drafts, or restart at 1 in every draft?** Recommendation:
**per-draft numbering**, the design's default, because a stable global pseudonym would let two
separately-approved drafts be joined into one picture by a recipient who has both.

**S4. May a brief be drafted from?** Recommendation: **not yet**. A brief concentrates participants and
context, which is exactly what makes it useful and what makes it the most exposing possible source for a
redaction pass to work on.

**S5. Should an approval expire?** Recommendation: **yes, a short time-to-live**, on the order of a day.
An approval taken today and released weeks later reviewed the same bytes but not the same situation, and
a lapsed approval costs nothing but a repeated `vpp share approve`.

**S6. Does the released file say it came from vpp?** Recommendation: **no, keep the generic
`source_line`** that only states the file is a redacted extract, not a verbatim record. Naming the tool
would also tell a recipient that a recording exists, which is information the design otherwise strips out
entirely.

**S7. Is a PDF release path in scope?** Recommendation: **not yet**. The vault already has a PDF export
recipe and an operator will eventually reach for it, but a PDF carries its own producer and timestamp
metadata that a Markdown file does not, which needs its own answer rather than an assumed one.

**Unblocks:** S1 sets the shipped default mode; S3 through S7 refine the gate without blocking it, since
the gate itself (approval bound to bytes, no bypass flag) is not one of the open questions.

## 7. vpp's optional Open Notebook handoff

`docs/superpowers/specs/2026-09-14-vpp-open-notebook-handoff-design.md`

This document defines `vpp handoff`, a pure command that prints one document and performs no request, so
that handing content to Open Notebook cannot become a second capture-and-transcription workflow. The
deciding fact, read from Open Notebook's own documentation: dropping an M4A file onto it starts an
automatic transcription on a second engine with no flags, no alternatives, and no `known-terms.txt`,
which is exactly what the homelab plan already forbids and which vpp's document format makes structurally
impossible by having no field that can hold audio.

**H1. May a private note cross into Open Notebook, or only a released redacted draft?** Recommendation:
**only a released draft**, once Section 6's sharing gate exists. The design's own default allows private
notes to cross with the document recording that they did; the safer answer is to require the same review
gate this chain built for every other outbound copy, since a hosted model provider reading a private
transcript is an egress no different from emailing it.

**H2. Is the no-push rule right, three lines of `jq` and `curl` at every handoff, forever?**
Recommendation: **yes, keep it**. It is what makes the redacted-sharing gate in Section 6 still mean
something: an authenticated write channel inside vpp itself would turn that gate into a policy check in
front of an egress that already exists rather than a real boundary.

**H3. Which artifacts may be handed off at all?** Recommendation: **transcripts and analysis notes only,
for now**, excluding briefs and drafts until R5 and S4 are settled, since a brief names a room full of
people and a draft is meant to be the one thing that has already passed review.

**H4. Should an unreviewed artifact be refusable rather than labelled?** Recommendation: **keep it
labelled**, the design's current default. Refusing it sends the operator to copy and paste by hand, which
carries no header, no flag count, and no markers, which is strictly worse than a clearly labelled copy.

**H5. Who holds the shared password when Open Notebook exists, and does an agent get write access through
the `uvx open-notebook-mcp` server?** Recommendation: **KeePassXC on the laptop, no agent write access**,
matching the credential story the rest of this repository already uses and keeping the write path in the
operator's hands until there is a specific reason to automate it.

**H6. Does the handoff header need to say it came from vpp?** Recommendation: **yes, keep the current
header**, which names vpp and the record identity. That is honest provenance for a document a reader
might otherwise mistake for a verbatim record, and it is a smaller disclosure than the recording's actual
existence, which the document reveals regardless through its unresolved-flag count.

**H7. What happens to a copy in the notebook when the note it came from is corrected?** Recommendation:
**accept duplication for now**. vpp never learns Open Notebook's remote identifier, so a second handoff
creates a second source rather than replacing the first; building a write-back mechanism against a
service that is not deployed yet would be designing against an interface nobody has called.

**H8. Where does the transport recipe (the four lines of `jq` and `curl`) eventually live?**
Recommendation: **nowhere yet**. Deciding between a dotfiles `libexec` script, a `just` recipe, or a
homelab-side ingester is premature while Open Notebook itself is still queued behind two other homelab
deployments.

**Unblocks:** H1 through H3 decide what this feature is actually for before any transport is built; H5
through H8 are deferred by the design itself until Open Notebook is deployed and do not block anything in
vpp today.
