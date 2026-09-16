# vpt source reconciliation, 2026-09-14

`docs/remaining-work.md` line 2094 asks for one thing before any vpt (Voice Processing Tool) design work
starts: reconcile the transcription plans that already exist, name each one's scope, and name the gap vpt
fills. The task names three sources (homelab `PLAN-v12-experiments-backlog.md` L-R5, the Ivy vault's
`agent-processing-pipeline/` layout, and homelab `PLAN-v11.md` Phase 6) and asserts that none of them
specifies a watcher for Apple Voice Memos synced to macOS.

This record answers that question, and it reports one thing the task did not ask for because the
reconciliation turned it up: there is a **fourth** source, it is installed and working on this machine
today, and it already implements most of the vpt feature list. No transcription system was designed, no
code was written, nothing was installed, configured or applied, and no notification was raised.

## Verdict

**Defer the vpt ingestion design. The reconciliation is complete and it does not clear vpt to start.**

The three named sources reconcile cleanly and the task's assertion about them holds: none specifies a
Voice Memos watcher, and the gap vpt would fill is real. But the task's premise, that reconciling those
three is sufficient to start designing, does not hold. The `minutes` command-line interface (CLI),
version 0.26.1, installed as a declared Homebrew cask on 2026-09-08, four days before vpt was planned,
already ships: a folder watcher with a launchd service, a first-class `memo` content type, `transcribe`
with a JSON envelope and speaker diarization, Obsidian vault sync by symlink, speaker voiceprints,
retention policy with a cleanup preview, structured meeting insights, commitments and action tracking,
weekly-summary and proactive-context automation primitives, and a redacted "process-private projection"
used by its people and commitments commands.

Six of the seven vpt feature bullets in `remaining-work.md` overlap that tool. Three of vpt's stated open
questions (summary format, retention, speaker labels) are settings it already has. Two vpt requirements
are genuinely absent from it: **redundant transcription with disagreement comparison**, and **pns
notification of uncertain output**. One more is absent from every source including `minutes`: **an Apple
Voice Memos watcher**.

So the honest shape of the gap is much smaller than the task assumes, and the design that follows depends
entirely on a ruling nobody has made: is `minutes` kept, or replaced? `remaining-work.md` line 2084 and
Todoist task `6hPV483GJgGHX95M` both still carry the `minutes` evaluation as open, with "their old scope"
retained. vpt cannot be scoped until that closes, because the two candidate scopes differ by roughly an
order of magnitude:

- **vpt as an adapter** (if `minutes` is kept): a Voice Memos discovery front end plus a second engine
  and a disagreement comparison, handing audio to `minutes transcribe --json` and notes to the existing
  vault sync. Small, and it does not duplicate a working tool.
- **vpt as a replacement** (if `minutes` goes): the whole feature list, including the pieces `minutes`
  already solved, and a `minutes` retirement.

`PLAN-v12.md` L6 already states the governing rule, for a different service: "Open Notebook must not
create a second automatic Voice Memos capture/transcription workflow." That rule was never applied to
`minutes` against vpt, and it is the reason this record stops short of a design.

There is also a live drift worth fixing regardless of the vpt decision: the vault's documented `minutes`
integration is broken. The symlink is committed, but `minutes` no longer has a configuration file and
reports `Vault: not configured`, so its sync does not know the target. Details below.

## What was checked and how

Read, in full, in the sections they cover:

- `/Users/stephen/workspaces/Ivy/webdavis/dotfiles/docs/remaining-work.md`, the
  `vpt (Voice Processing Tool)` section (lines 2168 to 2207) and its "Homelab plan coordination"
  predecessor.
- `/Users/stephen/workspaces/Ivy/webdavis/homelab/docs/plans/PLAN-v12-experiments-backlog.md`, section
  `L-R5. vpt (Voice Processing Tool)`, line 106 onward.
- `/Users/stephen/workspaces/Ivy/webdavis/homelab/docs/plans/PLAN-v11.md`,
  `Phase 6, Transcription pipeline (cloud-first)`, line 866 onward, plus its cost table rows and its
  Phase 3 skill reference.
- `/Users/stephen/workspaces/Ivy/webdavis/homelab/docs/plans/PLAN-v12.md`, L5 and L6 (lines 355 to 373),
  and `PLAN-v12-service-matrix.md` line 145.
- `/Users/stephen/workspaces/Ivy/CLAUDE.md`, the `agent-processing-pipeline/` paragraph (lines 36 to 43),
  and the vault `.gitignore`.
- Todoist tasks `6hVpPJC2cjJW3V9M` (vpt planning), `6gjGcHp69phXmXj3` (Phase 6, five active subtasks) and
  `6hPV483GJgGHX95M` (SP7 tool evaluations), read with `td task view`, `td` 5.3.4.

Commands run on dresden, 2026-09-13 evening local time:

```
minutes --version                  -> minutes 0.26.1 (github.com/silverstein/minutes v0.26.1)
brew info --cask minutes           -> 0.26.1 -> 0.26.2 available; installed 2026-09-08 15:23:16
                                      from github.com/silverstein/homebrew-tap
minutes --help                     -> 60 subcommands (full list summarized below)
minutes capabilities               -> api_version 1; 40-plus named features
minutes paths                      -> config /Users/stephen/.config/minutes/config.toml (ABSENT)
                                      minutes_dir ~/.minutes, output_dir ~/meetings
minutes service status             -> missing dev.getminutes.watcher
                                      missing dev.getminutes.weekly-summary
                                      missing dev.getminutes.proactive-context
minutes vault status               -> "Vault: not configured"
minutes storage --json             -> per-file retention verdicts, class/action/reason
minutes watch --help               -> default watch dir ~/.minutes/inbox/
minutes process --help             -> -t meeting|memo, default memo
minutes transcribe --help          -> --json envelope, --diarize
minutes vault setup --help         -> --strategy symlink|copy|direct, --subdir (default areas/meetings)
uv tool list                       -> whisply v0.14.2
whisply --help                     -> run / app / list
whisper --help                     -> openai-whisper, Homebrew formula
ffmpeg -version                    -> 9.0.1
elevenlabs --version               -> 1.2.0 (fnm node 24 lane)
ffprobe <one Voice Memo>           -> codec alac, 48000 Hz, 2 channels, mov/mp4/m4a container
pns --version                      -> 0.1.0
pns submit --help                  -> {"schema":"pns.result/1",...,"diagnostics":["submit_usage"]}
```

Source read rather than executed, to avoid raising a real notification while the operator is asleep:
`pns/crates/pns/src/submit.rs`, `pns/crates/pns/src/invocation.rs` line 119, and
`pns/crates/pns-protocol/src/request.rs`.

The Apple Voice Memos store was inspected read-only. The `CloudRecordings.db` SQLite file was **copied**
to the session scratchpad and queried there; the live file was never opened by a writer and never
modified. Queries were aggregate counts only, so no recording titles are reproduced in this record.

Nothing from training data is relied on below. Where a mechanism is inferred rather than measured, the
sentence says so.

## Findings

### Source 1, homelab `PLAN-v12-experiments-backlog.md` L-R5: the vpt specification itself

Status line: "queued, planning only. The operator named vpt on 2026-09-12 and chose Rust." Eight
unchecked bullets. This is not a separate transcription system; it **is** vpt, and it is the most
detailed statement of it. The dotfiles ledger section is a near-paraphrase of it.

L-R5 carries three things the dotfiles copy does not:

1. **The exact pns integration contract**: "`pns submit --json`, with `producer: "vpt"` and
   `signal.kind: "needs_attention"`. Keep review state in vpt; a notification receipt does not mean the
   user reviewed or corrected the note. Reverify the protocol when implementing."
1. **The disagreement targets**: "particularly names, numbers and dates."
1. **The evaluation aid carried forward from the earlier trial**: "three uses per week, review usefulness
   after a month," explicitly "not permission to delete recordings or notes."

I reverified the pns contract as L-R5 asks. It holds: `pns/crates/pns/src/invocation.rs:119` routes the
bare word `submit` into `event_flow::submit_mode`, and `submit.rs` accepts exactly one argv form,
`--json`, reads a `pns.request` envelope from stdin and writes a `pns.result` envelope to stdout.
`pns-protocol/src/request.rs` pins `SCHEMA_NAME = "pns.request"`, `SCHEMA_MAJOR = 1`, and 15 known
top-level fields including `producer`, `signal`, `event`, `elapsed_secs`, `detail` and `context`. The
`Signal` enum is `#[serde(tag = "kind", rename_all = "snake_case")]` with a `NeedsAttention` variant, so
`{"signal":{"kind":"needs_attention"}}` is the correct spelling. Unknown top-level keys are "ignored and
named, never refused."

One correction for whoever implements it: **`submit` is absent from `pns --help`.** The usage text lists
`hook`, `gate`, `pulse`, `quiet`, `daemon`, `lights`, `presence`, `shell`, `loop`, `nag`, `recap`,
`setup`, `doctor`, `tap` and `home`, and the producer-flag form, but not `submit`. The subcommand works;
it is just undiscoverable from the CLI. That is a pns documentation gap, not a vpt blocker.

### Source 2, the Ivy vault `agent-processing-pipeline/` layout: a filing convention, never used

`/Users/stephen/workspaces/Ivy/CLAUDE.md` defines it as "the staged workspace where recordings become
transcripts and then analysis", with `raw/audio/` for unprocessed input, `transcripts/` for raw
transcripts, `analysis/` for agent output, and audio excluded from git vault-wide.

Measured state: **all three directories are empty.** Each holds only a `.gitkeep`, and every mtime in the
tree is `2026-07-22 01:35`, the moment it was created. The vault `.gitignore` does exclude audio (lines
79 to 81: `*.wav`, `*.m4a`, `*.mp3`). `git ls-files` shows four tracked entries: the three `.gitkeep`
files and the `minutes` symlink.

So this source contributes a **naming and separation convention** and nothing else. It is the answer to
vpt's "keep original recordings, transcripts and agent analysis separately linked using the existing
vault layout", and it costs nothing to adopt because no data has to be migrated into it.

The `minutes` entry is where this source stops being inert and becomes a live finding. `CLAUDE.md` says
it is "a symlink to the `minutes` tool's output directory, managed by `minutes vault setup --subdir`, so
its recordings live outside the repo and git only ever stores the link." The first half is true: the
symlink exists, points at `/Users/stephen/meetings`, is committed, and `minutes vault setup` really does
take `--strategy symlink` and `--subdir`. The second half is **no longer true**:

- `minutes paths` reports a config at `~/.config/minutes/config.toml`, and that directory **does not
  exist**.
- `minutes vault status` reports `Vault: not configured. Run "minutes vault setup" to connect a vault.`

The link is therefore an orphan. `minutes` writes to its default `~/meetings`, the symlink happens to
point there so notes are visible in the vault, and nothing the tool does is actually vault-aware:
`minutes vault sync`, the catch-up path, has no target to sync to. A second hazard follows from the same
gap: `minutes vault setup`'s default `--subdir` is `areas/meetings`, not
`agent-processing-pipeline/minutes`, so anyone who re-runs setup without the flag creates a second
meetings location in the vault under a different PARA (Projects, Areas, Resources) folder.

### Source 3, homelab `PLAN-v11.md` Phase 6: a homelab service for URLs, not a Mac tool for recordings

Phase 6 is titled "Transcription pipeline (cloud-first)" and its engine line is explicit: "ElevenLabs
Scribe v2 + whisply. Cloud-first because there's no dedicated local ML node. Local `whisply` on
faster-whisper (on `lash`) handles API outages or sensitive audio."

Its five subsections, and their Todoist mirrors `6gjGcHp69phXmXj3` plus subtasks 6.1 to 6.5, are:

| Piece | What it is                                                                                                                  | Where it runs |
| ----- | --------------------------------------------------------------------------------------------------------------------------- | ------------- |
| 6.1   | `ELEVENLABS_API_KEY` per user, from OpenBao via the secret operator                                                         | homelab       |
| 6.2   | a hermes `transcribe` skill: `yt-dlp` audio extract, Scribe v2, whisply word timings, `.md` plus `.srt`/`.vtt` plus `.json` | `lash`        |
| 6.3   | workflow 1: a Markdown URL queue file, edited in Obsidian, watched by inotify                                               | `lash`        |
| 6.4   | workflow 2: an n8n four-hour cron over FreshRSS items tagged `transcribe`                                                   | homelab       |
| 6.5   | workflow 3: an n8n webhook an iPhone Shortcut or Mac CLI posts to                                                           | homelab       |

Three facts settle the relationship to vpt:

1. **Its input is a URL, not a recording.** Every one of the three workflows takes a link (a YouTube URL
   in 6.3 and 6.4, a posted payload in 6.5) and runs `yt-dlp` to get audio. It is a "transcribe this
   video" pipeline. Nothing in it discovers a local audio file.
1. **It runs on `lash` under hermes, not on the Mac.** Its outputs land in
   `transcripts/<YYYY-MM-DD>-<title>.{md,srt,json}` on the homelab side. The only Mac-side surface is the
   client that posts a URL to 6.5.
1. **Its state is planned, not built.** Every bullet carries the plan marker. The ledger's acceptance
   line ("All three transcription trigger paths produce a transcript within ~2 min") is unchecked, and
   the cost table lists ElevenLabs Scribe v2 at "~$10 to 60 by volume, Planned (Phase 6)."

A provenance wrinkle: Todoist `6gjGcHp69phXmXj3` names its source plan as `PLAN-v7.md` Phase 6, while
`remaining-work.md` and the current plan text cite `PLAN-v11.md` Phase 6. The content is the same engine
choice; the task description is just pinned to an older plan revision.

What Phase 6 genuinely contributes to vpt is **the engine decision and the output format**, already made:
cloud-first Scribe v2, local whisply on faster-whisper as the outage and sensitive-audio fallback, and
Markdown plus subtitle plus word-level JSON as the three outputs. vpt's open question "choose
transcription engines, local/cloud processing" is partly answered by a decision that already exists, for
a sibling pipeline, with a paid-metered cost line attached.

And the engines are on this machine already, unwired: `whisply` 0.14.2 via uv, `openai-whisper` via
Homebrew, `@elevenlabs/cli` 1.2.0 on the fnm node 24 lane, `ffmpeg` 9.0.1. A grep across the dotfiles
source finds no script, template, recipe or LaunchAgent that invokes any of them for a transcription
pipeline. They are declared and installed because Phase 6 and the earlier experiment named them.

### Source 4, the `minutes` cask: the one the task does not name, and the one that matters

`minutes` 0.26.1, a third-party cask from `silverstein/homebrew-tap`, described upstream as a "Meeting
recorder and transcriber that runs on-device", declared in
`.chezmoidata/system_packages_autoinstall.yaml` under `packages.macos.homebrew.casks`, installed
2026-09-08. Its `capabilities` output reports `api_version: 1` and a 40-plus feature list, which exists
precisely so another program can feature-detect it.

Its 60 subcommands include, matched against the vpt bullets in `remaining-work.md`:

| vpt requirement                                                 | `minutes` today                                                                                                                                                                                                                                   |
| --------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| watch for new audio and process it automatically                | `minutes watch [DIR]`, plus `minutes service install` as a launchd login service                                                                                                                                                                  |
| voice memos as a distinct kind from meetings                    | `-t memo` on `process`, `--content-type memo` on `list`, first-class in `search`                                                                                                                                                                  |
| transcribe, for integration use                                 | `minutes transcribe <path> --json [--diarize]`, "audio in, transcript (or JSON) out"                                                                                                                                                              |
| preserve originals with a retention policy                      | `minutes storage`, `minutes cleanup` (preview or apply), `delete` archives by default                                                                                                                                                             |
| Markdown out, optionally into an Obsidian vault, vault optional | `minutes vault setup --strategy symlink\|copy\|direct --subdir <path>`                                                                                                                                                                            |
| speaker labels (listed as an unapproved candidate)              | `minutes voice` enroll/revoke, `enroll`, `voices`, `confirm`, `redo-speaker-mapping`, `--diarize`                                                                                                                                                 |
| tags and relationships across recordings                        | `minutes ingest`, `insights`, `person`, `people`, `relationship_map`, `research`, `vocabulary`                                                                                                                                                    |
| summary format choice                                           | `minutes template list/show/validate`, `resummarize`                                                                                                                                                                                              |
| meeting briefs, commitments, follow-ups                         | `minutes actions`, `commitments`, `consistency`, `automate weekly-summary`, `automate proactive-context`                                                                                                                                          |
| a redacted draft for sharing, reviewed before release           | `automate --delivery-target slack-json\|email-markdown` is explicitly "draft-only"; `people`/`commitments` run over a "bounded, process-private projection" and exclude `sensitivity: restricted` unless an override is recorded on its event bus |
| **redundant transcription, disagreement comparison**            | **absent.** One pipeline. `clean` fixes hallucinated repetitions; nothing compares two engines                                                                                                                                                    |
| **pns notification of uncertain output**                        | **absent.** It has `events`, `jobs` and an event bus, so a watcher could derive one, but it raises nothing through pns                                                                                                                            |
| **Apple Voice Memos discovery**                                 | **absent.** Its "voice memo" means a memo it recorded or was handed, not an Apple Voice Memos recording. `watch` takes a plain directory; `import` takes Granola, a text archive, a transcript directory or one audio file                        |

Its automation is installed but **not running**: all three of its launchd jobs report `missing`, and
`dotfiles` tracks no plist, config template or recipe for it. The only references in the source tree are
the cask line and one comment in `scripts/cutover-gate.sh` listing it among packages a cutover apply
installs. So the tool is present, the operator has used it (three notes plus an archive of paired
`.md`/`.wav` demo recordings under `~/meetings`), and none of its automation is wired.

Two collisions to note before any design leans on it:

- **Its retention policy deletes originals, and vpt forbids that.** `minutes storage --json` currently
  classifies every archived demo recording as `"action": "delete-candidate"`,
  `"reason": "successful recording audio older than 30 days"`. vpt's requirement is "preserve original
  audio", and L-R5 says the evaluation aid is "not permission to delete recordings or notes". Routing
  vpt's audio through the `minutes` tree without settling retention puts a 30-day clock on the originals.
  `cleanup` previews by default, so nothing deletes unattended today, but the classification is already
  there.
- **It is third party, so the global rule forbids patching it.** The standing rule is never to patch,
  fork or modify the code of a tool the operator does not own; configure it through its own supported
  options. So "vpt is minutes plus the two missing pieces" is only available as **configure and call**
  (`minutes transcribe --json`, `minutes process`, `minutes watch`, `minutes capabilities` for feature
  detection), never as a patch. Its `capabilities` command and JSON envelopes say upstream intends that
  use.

### The Apple Voice Memos store on this Mac, measured

The task's assertion is correct: no source specifies this watcher. Because "verify the supported macOS
access/export path and actual audio format" is the **next** ledger task, not this one, what follows is
input for that task, not a chosen mechanism. It is reported because it changes how hard the gap is.

Location: `~/Library/Group Containers/group.com.apple.VoiceMemos.shared/Recordings/`. Contents right now:
28 `.m4a` recordings, 8 `.waveform` sidecars, `CloudRecordings.db` with its `-wal` and `-shm` companions,
an `EncryptedCloudRecordings/` subtree with its own database, an empty `CloudRecordings_ckAssets/`, and
empty `Capture/` and `CaptureRecovery/` directories. Filenames are `YYYYMMDD HHMMSS-HEX.m4a`, so they
carry a timestamp and no title.

**Format: Apple Lossless Audio Codec (ALAC), 48000 Hz, 2 channels, in an `.m4a` container.** `ffprobe` on
one file reports `codec_name=alac`. This matters twice. First, the extension invites the assumption that
these are AAC, and they are not. Second, they are large: the 28 files run to roughly 1 GB, with single
recordings at 138 MB and 188 MB. Anything that copies originals into the vault tree inherits that size,
and anything that uploads to a metered cloud engine pays for it.

**Titles and capture metadata live in the database, not the filesystem.** `CloudRecordings.db` is a Core
Data store with CloudKit mirroring (`ANSCK*` tables alongside `Z_PRIMARYKEY` and `Z_METADATA`). The
`ZCLOUDRECORDING` table carries `ZDATE`, `ZDURATION`, `ZCUSTOMLABEL` (the user-visible title),
`ZCUSTOMLABELFORSORTING`, `ZENCRYPTEDTITLE`, `ZPATH`, `ZUNIQUEID`, `ZFOLDER`, `ZEVICTIONDATE`, `ZFLAGS`
and playback and mix settings. So vpt's "capture metadata" and its "stable recording identifiers" both
require reading Apple's private schema; the folder alone yields a timestamp and a hex string.

Three measured facts the ingestion design should not have to rediscover:

1. **Reading the database without its write-ahead log gives a stale answer.** Copying
   `CloudRecordings.db` alone reported 27 rows, 26 with a path. Copying it together with `-wal` and
   `-shm` reported 28 rows, 28 with a path, matching the 28 files exactly. The checkpointed file was nine
   days and one recording behind. A reader that copies one file and not three will silently miss the
   newest recordings, which are the only ones a watcher cares about.
1. **`ZEVICTIONDATE` is not an "audio is absent" signal.** Two rows carry an eviction date and **both**
   their local files are present. Every one of the 28 rows' paths exists on disk. A design must test the
   file, not trust the column.
1. **Nothing is a dataless placeholder at the moment of reading.** `ls -lO` shows no file flags set on
   any of the 28 recordings, so none is an APFS dataless stub awaiting download. That is the current
   state, not a guarantee: the empty `CloudRecordings_ckAssets/` directory is very likely where an
   in-flight CloudKit asset lands, which would make it the place to watch for an incomplete download, but
   that is inference from the name and the timestamps, **not measured**, because no download was in
   flight to observe.

One access caveat the next task must settle: every read above ran from a terminal that already holds
broad disk access. A launchd agent has its own privacy-permission identity and may not inherit it, so "a
script can read this today" does not establish "a background service can read it." That needs a real test
under launchd before a mechanism is chosen.

## Reconciliation summary

| Source                                 | Scope                                                                              | State                                                                            | What it gives vpt                                                                  |
| -------------------------------------- | ---------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| `PLAN-v12` L-R5                        | vpt itself: the feature list, the pns contract, the rejected features              | queued, planning only                                                            | the specification, and the verified `pns submit --json` contract                   |
| Ivy vault `agent-processing-pipeline/` | a filing convention: raw audio, transcripts, analysis, audio out of git            | directories exist, all empty since 2026-07-22                                    | the separation and linking layout, free to adopt                                   |
| `PLAN-v11` Phase 6                     | a homelab hermes skill that transcribes URLs on `lash` via n8n and a queue file    | planned, nothing built                                                           | the engine decision (Scribe v2 plus whisply fallback) and the three output formats |
| `minutes` 0.26.1 cask                  | an on-device Mac meeting and memo recorder, transcriber, summarizer and vault sync | installed and used; all automation unwired; vault link orphaned; evaluation open | six of seven vpt feature bullets, already working                                  |

**The gap vpt adds, after all four are accounted for**, is three items and no more:

1. **Apple Voice Memos discovery**: finding fully synced recordings in the group container, reading their
   titles and capture metadata out of the Core Data store, and handling interrupted sync, retries and
   repeated discovery without duplicates or loss. Absent from all four sources.
1. **Redundant transcription with disagreement comparison**, focused on names, numbers and dates,
   preserving alternatives and source references. Absent from all four.
1. **pns notification of uncertain text and unsupported notes** through `pns submit --json` as
   `producer: "vpt"` with `signal.kind: "needs_attention"`, with review state held in vpt. Absent from
   all four.

Everything else on the vpt list either exists in `minutes` today or is a convention the vault already
defines. That is the finding, and it is why this record refuses to hand the next task a clean start.

## Assumptions made in the operator's place

Each of these is a choice I had to make to write the record at all. None is a decision; each names its
alternative, and the operator can reverse any of them without invalidating the measurements.

1. **I treated `minutes` as in scope for this reconciliation even though the task did not name it.**
   Alternative: read the three named sources only, report that they reconcile, and let the design task
   discover the overlap later. I rejected that because the task's stated purpose is "before designing a
   second transcription system", and `minutes` is the first one.
1. **I treated the `minutes` evaluation as genuinely open rather than as tacit acceptance.** Its cask is
   declared and it has been used, which could be read as "adopted". Alternative reading: it is adopted,
   so vpt is automatically the adapter scope. I did not take that reading because `remaining-work.md`
   line 2084 and Todoist `6hPV483GJgGHX95M` both still list it as an evaluation retaining its old scope,
   and because none of its automation is wired, which is not what an adopted tool looks like.
1. **I read the vault `minutes` symlink as drift to be repaired, not as a deliberate unmanaged link.**
   Alternative: the operator wanted the link without the tool's vault integration, and
   `minutes vault status` reporting "not configured" is the intended state. I flagged it as drift because
   the vault `CLAUDE.md` claims the link is "managed by `minutes vault setup --subdir`", and that claim
   is now false either way.
1. **I did not run `pns submit --json` live.** Alternative: send a real request with `producer: "vpt"` to
   prove the contract end to end. I verified it from source instead, because a live submit would have
   raised a real banner, a Discord message and possibly a phone notification while the operator was
   asleep. The contract is verified at the level of "the subcommand exists, the field names and the enum
   spelling are correct"; it is not verified at the level of "a vpt-shaped request is accepted and
   routed."
1. **I did not run `minutes cleanup`, `minutes vault setup`, or `minutes service install`.** Alternative:
   exercise the retention and vault paths to measure them rather than reading their help text and JSON. I
   did not, because all three mutate state outside this repository.
1. **I left the ALAC size implication unresolved rather than proposing a storage policy.** Roughly 1 GB
   for 28 recordings, with a 188 MB single file, bears on whether originals are copied, hard-linked or
   referenced in place, and on cloud-engine cost. That is the ingestion task's decision, not this one's.

## What would change the verdict

- **The operator rules that `minutes` is rejected or replaced.** Then vpt takes the full scope, the
  overlap table becomes a feature checklist rather than a duplication warning, and the design task can
  start immediately on the whole list.
- **The operator rules that `minutes` is adopted.** Then vpt is scoped to the three gap items, the design
  task starts on a much smaller surface, and the `minutes` configuration (retention, vault target,
  watcher service) becomes dotfiles-owned work under vpt's "dotfiles owns Mac installation and service
  configuration" split.
- **A background service turns out to be unable to read the Voice Memos group container** under launchd's
  own privacy-permission identity. Then Voice Memos discovery is not a watcher at all, and the gap item
  becomes an export path question (the Voice Memos share sheet, a Shortcut, or a foreground agent), which
  changes what vpt is.
- **Apple's Core Data schema is judged off limits** as an unsupported private interface. Then titles and
  capture metadata are unavailable, and vpt's "preserve capture metadata" reduces to filesystem
  timestamps plus whatever an export path carries.
- **`minutes` ships redundant transcription upstream.** 0.26.2 is already available and unread; a release
  that adds a second engine and disagreement reporting would shrink the gap to two items.

## Open questions for the operator

1. **Is `minutes` kept or replaced?** This is the blocking one. Everything about vpt's scope follows from
   it, and the ledger has carried it as an open evaluation since before vpt existed.
1. **If `minutes` is kept, does vpt call it or run beside it?** Calling `minutes transcribe --json` makes
   it one of vpt's two engines and reuses its summarization, vault sync and speaker work. Running beside
   it means two tools writing notes about recordings, which is the thing `PLAN-v12` L6 forbids for Open
   Notebook.
1. **Does the `PLAN-v12` L6 rule ("must not create a second automatic Voice Memos capture/transcription
   workflow") bind `minutes` against vpt?** It was written for Open Notebook. If it is a general rule, it
   decides question 2 on its own.
1. **Which two engines are the redundant pair?** Phase 6 already chose ElevenLabs Scribe v2 with whisply
   on faster-whisper as a fallback, and all three of Scribe, whisply and `openai-whisper` are installed
   here, as is `minutes`'s own on-device pipeline. "Cloud plus local" and "two local" differ in cost, in
   what audio leaves the machine, and in whether disagreement is meaningful.
1. **Do Voice Memos originals leave the machine?** L-R5 inherits Phase 6's "local processing for
   sensitive audio", but a Voice Memos recording is everyday personal audio by definition. A blanket
   local-only rule for this source would remove the metered cost line and one of the two candidate
   engines at the same time.
1. **Repair the vault `minutes` link now, or fold it into the vpt work?** The immediate repair is the
   operator running `minutes vault setup --strategy symlink --subdir agent-processing-pipeline/minutes`
   (which would also restore the missing config file), plus a correction to the vault `CLAUDE.md` claim.
   Folding it into vpt means the documented integration stays false until vpt ships.
1. **Should the four unwired transcription installs stay declared?** `whisply`, `openai-whisper`,
   `@elevenlabs/cli` and the `minutes` cask are all in the package data and none is reached by any
   script. They are either vpt's future inputs or removable weight, and the answer depends on question 4.
1. **Does `minutes` get upgraded to 0.26.2 before the disposition decision?** Its feature set is the
   input to that decision, and the installed copy is one release behind.
