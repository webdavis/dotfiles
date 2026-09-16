# vpp recording discovery

Status: design, written 2026-09-14 while the operator was asleep. Not approved, not built. Every choice
made in the operator's place is listed under "Assumptions made in the operator's place", and the open
questions are at the end.

Scope: `docs/remaining-work.md`, the `vpp (Voice Processing Pipeline)` section, second bullet. "Design
automatic discovery of fully synced recordings, preserving original audio and capture metadata without
modifying Apple's source recordings. Verify the supported macOS access/export path and actual audio
format before choosing an ingestion mechanism. Handle interrupted sync, retries and repeated discovery
without duplicate notes or lost audio. Keep original recordings, transcripts and agent analysis
separately linked using the existing vault layout."

Nothing was installed, configured, applied or transcribed. Three transient launchd jobs were submitted
and removed as part of the permission measurement below; they only read files, slept, and wrote to the
session scratchpad. The Apple Voice Memos store was opened read-only and never written to.

## Why this can be designed now, when the previous document said to defer

The predecessor in this chain, the source reconciliation of 2026-09-14, ended by deferring the vpp
ingestion design. Its reason was scope: the installed `minutes` 0.26.1 cask already covers six of the
seven vpp feature bullets, the operator has not ruled on whether `minutes` is kept or replaced, and the
two candidate vpp scopes differ by roughly an order of magnitude.

That reasoning holds, and this document does not reopen it. It designs the one piece the reconciliation
named as absent from all four sources under **both** candidate scopes:

> **Apple Voice Memos discovery**: finding fully synced recordings in the group container, reading their
> titles and capture metadata out of the Core Data store, and handling interrupted sync, retries and
> repeated discovery without duplicates or loss. Absent from all four sources.

Discovery is scope-independent because its output is an ingested recording, not a note. Whether the audio
is then handed to vpp's own engines, to `minutes transcribe --json`, or to nothing at all is a decision
downstream of the boundary this design draws. The `minutes` ruling changes the consumer, never the
producer. That is the framing that makes this task safe to do before the ruling lands, and it is the one
constraint the design is built to satisfy.

If the operator disagrees and wants the whole vpp ingestion path settled at once, this document is still
the input for its first stage rather than wasted work.

## Constraints this design is bound by

From the ledger bullet and its section intro:

1. vpp is written in Rust.
1. Original audio format is preserved.
1. Apple's source recordings are never modified.
1. Interrupted sync, retries and repeated discovery produce neither duplicate notes nor lost audio.
1. Originals, transcripts and agent analysis are separately linked using the existing vault layout.
1. vpp application code lives in its own project; Mac installation and service configuration live in
   dotfiles; output content lives in the configured directory, which is the Ivy vault for this operator.
1. vpp works without Bob, Forzare or the full homelab.

From the homelab backlog entry `PLAN-v12-experiments-backlog.md` L-R5, which is the fuller statement of
the same feature set: notification goes through `pns submit --json` with `producer: "vpp"` and
`signal.kind: "needs_attention"`, review state is kept in vpp, and the evaluation aid is explicitly "not
permission to delete recordings or notes".

From the repository's own standing rules, in `CLAUDE.md` and the operator's global rules, labelled R1 to
R5 so the numbering above stays readable:

- **R1.** No workspace may depend on another. vpp is a fifth independent cargo workspace, and it must
  still build with pns, uu, posture and lights absent from the filesystem. Runtime integration by
  spawning a deployed binary stays allowed, which is how posture already reaches pns.
- **R2.** Rust files target 300 lines and never exceed 500, unit tests included.
- **R3.** Never patch, fork or modify a third-party tool. Apple's Voice Memos is the third-party tool
  here, and the rule is why this design reads its store and never writes to it.
- **R4.** Tests cover the behavior of tools we wrote and nothing else. A test that asserts a plist field
  or a declaration is out of scope and gets deleted on sight.
- **R5.** The operator runs applies. An agent proposes.

## What was measured, and how

Everything below was measured on dresden on 2026-09-13 and 2026-09-14. Where a statement is inference
rather than measurement, the sentence says so. Nothing rests on training data.

| Question                    | Command                                                           | Result                                                                                                          |
| --------------------------- | ----------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| audio codec                 | `ffprobe` on one recording                                        | `alac`, 48000 Hz, 2 channels, `.m4a` container                                                                  |
| store size                  | `du -sh` on `Recordings/`                                         | 988 MB for 28 recordings, largest single file 187,903,757 bytes                                                 |
| store contents              | `ls -1a`                                                          | 28 `.m4a`, 8 `.waveform`, the database plus `-wal` and `-shm`, and five subdirectories                          |
| file-to-row mapping         | `ZPATH` join against the filesystem                               | 28 rows, 28 files, zero missing, zero orphans                                                                   |
| capture time in the file    | `ffprobe -show_entries format_tags=creation_time` against `ZDATE` | identical to the second on all five sampled                                                                     |
| container wholeness         | top-level box walk in Python                                      | `ftyp`, `mdat`, `moov`, sometimes `free`; `moov` last; box lengths sum exactly to file size on all four sampled |
| local versus cloud duration | `ZLOCALDURATION` against `ffprobe` duration                       | equal to the millisecond on all four sampled                                                                    |
| dataless stubs              | `ls -lO`                                                          | no flags set on any of the 28; `SF_DATALESS` is `0x40000000` in the SDK's `sys/stat.h`                          |
| arrival window              | `stat` birth time and mtime against `ZDATE` on six recordings     | mtime is 0 to 108 s after birth time and both are well after capture, so mtime is when the local write finished |
| clone cost                  | `/bin/cp -c` of the 187.9 MB recording into the scratchpad        | 0.00 s real, 16 KB of free space consumed                                                                       |
| volume identity             | `df` on both paths                                                | both on `/dev/disk3s5`, the one Apple File System (APFS) data volume, 237 GiB free                              |
| Full Disk Access holder     | system privacy database                                           | `com.mitchellh.ghostty` allowed for `kTCCServiceSystemPolicyAllFiles`                                           |
| launchd read                | transient `launchctl submit` job, parent `/sbin/launchd`          | every read succeeded, including the Full Disk Access canary; see the caveat below                               |
| scripting support           | app bundle inspection                                             | no scripting definition file, no `NSAppleScriptEnabled` key                                                     |
| Shortcuts support           | `Metadata.appintents` parse                                       | 14 actions, entity `RCRecordingEntity` exposing only `title`, `creationDate`, `duration`                        |
| Spotlight                   | `mdls` and `mdfind -onlyin`                                       | no content metadata, container not indexed for file queries                                                     |
| pns contract                | `pns/crates/pns-protocol/src/request.rs`                          | `pns.request` major 1, `producer` field, `NeedsAttention` signal variant                                        |

## The access and export path question, answered

The ledger asks to verify the supported macOS access and export path before choosing a mechanism. The
answer is unwelcome and it shapes everything that follows.

**There is no supported programmatic path that yields the audio.** Four candidates were checked and three
are closed:

- **AppleScript.** `/System/Applications/VoiceMemos.app` ships no scripting definition file and its
  `Info.plist` has no `NSAppleScriptEnabled` key. There is no dictionary to drive.
- **Shortcuts and App Intents.** The app does expose App Intents, through
  `VoiceMemosIntentsExtension.appex` and a `Metadata.appintents` bundle, and `/usr/bin/shortcuts` can run
  a shortcut headlessly. But the recording entity carries exactly three properties, `title`,
  `creationDate` and `duration`, and no action in the set outputs a file. `RCImportRecording`, by its
  name and its signature of a file plus a string returning a recording entity, runs the import direction;
  nothing in the set runs the export one. Both search actions also set `openAppWhenRun`, so they are not
  headless. This path yields metadata and never bytes.
- **Spotlight.** `mdls` on a recording returns filesystem attributes only, and `mdfind -onlyin` over the
  container returns nothing. The app's Spotlight extension indexes into its own app-search domain, not
  into the file index. No titles here.
- **The group container on disk.** `~/Library/Group Containers/group.com.apple.VoiceMemos.shared/`
  `Recordings/`, mode 700, owned by the operator, no access control lists. The audio is here, in its
  original form, and `CloudRecordings.db` holds the titles. Apple documents none of it.

So the real choice is between reading an undocumented store read-only and asking a human to export each
recording by hand through the share sheet. There is no third option that a background service can drive.

This is not as bad as it first reads, because of one measured fact: **the capture timestamp is inside the
audio file.** The MPEG-4 `mvhd` creation time matched the database's `ZDATE` to the second on every
sampled recording. So the private schema is needed for the human-assigned **title** and for nothing else.
A design that treats the database as an optional enrichment, and the audio file as the source of truth
for identity, timing and completeness, degrades to "correct but untitled" if Apple ever changes the
schema, instead of breaking.

### The permission caveat, stated precisely because it is the one thing still unresolved

Every read in this session ran under Ghostty, and the system privacy database records
`com.mitchellh.ghostty` as allowed for `kTCCServiceSystemPolicyAllFiles`, which is Full Disk Access. Both
privacy databases are mode 644, so the only thing keeping an ordinary process out of them is the privacy
system itself, and being able to read them is proof that the reading process had the grant.

A transient `launchctl submit` job, whose parent process was `/sbin/launchd` at process id 1, read the
Voice Memos container **and** the Full Disk Access canary successfully. That looks like good news and it
is not conclusive, for one reason: a job submitted from a Ghostty session may inherit Ghostty's
responsible-process attribution, which is exactly the attribution the privacy system resolves grants
against. The unified log shows that attribution in action for other requests, naming
`responsible={... identifier=com.mitchellh.ghostty ...}` for a process whose binary is something else
entirely. No privacy-system message is logged for plain file reads, so the log cannot settle it either,
and `launchctl procinfo`, which would print the responsibility, needs root.

The remaining distinguishing test needs a job launchd starts on its own, with no submitting session in
the picture. It is one operator step, step 1 below. The design is built so that the answer changes a
configuration value, not the architecture: if a background agent is refused, the same binary runs from a
login-shell context or on demand, and the preflight below is what tells the operator which world they are
in instead of silently ingesting nothing.

## Existing state this design builds on

**The vault layout exists and is empty.** `~/workspaces/Ivy/agent-processing-pipeline/` holds
`raw/audio/`, `transcripts/` and `analysis/`, each with only a `.gitkeep`, every mtime
`2026-07-22 01:35`. The vault `.gitignore` excludes `*.m4a`, `*.wav`, `*.mp3` and `*.flac` at lines 79 to
83\. So audio placed in the vault stays local: it never reaches the repository and never reaches
Obsidian's mobile sync, which rides on the same git history. Adopting the layout costs nothing because no
data has to be migrated.

**The `minutes` symlink in that directory is orphaned.** `agent-processing-pipeline/minutes` points at
`/Users/stephen/meetings` and is committed, but `minutes paths` names a configuration file at
`~/.config/minutes/config.toml` that does not exist and `minutes vault status` answers
`Vault: not configured`. The vault's own `CLAUDE.md` claims the link is "managed by
`minutes vault setup --subdir`", and that claim is currently false. This design does not touch it; the
repair is an operator step carried forward from the reconciliation.

**The repository's Rust conventions are settled.** Four independent cargo workspaces at the repository
root, no root manifest, command crate named for the tool, binaries installed to `~/.cargo/bin` through
`.chezmoidata/rust_tools.yaml`, and a LaunchAgent per scheduled job under `Library/LaunchAgents/` with a
matching `run_onchange_after_*` loader. The dependency set a fifth workspace would reuse is already
proven here: `rusqlite = "=0.39.0"` with `default-features = false` and the `bundled` feature,
`libc = "0.2.189"`, `sha2 = "0.11.0"`.

**launchd's scheduling behavior is documented and matters.** From `launchd.plist(5)`: `WatchPaths` is
"highly discouraged, as filesystem event monitoring is highly race-prone, and it is entirely possible for
modifications to be missed. When modifications are caught, there is no guarantee that the file will be in
a consistent state when the job is launched." `StartInterval` misses any firing that falls while the
machine is asleep. `StartCalendarInterval`, by contrast, "will start the job the next time the computer
wakes up", coalescing multiple missed intervals into one event. The `com.webdavis.uu` plist already
relies on exactly that property and says so in a comment.

## Approaches

### A. Long-running daemon with a filesystem event stream

A `vpp daemon run` process, supervised by a LaunchAgent the way `com.webdavis.pns-daemon` is, holding a
filesystem event subscription over the Recordings directory and reacting to each event.

Good: lowest latency, and state can live in memory.

Bad: a fifth supervised process on the machine, a crash-loop policy to get right, a new dependency for
the event stream, and in-memory state that has to be rebuilt on every restart anyway because the events
missed while the process was down are gone. The event stream buys latency on a workload measured in
recordings per week. It also inherits the same wholeness problem as every other trigger, because an event
fires while a download is still running.

### B. Idempotent sweep on a calendar schedule

One short-lived command, `vpp ingest`, started by launchd on a `StartCalendarInterval` array. It lists
the directory, decides what is new and whole, ingests it, and exits. No supervised process, no resident
state, and no event subscription. Correctness comes from the sweep being complete rather than from any
event being caught: whatever was missed last time is found this time.

Good: the shortest thing that works. A missed wakeup, a crash mid-sweep, a machine asleep through a sync,
and a manual `vpp ingest` from a terminal all converge to the same state. It matches the repository's
existing weekly-job idiom, and `StartCalendarInterval` fires once on wake after any number of missed
slots.

Bad: up to one interval of latency before a new recording is noticed.

### C. Sweep plus `WatchPaths` as a latency hint

B, with `WatchPaths` on the Recordings directory added to the same plist so a sync also pokes the job.

Good: near-immediate discovery in the common case, while the sweep still guarantees the uncommon ones.

Bad: `launchd.plist(5)` argues against `WatchPaths` in its own words, and every extra wakeup runs a sweep
that usually finds nothing. It is strictly an optimization over B, and it is only worth taking if the
interval latency proves annoying in use.

### Recommendation

**B, with C available as a one-key plist change.** The workload is a handful of recordings a week and the
consumer is a human reading notes later, so a fifteen-minute worst case is not a defect. Taking C on day
one would mean accepting a mechanism its own manual page discourages, to solve a latency problem that has
not been observed yet. A is rejected outright: it adds a supervised process and a dependency to buy the
same latency C buys for one plist key.

## The recommended design

### The boundary

One command that runs to completion and exits:

```
vpp ingest [--dry-run] [--once <path>]
```

It answers exactly one question, "which recordings on this Mac are whole and not yet taken, and take
them", and it emits a record per newly ingested recording. It does not transcribe, summarize, tag, or
write a note. That boundary is what keeps this design independent of the `minutes` ruling: a transcriber
is a consumer of ingested recordings, and it can be vpp's own engines, `minutes transcribe --json`, or
nothing yet.

The library beneath it splits along the same seam the other four tools use: a domain crate that knows the
rules and touches no input or output, and an adapters crate that owns the filesystem, the clone syscall,
the SQLite read and the pns spawn. The rules below are all domain-crate behaviors, which is what makes
them testable without a Voice Memos store.

### Identity, and why it prevents duplicates by construction

A recording's identity is `<local-capture-timestamp>-<hash12>`, for example
`2026-08-24T144736-4f3ab19c02de`:

- the capture timestamp comes from the MPEG-4 `mvhd` creation time in the audio file itself, converted to
  the machine's local time, formatted without colons because Obsidian forbids a colon in a filename;
- `hash12` is the first 12 hex characters of the SHA-256 (secure hash algorithm 256-bit) digest of the
  complete audio file.

Both halves come from the file and neither comes from Apple's schema, so identity survives a schema
change, a rename inside Voice Memos, and a store migration.

Duplicate prevention then needs no ledger and no bookkeeping, because the destination filename is a pure
function of the content. The same recording discovered a second time derives the same identity, finds the
clone already sitting at `raw/audio/<id>.m4a`, and stops. Two racing sweeps cannot both win, because the
clone is created with `clonefile(2)` directly, which fails with `EEXIST` when the destination exists.
That syscall is the mutual exclusion, and it needs no lock file.

One measured warning for whoever implements it: **`/bin/cp -c` does not give that guarantee.** Cloning
over an existing destination with `cp -c` returned 0 and replaced the file. The `EEXIST` behavior belongs
to the raw syscall, which `libc` exposes and which the repository already depends on.

### The sweep

1. List `Recordings/` at depth one only. Take `*.m4a` and nothing else. Never recurse: `Capture/` and
   `CaptureRecovery/` are where an in-progress local recording lives, and `CloudRecordings_ckAssets/` is,
   by inference from its name and its emptiness rather than by measurement, where an in-flight download
   asset lands. All three are empty right now and none of them holds a finished recording.
1. Skip any entry whose `st_flags` has `SF_DATALESS` (`0x40000000`) set. A dataless file is a placeholder
   for audio that lives only in iCloud, and opening it would trigger a silent multi-hundred-megabyte
   download. Record it as deferred and move on; the next sweep picks it up if the operator materializes
   it. None of the 28 current recordings carries the flag.
1. Skip anything already ingested, cheaply, before hashing: keep a per-source note of
   `(filename, size, mtime)` and skip an unchanged triple. This is an optimization and not a correctness
   mechanism; deleting the whole state directory costs one full rehash and changes no outcome.
1. Run the wholeness gate below.
1. Hash, derive the identity, clone, write the sidecar, emit the record.

### The wholeness gate, which is how interrupted sync is handled

A recording is taken only when all of these hold. They are cheap, they are ordered cheapest first, and
none of them needs Apple's schema.

1. **The container is whole.** Walk the top-level MPEG-4 boxes. The sum of their lengths must equal the
   file size exactly, and a `moov` box must be present. This was measured on four recordings and held
   exactly on all four. It works here specifically because Voice Memos writes `moov` **last**, after
   `mdat`: the observed order is `ftyp`, `mdat`, `moov`, with an optional trailing `free`. A truncated
   download therefore loses the very box the check requires. This is about forty lines of parsing and it
   needs no audio library and no `ffmpeg` on the target machine, which matters because vpp is a tool
   other people install.
1. **The file is at rest.** Its mtime is at least `quiet_period_secs` in the past, default 30 seconds.
   One `stat` call, no sleeping inside the sweep. This works because mtime on these files is the moment
   the local write finished, not the capture time: measured across six recordings, birth time runs 0 to
   108 seconds before mtime and both run minutes to days after the recording was captured, so birth is
   when the download started and mtime is when it stopped. The 108-second case was the 188 MB recording,
   which sets the scale the quiet period has to clear. For a file that was deferred on an earlier sweep,
   also require that its size is unchanged since that sweep recorded it.
1. **The cross-check, when the database is readable.** `ZLOCALDURATION` compared against the duration the
   container reports, and `ZDURATION` compared against `ZLOCALDURATION` with a tolerance of 0.25 seconds.
   `ZLOCALDURATION` was measured to equal the container's own duration to the millisecond on all four
   sampled recordings, which is strong evidence that it is derived from the local file rather than from
   the cloud record, and therefore that a large gap between it and `ZDURATION` means missing audio. This
   is stated as a cross-check and not as the primary gate because **no partially downloaded recording was
   available to observe**, so the behavior of these columns during an interrupted sync is inferred, not
   measured. A failed cross-check defers the recording and says why; it never deletes and never repairs.

A recording that fails any gate is deferred with a reason and retried on the next sweep. It is never
partially ingested, because the clone is the first durable act and it happens only after every gate has
passed.

### Reading the database without touching it

The store is SQLite in write-ahead-log mode, which is a trap for a reader. Two measured facts from the
reconciliation and this session:

- Copying `CloudRecordings.db` alone gave 27 rows, 26 with a path. Copying it together with `-wal` and
  `-shm` gave 28 rows, all with paths, matching the 28 files exactly. The checkpointed file alone was
  nine days and one recording stale, and the missing recording was the newest one, which is the only kind
  a watcher cares about.
- Opening the live database, even read-only, invites SQLite to create or recover the shared-memory and
  log files, which is a write into Apple's store and is forbidden by constraint 3.

So the procedure is: copy all three files into a private directory created with mode 0700, open the copy,
read, delete the copy. `immutable=1` in the connection string is the wrong tool here, because it makes
SQLite ignore the log and hand back exactly the stale answer measured above.

Read `ZCUSTOMLABEL` for the title, keyed by `ZPATH`. Everything else the design needs is already in the
audio file. If the copy fails, the schema has changed, or the join finds no row, the recording is
ingested **without** a title and the sidecar records `title_source: "unavailable"`. Missing a title is
not a reason to lose audio.

### What lands where, and how the three artifacts link

The vault layout the ledger names is adopted as it stands, one file per stage, sharing one identity:

```
agent-processing-pipeline/
  raw/audio/2026-08-24T144736-4f3ab19c02de.m4a     clone of the original, gitignored, mode 0600
  transcripts/2026-08-24T144736-4f3ab19c02de.md    written later, by whatever transcribes
  analysis/2026-08-24T144736-4f3ab19c02de.md       written later, by the agent
```

The linking is the shared identity plus explicit wiki links in frontmatter, which is what the vault's own
`CLAUDE.md` requires and what its DataviewJS listings read. Discovery writes no note; it writes the clone
and a sidecar. When the transcript note is created, its frontmatter carries
`reference: "[[2026-08-24T144736-4f3ab19c02de.m4a]]"` and `hub:` pointing at the folder note, mirroring
the meeting-note convention already in the vault.

**The clone is the preservation mechanism, and it is the part of this design that earns its keep.**
`clonefile(2)` creates a copy-on-write clone that shares data blocks with the source but is an
independent file: "Subsequent writes to either the original or cloned file are private to the file being
modified". Measured here, cloning the 187.9 MB recording took 0.00 seconds and consumed 16 KB. Both
locations are on `/dev/disk3s5`, the single APFS data volume, so `EXDEV` cannot occur on this machine.

That gives three properties at once that a plain copy or an in-place reference would each give only one
of: the vault holds a real, independent original that survives Apple evicting or deleting the source; the
988 MB store does not become 1.9 GB; and Apple's file is opened read-only and never modified. If the
destination is ever moved to another volume, `clonefile(2)` fails with `EXDEV` and the implementation
falls back to a byte copy with a warning rather than silently doing something slower and larger.

Per-recording state lives in `~/.local/state/vpp/recordings/<id>.json`, one sidecar each, holding the
source path, the hash, the capture instant in coordinated universal time, the duration, the title and its
source, the ingest time, and the stage. A sidecar per recording rather than a database, because the
workload is hundreds of rows, the file is readable with `cat` when something goes wrong, and nothing here
needs a query. If review state later grows aggregate queries, the upgrade to `rusqlite`, already the
repository's pinned dependency, is mechanical.

### Failure modes, and which ones page

The worst outcome in this component is silence: a sweep that finds nothing because it cannot see
anything, indistinguishable from a week with no recordings. The preflight exists for that.

| Condition                                    | What the sweep does                   | Notification                                                                |
| -------------------------------------------- | ------------------------------------- | --------------------------------------------------------------------------- |
| Recordings directory unreadable              | abort the sweep, exit non-zero        | page, this is the permission failure                                        |
| Directory readable, no `.m4a` entries at all | abort, exit non-zero                  | page, the store should never be empty once it has been seen non-empty       |
| Database copy or read fails                  | continue, ingest untitled             | no page, record in the sidecar                                              |
| A recording fails a wholeness gate           | defer with a reason, retry next sweep | page only after a configurable number of consecutive deferrals, default 4   |
| `SF_DATALESS` set                            | defer, never open                     | no page on the first sweep; page if it persists past the deferral threshold |
| `clonefile(2)` returns `EEXIST`              | already ingested, skip silently       | none, this is the normal idempotent path                                    |
| `clonefile(2)` returns `ENOSPC`              | abort the sweep                       | page                                                                        |
| Vault destination missing                    | abort the sweep                       | page                                                                        |

Paging is `pns submit --json` on stdin with `producer: "vpp"` and `signal.kind: "needs_attention"`, which
is the contract L-R5 names and which was verified in source: `pns/crates/pns-protocol/src/request.rs`
pins `pns.request` at major 1 with a `producer` field and a `NeedsAttention` variant on the signal enum,
and unknown top-level keys are ignored and named rather than refused. vpp spawns the deployed `pns`
binary the way posture already does, which couples nothing at build time and keeps R1 intact. If `pns` is
absent, vpp writes the same record to its log and exits non-zero; it must not fail to ingest because a
notifier is missing.

One documentation gap worth knowing before implementing: **`submit` is absent from `pns --help`**. The
subcommand works and is routed in `invocation.rs`, it is simply undiscoverable from the command-line
interface. That is a pns defect, not a vpp blocker.

### Configuration

One file, `~/.config/vpp/config.toml`, following the repository's ruling that defaulted keys ship
uncommented at their default so the shipped file shows the real posture:

```toml
[source]
# The Apple Voice Memos group container. Stated rather than compiled in, because
# the path is Apple's and an operating system release can move it.
recordings_dir = "~/Library/Group Containers/group.com.apple.VoiceMemos.shared/Recordings"
# Seconds a file must be unchanged before it is considered at rest.
quiet_period_secs = 30
# Read the Core Data store for human titles. Setting this to false gives untitled
# but otherwise complete ingestion, and touches nothing Apple does not document.
read_titles = true

[destination]
# The vault root. "Output content in the configured directory (Ivy for this operator)."
pipeline_dir = "~/workspaces/Ivy/agent-processing-pipeline"

[ingest]
# Consecutive deferrals of one recording before vpp raises a page.
deferral_page_threshold = 4
```

No secret is involved, so this file never reaches KeePassXC and stays outside the fifteen vault-backed
targets.

### Security and privacy

Voice memos are everyday personal audio, and this component is the point where that audio is copied.

- **Nothing leaves the machine.** Discovery is local-only by construction. The question of whether audio
  may reach a metered cloud transcription service belongs to the engine decision and is an open question
  below, not something this component settles by accident.
- **Clones are mode 0600.** `clonefile(2)` gives the destination its own attributes and inherits the
  target directory's access control lists, so the mode is set explicitly after the clone rather than
  assumed.
- **The vault never commits audio.** Verified: `*.m4a` is excluded at `.gitignore` line 80. Since
  Obsidian's mobile sync rides the same git history, the audio is also invisible to mobile, which is the
  correct default for personal recordings.
- **Titles are never logged.** A recording's title is human-assigned and can be sensitive. Logs and pns
  notifications carry the identity, never the title. The sidecar holds the title, and the sidecar is a
  local file under the operator's own state directory.
- **The database copy is transient.** Created in a 0700 directory, deleted at the end of the sweep,
  including on the error path.
- **The store is opened read-only, always.** Every file handle into the group container is `O_RDONLY`,
  and the only reason the database is copied at all is that opening the live one read-only still risks a
  write.

### The behaviors to drive the implementation, test-first

Each of these is one failing test before it is one piece of code, in the repository's own decomposition
style where a unit is a behavior and not a task. None of them needs a Voice Memos store: each takes a
fixture directory and a fixture `.m4a`.

1. A whole container with `ftyp`, `mdat` and `moov` whose box lengths sum to the file size passes the
   wholeness gate.
1. The same file truncated by one byte fails the gate, and the reason names the byte shortfall.
1. A file with a `moov` box removed fails the gate even when the remaining lengths sum correctly.
1. A file whose mtime is inside the quiet period is deferred, not ingested, and a file whose size changed
   since the sweep that deferred it is deferred again.
1. Identity is a pure function of file content and `mvhd` time: the same bytes at two different source
   paths derive the same identity.
1. Ingesting the same recording twice produces one clone and one sidecar, and the second run reports
   "already ingested" rather than an error.
1. A destination that already exists causes the clone to fail with `EEXIST`, and the sweep treats that as
   already-ingested rather than as a failure.
1. A source file with `SF_DATALESS` set is deferred without being opened.
1. A sweep over a directory containing `Capture/`, `CaptureRecovery/` and a `.waveform` sidecar takes
   only the top-level `.m4a` entries.
1. An unreadable recordings directory aborts the sweep with a non-zero exit and a page record, and
   ingests nothing.
1. A readable but empty recordings directory aborts with a page record once the store has been seen
   non-empty.
1. A failed database read yields ingestion with `title_source: "unavailable"` and no page.
1. A recording deferred for the configured number of consecutive sweeps produces exactly one page, not
   one per sweep.
1. `--dry-run` performs every gate and writes nothing: no clone, no sidecar, no page.
1. The source file's mtime, size and flags are unchanged after a full successful sweep.

Behavior 15 is the one that pins constraint 3, and it is worth writing first.

## Out of scope

Deliberately not designed here, and not to be smuggled in during implementation:

- **Transcription of any kind.** Engine choice, redundant engines, disagreement comparison, and the
  uncertain-text notification are the next ledger bullet, and they are blocked on the engine and cost
  questions.
- **Note authoring, summaries, tags, relationships, meeting briefs, redacted drafts, speaker labels.**
  Later bullets, and several of them overlap `minutes` directly.
- **The `minutes` disposition.** Untouched. This design's boundary is drawn so that either ruling works.
- **Repairing the orphaned `minutes` vault link.** An operator step carried forward, not vpp's job.
- **A retention policy for the clones.** L-R5 is explicit that nothing here is permission to delete
  recordings or notes. Clones accumulate; at the measured rate and with copy-on-write sharing, the cost
  is metadata until the source is deleted. A policy is a later decision.
- **Anything on the iPhone side.** vpp reads what iCloud has already put on this Mac.
- **Open Notebook and Bob.** Consumers, later, and vpp must work without either.
- **Any modification to Voice Memos, including enabling or configuring it.**

## Assumptions made in the operator's place

Each of these was a choice this document had to make to be written at all. None is a decision, each names
its alternative, and reversing any of them changes the design without invalidating the measurements.

1. **Discovery can be designed before the `minutes` ruling, because its output boundary is
   engine-agnostic.** Alternative: hold the whole vpp design until the ruling lands, as the
   reconciliation recommended for ingestion overall. Rejected because the reconciliation itself
   identified Voice Memos discovery as absent under both candidate scopes, which makes it the one piece
   the ruling cannot invalidate.
1. **Reading Apple's undocumented group container is acceptable, read-only, as the primary path.**
   Alternative: treat the private store as off limits and require a human export through the share sheet
   or a Shortcut, which would make vpp a manual filing tool rather than a watcher. Taken because the
   supported paths were measured to yield no audio at all, and because the design confines the
   undocumented dependency to the title lookup, which degrades cleanly.
1. **The title is optional and the audio is not.** Alternative: refuse to ingest a recording whose title
   cannot be read, so that every note is properly named. Rejected because losing audio to a schema change
   is the worse failure, and constraint 4 names lost audio explicitly.
1. **Identity is content-derived, so a recording edited in Voice Memos becomes a new recording.** A trim
   changes the bytes, changes the hash, and is ingested again beside the original. Alternative: key on
   `ZUNIQUEID` from the database, which would follow an edit and keep one identity. Rejected because it
   puts the private schema back on the critical path for correctness, and because keeping both the
   original and the trimmed version is consistent with "preserve original audio".
1. **A clone, rather than a reference in place or a byte copy.** Alternative A, reference the file in the
   container and never copy: zero storage, but the vault holds no original and Apple's eviction or a
   deletion in the app takes the audio with it. Alternative B, byte copy: a true independent original at
   roughly 1 GB today and growing. The clone gives A's cost and B's durability on this machine, and the
   measurement backs it.
1. **Sweep on a schedule rather than a filesystem watcher.** Alternative: `WatchPaths`, or a resident
   daemon. Argued above; the manual page argues against `WatchPaths` in its own words and the sweep is
   the correctness mechanism either way.
1. **Fifteen minutes is the default sweep interval.** Nothing measured says fifteen rather than five or
   sixty. Alternative: any other interval, or `WatchPaths` for near-immediate discovery. It is a
   one-value change in the plist.
1. **The identity's timestamp is in local time, with coordinated universal time in the sidecar.**
   Alternative: name files in coordinated universal time, which is unambiguous across daylight-saving
   transitions but reads wrong to a human scanning a folder. Local time was taken because the vault's own
   meeting-note convention is a local date and because the operator reads these filenames.
1. **Per-recording sidecar files rather than a database for review state.** Alternative: SQLite from the
   start, matching pns and posture. Rejected for now on volume and inspectability; the upgrade path is
   named.
1. **A page on an unreadable or empty store, rather than a quiet log line.** Alternative: log and stay
   silent, on the grounds that an alert for a permission problem is noise. Rejected because a silent
   watcher that sees nothing is the failure this component is most likely to have and least likely to
   notice.
1. **vpp becomes a fifth independent cargo workspace in this repository, following the existing four.**
   Alternative: its own repository from day one, given that it is a product other people might install.
   Taken because the four existing tools all started here and the layout exists precisely so that lifting
   one out later is a `git subtree split`.
1. **No live `pns submit` was run to prove the notification end to end.** Alternative: send a real
   request with `producer: "vpp"` and watch it arrive. Not done because it would have raised a banner, a
   Discord message and possibly a phone notification while the operator was asleep. The contract is
   verified in source at the level of field names and enum spelling, not at the level of "a vpp-shaped
   request is accepted and routed".

## Operator steps

Two, and only the first is needed before this design can be approved.

**1. Settle the LaunchAgent permission question.** The one measurement this document could not make
without either root or a privacy prompt appearing overnight. Write a plist whose program reads one byte
of `~/Library/Application Support/com.apple.TCC/TCC.db` and one byte of a recording, load it, log out and
back in so that launchd starts it with no submitting session in the picture, and read the result:

```bash
# after logging back in
cat ~/.local/state/vpp-permission-probe.txt
```

The logout is the load-bearing part. A job submitted from a terminal may inherit that terminal's
responsible-process attribution, which is what this test exists to rule out. `DENIED` on the recording
read means a background agent cannot see the store, and open question 2 becomes a design change rather
than a configuration value.

**2. Repair or retire the orphaned vault `minutes` link, if it is still wanted.** Carried forward from
the reconciliation, unrelated to this design, and listed here only so it is not lost:

```bash
minutes vault setup --strategy symlink --subdir agent-processing-pipeline/minutes
```

That also recreates the missing `~/.config/minutes/config.toml`. Without the `--subdir` flag the default
lands in `areas/meetings` and creates a second meetings location under a different Projects, Areas,
Resources folder. The vault's `CLAUDE.md` claim that the link is "managed by
`minutes vault setup --subdir`" needs a correction either way.

## Open questions for the operator

**Triage, 2026-09-15:** every question below is closed except where noted. See
`docs/decisions/2026-09-15-vpp-question-triage.md` (rows D1-D6) for the reasoning.

1. **Is reading Apple's undocumented Voice Memos store acceptable at all?** This is the gating one. The
   measurements say there is no supported programmatic path to the audio, so a "no" turns vpp from a
   watcher into a manual filing tool driven by the share sheet, and the design above becomes the wrong
   shape rather than a design needing edits.
1. **Can a LaunchAgent read the group container?** Unresolved, and the single test that settles it is
   step 1 under operator steps above. If the answer is no, the sweep still works from a login-shell
   context or on demand, but the "automatic" in "automatic discovery" gets an asterisk.
1. **Does the `minutes` ruling change this boundary?** The design assumes discovery is a producer and
   transcription is a consumer. If the operator intends vpp to be a thin front end on `minutes watch`
   instead, the clone destination changes to a directory `minutes` watches and most of this design is
   replaced by configuring a third-party tool. **Decided 2026-09-15: no.** `minutes` is out entirely, so
   the boundary in this design is unchanged: discovery stays a producer, transcription stays vpp's own
   consumer, and there is no third-party tool to configure instead. See
   `docs/decisions/2026-09-15-vpp-architecture-decisions.md`, decision 1.
1. **Clones, or references?** Assumption 5 chose clones on measured cost. Worth a sentence of
   confirmation, because it is the decision that puts a second copy of every personal recording inside
   the vault directory, gitignored but present.
1. **Fifteen minutes, or something else?** And is near-immediate discovery worth taking `WatchPaths`
   despite its manual page?
1. **What happens to a recording deleted in Voice Memos after vpp has cloned it?** The clone survives,
   which is the point of cloning. Is that correct, or should vpp notice the disappearance and mark the
   sidecar? L-R5 says the evaluation is "not permission to delete recordings or notes", which argues for
   keeping the clone silently, but noticing costs nothing.
1. **Do the eight `.waveform` sidecars matter?** They belong to 2022-era recordings only, they are
   Apple's rendering cache, and this design ignores them. Confirm that nothing wants them preserved.
1. **Should `vpp ingest` emit its per-recording record on stdout as JSON (JavaScript Object Notation) for
   a caller to pipe, or only write sidecars?** The design does both, on the assumption that the next
   stage will want a stream. If nothing will consume it, the stdout record is unneeded surface.
