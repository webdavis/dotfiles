# vpp metadata schema, tags, relationships and filing rules

Status: design, written 2026-09-14 while the operator was asleep. Not approved, not built. No code was
written or changed. Every choice made in the operator's place is listed under "Assumptions made in the
operator's place" with its alternative, and the questions that need an answer are at the end.

Scope: `docs/remaining-work.md`, the `vpp (Voice Processing Pipeline)` section, fourth bullet, line 2193.

> Support agent-suggested tags and relationships between recordings, with a defined metadata schema and
> configurable output paths. Use explicit links and deterministic filing rules for automatic routing.
> Markdown output can live in an Obsidian vault and use its existing mobile sync, but Obsidian is
> optional.

The section intro, added 2026-09-12, sets the frame: "Build vpp in Rust to collect everyday Apple Voice
Memos synced to the Mac, preserve their original audio format, transcribe them, and produce agent notes
and summaries."

The operator's half of this task is the last sentence of Open Question 8, line 2326: "Choose output
layout and retention before ingestion starts." This document does not choose for them. It lays out the
layouts as priced options, measures what retention actually costs, and designs everything that does not
depend on which layout wins.

One question is already settled and is not reopened here. The ledger's open-questions preamble names
"optional Obsidian support" among the choices not to ask about again, so Obsidian being optional is a
constraint in this document, never a question.

## What this builds on

This is the fourth document in the vpp chain and it assumes the first three.

**The source reconciliation** established that `minutes` 0.26.1 already covers six of the seven vpp
feature bullets, including "tags and relationships across recordings", that the `minutes` keep-or-replace
ruling is still open, and that the vault's `agent-processing-pipeline/` layout is a filing convention
that exists and has never been used. This bullet is therefore the one place in the chain where a
third-party tool's existing answer has to be examined rather than assumed away.

**The discovery design** produced the identity every artifact hangs off:
`2026-08-24T144736-4f3ab19c02de`, a local capture timestamp from the audio file's own `mvhd` box plus
twelve hexadecimal characters of the file's SHA-256 (secure hash algorithm 256-bit) digest. It writes a
clone into `raw/audio/` and a sidecar into `~/.local/state/vpp/recordings/<id>.json`, and it stops there.

**The redundant transcription design** added the transcript note, its `Review` section, the timecode
source-reference convention (`[04:12]`), a `known-terms.txt` the operator grows by confirming a term
once, and one `pns submit --json` per recording that carries counts and paths and never any flagged text.
Its confirm-once-and-suppress-forever mechanism is reused below for tags rather than duplicated.

**The boundaries design** ruled on where things live, and one of its rulings is load-bearing here:
"Frontmatter key names come from configuration, not from code, so the vault's schema (`hub`, `status`,
`startDate`, `description`) is this operator's configuration rather than vpp's contract." This document
is the detailed form of that sentence. It also recommends that vpp be its own repository rather than a
fifth cargo workspace in dotfiles, which is still the operator's to settle and which nothing below
depends on.

Two disagreements inside the chain are still open and are flagged rather than resolved here: the audio's
home (the discovery design clones into the vault's `raw/audio/`, the boundaries design prefers an archive
directory outside the vault with an optional symlink), and vpp's shipping name (`VPP` collides with
FD.io's Vector Packet Processing). The filing design below works with either audio home, because the
audio path is one configured value.

## Constraints this design is bound by

From the ledger bullet and from `PLAN-v12-experiments-backlog.md` L-R5, which is the fuller statement of
the same feature:

1. Tags are **agent-suggested**, so they are proposals rather than facts.
1. Relationships hold **between recordings**, so this is a graph over vpp's own artifacts, not only a
   link from a note to its audio.
1. There is a **defined metadata schema**, written down, not implied by whatever the writer emitted.
1. Output paths are **configurable**.
1. Links are **explicit**, and filing rules are **deterministic**, and routing is **automatic**.
1. Markdown may live in an Obsidian vault and use its mobile sync, and **Obsidian is optional**.
1. The done-means for this item: "A written schema and filing rules that route every note
   deterministically and work with Obsidian absent."

From the vault's own `CLAUDE.md`, which governs every note in `~/workspaces/Ivy` and therefore governs
anything vpp writes there:

- **V1.** Frontmatter uses the documented property set in the documented order: `aliases`, `reference`,
  `hub`, `tags`, `status`, `description`, `startDate`, `endDate`.
- **V2.** `status` is one of `active`, `capture`, `needs-correction`, `complete`, `archived`.
- **V3.** Internal references are `[[wiki links]]`, not Markdown links.
- **V4.** "Do not alter tag naming conventions or folder-based implicit tags", and "Do not rename or
  remove frontmatter keys" and "Do not change value types".
- **V5.** A colon is forbidden in a filename.
- **V6.** A folder note is a Markdown file named after its parent directory, and it carries the
  DataviewJS listing that makes the folder's contents visible.
- **V7.** Obsidian Git auto-commits the whole vault on a timer. "Do not manually stage or commit vault
  files."
- **V8.** Prefer DataviewJS, retrieve values programmatically, never hardcode a filename into a query.

From the repository's standing rules, carried over from the earlier documents with the same labels:

- **R1.** No workspace may depend on another, and a tool never assumes this checkout exists.
- **R2.** Rust files target 300 lines and never exceed 500, unit tests included.
- **R3.** Never patch, fork or modify a third-party tool. `minutes` and Obsidian are both third party
  here; each is configured and called, never modified.
- **R4.** Tests cover the behavior of tools we wrote and nothing else.
- **R5.** The operator runs applies. An agent proposes.
- **R6.** This repository builds no removal mechanisms, and L-R5 says the evaluation is "not permission
  to delete recordings or notes". Retention, below, is a report rather than a reaper.

## What was measured, and how

Everything in this section was measured on dresden on 2026-09-14. Where a statement is inference rather
than measurement, the sentence says so. No note was written, no vault file was modified, and the Voice
Memos container was read for filenames only.

### The vault this output would land in

| Question                                     | Method                                        | Result                                      |
| -------------------------------------------- | --------------------------------------------- | ------------------------------------------- |
| Markdown files outside the nested repos      | walk, excluding the standalone git checkouts  | 753 files, 722 distinct note names          |
| notes carrying frontmatter                   | parse the leading `---` block                 | 738                                         |
| aliases declared across the vault            | same parse                                    | 246                                         |
| cost of a full name-and-alias scan           | timed, Python, cold-ish cache                 | **0.245 s**                                 |
| distinct tags in use                         | same parse                                    | **122**, over 1,109 uses                    |
| notes serializing `tags` as a block list     | same parse                                    | 445 (includes empty)                        |
| notes serializing `tags` as an inline array  | same parse                                    | 283                                         |
| notes serializing `tags` as a bare scalar    | same parse                                    | 4                                           |
| property names registered in `types.json`    | read the file                                 | 87, and `status`, `hub`, `description` are **not** among them |

Key frequencies across those 738 notes: `status` 738, `tags` 732, `aliases` 706, `reference` 474,
`startDate` 451, `endDate` 425, `hub` 417, `description` 324. So the documented schema is real and
near-universal, which is why this design conforms to it rather than proposing a better one.

Two of those rows change decisions.

**The tag vocabulary is small and its naming is not uniform.** 122 tags over 738 notes, of which 23 are
camelCase (`ironmanTraining`, `workoutBro`, `atHome`, `mentalHealth`), 10 are kebab-case
(`job-application`, `deep-dive`, `phase-2`), and the rest are single lowercase words (`review` 152,
`article` 140, `contact` 122, `friend` 78, `workout` 74). V4 forbids altering that convention, and there
is no single convention to follow, so a newly invented tag has no correct shape derivable from the
corpus. That is an open question below rather than a silent choice, and it is the reason new tags are
held rather than written.

**Unregistered property names are normal in this vault.** `status`, `hub` and `description` are used by
738, 417 and 324 notes respectively and appear in none of the 87 entries of `.obsidian/types.json`. So
vpp adding its own keys needs no edit to Obsidian's type registry, and a proposal to edit that registry
would be adding a dependency the vault does not itself have.

### The pipeline directory, as it stands

`agent-processing-pipeline/` holds `raw/audio/`, `transcripts/` and `analysis/`, each containing exactly
one file, `.gitkeep`, plus the committed `minutes` symlink to `/Users/stephen/meetings`. **No folder note
exists for any of them**, so under V6 none of these directories currently surfaces in any DataviewJS
listing. Adopting the layout therefore has a small furniture cost that this design names rather than
discovers later.

The vault's `.gitignore` excludes `*.wav` (line 79), `*.m4a` (80), `*.mp3` (81) and `*.opus` (85). The
Opus exclusion matters because the transcription design's cloud transcode is Opus: a stray one inside the
vault would not be committed by accident.

### How the vault reaches a phone, and how fast

Obsidian's core `sync` plugin is enabled (`"sync": true` in `.obsidian/core-plugins.json`, with
`"publish": false`), and the community `obsidian-git` plugin is installed and configured with
`autoSaveInterval: 10`, `autoPushInterval: 15`, `autoBackupAfterFileChange: true` and
`pullBeforePush: true`. The plugin's own settings label reads "Auto push interval (minutes)", so those
numbers are minutes.

Two consequences, both design inputs rather than opinions. First, **a file vpp writes into the vault
reaches GitHub without the operator doing anything**, within minutes, and reaches the phone by whichever
sync path is actually live. Second, **which sync path is live is unresolved**: the core Sync plugin being
enabled is not proof of an active subscription, and the two paths have different third parties
(Obsidian's servers against GitHub). The transcription design already asked whether transcripts may be
committed at all; this measurement sharpens that question rather than answering it.

Under V7, vpp never runs `git` against the vault. It writes files and lets the plugin commit them.

### What `minutes` already defines, read rather than remembered

`minutes schema` prints a JSON (JavaScript Object Notation) Schema, draft 2020-12, titled "Frontmatter",
described as "Frontmatter for a meeting/memo markdown file". It has 32 top-level properties, including:

- `tags`, an array of strings.
- `entities`, an object with `people` and `projects`, each an array of `EntityRef` objects carrying
  `slug`, `label` and `aliases`.
- `type`, a `ContentType` enum of `meeting`, `memo`, `dictation`.
- `status`, an `OutputStatus` of `complete`, `no-speech`, `transcript-only` or `degraded`.
- `sensitivity` (`normal` or `restricted`), `visibility` (`private` or `team`), `speaker_map`,
  `action_items`, `decisions`, `intents`, `name_corrections`, `processing_warnings`, `recording_health`.

The one live `minutes` note on this machine carries `title`, `type: meeting`, `date`, `duration`,
`status: transcript-only`, a nested `entities.projects` list, and `recorded_by`. **Not one of those keys
is a vault key**, and `transcript-only` is not one of the five vault `status` values. Its transcript body
uses `[SPEAKER_1 0:00]` line prefixes, which is the same bracketed timecode idea the transcription design
adopted, arrived at independently.

One correction to the earlier reconciliation while I was in here: **`relationship_map` is not a `minutes`
subcommand.** It is a feature flag in `minutes capabilities` (`relationship_map: yes`,
`relationship_map_policy_fresh_v1: yes`), which exists for MCP (Model Context Protocol) feature
detection. The command-line surface for that capability is `minutes people`, described as "Rank
relationships from a bounded, process-private projection of authorized meetings".
`minutes relationship_map` exits with `error: unrecognized subcommand`.

### Obsidian's own rules, from its documentation

Fetched from the vendor's help site, not recalled:

- **Tags** may contain "Alphabetical letters, Numbers, Underscore (\_), Hyphen (-), Forward slash (/) for
  Nested tags, Commonly accepted Unicode characters". "Tags must contain at least one non-numerical
  character. For example, #1984 isn't a valid tag, but #y1984 is." In frontmatter, "Tags in YAML (YAML
  Ain't Markup Language) should always be formatted as a list", without the hash.
- **Links** come in two formats and Obsidian supports both: `[[Three laws of motion]]` and
  `[Three laws of motion](Three%20laws%20of%20motion)`. "If interoperability is important to you, you can
  disable Wikilinks and use Markdown links instead." In the Markdown form, "make sure to URL encode the
  link destination. For example, blank spaces become `%20`."
- **Properties** support Text, List, Number, Checkbox, Date, Date and time, and Tags. On nesting: "To
  view nested properties, we recommend using the source mode." So a nested object in frontmatter, which
  is exactly what `minutes` writes as `entities`, is not a first-class property in the editor.

### The YAML trap, measured

A wiki link in frontmatter must be quoted, because YAML reads the bare form as a nested sequence. Run
through `yq`:

```
input                          parsed
reference: [[Note Name]]   ->  [["Note Name"]]     a list containing a list
quoted: "[[Note Name]]"    ->  "[[Note Name]]"     a string
```

The vault's own template quotes them (`reference: "[[2026-07-21-...pdf]]"`), so this is a rule the vault
already follows; it is written down here because a generator that emits the unquoted form produces valid
YAML with the wrong type, which V4's "do not change value types" forbids and which Dataview would read as
a list of lists.

### The recordings themselves, as a filing workload

From the Voice Memos container, filenames only, read-only:

| Question                            | Result                                                                 |
| ----------------------------------- | ---------------------------------------------------------------------- |
| files                               | 28                                                                     |
| filenames matching `YYYYMMDD HHMMSS-HEX` | **27**. The 28th is `20260819 174030.m4a`, with no hex suffix     |
| capture span                        | 2026-07-20 to 2026-09-03, 44 days                                      |
| rate                                | **4.3 recordings a week**, higher than L-R5's hypothesized three       |
| distinct capture days               | 15                                                                     |
| adjacent pairs captured on one day  | 12 of 26                                                               |
| adjacent gaps under 60 minutes      | **8**; under 10 minutes, 4                                             |
| busiest hour                        | 19:00, with 8 of 27                                                    |

Two findings. **The filename shape is not uniform**, which is one more reason the discovery design's
content-derived identity is right: a filing scheme that parsed the filename for a timestamp would have to
special-case the 28th file today and something else tomorrow. And **recordings cluster**: nearly half the
adjacent pairs share a day and 8 of 26 gaps are under an hour, so a "these two belong to one sitting"
relationship has measured support rather than being a feature someone imagined.

### What the artifacts cost, so retention can be priced

Measured from the transcription session's own outputs against its 34.38 second clip, then scaled by the
measured 16,584.45 second (4.607 hour) back catalogue:

| Artifact                          | Measured rate            | Whole back catalogue | A year at 4.3 a week  |
| --------------------------------- | ------------------------ | -------------------- | --------------------- |
| transcript text, Markdown         | 11.3 bytes per second    | about 187 KB         | about 1.5 MB          |
| openai-whisper JSON, word timings | 247 bytes per second     | about 4.1 MB         | about 33 MB           |
| whisply MLX JSON, word timings    | 559 bytes per second     | about 9.3 MB         | about 74 MB           |
| audio, Apple Lossless originals   | 498 kbit/s (prior doc)   | 983.6 MiB            | about 8 GiB           |

The interesting row is the last one, and it is not what it looks like. The discovery design clones with
`clonefile(2)`, and a copy-on-write clone consumes metadata only while the source still exists: the
187.9 MB recording cloned in 0.00 seconds and consumed 16 KB. **The clone only starts costing real bytes
when Apple's original goes away**, which is exactly when the clone becomes the archive and is worth
paying for. So retention on audio is not a storage decision, it is a decision about whether vpp's copy is
the backup. Everything else in the table is small enough that a retention mechanism would cost more to
build and audit than the bytes it saves.

## Approaches

The decision is which schema vpp writes and how notes are routed. Three candidates.

### A. vpp adopts the vault's note schema as its own

vpp emits `aliases`, `reference`, `hub`, `tags`, `status`, `description`, `startDate`, `endDate` in the
documented order and nothing else, with `status` carrying the pipeline state and `reference` carrying the
link to the audio.

Good: nothing new for the operator to learn, Dataview and the folder notes work on day one, V1 through V4
are satisfied by construction, and the schema is already proven across 738 notes.

Bad: it is a personal vault's convention, not a tool's contract. `hub` means nothing on a machine with no
vault. There is no room for the recording identity, the engine list or the open-flag count, so either
they go in the body where nothing can query them or the schema grows keys that are not the vault's,
which is approach C wearing a disguise. And `status` cannot carry the pipeline state honestly, because V2
fixes its five values.

### B. vpp adopts the `minutes` frontmatter schema

Take `minutes schema` as the contract: `type`, `date`, `duration`, `tags`, `entities`, `status`,
`sensitivity`, `visibility`. vpp's notes then join the same corpus, and `minutes ingest`, `minutes
people` and `minutes search` can read them.

Good: it is the reuse answer, it is already a published JSON Schema with an `api_version`, it already has
the two fields this bullet needs (`tags` and `entities` with slug, label and aliases), and if the
operator keeps `minutes` the two tools stop being two tools.

Bad: four problems, each fatal on its own. It is a third-party schema under a tool whose disposition is
undecided, and the whole chain has been careful to stay independent of that ruling. R3 means we cannot
fix it when it moves. Its `status` values collide with V2's, so a note cannot satisfy both the vault and
`minutes`. And its `entities` is a nested object, which Obsidian's own documentation says is viewable
only in source mode, so the vault half of the requirement degrades the moment the corpus half is
satisfied.

### C. A versioned record, a flat key block in the note, and the vault's keys from an output profile

The sidecar that already exists is the authority. The note is a rendering of it. vpp writes a small fixed
set of flat, prefixed keys (`vppSchema`, `vppRecording`, `vppStage`, and so on) plus whatever the
configured output profile adds: nothing at all in the portable profile, the vault's eight documented keys
in the Obsidian profile. Links live in a marked, managed block in the body, rendered as wiki links or
Markdown links by one configuration value. Filing is a rule table over an allowlist of recorded fields,
and the resolved path is pinned in the sidecar at first write.

Good: the done-means falls out of the structure. "Works with Obsidian absent" is the portable profile,
which is the shipped default and therefore the tested one. The vault's rules are satisfied by
configuration, which is what the boundaries design already ruled. A human edit to the note survives,
because vpp only ever rewrites inside its own markers. And nothing here is hostage to the `minutes`
ruling: if `minutes` wins, its notes are an input that vpp files, and the sidecar still holds the record.

Bad: two representations, the record and the note, which must not drift. The answer to drift is that only
one direction is authoritative and the other is regenerable, but that is a rule someone has to keep. It
is also more machinery than A, by roughly one profile table and one marker-aware writer.

### Recommendation

**C**, with one deliberate borrowing from B: where `minutes` has already named a concept, vpp uses its
name rather than inventing a synonym. `tags` is a list of strings. A relationship target carries a
`slug`, a `label` and `aliases`, because `EntityRef` already chose those three words and a second
vocabulary for the same idea helps nobody.

A is rejected because it cannot hold the recording identity without becoming C. B is rejected because a
schema the operator cannot change, under a tool that may be replaced, is a poor foundation for the one
file format that outlives everything else here.

## The recommended design

### Three layers, one authority

| Layer          | Where                                       | Authority                    | Survives                        |
| -------------- | ------------------------------------------- | ---------------------------- | ------------------------------- |
| The record     | `~/.local/state/vpp/recordings/<id>.json`   | **yes**, the source of truth | vault deleted, Obsidian absent  |
| The note       | the configured output directory, Markdown   | no, a rendering              | vpp uninstalled                 |
| The index      | derived at run time from the output tree    | no, a cache                  | nothing, it is rebuilt in 0.245 s |

The record is the one the design defends. The note is regenerable from it, and the index (note names,
aliases and the recording identity each note claims) is rebuilt by one scan of the output directory,
measured at 0.245 s over 753 files, so it is never persisted and never stale.

That ordering is what makes "Obsidian optional" structural rather than aspirational: Obsidian, the vault,
the wiki links and the folder notes all live in the rendering layer, and the layer beneath them does not
know they exist.

### The metadata schema

Written down, versioned, and small. Every key is flat, because Obsidian's documentation says a nested
property is source-mode only, and scalar or list-of-scalars, because those are the property types
Obsidian supports.

**The record** (`recordings/<id>.json`), extending the discovery and transcription designs' sidecar
rather than replacing it:

```json
{
  "schema": 1,
  "recording": "2026-08-24T144736-4f3ab19c02de",
  "captured_at": "2026-08-24T14:47:36-06:00",
  "duration_secs": 612,
  "title": "Invoice call",
  "title_source": "database",
  "audio_path": "raw/audio/2026-08-24T144736-4f3ab19c02de.m4a",
  "stages": {
    "transcript": { "path": "transcripts/2026-08-24-invoice-call-4f3ab19c.md", "written_at": "..." },
    "analysis":   { "path": "analysis/2026-08-24-invoice-call-4f3ab19c.md",    "written_at": null }
  },
  "tags": [
    { "tag": "invoice", "state": "confirmed", "provenance": "operator", "at": "..." },
    { "tag": "billing/quarterly", "state": "suggested", "provenance": "agent",
      "by": "claude-sonnet", "reason": "three mentions of the quarterly invoice", "at": "..." }
  ],
  "relations": [
    { "kind": "continues", "target": "2026-08-24T143012-91b2c740ab31",
      "state": "confirmed", "provenance": "derived", "rule": "session_gap_minutes=60" },
    { "kind": "mentions", "target_note": "Rajesh Muthukrishnan", "slug": "rajesh-muthukrishnan",
      "state": "confirmed", "provenance": "derived", "rule": "known-terms exact match" },
    { "kind": "related", "target": "2026-08-19T174030-6b1c0ddc4410",
      "state": "suggested", "provenance": "agent", "reason": "same invoice number" }
  ],
  "open_flags": 6,
  "filed_by_rule": 2
}
```

**The note**, portable profile, which is what ships and what every test runs against:

```markdown
---
vppSchema: 1
vppRecording: 2026-08-24T144736-4f3ab19c02de
vppStage: transcript
vppCapturedAt: 2026-08-24T14:47:36-06:00
vppDurationSecs: 612
vppOpenFlags: 6
vppEngines:
  - whisply:large-v3-turbo
  - elevenlabs:scribe_v2
tags:
  - invoice
vppSuggestedTags:
  - billing/quarterly
---

# 2026-08-24-invoice-call-4f3ab19c

<!-- vpp:links start -->
- Audio: `raw/audio/2026-08-24T144736-4f3ab19c02de.m4a`
- Continues: [2026-08-24-invoice-prep-91b2c740](2026-08-24-invoice-prep-91b2c740.md)
- Mentions: [Rajesh Muthukrishnan](../../areas/contacts/Rajesh%20Muthukrishnan.md)
<!-- vpp:links end -->

## Transcript
...
```

**The note**, Obsidian profile, which is what this operator's machine renders. The eight vault keys come
first in the documented order, the `vpp` keys follow, and every wiki link is quoted:

```markdown
---
aliases:
  - Invoice call
reference:
  - "[[2026-08-24T144736-4f3ab19c02de.m4a]]"
hub: "[[transcripts]]"
tags:
  - invoice
status: needs-correction
description: Voice memo recorded 2026-08-24, 6 spans awaiting review
startDate: 2026-08-24
vppSchema: 1
vppRecording: 2026-08-24T144736-4f3ab19c02de
vppStage: transcript
vppDurationSecs: 612
vppOpenFlags: 6
vppSuggestedTags:
  - billing/quarterly
---
```

Four rules govern the schema and they are the whole contract:

1. **`vppSchema` is an integer and it is the only compatibility signal.** A note whose `vppSchema` is
   higher than the running vpp knows is read but never rewritten, and the mismatch is reported. This is
   the cheapest possible version story and it is the one `minutes` uses (`api_version: 1`).
1. **`vppRecording` is the join key.** Every artifact of one recording carries the same value, in every
   profile. It is what lets a renamed note be found again, and it is the only key vpp searches on.
1. **vpp's own keys are prefixed `vpp` and are never keys the vault already uses.** Measured: `status`,
   `hub` and `description` are used by 738, 417 and 324 vault notes, so a bare `duration` or `source`
   would be a collision waiting for a Dataview query to trip over. The prefix is camelCase because the
   vault's own keys are (`startDate`, `payPeriodStart`, `relationshipDescription`), and Dataview reads
   `p.vppRecording` the same way it reads `p.status`.
1. **The profile adds keys, never renames vpp's.** A profile is a small table of literals and derived
   values, not a general renaming layer. Two profiles ship, `portable` and `obsidian`, and a third would
   need a reason.

The `status` mapping in the Obsidian profile is the one place where the vault's vocabulary happens to fit
vpp's state exactly, and using it is better than adding a parallel key:

| vpp state                         | vault `status`     |
| --------------------------------- | ------------------ |
| open review flags exist           | `needs-correction` |
| no open flags                     | `active`           |
| operator marked the note finished | `complete`, by hand, and vpp never overwrites it |

### Explicit links, and the managed block

Links live in the body between `<!-- vpp:links start -->` and `<!-- vpp:links end -->`. The mechanism is
the one this repository already uses for the shared agent-rules partial, where a generated block sits
between markers inside a file a human also edits.

Five rules:

1. **vpp rewrites only between the markers.** Prose above or below them is never touched. This is what
   makes the note safe for a human to edit and still safe for vpp to regenerate.
1. **If the markers are missing, unbalanced, or appear more than once, vpp refuses to touch the file**
   and reports the path. A generator that mangles its own markers gets a failure, not a second block.
1. **Both directions are written.** When the analysis note appears, the transcript note's block gains a
   link to it and the analysis note's block gains a link back. Obsidian's backlinks pane makes one
   direction enough inside Obsidian; with Obsidian absent there are no backlinks, so both sides are
   written. It also keeps the `find-unlinked-files` plugin quiet, which is installed in this vault.
1. **Link style is one configuration value.** `wiki` emits `[[Note Name]]`, `markdown` emits
   `[Note Name](relative/path.md)` with the destination URL-encoded, which the vendor's documentation
   requires (blank spaces become `%20`). The shipped default is `markdown`, because that is the form that
   works in both worlds, and this operator's machine configures `wiki` to satisfy V3.
1. **A wiki link in frontmatter is always quoted.** Measured above: the unquoted form is a nested
   sequence in YAML, not a string.

Generated filenames carry no spaces, so the two link forms differ only in their punctuation and the
URL-encoding rule never bites on vpp's own targets. It still applies to a `mentions` link into the
operator's existing notes, whose names do contain spaces.

### Tags: the agent proposes, the gate decides, the operator confirms

vpp does not contain a language model and does not call one. A proposal arrives from whatever wrote or
read the note, as one JSON document on standard input:

```
vpp tag propose <recording-id> --json -
{"tags": [{"tag": "billing/quarterly", "reason": "three mentions of the quarterly invoice"}],
 "by": "claude-sonnet"}
```

Every proposed tag passes a gate before it is recorded, and the gate is four deterministic checks in
order. A tag that fails any of them is rejected with the rule that rejected it, and rejection is a log
line, never a page.

1. **Syntax.** Letters, digits, `_`, `-` and `/` only, at least one non-numeric character, no leading
   `#`, no whitespace. Taken from Obsidian's documented tag rules so that a tag vpp writes is always a
   valid Obsidian tag, whether or not Obsidian is installed.
1. **Length and count.** At most `max_per_note` confirmed tags on a note, default 5, and at most
   `max_suggested` held suggestions, default 10. Excess is dropped in proposal order and recorded.
1. **Vocabulary.** A tag already present in the vault's own vocabulary, or in `known-tags.txt`, is
   accepted as `confirmed`. Anything else is recorded as `suggested` and written to `vppSuggestedTags`,
   not to `tags`. The vocabulary is the index described above, rebuilt in 0.245 s, so it is always the
   live vault rather than a copy that drifts.
1. **Shape.** A new multi-word tag is normalized to the configured `new_tag_style`, default `kebab`.
   Existing tags are never reshaped, because V4 forbids it: a proposal of `Invoice` matching the existing
   `invoice` is folded onto the existing spelling, and a proposal of `ironman-training` does **not**
   rewrite the vault's 41 uses of `ironmanTraining`, it is folded onto them by a case-and-separator
   insensitive comparison.

Confirmation reuses the transcription design's mechanism rather than inventing a second one:

```
vpp confirm <recording-id> --tag billing/quarterly      promote to tags, append to known-tags.txt
vpp confirm <recording-id> --reject-tag billing/quarterly
```

So the suggestion list shrinks as the operator uses it and converges on the operator's actual vocabulary,
exactly as `known-terms.txt` does for names, and exactly as `minutes vocabulary` does for the same reason
in its own corpus. The measured vault vocabulary is 122 tags; the design's whole purpose is to keep that
number growing at the operator's pace rather than at an agent's.

**Why suggestions are held rather than written.** A tag is a claim about what a recording is about, it is
visible in the tag pane forever, it is committed to git within minutes by V7's auto-commit, and V4 says
the naming convention is not the agent's to alter. An agent that tags 27 recordings a month unsupervised
would roughly double a 122-tag vocabulary within a year, and every wrong tag is in the history
permanently. Holding is cheap, and confirming is one command.

### Relationships between recordings

Four kinds, a closed set. Two are derived by rule and two are proposed. The kind name is part of the
schema and a fifth kind is a schema change, not a configuration value.

| Kind       | Meaning                                          | Provenance | Rule                                                          |
| ---------- | ------------------------------------------------ | ---------- | ------------------------------------------------------------- |
| `continues`| the previous recording of the same sitting       | derived    | previous capture ends within `session_gap_minutes`, default 60 |
| `mentions` | a note in the output tree this recording names   | derived    | exact match of a confirmed known term against a note name or alias |
| `related`  | another recording an agent thinks is connected   | agent      | held as `suggested` until confirmed                            |
| `supersedes`| this recording replaces an earlier one          | operator   | recorded only by `vpp confirm`                                 |

`continues` has measured support: 8 of 26 adjacent gaps in the existing corpus fall under 60 minutes and
4 under 10, so the relation fires on real clusters rather than on an imagined workflow. The threshold is
one configuration value and the default is deliberately generous, since a false `continues` link is a
link a human can see and remove, while a missing one is invisible.

`mentions` is the one relation that reaches outside vpp's own artifacts, and it is deliberately the
dumbest possible matcher: a term the operator has already confirmed in `known-terms.txt`, matched as a
whole token, case-insensitively, against the 722 note names and 246 aliases in the index. No fuzzy
matching, no embedding, no model. The transcription design's measurement is the reason: the engines'
errors are all in personal names, so a fuzzy matcher over transcript text would link "Muthakrishnan" to
the wrong contact note with total confidence. An exact match against a confirmed term cannot do that.

`related` never affects filing and never affects the vault's `tags`. It is a line in the managed link
block prefixed with its provenance, so a reader can see that a machine proposed it.

### Deterministic filing, defined precisely

"Deterministic" is the done-means, so it gets a definition rather than an adjective. Five properties, all
testable:

1. **Total.** The rule list ends in a catch-all, and vpp refuses to start if it does not. Every note
   matches exactly one rule, so there is no unrouted case and no default hidden in the code.
1. **Pure.** A path is a function of an allowlisted set of recorded fields, and of nothing else. The
   allowlist is `stage`, `captured_at`, `duration_secs`, `title_source`, and **confirmed** tags. Not
   suggested tags, not the transcript text, not the note body, not the clock, not the contents of the
   destination directory.
1. **Stable.** The resolved path is written into the record at first write and reused thereafter. A rule
   edit moves nothing until `vpp file --replan --apply` is run, which prints every move first and
   rewrites the managed link blocks that point at the moved files.
1. **Explainable.** `vpp file --explain <recording-id>` prints the matching rule's index, the field
   values that matched it, and the resolved path, without writing anything.
1. **Collision-free.** The name carries eight hexadecimal characters of the identity, and if the target
   path exists while carrying a different `vppRecording`, vpp refuses and names both.

The rule table:

```toml
[[output.rules]]
when = { stage = "transcript" }
dir = "transcripts"

[[output.rules]]
when = { stage = "analysis", tags = ["meeting"] }
dir = "analysis/meetings"

[[output.rules]]
when = {}          # the catch-all. Required, and required to be last.
dir = "analysis"
```

`when = {}` matches everything; a `when` naming several fields matches when all of them match; `tags`
matches when every listed tag is confirmed on the record. First match wins, and the matched index is
recorded in the note's own record as `filed_by_rule`, so a note can always say why it is where it is.

**The filing engine is exposed as a command**, which is how the later note generator files
deterministically without embedding a copy of the rules:

```
vpp path <recording-id> --stage analysis
~/workspaces/Ivy/agent-processing-pipeline/analysis/2026-08-24-invoice-call-4f3ab19c.md
```

That is the whole integration contract for "automatic routing". An agent, a `minutes` wrapper or the
operator asks vpp where a note goes and writes it there. Nothing registers, nothing subscribes, and there
is no second implementation of the rules to drift.

### Names

`name_template` defaults to `{date}-{slug}-{hash8}`, giving
`2026-08-24-invoice-call-4f3ab19c.md`. Three candidate shapes were considered:

- **The bare identity**, `2026-08-24T144736-4f3ab19c02de.md`, which the discovery design used for the
  audio clone. Perfectly stable and unreadable in a file list.
- **The vault's meeting-note convention**, `YYYY-MM-DD-<core-topic>.md`, which is readable and collides
  the moment two recordings share a day and a topic, and which changes if the title changes.
- **Date, slug and hash**, which reads like the vault's convention, cannot collide, and carries the tie
  back to the recording in the name itself.

The third is the recommendation. The slug comes from the Voice Memos title and from nothing else: not
from the transcript, because a transcript-derived slug would change when the engine changes, which breaks
property 3. A recording with no title (the discovery design's `title_source: "unavailable"` path) gets
`untitled`, and the name is pinned, so a title that arrives later does not rename anything. The title is
still recorded, as `aliases` in the Obsidian profile, so search finds it.

Slug sanitization is a trust boundary and is not negotiable. A title is operator-supplied text that
reaches a filesystem path:

1. Unicode normalization form KC, then case folding.
1. Every character that is not a letter, a digit or a hyphen becomes a hyphen; runs collapse; leading and
   trailing hyphens are trimmed.
1. Truncate to `slug_max_chars`, default 60, at a hyphen boundary.
1. Refuse, rather than sanitize, if the result is empty (use `untitled`), if the resolved path escapes
   the output root, or if any component of the resolved path is a symbolic link.

Rule 2 disposes of `:` (V5), of `/` and `..`, and of every character Obsidian's own filename guidance
warns about, and rule 4 is what stops a title of `../../.ssh/authorized_keys` from being a path.

### When the operator renames the note in Obsidian

This will happen. Obsidian renames files and rewrites the links pointing at them, and it knows nothing
about vpp's pinned path. The design handles it with the index rather than by forbidding it:

1. vpp resolves the pinned path and finds nothing there.
1. It scans the output root for a note whose `vppRecording` equals the identity. Measured cost: 0.245 s.
1. Exactly one match: re-pin to the new path, log one line, continue.
1. No match: treat the note as absent and rewrite it at the pinned path, which is the same behavior as a
   deleted note.
1. More than one match: refuse, and name every claimant. Two notes claiming one recording is a
   copy-paste, and guessing between them would destroy the operator's work.

### Folder notes, and why filing is flat by default

Under V6 every directory that should surface in a listing needs a folder note carrying the DataviewJS
query, and measured above, none of the three pipeline directories has one today. A filing scheme that
buckets by year or by tag therefore creates directories that are invisible in the vault until somebody
writes furniture for them.

So the default rule table is flat: everything of one stage lands in one directory. At the measured 4.3
recordings a week, `transcripts/` gains about 224 notes a year, which Obsidian handles without complaint
and which Dataview groups on demand by any property vpp writes. Bucketing is available by editing the
rule table, and the cost is one folder note per directory created, which is the operator's to pay.

vpp does not write folder notes. It is content, not furniture, and V8's DataviewJS pattern is the vault's
own. The three that are missing are named as an operator step below.

### Configuration

Extending the file the earlier designs already define. Following the repository's ruling that defaulted
keys ship uncommented at their default, so the shipped file shows the real posture:

```toml
[output]
# Where notes are written. The audio's home is [destination] in the discovery
# design and is deliberately a separate value: audio may live outside the vault.
root = "~/workspaces/Ivy/agent-processing-pipeline"
# "portable" writes only vpp's own keys. "obsidian" also writes the eight vault
# keys, in the vault's documented order.
profile = "portable"
# "markdown" links work everywhere. "wiki" is Obsidian's form.
link_style = "markdown"
name_template = "{date}-{slug}-{hash8}"
slug_max_chars = 60

[[output.rules]]
when = { stage = "transcript" }
dir = "transcripts"

[[output.rules]]
when = {}
dir = "analysis"

[tags]
known_tags_path = "~/.config/vpp/known-tags.txt"
# A proposed tag that is not already in the vault vocabulary or the known list is
# held in vppSuggestedTags until confirmed. Confirming appends it to that file.
accept_new = false
max_per_note = 5
max_suggested = 10
# Shape applied to a NEW tag only. Existing tags are never reshaped.
new_tag_style = "kebab"

[relations]
# Two recordings less than this apart are one sitting. Measured on this machine:
# 8 of 26 adjacent gaps fall under 60 minutes, 4 under 10.
session_gap_minutes = 60
# "mentions" links are exact matches of confirmed known terms against note names
# and aliases in the output root. No fuzzy matching.
mentions = "known-terms"
max_suggested_relations = 5
```

The machine's rendered file under chezmoi carries only what differs, which for this operator is
`profile = "obsidian"` and `link_style = "wiki"`. No secret is involved, so this stays outside the
fifteen KeePassXC-backed targets.

### The command surface

Five verbs, all of them small, three of them pure functions with no side effect:

```
vpp path <recording-id> --stage <stage>            print where a note goes. Writes nothing.
vpp file --explain <recording-id>                  print the matching rule and why. Writes nothing.
vpp file --replan [--apply]                        re-resolve pinned paths after a rule edit.
vpp tag propose <recording-id> --json -            validate and record a proposal.
vpp confirm <recording-id> --tag T | --reject-tag T | --relation kind:target
vpp note write <recording-id>                      re-render the managed regions from the record.
vpp storage                                        print what each artifact class costs. Deletes nothing.
```

### Failure modes

| Condition                                                | What vpp does                                            | Notification |
| -------------------------------------------------------- | -------------------------------------------------------- | ------------ |
| Last rule is not a catch-all                             | refuse at startup, naming the rule index                  | page once    |
| A rule's `dir` is absolute, escapes the root, or is a symlink | refuse at startup, naming the rule and the resolved path | page once    |
| Output root missing                                      | refuse; create a directory only when its parent exists    | page         |
| Target path exists with a different `vppRecording`        | refuse to write, name both identities                     | page         |
| Pinned path missing, exactly one note claims the identity | re-pin, continue                                          | none, log    |
| Pinned path missing, several notes claim the identity     | refuse, name every claimant                               | page         |
| Managed markers missing, doubled or unbalanced            | refuse to touch the file, name the path                   | page         |
| `vppSchema` higher than this build understands            | read, never rewrite, report                               | page once    |
| A proposed tag fails the syntax gate                      | drop it, record the rule that rejected it                 | none, log    |
| A proposed tag is new and `accept_new = false`            | hold as suggested                                         | none         |
| More than `max_per_note` confirmed tags                   | keep the first by proposal order, record the remainder    | none, log    |
| `mentions` target resolves to several notes               | write no link, record the ambiguity                       | none, log    |
| Title is absent                                           | slug becomes `untitled`, note is written                  | none         |
| Title sanitizes to nothing                                | slug becomes `untitled`                                   | none         |
| Output tree scan finds zero notes when the record says otherwise | continue, rebuild the index, report the count      | none, log    |

The split follows the transcription design's: a configuration mistake or an ambiguity that could destroy
work refuses loudly, and a routine rejection is a log line. Nothing in this component pages per
recording, because tags and links are not urgent and the transcription design already owns the one page
per recording that matters.

### Security and privacy

- **A tag is a content summary and it is permanent.** It is visible in the tag pane, it rides the vault's
  git history, and V7's auto-commit pushes it within minutes. That is the whole argument for holding new
  tags rather than writing them, and for never letting a suggestion reach `tags` without a command.
- **No tag, no title, no relation and no reason string ever rides in a pns request.** The transcription
  design set that rule for flagged spans; a tag is the same class of data and sometimes worse, because it
  is a compact statement of what a private recording is about. The notification carries counts and paths.
- **vpp sends nothing to a model.** It accepts a proposal on standard input. If an agent produced that
  proposal, the agent read the transcript, and that egress decision belongs to whoever ran the agent, not
  to vpp. The design deliberately keeps vpp out of it so the boundary stays visible.
- **The slug is sanitized before it is a path**, and a resolved path that escapes the output root or
  traverses a symbolic link is refused rather than repaired. The refusal-not-repair rule is the one the
  osquery converge already uses for the same reason.
- **The record is mode 0600 in a 0700 directory**, as the earlier designs set. The note inherits the
  vault's protections and is committed, unlike the audio, which `.gitignore` excludes by extension.
- **vpp never runs git**, per V7. It writes files and lets Obsidian Git commit them, which also means vpp
  cannot accidentally commit something the operator was about to delete.

### Retention, which is the operator's half

vpp deletes nothing, and no deletion mechanism is designed. R6 and L-R5 both say so, and the measurement
says it would not be worth building anyway: the entire back catalogue's transcripts are about 187 KB and
both engines' raw outputs together are about 13 MB. `vpp storage` reports the four classes with their
current sizes and the growth rate, and any pruning is the operator's own command.

The one real retention question is the audio, and it is not about bytes. A clone costs metadata while
Apple's original exists and starts costing its full size only when the original is deleted, which is the
moment it becomes the only copy. So the decision the operator is actually making is **whether vpp's copy
is the backup**, and the answer changes where it should live: inside the vault directory (the discovery
design, gitignored, synced nowhere) or outside it in an archive directory with a symlink (the boundaries
design, which is what `minutes` already does with `~/meetings`). This design works with either, which is
why `[output].root` and the audio destination are two separate configuration values.

One hazard to carry forward: if audio is ever routed through the `minutes` tree, `minutes storage`
already classifies recordings older than 30 days as `delete-candidate`. Nothing deletes unattended today,
because `cleanup` previews by default, but the classification is there and vpp's requirement is to
preserve originals.

### The behaviors to drive the implementation, test-first

Each is one failing test before one piece of code, in the repository's own style where a unit is a
behavior rather than a task. None needs Obsidian, a vault, an agent or a recording: each takes a record
structure and a temporary directory.

1. A rule table whose last rule is not a catch-all is refused at startup, naming the rule index, and no
   note is written.
1. A record matching two rules is filed by the earlier one, and `filed_by_rule` records that index.
1. `vpp path` is a pure function: two calls with the same record and rules return the same path, and
   neither creates a file.
1. A rule edit does not move an already-filed note; `--replan` reports the move and `--replan --apply`
   performs it and rewrites the link blocks that pointed at the old path.
1. A title containing `../`, a colon and a slash produces a slug with none of them, and the resolved path
   is inside the output root.
1. A title that sanitizes to an empty string files as `untitled`, and the note is still written.
1. A target path that exists carrying a different `vppRecording` is refused, and both identities appear
   in the message.
1. A note moved to a new path inside the output root is found by its `vppRecording` and re-pinned; two
   notes carrying that identity cause a refusal naming both.
1. Rewriting a note preserves every byte outside the managed markers, including prose a human added above
   and below them.
1. A note whose markers are missing, doubled or unbalanced is refused, not repaired, and no second block
   is added.
1. With `link_style = "markdown"` the rendered block contains no `[[`, and a target whose name contains a
   space is URL-encoded.
1. With `link_style = "wiki"` every wiki link written into frontmatter is quoted, and parsing that
   frontmatter yields a string rather than a nested list.
1. In the `portable` profile the frontmatter contains none of `aliases`, `reference`, `hub`, `status`,
   `description`, `startDate` or `endDate`, and the note parses as plain Markdown with no vault-specific
   syntax.
1. In the `obsidian` profile the eight vault keys appear first and in the documented order, and the `vpp`
   keys follow.
1. A record with open flags renders `status: needs-correction`; the same record with none renders
   `status: active`; a note already carrying `status: complete` keeps it across a rewrite.
1. A proposed tag of `#1984` is rejected by the syntax gate, and `y1984` is accepted.
1. A proposed tag containing a space, or beginning with `#`, is rejected with the rule that rejected it.
1. A proposed tag not in the vocabulary is written to `vppSuggestedTags` and not to `tags`;
   `vpp confirm --tag` moves it and appends it to `known-tags.txt`.
1. A proposal of `Invoice` where the vocabulary holds `invoice` is folded onto the existing spelling, and
   a proposal of `ironman-training` where the vocabulary holds `ironmanTraining` is folded onto the
   existing spelling rather than reshaping it.
1. More than `max_per_note` confirmed tags keeps the first by proposal order and records the remainder in
   the record, and the note's `tags` list never exceeds the cap.
1. Two recordings whose captures fall inside `session_gap_minutes` produce one `continues` relation on
   the later one and none on the earlier; outside the window, neither gets one.
1. A `mentions` relation is written only for a confirmed known term matching a note name or alias exactly
   as a whole token; a near-miss spelling produces nothing.
1. A known term matching two notes produces no link and records the ambiguity.
1. An agent-proposed `related` relation never changes the resolved path of any note.
1. The pns request emitted after a tagging pass contains no tag, no title, no reason string and no note
   name.
1. A full run against an output root that contains no `.obsidian` directory produces notes, links and
   filing with no error and no vault-specific syntax.

Behaviors 1 through 4 pin the determinism definition, 9 and 10 pin the safety of rewriting a human's
file, 13 and 26 pin the Obsidian-absent requirement, and 18 and 19 pin the tag gate. Those are the ones
worth writing first.

## Out of scope

Deliberately not designed here, and not to be smuggled in during implementation.

- **Generating tags.** vpp accepts a proposal. Which agent produces one, with what prompt, is a later
  bullet and an egress decision.
- **Writing the analysis note.** vpp says where it goes and links it once it exists. The note generator
  is a later bullet and may be `minutes`, an agent, or vpp itself.
- **Summaries, meeting briefs, redacted drafts, speaker labels.** Later bullets, and two of them are
  listed in the ledger as unapproved candidates.
- **A retention or deletion mechanism.** R6 and L-R5. `vpp storage` reports; nothing prunes.
- **Folder notes and DataviewJS queries in the vault.** Vault furniture, written once by the operator,
  not content vpp owns.
- **Migrating the vault's existing tag vocabulary.** V4 forbids altering tag naming conventions, and the
  measured 122-tag vocabulary is the operator's, not a mess to normalize.
- **A second output profile beyond `portable` and `obsidian`.** Logseq, a static site or a database would
  each need a reason before they need a profile.
- **Any change to the discovery or transcription designs' boundaries.** This stage consumes their records
  and adds keys to them.
- **The `minutes` disposition.** Untouched. Its schema was read and partly borrowed from; its tool is not
  depended on.
- **Bob, Forzare and Open Notebook.** Consumers, later.

## Assumptions made in the operator's place

Each of these was a choice this document had to make to be written at all. None is a decision, each names
its alternative, and reversing any of them changes the design without invalidating the measurements.

1. **The record in vpp's state is authoritative and the note is a rendering.** Alternative: the note is
   the record, which is what `minutes` does, and vpp reads frontmatter back. Taken because the done-means
   requires working with Obsidian absent, and a design whose only durable copy lives in a vault that may
   not exist has nowhere to stand. The cost is two representations that must not drift.
1. **vpp's own keys are flat and prefixed `vppSomething`.** Alternative: a nested `vpp:` object, which is
   tidier in the file. Rejected on the vendor's own documentation: nested properties are viewable only in
   source mode, so a nested block is invisible in the editor the operator uses.
1. **The shipped defaults are `portable` and `markdown`, and this machine configures `obsidian` and
   `wiki`.** Alternative: ship the vault-shaped defaults, since this operator is the only user today.
   Taken because the tested default should be the one the done-means names, and because the boundaries
   design already ruled that key names are configuration.
1. **The Obsidian profile maps open flags to the vault's `needs-correction` status.** Alternative: a
   separate `vppState` key and `status: active` always. Taken because the vault's own vocabulary already
   has a value meaning exactly this, and inventing a parallel state where one exists is the kind of
   duplication V4 exists to prevent.
1. **New tags are held as suggestions by default (`accept_new = false`).** Alternative: accept them, on
   the grounds that an agent's tag is usually fine and confirming 27 recordings a month is friction.
   Rejected on the measurement: 122 tags across 738 notes is a deliberately small vocabulary, the vault
   has no single naming convention for a new tag to follow, and every mistake is in git within minutes.
1. **New multi-word tags are kebab-case.** Alternative: camelCase, which the fitness subtree uses
   (`ironmanTraining`, `workoutBro`), or no normalization at all. Genuinely arbitrary: the corpus has 23
   camelCase and 10 kebab tags, so there is no majority to follow. This is the weakest assumption in the
   document and it is one configuration value.
1. **Tags come from a proposal on standard input, not from vpp calling a model.** Alternative: vpp spawns
   an agent itself, for which this repository has precedent (the `prepare-commit-msg` hook runs
   `claude -p --model=sonnet`). Taken because the note generator is undecided, because a tool that ships
   to other people should not require a particular agent, and because it keeps the egress boundary
   visible instead of hiding a transcript upload inside a tagging command.
1. **Filing keys only on confirmed metadata.** Alternative: let suggested tags route, which is more
   automatic. Rejected because it would make the path depend on a model's output, and the done-means asks
   for deterministic routing.
1. **A path is pinned at first write and a rule edit moves nothing until `--replan --apply`.**
   Alternative: recompute every run, so the tree always matches the rules. Rejected because it would move
   the operator's notes under them and break every link that Obsidian had rewritten.
1. **Names are `{date}-{slug}-{hash8}` with the slug from the Voice Memos title only.** Alternative:
   the bare identity (stable, unreadable), or a slug from the transcript's first words, which is what
   `minutes` does and which would change when the engine changes.
1. **Filing is flat by default.** Alternative: bucket by year or by tag, which scales further and costs a
   folder note per directory under V6. At 4.3 recordings a week the flat tree is fine for years.
1. **Four relation kinds, closed.** Alternative: a free-form `kind` string, which is more expressive and
   makes every consumer guess. A fifth kind is a schema change, which is the point.
1. **`mentions` is exact matching of confirmed terms only.** Alternative: fuzzy matching, which would
   catch more. Rejected on the transcription design's measurement: every engine error was a personal
   name, so a fuzzy matcher would confidently link the wrong contact.
1. **vpp writes no folder notes and runs no git.** Alternative: have vpp create the three missing folder
   notes on first run. Rejected because V7 and V8 make vault furniture the operator's, and a tool that
   writes Dataview queries into someone's vault has stopped being optional.
1. **No note was written into the vault while measuring.** Alternative: write one example note and look
   at it in Obsidian. Not done because the vault auto-commits and auto-pushes within minutes, so a probe
   note would have landed in the operator's git history while they slept.

## Operator steps

Four. The first is the one this design is waiting on.

**1. Answer the output layout half of Open Question 8.** Three layouts, and the difference that matters
is where the audio lives, not where the notes do:

| Layout                                                        | Audio                            | Notes                                | Cost                                             |
| ------------------------------------------------------------- | -------------------------------- | ------------------------------------ | ------------------------------------------------ |
| Discovery design: clone into the vault's `raw/audio/`         | inside the vault, gitignored     | `transcripts/`, `analysis/`, flat    | audio is inside a directory that syncs nothing   |
| Boundaries design: archive outside the vault, optional symlink | outside, like `minutes`' `~/meetings` | same                            | one more configured path, matches an existing precedent |
| Folder per recording                                          | with its note                    | `recordings/<id>/`                   | a folder note per recording under V6. Not recommended |

**2. Answer the retention half, which is smaller than it looks.** Transcripts are about 187 KB for the
whole back catalogue and about 1.5 MB a year; both engines' raw outputs together are about 13 MB. The
audio is 983.6 MiB logically and nearly free physically until Apple's original is deleted. The real
question is whether vpp's copy is the backup, and no machine backup exists yet.

**3. Write the three missing folder notes, if the vault layout is adopted.**
`agent-processing-pipeline/`, `transcripts/` and `analysis/` each need a folder note carrying the
reference DataviewJS query from the vault's `CLAUDE.md`, or nothing vpp writes will appear in a listing.
Three small files, once.

**4. Decide the new-tag shape.** kebab-case or camelCase for a tag that does not exist yet. The corpus
splits 10 to 23 the other way, so the default in this document may be the wrong one. One configuration
value either way.

## Open questions for the operator

1. **Which output layout, and is vpp's audio copy the backup?** Step 1 and step 2 above. Everything in
   this design is a configuration value once they are answered.
1. **New tags: held, or accepted?** The design holds them, because the vault's vocabulary is small and
   deliberate and git makes a mistake permanent. Accepting them removes a confirmation step per recording
   and doubles the vocabulary faster than any human would.
1. **kebab-case or camelCase for a new tag?** No majority exists in the corpus to derive it from.
1. **May vpp write into notes it did not create?** The `mentions` relation links out to the operator's
   existing contact and project notes. The design writes that link on the transcript's side only and adds
   nothing to the target note. The alternative, a backlink written into the contact note, is more useful
   inside Obsidian and is vpp editing the operator's own writing.
1. **Which mobile sync actually carries the vault?** Obsidian's core Sync plugin is enabled and
   `obsidian-git` is configured to push every 15 minutes. They send the same transcripts to different
   third parties, and the transcription design's open question about committing transcripts at all cannot
   really be answered without knowing which.
1. **Should `vpp path` be the contract for the later note generator, or should vpp write the analysis
   note itself?** The design exposes the rules as a command so any generator can file correctly. If vpp
   ends up owning generation too, the command is still the right seam, but it stops being load-bearing.
1. **Does `minutes` stay?** If it does, its notes are input that vpp files, and the two schemas sit side
   by side in one vault with no key in common. That is workable and slightly ugly, and the alternative,
   adopting its schema, was rejected above for reasons that would need revisiting if `minutes` becomes
   the note generator rather than a candidate. **Decided 2026-09-15: no, `minutes` is out entirely.**
   There is only vpp's own schema; the two-schemas-side-by-side question does not arise. See
   `docs/decisions/2026-09-15-vpp-architecture-decisions.md`, decision 1.
1. **Where does vpp's code live, and what is it called?** Carried forward unresolved from the boundaries
   design, because the chain should not stay in disagreement with itself.
