# vpp meeting briefs, with optional calendar and Todoist context

Status: design, written 2026-09-14 while the operator was asleep. Not approved, not built. No code was
written or changed. Every choice made in the operator's place is listed under "Assumptions made in the
operator's place" with its alternative, and the questions that need an answer are at the end.

Scope: `docs/remaining-work.md`, the `vpp (Voice Processing Pipeline)` section, fifth bullet.

> Plan meeting briefs using relevant notes, with optional read-only calendar and Todoist inputs. Record
> Bob, the future Hermes executive assistant, as a consumer of vpp's notes and briefs. The exact trigger,
> scheduling owner, access scopes and provider choices remain under discussion. Keep source references
> and unresolved transcription issues visible to Bob and in the brief.

The fuller statement of the same feature is homelab `PLAN-v12-experiments-backlog.md`, L-R5, fifth
bullet, read today rather than remembered:

> Prepare meeting briefs from relevant notes, including source references and unresolved issues. Optional
> read-only calendar context can identify meetings, times and participants; optional Todoist context can
> supply relevant tasks, deadlines and completion status. Limit sources to user-selected
> calendars/projects and verify the available permission boundaries. Requested versus automatic briefs,
> scheduling ownership, providers and lead times remain design questions.

The operator's half is Open Question 8 in the ledger, which asks them to "settle whether vpp reads
optional calendar/Todoist context directly or accepts it from Bob, which calendars and projects it may
read, and requested versus scheduled briefs. Discuss these separately rather than treating them as one
approval." This document does not settle any of the three. It verifies the permission boundaries the
ledger asks to be verified, measures what a brief would actually have to work with on this machine, and
designs the parts that do not depend on the three answers, in a shape where each answer sets a
configuration value rather than rewriting anything.

The done-means for this item, from the triage record: "A brief plan exists with every undecided choice
listed as an operator question rather than assumed."

## What this builds on

This is the fifth document in the vpp chain and it assumes the four before it.

**The source reconciliation** established that `minutes` 0.26.1 already covers six of the seven vpp
feature bullets, that its keep-or-replace ruling is still open, and that the vault's
`agent-processing-pipeline/` layout is a filing convention that exists and has never been used. That open
ruling binds here too, and it bites harder in this bullet than in the others, because `minutes` ships a
command that overlaps with half of what a brief does. It is measured below rather than waved away.

**The discovery design** produced the recording identity every artifact hangs off,
`2026-08-24T144736-4f3ab19c02de`, and the sidecar record at
`~/.local/state/vpp/recordings/<id>.json`.

**The redundant transcription design** produced the transcript note, its `Review` section, the timecode
source-reference convention (`[04:12]`), the `known-terms.txt` the operator grows by confirming a term
once, and the rule that one `pns submit --json` per recording carries counts and paths and never any
flagged text. Its review record is where "unresolved transcription issues" physically live, so this
document consumes it rather than inventing a second notion of uncertainty.

**The tags, schema and filing design** produced the three-layer model this document extends: the record
in vpp's state is authoritative, the note is a rendering, and the index over the output tree is a cache
rebuilt in 0.245 seconds. It also produced the two structures the brief's selection rule reads, confirmed
tags and four closed relation kinds, and the filing engine exposed as `vpp path` so any later generator
routes deterministically without a second copy of the rules. A brief is a new stage in that same rule
table, not a new filing system.

Two disagreements inside the chain remain open and are not resolved here: the audio's home, and vpp's
shipping name (`VPP` collides with FD.io's Vector Packet Processing). Neither affects this bullet.

## Constraints this design is bound by

From the two ledger statements above:

1. A brief is built **from relevant notes**. Notes are the primary source; calendar and Todoist are
   **optional** context, and the word optional is load-bearing, not a hedge.
1. Calendar context identifies **meetings, times and participants**. Todoist context supplies **relevant
   tasks, deadlines and completion status**.
1. Both are **read-only**, and sources are **limited to user-selected calendars and projects**.
1. The **available permission boundaries are to be verified**, not assumed. That is a research
   deliverable of this document and it is section "The permission boundaries, verified at the provider".
1. **Source references and unresolved transcription issues stay visible** in the brief and to Bob.
1. **Bob is recorded as a consumer** of vpp's notes and briefs, and "Bob must retain uncertainty and
   provenance when using the outputs" (L-R5, sixth bullet).
1. **vpp remains useful independently.** L-R5: "vpp remains useful independently; this integration does
   not bring Forzare forward from its post-modernization schedule."
1. Trigger, scheduling ownership, providers and lead times are **open**, and the done-means says they
   must appear as operator questions rather than as assumptions.

From the Forzare section of the same ledger, which is the other side of the same integration:

> Let Bob consume vpp's transcripts, metadata and briefs for meeting preparation. Keep provenance and
> unresolved transcription warnings visible; do not turn uncertain notes into confirmed commitments.

That last clause is the sharpest requirement in the whole bullet, because it names a specific failure:
Bob's job is to turn things into commitments, in Todoist and on a calendar, and an uncertain line in a
brief becoming a confirmed commitment is exactly the damage the redundant transcription design exists to
prevent. It is designed against directly below.

From the repository's standing rules, carried over from the earlier documents with the same labels:

- **R1.** No workspace may depend on another, and a tool never assumes this checkout exists.
- **R2.** Rust files target 300 lines and never exceed 500, unit tests included.
- **R3.** Never patch, fork or modify a third-party tool. `minutes`, `gog`, `td` and Obsidian are all
  third party here; each is configured and called, never modified.
- **R4.** Tests cover the behavior of tools we wrote and nothing else.
- **R5.** The operator runs applies. An agent proposes.
- **R6.** This repository builds no removal mechanisms.

And from the vault's own `CLAUDE.md`, the rules a brief written into the vault has to satisfy, using the
same labels the previous document used: **V1** the documented frontmatter key set and order, **V2** the
five `status` values, **V3** wiki links for internal references, **V5** no colon in a filename, **V6** a
folder note per directory that should surface in a listing, **V7** Obsidian Git auto-commits the vault on
a timer and vpp never runs git.

## What was measured, and how

Everything in this section was measured on dresden on 2026-09-14. Where a statement is inference rather
than measurement, the sentence says so. Nothing was written, no calendar or task was created or modified,
no note was added to the vault, and every calendar query below ran against a throwaway copy of the store
opened with SQLite's `immutable=1` flag and deleted afterwards. Queries returned counts, never event
titles, participant names or addresses.

### The permission boundaries, verified at the provider

The ledger asks for these to be verified. Three providers were checked and they do not agree with each
other, which is the finding.

**Google Calendar, through its own documentation.** The application programming interface (API) defines
eight read-only scopes, including `https://www.googleapis.com/auth/calendar.readonly` ("See and download
any calendar you can access using your Calendar") and
`https://www.googleapis.com/auth/calendar.events.readonly` ("View events on all your calendars"), plus
narrower ones for the calendar list, calendar properties, settings, access control lists, owned events
and public events. Google's own guidance on the page is to "choose the most narrowly focused scope
possible". **There is no per-calendar scope.** The narrowest thing a token can say is "read events", not
"read these three calendars".

**Todoist, through its own documentation.** The scopes are `task:add`, `data:read`, `data:read_write`,
`data:delete`, `project:delete` and `backups:read`. `data:read` "grants read-only access to application
data, including tasks, projects, labels, and filters". **There is no per-project scope.** A read-only
Todoist token reads every project the account can see.

**Apple EventKit, through its own documentation and the current developer guidance.** Since macOS 14 the
authorization levels for calendar events are **full access** and **write-only**, requested with
`requestFullAccessToEvents()` (declaring `NSCalendarsFullAccessUsageDescription`) and
`requestWriteOnlyAccessToEvents()` (declaring `NSCalendarsWriteOnlyAccessUsageDescription`). **There is
no read-only level for events.** A process that wants to read the Mac's calendar through the supported
framework must hold a grant that also permits writing and deleting.

So the requirement "read-only calendar" is satisfiable at the permission layer by exactly one of the
three, and it is the Google path. The conclusion that follows is not a preference:

| Requirement                        | Google Calendar API | Todoist API   | Apple EventKit          |
| ---------------------------------- | ------------------- | ------------- | ----------------------- |
| a genuinely read-only credential   | yes                 | yes           | **no, full access only** |
| per-calendar or per-project scope  | no                  | no            | no                      |
| who enforces the source selection  | the client          | the client    | the client              |

The second and third rows are the other half of the finding and they set a design rule further down:
since no provider can enforce "only these calendars" or "only these projects", **vpp has to, and it has
to do so in a way the operator can audit.** A selection that exists only as a post-fetch filter is a
selection that a bug turns into "everything".

### The calendar on this machine, as a workload

Read from a copy of `~/Library/Group Containers/group.com.apple.calendar/Calendar.sqlitedb`, 8,028,160
bytes, which the copy proved is readable by a process holding Full Disk Access. This is the aggregate of
whatever accounts are configured in Calendar.app, so it is a lower bound on the operator's calendars
rather than a complete picture.

| Question                                             | Result                                     |
| ---------------------------------------------------- | ------------------------------------------ |
| calendars                                            | **16**, across 5 distinct store types      |
| calendar items in the whole store                    | 4,247                                      |
| items starting in the last year                      | 1,910                                      |
| items starting in the last 90 days                   | 48                                         |
| items in the last 90 days carrying any participant   | **8**                                      |
| participant rows in the whole store                  | 192                                        |
| items starting in the next 30 days                   | 2                                          |
| items starting in the next 90 days                   | 9                                          |
| items starting in the next year, and how many timed  | 44, of which **0 are timed**, all all-day  |
| live recurring series (no end date, or ending later) | **54**                                     |

Two caveats, both important enough that the conclusion is stated narrowly.

**A row scan undercounts recurring events.** A recurring event is one row plus a recurrence rule, and its
occurrences are generated at query time, so "2 items start in the next 30 days" counts non-recurring
items and masters only. There are 54 live series whose occurrences a row scan cannot see. Counting them
properly means expanding recurrence rules, which is a specification (RFC 5545) rather than a query, and
which the Google Calendar API already does server-side when single events are requested. That is an
argument for the provider path over the file, independent of the permission argument.

**The store-type-to-account-kind mapping is not asserted.** Nine of the 16 calendars share one store
type and I did not map type numbers to account kinds, because that mapping is training knowledge rather
than something I verified today.

What survives both caveats is the number the brief actually cares about: **8 items in the last 90 days
carried a participant at all**, which is the best available proxy for "a meeting with other people" and
which works out to under one a week. Together with the fact that not one non-recurring forward item is
timed, the honest reading is that this operator's calendar is not currently where their meetings live.
The one meeting note in the vault from the current era, `2026-07-21-autism-adhd-and-cpi-training.md`, is
a staff meeting at a job whose schedule lives in Forzare's `work_schedule` configuration rather than on a
calendar, which fits.

That measurement is the single strongest input to the trigger question, and it is why the recommendation
below is requested briefs rather than scheduled ones.

### What is installed that could supply the context

| Tool          | Version  | Where                       | Relevant surface                                             |
| ------------- | -------- | --------------------------- | ------------------------------------------------------------ |
| `gog`         | v0.40.0  | `/opt/homebrew/bin/gog`     | `gog calendar events list`, `--readonly`, `--wrap-untrusted`  |
| `td`          | 5.3.4    | the fnm node lane           | `td task list --json`, `td auth login --read-only`            |
| `minutes`     | 0.26.1   | cask                        | `research`, `person`, `commitments`, `actions`, `insights`    |

**`gog` is the calendar provider Bob was already going to use.** The Forzare surfacing-engine design
names "gog calendar" as Bob's source, defines `calendar-read` and `calendar-write` as separate skills,
and confines Bob's writes to a dedicated robot calendar. So choosing `gog` for vpp is not a new
dependency in this household; it is the same one, and `~/.config/gogcli/credentials.json` is already one
of the fifteen KeePassXC-backed chezmoi targets.

Four `gog` flags were read from its own help output today and all four matter:

- `--readonly`: "Block mutating API requests at runtime; auth add also requests read-only OAuth scopes."
  So the read-only posture is enforced twice, once at the grant and once at the call.
- `--enable-commands`, `--enable-commands-exact`, `--disable-commands`: restrict which commands the
  binary will run at all. A caller can hand `gog` a surface of exactly one command.
- `--wrap-untrusted`: "In JSON/raw output, wrap fetched text fields in external untrusted-content
  markers." This exists precisely for the hazard in the security section below, and using it costs one
  flag.
- `--client`: "OAuth client name (selects stored credentials + token bucket)", which is why a second,
  read-only credential can plausibly sit beside the operator's existing one. Plausibly, not verified: I
  did not run `gog auth add`, and `gog auth list` did not return inside a 20 second timeout, so whether
  two token buckets for one account coexist cleanly is an operator step below rather than a claim here.

`gog auth services` reports that the calendar service's default scope set is the single full scope
`https://www.googleapis.com/auth/calendar`. So today's credential, if one exists, is read-write, and a
read-only one is a separate authorization.

**`td` can do read-only, and it has one slot.** `td auth login --read-only` is a documented flag on the
installed version. `td auth status` reports the current credential as `Mode: read-write` for
`stephen@webdavis.io`, and `td accounts list` shows exactly one stored account, keyed by its numeric id.
Accounts are keyed by identity, not by scope, so **re-authorizing read-only would most likely replace the
operator's own read-write credential** and break their daily `td` usage. That is the sharp edge in the
Todoist half and it is an open question rather than something to work around silently.

**`minutes` already ships something brief-shaped.** `minutes research <query> --attendee <person>
--since <date>` is described as "Research a topic across meetings, decisions, and open follow-ups", and
`minutes person <name>` builds a bounded profile of somebody across meetings. Also measured:
`minutes health` reports "Calendar access is available for meeting suggestions", so a third-party binary
on this machine already holds the EventKit grant discussed above.

This is the ladder rung that has to be taken seriously rather than stepped over: an installed tool
already answers "what do I know about this person and this topic". Three things stop it from being the
answer to this bullet, and only the third is about vpp:

1. It searches **its own corpus** at `~/meetings`, not vpp's notes. It would have to be given them, which
   means adopting its schema, which the previous design rejected for reasons that still hold.
1. It has **no concept of vpp's unresolved transcription flags**, which are the one thing this bullet
   insists stay visible. A `minutes research` result cannot carry them because nothing in its model
   represents them.
1. Its **keep-or-replace ruling is open**, and the chain has been careful not to make anything depend on
   it.

So `minutes research` is not adopted, and it is also not dismissed: if `minutes` stays, it is a
legitimate additional source of relevant material for a brief, reachable through the same context input
as everything else. That is an open question below, not a decision here.

### The join between an occasion and the operator's notes

A brief has to answer "which of my notes are about this". Two joins are available and both were measured.

**By person, through the vault's own notes.** 136 contact notes under `areas/contacts/`. 103 notes under
`areas/` carry an `email:` frontmatter key, and here is the problem: **78 of them are literally
`email: []`**, 25 carry the bare key, and only 8 of those are followed by an actual list item. So an
email-to-contact join, which is the obvious way to turn a calendar participant into a vault note,
resolves roughly 8 notes today and nothing else. The join that does work is the one the previous design
already built: exact, whole-token, case-insensitive matching of a **confirmed known term** against the
722 distinct note names and 246 aliases in the output index, rebuilt in 0.245 seconds.

**By topic, through confirmed tags.** 122 distinct tags over 1,109 uses across 738 notes, and the
previous design's gate means a tag on a vpp note is either confirmed by the operator or held as a
suggestion. Confirmed tags are therefore usable as selection input; suggestions are not.

**The recording corpus, for scale.** 28 recordings over 44 days, 4.3 a week, capture span 2026-07-20 to
2026-09-03. A brief selecting from a 180 day window is selecting from roughly 110 recordings a year, not
from thousands, which is why the selection rule below can afford to be dumb and exact rather than clever.

### The scheduler question, answered from the source

The ledger asks who owns scheduling. Three candidates exist on this machine and one of them is already
excluded by its own design.

**The pns daemon cannot run a brief.** Its usage is
`pns daemon schedule --id <id> [--in <secs>] [--every <secs>] [--until ...] [--unless-marker <name>] --
<event args>`, and reading `pns/crates/pns/src/daemon_runtime.rs` shows the trailing arguments are pns
subcommand arguments handed to a child pns process (`args: vec!["daemon".into(), "retry".into()]`). The
clock schedules pns events, not arbitrary commands. It can **remind** the operator that a brief is due;
it cannot produce one.

**launchd can**, and it is this repository's standard for every scheduled or supervised job: a
chezmoi-tracked plist under `Library/LaunchAgents/` with a matching `run_onchange_after_*` loader.
Thirteen such agents exist today.

**Hermes cron can**, and in Forzare's design it already does: the surfacing-engine spec has cron firing
the morning brief, transitions and end-of-day. But Hermes is post-modernization work and L-R5 is explicit
that this integration "does not bring Forzare forward from its post-modernization schedule".

So the scheduling owner, if briefs are ever scheduled, is launchd today and Bob later, and the honest
recommendation given the calendar measurement is that neither is needed yet.

## Approaches

Three decisions have to be made and they are separable, which is what the ledger's Open Question 8 asks
for ("Discuss these separately rather than treating them as one approval"). Each is presented with its
candidates.

### Decision 1: what a brief is, and who writes its sentences

**A. vpp assembles an evidence pack and writes no sentences of its own.** The brief is sections of
extracted, cited material: the occasion, the notes selected and why, the quoted spans with their
recording identity and timecode, the open review flags, and the optional task and participant context.
Every line traces to something.

Good: vpp contains no model and calls none, which the previous design already established as the chain's
boundary. Nothing can be hallucinated because nothing is generated. It is deterministic, so it is
testable with fixtures and no network. It works the same on a machine with no agent installed.

Bad: it is not what most people mean by a brief. It is a dossier, and reading it is work. The ledger's
last bullet lists "summary format" among the things still to be chosen, which implies somebody expects
prose.

**B. vpp generates the brief by calling an agent.** vpp spawns a model, hands it the selected notes, and
files what comes back. There is precedent in this repository: the `prepare-commit-msg` hook runs
`claude -p --model=sonnet`.

Good: it produces the thing a person wants to read, in one command, with no second tool.

Bad: it puts an egress decision inside vpp, where it is invisible. It makes vpp depend on a particular
agent being installed and authorized, which breaks "vpp remains useful independently" and would embarrass
a `cargo install` on somebody else's machine. It also reintroduces exactly the risk the transcription
design was built to control: a model writing confident prose over text that is flagged as uncertain. And
the previous design already rejected this shape for tags, for the same reasons.

**C. vpp assembles the pack, and prose is a proposal that is checked before it is filed.** A is the
shipped default and the only path that ships tested. A generator (an agent, Bob, or `minutes` if it
stays) may write prose over the pack and hand it back, and it goes through the transcription design's
existing `vpp verify-note`, which already has a flag class for "a claim built on already-flagged text".
The prose is filed only with its check result attached.

Good: it is the only option where a generated sentence is checked against the transcript before it counts
as a brief. It keeps the egress boundary visible at the caller rather than inside vpp. It reuses a
command that already exists in the chain rather than adding a second checking mechanism. And it leaves
the "summary format" question genuinely open, because the format is the generator's, not vpp's.

Bad: two artifacts where B has one, and the generator is somebody else's problem, which means on day one
the operator gets A and has to run a second thing to get prose.

**Recommendation: C**, with A shipped and tested as the default posture. It is the same shape the chain
already chose twice: vpp produces structure and checks claims, and a model, if any, is invoked by the
person who decided to invoke one.

### Decision 2: who reads the calendar and Todoist

This is the operator's question in the ledger, and the point of this section is that it does not have to
be answered before implementation starts.

**A. vpp reads them directly.** vpp holds a read-only Google credential and a read-only Todoist token and
spawns `gog` and `td`.

Good: vpp is useful before Bob exists, which the ledger requires. No dependency on an unbuilt assistant.

Bad: two more credentials on the machine, one of which (Todoist) probably cannot coexist with the
operator's own. It also duplicates work Bob will do anyway, since Forzare's design already has
`calendar-read` and `todoist-surface` skills.

**B. Bob supplies them.** vpp accepts context from whoever asks for the brief and never talks to a
provider.

Good: one credential holder in the household. The access question moves to the component whose entire job
is access. vpp stays small.

Bad: vpp's briefs then need Bob, which the ledger forbids ("vpp remains useful independently"), and Bob
is post-modernization work.

**C. One validated input document that either can produce.** vpp defines a small context schema and
accepts it on standard input. Two optional collectors ship inside vpp, thin spawners of `gog` and `td`
that produce exactly that document, disabled by default. Bob, when it exists, produces the same document
from its own skills and hands it over.

Good: it answers the operator's question by making both answers work, and by making the choice a
configuration value. It gives the two paths one code path to test, so the Bob integration is not a second
implementation that drifts. And the collectors being off by default means the shipped, tested posture is
the one with no credentials at all, which is also the one the done-means cares about.

Bad: one more schema to version, and a temptation to grow it.

**Recommendation: C.** The operator still has to answer which provider they want to hold the credential,
but nothing waits on that answer.

### Decision 3: requested or scheduled

**A. Requested only.** `vpp brief` is typed, or called by something else.

**B. Scheduled with a lead time.** A launchd agent wakes, looks ahead by a configured lead time, and
writes a brief for anything it finds.

**C. Both, with the scheduler off by default.**

The measurement decides this one. Not one non-recurring forward calendar item on this machine is timed,
and participant-bearing items ran at 8 in the last 90 days. A lead-time scheduler would wake every
morning and find nothing, and it would need the recurrence expansion this document already established a
row scan cannot do. Building it now is speculative work against a workload that does not exist yet.

**Recommendation: A**, with the seam for B named and left unbuilt: `vpp brief` takes an occasion on its
command line or in the context document, so a launchd agent that later wants to call it in a loop needs
no change inside vpp. If the operator starts putting meetings on a calendar, B becomes a plist and a
`run_onchange_after_*` loader, which is the shape this repository already uses thirteen times.

## The recommended design

### The boundary

One new command, and one new stage in machinery that already exists.

```
vpp brief <occasion>  [--context -] [--collect] [--json] [--explain] [--dry-run]
```

`<occasion>` is either an occasion identity vpp has already recorded, or a new one described inline with
`--title` and `--at`. `--context -` reads a context document on standard input. `--collect` runs the
configured local collectors instead. The two are mutually exclusive and supplying both is an error, not a
merge, because a silent merge makes the provenance of a line ambiguous, and provenance is the point.

What it does not do: it does not transcribe, it does not tag, it does not write prose, it does not send
anything to a model, it does not write to a calendar or to Todoist, and it does not notify. A requested
brief is requested by somebody who is already looking at the terminal.

The internal seam is the one the other tools use and the one the transcription design already set: a
domain crate holding the occasion model, the selection rule, the pack assembly and the rendering, with no
input or output; an adapters crate owning the two collectors, the filesystem and any process spawning.
Under R2 the domain crate splits by stage: `occasion`, `select`, `pack`, `render`. Each collector is one
adapter file.

### The occasion, and why it needs an identity of its own

A brief is not about a recording, so it cannot hang off a recording identity. It is about an
**occasion**: a meeting, at a time, with people, possibly on a calendar and possibly not. The occasion
gets the same treatment the recording got in the discovery design, for the same reason: re-running the
command must produce the same identity, the same path and a rewritten file rather than a second one.

```
occasion id = <local date and time>-<12 hex characters>
```

The twelve hexadecimal characters are the leading digits of a SHA-256 (secure hash algorithm 256-bit)
digest over a canonical occasion key, which is exactly one of two things and never a mixture:

1. **Provider-anchored**, when the occasion came from a calendar: the provider name, the calendar
   identifier and the provider's own event identifier. Occurrence identifiers are used where the provider
   gives one, so a single occurrence of a recurring series is its own occasion rather than the series.
1. **Manual**, when there is no calendar: the start time in RFC 3339 form with its offset, plus the title
   normalized the way the previous design normalizes a slug.

Two consequences that are the whole reason for the rule. A brief re-run for the same calendar event
updates the existing file, because the provider identifier is stable even when the title changes. And a
brief typed by hand for "staff meeting at 15:00 on Tuesday" is reproducible from the two things the
operator typed, so the second run finds the first.

The occasion record lives beside the recording records, in vpp's own state, and it is authoritative in
exactly the way the previous design made the recording record authoritative:

```
~/.local/state/vpp/occasions/<occasion-id>.json
```

### Selection: which notes are relevant, defined so it can be tested

"Relevant" is the word in the ledger and it is the word most likely to become a fuzzy search bar. It gets
the same treatment "deterministic" got in the previous design: a definition with properties, each
testable.

A note is selected when it matches at least one **selector**, and there are exactly four. Each records
which selector chose it and with what value, so the brief can say why every note is in it.

| Selector     | Matches when                                                                     | Input it needs                    |
| ------------ | -------------------------------------------------------------------------------- | --------------------------------- |
| `participant`| a confirmed known term for a participant matches a note name or alias exactly     | context, or `--participant`       |
| `term`       | a confirmed known term from the occasion title matches a note name or alias       | the title alone                   |
| `tag`        | the note carries a confirmed tag that the occasion also carries                   | the occasion's confirmed tags     |
| `recent`     | the note's recording was captured inside `lookback_days` and shares a `continues` chain or a confirmed tag with an already-selected note | nothing |

Four properties hold, and each is one test:

1. **Total and bounded.** Selection returns at most `max_notes`, default 12, in a defined order: selector
   rank first (`participant`, then `term`, then `tag`, then `recent`), then capture time, newest first.
   Beyond the cap the remainder is counted and reported, never silently dropped.
1. **Pure.** Selection is a function of the occasion record, the configured window and the output index.
   Not of the clock beyond the window boundary, not of the destination directory's contents, not of any
   network call made during the run.
1. **Exact, never fuzzy.** Every string match is a whole-token, case-insensitive comparison against a
   term the operator has already confirmed in `known-terms.txt`. This is the transcription design's own
   rule and its reason carries over exactly: every measured engine error was a personal name, so a fuzzy
   matcher would pull the wrong person's notes into a brief about them with total confidence.
1. **Explainable.** `vpp brief --explain` prints each candidate note, the selector that chose it, the
   value that matched, and the ones that were considered and rejected, and writes nothing.

**A participant with no confirmed term selects nothing, and the brief says so.** Given the measurement
above, where an email join would resolve about 8 notes, this will be the common case at first. The brief
carries a visible line naming each participant that matched nothing, which is both honest and the thing
that prompts the operator to confirm the term once, after which it works forever. That is the same
confirm-once mechanism the transcription and tagging designs already use, and it is deliberately the only
way the term list grows.

### What a brief contains

The Markdown, in the portable profile, which is what ships and what every test runs against. The
`vpp:brief` markers follow the previous design's managed-block rule exactly: vpp rewrites only between
them, refuses a file whose markers are missing, doubled or unbalanced, and never touches a human's prose
above or below.

```markdown
---
vppSchema: 1
vppKind: brief
vppOccasion: 2026-09-16T1500-7c41aa02de19
vppOccasionAt: 2026-09-16T15:00:00-06:00
vppSource: manual
vppNotes: 4
vppUnresolved: 6
vppContext: none
---

# 2026-09-16-staff-meeting-7c41aa02

<!-- vpp:brief start -->

## Occasion

- When: 2026-09-16 15:00, 60 minutes
- Participants: 2 named, 1 matched to a note, 1 unmatched
- Source: stated by hand. No calendar was read.

## Unresolved

6 spans across 2 of the 4 selected notes are flagged and unreviewed. Every line below that rests on one
is marked `[unverified]` in place.

- `2026-08-24T144736-4f3ab19c02de`, 4 flags, review: transcripts/2026-08-24-invoice-call-4f3ab19c.md
- `2026-08-19T174030-6b1c0ddc4410`, 2 flags, review: transcripts/2026-08-19-prep-6b1c0ddc.md

## From your notes

### 2026-08-24-invoice-call-4f3ab19c  (selector: term "invoice")

- [04:12] "...the quarterly invoice has not cleared yet..."
- [11:38] `[unverified]` "...Rajesh said the 23rd of October..."  (flag: date, engines disagreed)

### 2026-08-19-prep-6b1c0ddc  (selector: recent, shares tag "invoice")

- [02:07] "...bring the last three statements..."

## Open tasks

None. No task context was supplied.

## Not selected

- Participant "Dana" matched no note name or alias. Confirm the term with `vpp confirm --term Dana`.

<!-- vpp:brief end -->
```

Five rules about that document, and the second is the one that matters most:

1. **Every quoted line carries its recording identity and its timecode**, using the transcription
   design's `[mm:ss]` convention, so a claim can be played back or re-read at its source. That is "source
   references stay visible", discharged literally.
1. **Uncertainty is marked in place, not in a footer.** A `[unverified]` marker sits on the line itself,
   inside the sentence a reader or a consumer would quote. A footer survives nothing: a consumer that
   lifts one bullet out of a brief drops a footer and keeps the sentence, and the whole hazard named in
   the Forzare bullet is exactly that lift. The `Unresolved` section exists as well, for counts, but the
   per-line marker is what is load-bearing.
1. **The brief never states anything that is not in a note.** No synthesis, no inference, no "it seems
   that". If a section has no content it says so, as the `Open tasks` section does above.
1. **`Not selected` is a section, not a silence.** A brief that quietly omitted an unmatched participant
   would be indistinguishable from a brief about someone vpp has nothing on.
1. **The Obsidian profile adds the vault's eight documented keys** in the documented order, exactly as
   the previous design defines, with `status: needs-correction` whenever `vppUnresolved` is above zero.
   The brief is a note like any other note and it obeys V1 through V7.

### The machine form, and Bob as a recorded consumer

`vpp brief --json` prints the same brief as one document. This is the consumer contract, and recording
Bob as a consumer means writing it down here and holding vpp to it, not adding a Bob-shaped feature.

```json
{
  "schema": "vpp.brief/1",
  "occasion": {
    "id": "2026-09-16T1500-7c41aa02de19",
    "at": "2026-09-16T15:00:00-06:00",
    "duration_secs": 3600,
    "title": "staff meeting",
    "source": "manual",
    "participants": [
      { "name": "Rajesh Muthukrishnan", "matched_note": "areas/contacts/…", "term": "confirmed" },
      { "name": "Dana", "matched_note": null, "term": "unconfirmed" }
    ]
  },
  "unresolved": { "flags": 6, "notes_with_flags": 2, "reviewed": false },
  "items": [
    {
      "text": "the quarterly invoice has not cleared yet",
      "certainty": "confirmed",
      "sources": [
        { "recording": "2026-08-24T144736-4f3ab19c02de", "at_secs": 252,
          "note": "transcripts/2026-08-24-invoice-call-4f3ab19c.md" }
      ],
      "selector": { "kind": "term", "value": "invoice" }
    },
    {
      "text": "Rajesh said the 23rd of October",
      "certainty": "unverified",
      "flags": [ { "class": "date", "reason": "engines disagreed", "alternative": "23 October" } ],
      "sources": [
        { "recording": "2026-08-24T144736-4f3ab19c02de", "at_secs": 698,
          "note": "transcripts/2026-08-24-invoice-call-4f3ab19c.md" }
      ],
      "selector": { "kind": "term", "value": "invoice" }
    }
  ],
  "not_selected": [ { "participant": "Dana", "reason": "no confirmed term" } ],
  "context": { "calendar": "absent", "tasks": "absent" },
  "generated_at": "2026-09-14T02:10:00-06:00"
}
```

Four rules, and they are the whole of the "Bob must retain uncertainty and provenance" requirement made
mechanical:

1. **Every item carries `certainty` and `sources`, always, with no default.** vpp refuses to emit an item
   without both. A consumer cannot receive a clean-looking item that happens to be uncertain, because
   there is no such shape; the worst it can do is discard a field, which is visible in its own code
   rather than in vpp's output.
1. **`certainty` has exactly two values**, `confirmed` and `unverified`, and `unverified` means one or
   more flags from the transcription design's review record touch the span. Not "low confidence", not a
   score. A consumer that has to decide what 0.62 means will decide wrongly.
1. **The `text` of an item is a verbatim span from a transcript**, never a paraphrase, which is what
   makes `sources` checkable rather than decorative. `vpp verify-note` can be run against any consumer's
   output to test whether its sentences still trace back.
1. **Bob's obligation, stated so it can be tested on Bob's side:** an item whose `certainty` is
   `unverified` may not become a Todoist task, a calendar entry or any other commitment without the
   operator confirming it, and any surface that shows an item shows its certainty. vpp cannot enforce
   this; it can only make the violation obvious. The corresponding Forzare ledger bullet is where the
   enforcement belongs, and it already says so.

The reciprocal entry in this ledger's Forzare section ("Let Bob consume vpp's transcripts, metadata and
briefs for meeting preparation") is the other half, and nothing about it changes: Bob reads
`vpp brief --json`, `vpp path`, and the notes themselves. There is no daemon, no socket, no registration
and no push. A consumer runs a command and gets a document, which is the same integration shape the whole
chain has used.

### The context document, and the two collectors

`vpp.context/1` is the one shape both paths produce. It is small on purpose, and it carries only the
three things the ledger names: meetings with times and participants, and tasks with deadlines and
completion status.

```json
{
  "schema": "vpp.context/1",
  "produced_by": "bob",
  "produced_at": "2026-09-16T13:00:00-06:00",
  "occasions": [
    { "source": "google-calendar", "calendar": "<calendar id>", "event": "<event id>",
      "occurrence": "<occurrence id or null>",
      "title": "…", "at": "2026-09-16T15:00:00-06:00", "duration_secs": 3600,
      "participants": [ { "name": "…", "email": "…" } ] }
  ],
  "tasks": [
    { "source": "todoist", "project": "<project name>", "id": "…", "content": "…",
      "due": "2026-09-17", "completed": false, "priority": 1 }
  ]
}
```

Two collectors ship inside vpp, each one thin adapter file, and **both are disabled by default**:

```
calendar:  gog calendar events list --cal <each configured calendar> --from <t0> --to <t1>
           --json --readonly --wrap-untrusted --no-input
tasks:     td task list --project <each configured project> --json
```

Six rules govern them, and the first is the one that discharges "limit sources to user-selected
calendars/projects" given that no provider can:

1. **An empty selection list is a refusal, never "everything".** `calendars = []` and `projects = []`
   both make `--collect` exit non-zero naming the empty key. Since the token cannot scope the read, the
   configuration is the scope, and a configuration that can be read as "all" by accident is the one bug
   that turns a bounded read into an unbounded one. This follows the repository's own rule that an
   unknown argument is an error rather than a silent fallthrough.
1. **The selection is in the request, not in a filter afterwards.** Each configured calendar is named
   with `--cal` and each configured project with `--project`, so an unselected calendar's events are
   never fetched, never in memory and never in a log. A post-fetch filter would still have read them.
1. **Read-only is asserted twice.** `--readonly` on every `gog` invocation, and the credential itself
   authorized read-only. Either alone is a single point of failure; a runtime flag can be dropped by an
   edit and a scope cannot be checked from inside vpp.
1. **A collector never writes.** No `gog calendar events create`, no `td task add`, no completion, no
   acknowledgement. `--enable-commands-exact` is available on `gog` to make that structural rather than
   a matter of which argument vector vpp builds, and the design uses it.
1. **A collector failure degrades the brief; it never fails it.** A missing binary, an expired token, a
   network error or a non-zero exit leaves the corresponding context absent, the brief is still written,
   and the reason appears in the `Occasion` section and in `context` in the machine form. A brief built
   from notes alone is the shipped posture, so a brief built from notes alone cannot be an error.
1. **Fetched text is untrusted.** See the security section; `--wrap-untrusted` exists for this and is
   used.

### Filing, names and the fourth stage

Nothing new. A brief is a fourth stage in the previous design's rule table, which routes on `stage`:

```toml
[[output.rules]]
when = { stage = "brief" }
dir = "briefs"
```

`vpp path --occasion <id> --stage brief` answers where it goes, exactly as `vpp path --stage analysis`
does for a recording, so the same single implementation of the rules serves the brief and there is no
second copy to drift. The filing allowlist gains exactly one field, the occasion's `at`, and gains
nothing else: not participants, not the context, not the selected notes. The name template is unchanged,
`{date}-{slug}-{hash8}`, with the slug taken from the occasion title through the previous design's
sanitizer and its refusal rules, which already dispose of a colon (V5), of path separators, of `..` and
of a resolved path that escapes the root or traverses a symbolic link.

Under V6 a `briefs/` directory needs a folder note or nothing in it surfaces in a listing. That is a
fourth entry in the operator step the previous design already raised, not a new kind of work, and vpp
still writes no folder notes.

### Configuration

Extending the file the earlier designs define, and following the repository's ruling that defaulted keys
ship uncommented at their default so the shipped file shows the real posture:

```toml
[brief]
# Requested only. Nothing in vpp schedules a brief; `vpp brief` is typed, or
# called by launchd or by an assistant. Measured on this machine: not one
# non-recurring forward calendar item is timed, so a scheduler would find nothing.
trigger = "requested"
# How far back selection reaches for notes.
lookback_days = 180
# The cap on selected notes. The remainder is counted and reported, never dropped
# silently.
max_notes = 12
# Verbatim spans quoted per selected note.
max_spans_per_note = 5

[brief.context]
# "none" builds the brief from notes alone, which is the shipped posture and the
# one every test runs against. "stdin" accepts a vpp.context/1 document from
# whoever asks. "collect" runs the local collectors below.
source = "none"

[brief.context.calendar]
enabled = false
provider = "gog"
account = ""
# Required when enabled. An empty list is a refusal, not "every calendar": no
# provider offers a per-calendar scope, so this list is the only scope there is.
calendars = []
lookahead_minutes = 120

[brief.context.tasks]
enabled = false
provider = "td"
# Required when enabled. Same rule: Todoist's data:read scope is account-wide,
# so this list is the only scope there is.
projects = []
# An optional extra Todoist filter query, applied in the request.
filter = ""
```

The machine's rendered file under chezmoi carries only what differs. If the operator turns the collectors
on, the Google credential is already covered by `~/.config/gogcli/credentials.json`, which is one of the
fifteen KeePassXC-backed targets; a separate Todoist token, if one is needed, would be a sixteenth and is
an open question rather than a decision here.

### The command surface

Two new verbs, and one existing one gains a flag. Everything else is reused.

```
vpp brief <occasion> [--title T --at TS] [--context - | --collect] [--json] [--dry-run]
vpp brief --explain <occasion>          print each candidate, its selector and why. Writes nothing.
vpp occasions [--json]                  list recorded occasions. Writes nothing.
vpp path --occasion <id> --stage brief  where the brief goes. Writes nothing. Already exists.
vpp verify-note <path>                  check a generated brief's claims. Already exists.
vpp confirm --term <T>                  confirm a participant's term. Already exists.
```

### Failure modes

| Condition                                                       | What vpp does                                     | Notification |
| --------------------------------------------------------------- | ------------------------------------------------- | ------------ |
| `--context -` and `--collect` both given                        | refuse, naming both                                | none, exit 2 |
| Context document fails schema validation                        | refuse, naming the first failing field and its path | none, exit 2 |
| A collector is enabled with an empty `calendars` or `projects`  | refuse at startup, naming the empty key            | page once    |
| A collector binary is missing, or exits non-zero                | context absent, brief still written, reason recorded | none, log   |
| A collector returns an occasion whose time is outside the window | drop it, count it                                  | none, log    |
| An occasion identity resolves to two recorded occasions         | refuse, naming both                                | page         |
| Selection matches no note at all                                | write the brief with an empty `From your notes` section and a `Not selected` line per participant | none |
| A participant has no confirmed term                             | record it in `not_selected`, print the confirm command | none       |
| A selected note's review record is missing                      | treat every span from it as `unverified`, say so   | none, log    |
| `max_notes` exceeded                                            | keep the ordered prefix, report the remainder count | none, log   |
| Managed brief markers missing, doubled or unbalanced            | refuse to touch the file, name the path            | page         |
| Target path exists carrying a different `vppOccasion`           | refuse, name both identities                       | page         |
| `vppSchema` higher than this build understands                  | read, never rewrite, report                        | page once    |

The split is the chain's: a configuration mistake or an ambiguity that could destroy work refuses loudly,
a routine absence is a log line. **A requested brief sends no pns notification at all**, because the
person who asked for it is reading the output. If scheduled briefs are ever built, their notification is
the transcription design's shape without exception: one `pns submit --json` carrying the occasion
identity, counts and paths and no brief text, on a route the hermes gateway actually declares (today
`priority`, `pns` and `unattended-upgrades`; naming a route the gateway does not declare is how posture
is currently dead-lettering its Discord legs).

### Security and privacy

**A brief is a concentration, and that is what makes it the most sensitive artifact in the chain.** A
transcript is one recording. A brief gathers several recordings, the names of the people who will be in
the room, and possibly their task and deadline context, into one document. If it is written into the
vault it is auto-committed and pushed within roughly fifteen minutes by V7's timer, which means
participant names reach GitHub without anybody choosing that in the moment. The transcription design
already asked whether transcripts may be committed at all; a brief raises the same question at higher
stakes and it is an open question below rather than a decision here.

**Fetched context text is untrusted input, and a brief is read by an agent.** A calendar event title and
description are written by whoever sent the invitation, and a task in a shared project is written by
whoever shares it. Bob reads briefs. That is the classic shape of an injection: text an outsider
controls, placed into a document a model will read as context. Four defences, all cheap:

1. `gog --wrap-untrusted` wraps fetched text fields in the tool's own external-untrusted-content markers.
   It exists for exactly this and costs one flag.
1. **vpp never sends context text to a model**, because vpp never calls a model. Whatever reads the brief
   made its own decision to do so, which keeps the boundary visible rather than buried in a subcommand.
1. **Fetched text is rendered as quoted data with its source named in place**, so a reader and a consumer
   can both tell an operator-authored line from a line that arrived from a calendar.
1. **A fetched string never becomes a path.** A provider-supplied title goes through the previous
   design's slug sanitizer and its refusals, which reject a resolved path that escapes the output root or
   traverses a symbolic link, rather than sanitizing it into something that looks safe.

**vpp holds no credential.** It spawns `gog` and `td`, each of which owns its own credential store
(`gog`'s keyring directory under `~/.config/gogcli/`, `td`'s system credential store). vpp therefore adds
nothing to the fifteen KeePassXC-backed targets, and an agent that can run vpp has exactly the access
those two tools already grant it, which is a boundary the operator can inspect without reading vpp's
code. If the Todoist single-slot problem forces a separate token, that changes and it becomes a sixteenth
KeePassXC target, which is one of the open questions.

**Writes are refused structurally, not by convention.** Read-only credential, `--readonly` at every call,
and `--enable-commands-exact` restricting the spawned binary to the one command vpp needs.

**Nothing about a brief rides a pns request.** Not a title, not a participant, not a quoted span, not a
task. The transcription design set that rule for flagged text and it holds here with more force, since a
brief's subject line alone says who the operator is meeting.

**A brief is not a shareable draft.** The redacted-draft bullet is a separate ledger item with its own
human review gate, and a brief moves in the opposite direction: it accumulates rather than removes. No
path in this design produces something intended to leave the machine.

**Records are mode 0600 in a 0700 directory**, as the earlier designs set, and the brief note inherits
the vault's protections.

### The behaviors to drive the implementation, test-first

Each is one failing test before one piece of code, in this repository's style where a unit is a behavior
rather than a task. None needs a calendar, a Todoist account, a network, an agent or a recording: each
takes a record structure, a context fixture and a temporary directory.

1. The same occasion described twice by hand, with the same title and start time, produces the same
   occasion identity and rewrites one file rather than creating a second.
1. Two occasions at the same time with different titles produce different identities and different paths.
1. A provider-anchored occasion keeps its identity when its title changes, and a per-occurrence
   identifier makes two occurrences of one recurring series two occasions.
1. `--context -` and `--collect` together is an error with a message naming both, and no file is written.
1. A context document with an unknown top-level key is accepted and the key is reported; a document
   missing `schema` is refused naming the field.
1. A collector enabled with an empty `calendars` list refuses at startup, naming the key, and no request
   is made.
1. A collector enabled with two configured calendars issues a request naming exactly those two, and a
   third calendar's event present in a fixture response is not in the brief and not in any log line.
1. A collector whose binary is absent leaves context absent, writes the brief, and records the reason in
   both the Markdown and the machine form.
1. Selection is a pure function: two runs over one record set, one index and one window produce the same
   ordered selection and neither writes a file.
1. A participant whose term is confirmed selects the matching note; a near-miss spelling selects nothing
   and appears in `not_selected` with the confirm command.
1. A confirmed known term matching two notes selects neither and records the ambiguity, matching the
   previous design's `mentions` rule.
1. A suggested tag never selects a note; only a confirmed tag does.
1. More than `max_notes` candidates keeps the ordered prefix and reports the remainder count, and the
   order is selector rank then capture time, newest first.
1. Every item emitted by `--json` carries both `certainty` and `sources`, and an item constructed without
   either fails to serialize rather than defaulting.
1. A span touched by any flag in the review record renders `certainty: "unverified"` in the machine form
   **and** an in-line `[unverified]` marker in the Markdown, and removing the flag flips both.
1. A selected note whose review record is missing renders every one of its spans as `unverified`, and the
   brief says why.
1. Quoted item text is a verbatim span from the transcript: the assertion is that the item's text occurs
   in the transcript at the recorded offset.
1. `--explain` prints the selector and matched value for every candidate, including rejected ones, and
   writes no file.
1. Rewriting a brief preserves every byte outside the managed markers, including prose a human added, and
   a file whose markers are missing, doubled or unbalanced is refused rather than repaired.
1. A brief whose target path exists carrying a different `vppOccasion` is refused, naming both.
1. A provider-supplied title containing `../`, a colon and a slash produces a slug with none of them and
   a path inside the output root.
1. In the portable profile a brief's frontmatter contains none of the vault's eight keys, and a full run
   against an output root with no `.obsidian` directory produces a brief with no vault-specific syntax.
1. With `vppUnresolved` above zero the Obsidian profile renders `status: needs-correction`, and with none
   it renders `active`.
1. A brief with no calendar and no task context is written successfully, with `context: none` and the two
   empty sections stating their absence.

Behaviors 1 through 3 pin the identity, 6 and 7 pin the source limiting that no provider can enforce, 14
through 17 pin the uncertainty requirement that the Forzare bullet turns on, and 24 pins the
"context is optional" requirement which is the one a later change is most likely to erode.

## Out of scope

Deliberately not designed here, and not to be smuggled in during implementation.

- **Writing the brief's prose.** vpp assembles and cites. A generated summary is a proposal from
  something else and goes through `vpp verify-note`.
- **The summary format.** The ledger's last bullet lists it as undecided, and under this design it
  belongs to whatever generates prose, not to vpp.
- **Scheduled briefs, lead times and a LaunchAgent.** Named as the seam, left unbuilt, on the
  measurement.
- **Writing to a calendar or to Todoist.** Every path here is read-only. Bob's calendar writes are
  Forzare's design and confined to its own robot calendar.
- **Forzare's morning brief.** A different artifact with a different owner. See the naming question
  below.
- **The redacted draft for sharing.** A separate ledger bullet with a human review gate.
- **Speaker labels and dated digests.** Listed in the ledger as unapproved candidates.
- **Any change to the discovery, transcription or filing designs.** This stage consumes their records and
  adds one stage to their rule table.
- **The `minutes` disposition.** Its `research` and `person` commands were measured and are named as a
  possible context source, and nothing here depends on the ruling.
- **A per-participant profile, a relationship graph or a search index.** `minutes people` and
  `minutes person` already occupy that space and vpp has no reason to compete for it.
- **Email, Slack or any other context source.** The ledger names two, both optional. A third would need a
  reason before it needed a collector.

## Assumptions made in the operator's place

Each of these was a choice this document had to make to be written at all. None is a decision, each names
its alternative, and reversing any of them changes a configuration value or one section rather than
invalidating the measurements.

1. **A brief is assembled and cited, and vpp writes no sentences of its own.** Alternative: vpp spawns an
   agent and files the prose, for which this repository has precedent in the `prepare-commit-msg` hook.
   Taken because it keeps the egress decision at the caller, keeps vpp installable and useful with no
   agent present, and is the only shape where a generated sentence is checked against the transcript
   before it counts.
1. **Briefs are requested, not scheduled.** Alternative: a LaunchAgent with a lead time, which is what
   "automatic briefs" would mean. Taken on the measurement: not one non-recurring forward calendar item
   on this machine is timed and participant-bearing items ran at 8 in the last 90 days, so a scheduler
   would wake daily and find nothing. Reversing this is a plist and a loader, and the command itself
   does not change.
1. **Calendar and Todoist context arrives through one validated document that either vpp's own collectors
   or Bob can produce.** Alternative: pick one now. Taken because the operator's open question is exactly
   which, and one input contract makes both answers work without a second implementation to drift.
1. **The collectors ship disabled.** Alternative: enable them once credentials exist, since this operator
   is the only user. Taken because the tested default should be the one the requirement names as
   optional, and because a disabled collector cannot read a calendar nobody meant to select.
1. **Google Calendar through `gog` is the calendar provider, not Apple EventKit.** Alternative: EventKit,
   which reads the Mac's own aggregated store including non-Google accounts. Rejected on two verified
   facts: EventKit has no read-only access level for events, so "read-only calendar" is unachievable
   there, and a local-file read would have to reimplement recurrence expansion that the Google API
   already performs. The cost is real and is named: a calendar that is only in Calendar.app and not in
   Google is invisible to this design.
1. **Todoist through `td`.** Alternative: vpp holds its own token and calls the interface directly.
   Taken because `td` is installed, is on the managed fnm lane, and already has `--read-only`. The
   unresolved part is whether a read-only credential can coexist with the operator's read-write one,
   which is an open question, and if it cannot then the alternative becomes the answer.
1. **An empty selection list is a refusal rather than "everything".** Alternative: treat empty as all
   calendars or all projects, which is what most tools do. Taken because no provider offers a
   per-calendar or per-project scope, so the configuration is the only scope that exists, and a default
   that means "everything" is one typo away from an unbounded read.
1. **Selection is four exact selectors over confirmed metadata, capped at 12 notes.** Alternative: a
   full-text or embedding search, which would find more. Rejected on the transcription design's
   measurement, where every engine error was a personal name: a fuzzy matcher pulls the wrong person's
   notes into a brief about them with total confidence, and a brief is precisely where that error becomes
   a spoken claim.
1. **Uncertainty is marked in place, inside the line, as well as counted in a section.** Alternative: a
   single `Unresolved` section, which is tidier. Rejected because a consumer that quotes one bullet drops
   a section and keeps the sentence, and that lift is the exact failure the Forzare bullet forbids.
1. **`certainty` has two values, not a score.** Alternative: carry the engine confidence through.
   Rejected because the transcription design measured confidence to be a weak signal, and because a
   consumer handed a number will invent a threshold for it.
1. **The occasion gets its own identity and its own record, parallel to a recording's.** Alternative:
   hang the brief off the first selected recording. Rejected because re-running the brief would then move
   it whenever selection changed, and because an occasion with no matching notes could not be recorded at
   all.
1. **A brief is a fourth stage in the existing filing rule table, filed flat in `briefs/`.** Alternative:
   a folder per occasion holding the brief and its sources, which under V6 costs a folder note per
   occasion. Taken for the same reason the previous design filed flat.
1. **A requested brief notifies nothing.** Alternative: one pns notification per brief, for consistency
   with the transcription stage. Rejected because the operator typed the command and is reading the
   output; a notification about work they just asked for is noise.
1. **Nothing was written, fetched or authorized while measuring.** No calendar or task was created or
   modified, no `gog auth add` was run, no note was added to the vault, and the calendar store was read
   from a throwaway immutable copy that was deleted. The vault auto-commits and pushes within minutes, so
   a probe note would have landed in the operator's git history while they slept.

## Operator steps

Five. The first three are the three answers this design is waiting on, and they are deliberately separate
approvals, as the ledger asks.

**1. Decide who holds the calendar and Todoist credentials: vpp, or Bob.** This is the ledger's own
question. Either answer works without changing vpp, and the design's default (`source = "none"`) is the
answer to "neither, yet". If it is vpp, steps 4 and 5 follow. If it is Bob, this bullet's calendar and
Todoist half waits for Forzare and vpp ships useful without it, which is what L-R5 requires.

**2. Decide whether briefs are requested or scheduled, and say whether meetings are going to start
appearing on a calendar.** The measurement says a scheduler would find nothing today, but the measurement
is of the past. If the intent is to move meetings onto a calendar, a scheduler becomes worth building and
the lead time becomes a real question rather than a hypothetical one.

**3. Name the calendars and the Todoist projects a brief may read.** No provider can enforce this, so it
is a configuration value and it is the only scope that exists. An empty list refuses by design, so this
answer is required before the collectors can be turned on at all.

**4. If vpp is to read the calendar, authorize a read-only Google credential and confirm it does not
disturb the existing one.** The command is of the form
`gog auth add <email> --services=calendar --readonly`, and `gog auth services` shows the calendar
service's default scope is the full `https://www.googleapis.com/auth/calendar`, so this is a distinct
authorization rather than a narrowing of what is there. What needs confirming before relying on it is
whether `--client` lets a second read-only token bucket sit beside the existing credential for the same
account: `gog auth list` did not return inside a 20 second timeout in this session, so it was not
verified.

**5. If vpp is to read Todoist, settle the single-slot problem.** `td accounts list` shows one stored
account keyed by its numeric id and `td auth status` reports it as read-write. Running
`td auth login --read-only` would re-authorize that same account and most likely replace the operator's
own read-write credential, which would break their daily `td` usage. Three ways out, in increasing cost:
a second Todoist account that shares the relevant projects; a dedicated read-only token held in KeePassXC
and passed to the interface directly, making it a sixteenth secret-bearing target; or Bob supplying the
task context, which is step 1's other answer.

**Also, when the vault layout is adopted:** `briefs/` needs a folder note carrying the reference
DataviewJS query, making four in total alongside the three the previous design named. Small, once, and
vpp still writes no folder notes.

## Open questions for the operator

**Triage, 2026-09-15:** every question below is closed except where noted. See
`docs/decisions/2026-09-15-vpp-question-triage.md` (rows R1-R7) for the reasoning.

1. **Does vpp read the calendar and Todoist, or does Bob supply them?** The ledger's own question. The
   design makes both work through one input document, so this sets a configuration value, but it decides
   who holds two credentials and therefore what an agent with shell access can reach.
1. **Requested or scheduled, and at what lead time?** The recommendation is requested only, on the
   measurement that this machine's forward calendar holds no timed non-recurring events at all. The
   question behind it is whether that is the intended future or the current gap.
1. **Which calendars and which Todoist projects?** Required before any collector can run, because an
   empty list refuses. And the sharper version: given that a read-only token reads every calendar and
   every project regardless, is a client-enforced selection an acceptable boundary, or does that make the
   whole context feature not worth its access?
1. **Can a read-only Todoist credential coexist with the operator's read-write one?** If not, which of
   the three ways out in operator step 5 is acceptable. This is the one place where the design might have
   to add a sixteenth KeePassXC-backed target.
1. **May a brief be written into the vault, given that it is auto-committed and pushed within minutes?**
   A brief names the people who will be in a room, which is a different exposure from a transcript naming
   whoever happened to be mentioned. The transcription design's version of this question was about
   transcripts; this is the same question with participants in it, and it is one reason the answer might
   be that briefs live outside the vault even when transcripts live inside.
1. **Is "brief" the right word, given that Forzare already has a morning brief?** Bob's morning brief is
   a day plan delivered on a schedule; this is a per-occasion evidence pack produced on request. Two
   things called a brief, one of which Bob composes and the other of which Bob consumes, is a name
   collision waiting to confuse a future session. `vpp prep` or `vpp dossier` would avoid it at the cost
   of not matching the ledger's own word.
1. **If `minutes` stays, should its `research` and `person` output be a context source for a brief?** It
   is the one installed tool that already ranks material about a person or a topic, over its own corpus.
   Feeding it in is one more collector; leaving it out keeps vpp's brief entirely its own. This question
   is downstream of the `minutes` keep-or-replace ruling and does not need answering before it. **Moot,
   decided 2026-09-15:** `minutes` is out entirely, so there is no `research` or `person` output to draw
   from. See `docs/decisions/2026-09-15-vpp-architecture-decisions.md`, decision 1.
1. **Is the local Apple Calendar store representative of the operator's calendars?** The measurement came
   from that store, 16 calendars, and the recommendation to use the Google interface assumes the meetings
   that matter are in Google. A calendar that exists only in Calendar.app would be invisible to this
   design, and that is a consequence worth confirming rather than discovering later.
1. **Where does vpp's code live, and what is it called?** Carried forward unresolved from the boundaries
   design, because the chain should not stay in disagreement with itself.
