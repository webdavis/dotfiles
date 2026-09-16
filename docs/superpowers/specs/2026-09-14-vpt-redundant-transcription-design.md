# vpt redundant transcription and disagreement review

Status: design, written 2026-09-14 while the operator was asleep. Not approved, not built. No code was
written or changed. Every choice made in the operator's place is listed under "Assumptions made in the
operator's place" with its alternative, and the questions that need an answer are at the end.

Scope: `docs/remaining-work.md`, the `vpt (Voice Processing Tool)` section, third bullet, line 2189.

> Use redundant transcription and compare disagreements; flag uncertain text and unsupported notes for
> review, notifying through pns's producer application programming interface (API). Preserve the
> alternatives and source references. Multiple engines agreeing does not prove correctness. The proposed
> feature for playing audio from a summary sentence was rejected; original audio preservation remains.

The section intro, added 2026-09-12, sets the frame: "Build vpt in Rust to collect everyday Apple Voice
Memos synced to the Mac, preserve their original audio format, transcribe them, and produce agent notes
and summaries."

The operator's half of this task is Open Question 8 in the same file, line 2323: "vpt: decide permitted
local/cloud processing and cost before selecting redundant engines." This document does not answer that
question. It measures what each option actually costs, in wall-clock time and in dollars, so the answer
is a choice between priced options rather than a guess, and it designs everything that does not depend on
which engines win.

## What this builds on

This is the third document in the vpt chain and it assumes the first two.

**The source reconciliation** established that `minutes` 0.26.1 already covers six of the seven vpt
feature bullets, that the `minutes` keep-or-replace ruling is still open, and that exactly two vpt
requirements are absent from every source including `minutes`: redundant transcription with disagreement
comparison, and pns notification of uncertain output. Both of them are this bullet. So this is the one
vpt task that is new work under either ruling, which is what makes it safe to design now.

**The discovery design** drew the boundary this document starts from. `vpt ingest` sweeps Apple's group
container, gates each recording for wholeness, clones it into the vault with `clonefile(2)`, and writes a
sidecar. It ends there deliberately: "It does not transcribe, summarize, tag, or write a note. That
boundary is what keeps this design independent of the `minutes` ruling." Its output, a recording with a
content-derived identity of the form `2026-08-24T144736-4f3ab19c02de`, is this document's input.

**The boundaries design**, written in parallel, recommends that vpt live in its own repository rather
than as a fifth cargo workspace in dotfiles, which is where the discovery design assumed it would go.
That disagreement is unresolved and it belongs to the operator, not here. Nothing below depends on the
answer: the code home changes the build and install story, not the comparison. The same is true of the
shipping name, which the boundaries design already raised as an open question because `VPP` collides with
FD.io's Vector Packet Processing. "vpp" is used here as the working name.

## Constraints this design is bound by

From the ledger bullet and the homelab backlog entry `PLAN-v12-experiments-backlog.md` L-R5, which is the
fuller statement of the same feature:

1. Two or more engines transcribe each recording and their disagreements are found.
1. Disagreements matter most for "names, numbers and dates" (L-R5's own words).
1. Uncertain text is flagged for review.
1. Generated notes are checked against the transcript, and unsupported claims are flagged.
1. Alternatives and source references are preserved.
1. Agreement between engines is not proof of correctness.
1. Notification goes through `pns submit --json` with `producer: "vpt"` and
   `signal.kind: "needs_attention"`, the protocol is reverified at implementation time, review state is
   kept in vpt, and a notification receipt does not mean the operator reviewed anything.
1. Playing audio by selecting a summary sentence is rejected. Preserving the original recording is still
   required.
1. Engines, local versus cloud processing, summary format and retention are chosen before implementation.

From the repository's standing rules, carried over from the discovery design with the same labels:

- **R1.** No workspace may depend on another, and a tool never assumes this checkout exists. Runtime
  integration by spawning a deployed binary stays allowed, which is how posture already reaches pns and
  how every engine is reached below.
- **R2.** Rust files target 300 lines and never exceed 500, unit tests included.
- **R3.** Never patch, fork or modify a third-party tool. `minutes`, `whisply`, `openai-whisper` and
  Apple's Speech framework are all third party here. Each is configured and called, never modified.
- **R4.** Tests cover the behavior of tools we wrote and nothing else.
- **R5.** The operator runs applies. An agent proposes.

## What was measured, and how

Everything in this section was measured on dresden on 2026-09-14. Where a statement is inference rather
than measurement, the sentence says so.

No personal recording was transcribed. The engine comparison ran against a synthesized clip: macOS `say`
with the Samantha voice reading a 563-character script written for this measurement, containing four
personal names, an invoice number, two money amounts, a date, a phone number and one word spelled out
letter by letter. Ground truth is therefore exactly known, and no voice memo content left the machine or
reached a scratch file. The Voice Memos container was read for file sizes and durations only, read-only,
through `stat` and `ffprobe`.

### The corpus, priced

| Question                           | Method                             | Result                                            |
| ---------------------------------- | ---------------------------------- | ------------------------------------------------- |
| recordings on this Mac             | glob of the group container        | 28 files                                          |
| total audio duration               | `ffprobe` per file, summed         | 16,584.45 s, which is 4.607 hours                 |
| mean recording length              | derived                            | 592.3 s, just under 10 minutes                    |
| longest recording                  | `ffprobe` maximum                  | 3,153 s, 52.6 minutes, about 7,900 words          |
| total bytes of audio               | `stat -f %z` per file, summed      | 1,031,348,566 bytes, 983.6 MiB                    |
| mean bitrate                       | derived from the two above         | 498 kbit/s, consistent with Apple Lossless        |
| whole back catalogue at $0.22/hour | derived                            | $1.01 once, for every recording that exists today |
| 32 kbit/s Opus size of that corpus | derived                            | 63.3 MiB, a 15.6x reduction                       |
| Opus encode speed                  | `ffmpeg` 9.0.1 on the 34.38 s clip | 1.11 s, about 31x faster than real time           |

### The engines, run

Four runs against the same 34.38 second clip.

| Run | Engine                    | Model            | Device    | Wall time | Real-time factor | Per-word confidence |
| --- | ------------------------- | ---------------- | --------- | --------- | ---------------- | ------------------- |
| 1   | openai-whisper 20250625_6 | `base`           | cpu       | 36.08 s   | 1.05x            | yes, `probability`  |
| 2   | whisply 0.14.2            | `large-v3-turbo` | Apple MLX | 32.23 s   | 0.94x            | **no**              |
| 3   | openai-whisper 20250625_6 | `turbo`          | cpu       | 313.60 s  | 9.12x            | yes, `probability`  |
| 4   | minutes 0.26.1            | `ggml-small`     | local     | failed    | n/a              | unknown             |

Run 2's own progress display reported roughly 17 seconds inside the transcription step, so about half of
its wall time is process and model startup. Run 4 failed in 0.04 s with
`Transcription model not found. Expected model file "ggml-small.bin" in /Users/stephen/.minutes/models`,
so **`minutes transcribe` does not work on this machine today**. It needs `minutes setup --model small`,
which is a download and therefore an operator step, not something an overnight agent should run.

### What the engines got wrong

| Span     | Ground truth         | Run 1, `base`        | Runs 2 and 3, `large-v3-turbo` |
| -------- | -------------------- | -------------------- | ------------------------------ |
| name     | Siobhan Kowalczyk    | Shavon Kovalchik     | Siobhan Kovalchuk              |
| name     | Rajesh Muthukrishnan | Rajesh Muthakrishnan | Rajesh Muthakrishnan           |
| name     | Dave Ehrlich         | Dave Erlich          | Dave Ehrlich                   |
| name     | Aoife                | E-Foot               | EFA                            |
| number   | 4173-902             | 4173-902             | 4173-902                       |
| money    | $12,400 and $14,850  | correct              | correct                        |
| date     | the 23rd of October  | 23 October           | the 23rd of October            |
| phone    | 555-0147             | 555-0147             | 555-0147                       |
| spelling | C-Y-C-L-O-T-R-O-N    | CYCLOTRO             | C-Y-C-L-O-T-R-O-N              |

Every error is a personal name, and every number the engines were given came back intact. That is a
sample of one clip and it should not be over-read, but it does match L-R5's own ordering of the risk
classes, and it means the flagging design has to be strongest exactly where the ledger says it should be.

### Three findings that change the design

**Finding 1. Two runtimes of the same model are not redundant.** Runs 2 and 3 produced normalized
transcripts that were byte-for-byte identical: `diff` over the normalized token streams returned nothing
at all. They are the same `large-v3-turbo` weights executed by two different programs, one through
Apple's MLX array framework and one through PyTorch on the central processing unit. They agreed on all
three of their shared errors. So pairing `whisply` with `openai-whisper` looks like redundancy from the
outside, costs twice as much as one engine, and detects nothing. The design has to make this impossible
to configure by accident.

**Finding 2. Word confidence is a weak error signal, even on the strong model.** Run 1's median word
probability was 0.931 and seven of its 67 words scored below 0.5. Its five error words scored between
0.39 and 0.744, but so did correct ordinary words: "The" at 0.237, "Note" at 0.305, "Call" at 0.394 and
"Meeting" at 0.523 all scored below the worst error. On run 3, the stronger model, the bottom eight words
by probability contain three correct common words ahead of two of its own three errors, and its third
error, "Muthakrishnan", scored above 0.854 and is not in the bottom eight at all. A confidence threshold
tuned to catch that error would flag most of the transcript.

**Finding 3. The one error both engines shared is invisible to both signals.** "Muthakrishnan" for
Muthukrishnan was produced by every engine that ran, so disagreement does not see it, and it scored high
enough that confidence does not see it either. This is the ledger's own sentence, "Multiple engines
agreeing does not prove correctness", reproduced as a measurement on the first clip anyone tried. A
design that surfaces only disagreements and only low confidence would have shipped that name into a
meeting note as a confirmed fact.

### What each candidate engine can actually report

Verified by reading the installed source or the vendor's reference, not from memory.

| Engine                            | Confidence available                                                                     | How verified                                        |
| --------------------------------- | ---------------------------------------------------------------------------------------- | --------------------------------------------------- |
| openai-whisper, JSON output       | per-word `probability`, per-segment `avg_logprob`, `no_speech_prob`, `compression_ratio` | run and inspected                                   |
| whisply, `--device cpu`           | per-word `score`                                                                         | `transcription.py` lines 888 and 941, read, not run |
| whisply, `--device mlx` or `auto` | **none**, words carry text and timings only                                              | run and inspected                                   |
| ElevenLabs Scribe v2              | per-word `logprob`, documented as [-inf, 0]                                              | vendor API reference                                |
| Apple SpeechAnalyzer              | unknown                                                                                  | not probed; capability probe only                   |
| `minutes transcribe --json`       | unknown                                                                                  | could not run, model absent                         |

The `--device auto` detail is a trap worth naming: `little_helper.get_device` resolves `auto` to `mlx`
whenever Apple's Metal Performance Shaders backend is available, which it is on this machine. So the
default invocation of the fastest local engine is also the one that reports no confidence at all.

### What else is on this machine

- **Apple's SpeechAnalyzer is available and ready.** `minutes apple-speech capabilities` reports macOS
  26.2 build 25C56, runtime supported, `SpeechTranscriber available: true`, asset status `supported`, and
  nine installed English locales including `en_US`. (Stale, corrected 2026-09-15: this machine is now on
  macOS 27.0, `sw_vers` checked today; the finding itself does not depend on the build number and stands
  unchanged.) It is a different model family from Whisper, it is free, and it runs on device. Reaching it
  needs a small Swift helper, because the framework has no command-line interface; `minutes` builds
  exactly such a helper and keeps it at `~/.minutes/bin/apple-speech-helper` beside its source. That is
  `minutes`' private helper and R3 means vpt never calls it; it does establish that the approach works
  here.
- **Local model weights are already cached**, so no local engine needs a download: 794 MB under
  `~/.cache/whisper` (`base.pt`, `large-v3-turbo.pt`) and Systran faster-whisper plus MLX community
  Whisper repositories under `~/.cache/huggingface`.
- **An ElevenLabs key already exists in the vault.** `private_dot_hermes/private_dot_env.tmpl` line 23
  renders `ELEVENLABS_API_KEY` from the KeePassXC entry "ElevenLabs :: API Key". A cloud engine therefore
  needs no new account, only a decision and a second render target.
- **ffmpeg 9.0.1 with libopus is installed**, which is what makes the transcode numbers above real.

### The pns contract, reverified

L-R5 asks for the protocol to be reverified at implementation time. Read today in
`pns/crates/pns-protocol/src/request.rs` and `pns/crates/pns/src/submit.rs`:

- The envelope is `pns.request` major 1. `Signal` has a `NeedsAttention` variant spelled
  `{"kind": "needs_attention"}`. `producer` is a required field.
- `submit` reads one request from standard input, writes one result envelope to standard output, and
  exits 0 on `Accepted` or `Degraded` and 2 on `Rejected`.
- Unknown top-level keys are ignored and named back in `diagnostics`, never refused.
- `extensions` is a free map documented as "Producer-specific data, carried verbatim and never read
  here". That is where structured review data belongs.
- The bounds in `pns-protocol/src/bounds.rs` are hard: 65,536 bytes per envelope, 64 fields, 8,000
  characters per text value, 64 items per array, depth 8.

One live hazard, measured by the sibling alert-metadata design on the same day: **the hermes gateway
declares exactly three webhook routes, `priority`, `pns` and `unattended-upgrades`.** posture names a
`posture` route that does not exist, every one of its Discord legs answers 404, and eight of them are
dead-lettered right now. A vpt that names a `vpt` route before the gateway has one would reproduce that
failure exactly: banner delivered, durable record silently lost. vpt names an existing route until the
operator adds one.

## Approaches

### A. Two engines, compare, surface every difference

The literal reading of the bullet. Run two engines, diff the transcripts, and put every differing span in
front of the operator.

Good: it is exactly what was asked for, and it is the smallest amount of machinery.

Bad: Finding 3 says it misses the shared error, which is the error class that does the most damage
because it reaches a note as a confirmed fact. And its false-alarm rate is set by the weaker engine: run
1 against run 2 produced six spans on a 34 second clip, one of which ("23 October" against "the 23rd of
October") is a formatting difference with no semantic content. Scaled to the mean 10-minute recording
that is roughly 100 spans, most of them noise from the weaker model.

### B. One engine, confidence plus risk classes, no second engine

Drop redundancy. Transcribe once with the best available engine, flag every word whose confidence falls
below a floor, and additionally flag every span that belongs to a risk class (personal name, number,
date, time, money amount, spelled-out sequence) whether or not anything looks wrong.

Good: half the compute or half the money. The risk-class rule catches the shared-error case, because it
does not care whether anything disagreed.

Bad: it is not what the operator asked for, and Finding 2 shows the confidence half contributes little.
It also gives up the one genuinely independent signal available: a second model's opinion is evidence in
a way that a single model's self-reported probability is not.

### C. Two engines from different model families, three signals, one review list

A and B together, with the engine-family rule from Finding 1 made a hard constraint. Three signals feed
one ranked review list per recording:

1. **Disagreement** between two engines of different families, after normalization.
1. **Low confidence** from whichever engine reports it.
1. **Risk class**, applied to agreed text as well, and explicitly labelled as unverified rather than as
   an error.

The third signal is what makes the list honest, and a known-terms file is what keeps it short: confirming
a name once suppresses it forever.

Good: it is the only one of the three that would have caught all three of the measured errors. It keeps
the operator's requirement intact and adds the signal the measurement says is missing.

Bad: more moving parts than A, and the risk-class signal is only tolerable because of the suppression
list, which is state that has to be maintained.

### Recommendation

**C.** A and B each miss a class of error that the measurement produced on the first try. C's extra cost
over A is one classifier and one plain text file. The engine-family rule is not an optimization in C; it
is the thing that stops A from being an expensive no-op.

## The recommended design

### The boundary

Two commands, each running to completion and exiting.

```
vpt transcribe <recording-id> [--engines a,b] [--dry-run]
vpt verify-note <recording-id> [--note <path>]
```

`vpt transcribe` takes a recording that `vpt ingest` has already cloned, runs the configured engines,
compares them, classifies the flags, writes the transcript and the review record, and notifies. It does
not summarize and it does not write a note. `vpt verify-note` takes a note that something else generated
and checks its claims against the transcript and the review record.

They are separate because the note generator is a later ledger bullet and may turn out to be
`minutes process`, an agent, or vpt itself. Every one of those can be checked by the same command as long
as the note follows the source-reference convention below.

The internal seam is the one the other four Rust tools already use: a domain crate that knows
normalization, alignment, classification and grounding and touches no input or output, and an adapters
crate that owns process spawning, the network, the filesystem and the pns call. Every behavior listed
later is a domain behavior, which is what lets them be tested with two fixture transcripts and no engine.

Under R2 the domain crate splits along its own stages: `normalize`, `align`, `classify`, `grounding`,
`review`. Each engine is one adapter file.

### Engines are spawned, never linked

An engine adapter turns a path into one `Transcript`:

```rust
struct Transcript {
    engine: EngineId,          // tool name, tool version, model identifier, model family
    language: String,
    segments: Vec<Segment>,    // start, end, text, words
}
struct Word { text: String, start: f64, end: f64, confidence: Option<f32> }
```

`confidence` is an `Option` for one measured reason: whisply's MLX path, which is the fastest local
engine on this Mac and the one a person would reach for first, reports none. A design that made
confidence required would either exclude that engine or invite a fabricated default, and a fabricated
confidence is worse than an absent one.

Every engine is reached by spawning an already-installed binary and parsing its output, or by one HTTPS
request. No transcription library is linked into vpt. That keeps R1 intact, keeps vpt buildable with no
engine present, and makes engine selection a configuration value rather than a compile-time one, which is
precisely what "engine selection is left to the operator's cost decision" requires.

The adapters worth shipping first, each a thin file:

| Adapter      | Invocation                                                               | Family  |
| ------------ | ------------------------------------------------------------------------ | ------- |
| `whisply`    | `whisply run -f <path> -o <dir> -d <device> -m <model> -l en -e json`    | Whisper |
| `whisper`    | `whisper <path> --model <m> --output_format json --word_timestamps True` | Whisper |
| `elevenlabs` | one HTTPS POST to the speech-to-text endpoint with `model_id=scribe_v2`  | Scribe  |
| `minutes`    | `minutes transcribe <path> --json`                                       | Whisper |
| `apple`      | a Swift helper over SpeechAnalyzer, if the operator wants it             | Apple   |

The `family` column is not decoration. It is compared at startup.

### The engine-family rule, which is the whole point of Finding 1

At startup `vpt transcribe` resolves the configured engine pair and refuses to run when both report the
same model family, naming both engines and both families in the refusal. `whisply` with `large-v3-turbo`
and `openai-whisper` with `turbo` are the same family and were measured to produce identical output; a
configuration that pairs them is a mistake, not a preference, and vpt says so instead of burning ten
minutes of compute to produce zero findings.

Family is reported by the adapter, derived from the model identifier, not guessed from the tool name. Two
different Whisper model sizes are the same family and are refused by default, with an explicit
`allow_same_family = true` escape for the operator who wants a big-model-against-small-model pairing
anyway and accepts that the small model's errors dominate the list.

### The pipeline, stage by stage

1. **Prepare the input.** If a cloud engine is configured and a transcode tool is available, produce a 16
   kHz mono 32 kbit/s Opus copy in a private 0700 directory. This is measured at 31x faster than real
   time and 15.6x smaller than the Apple Lossless original. The transcode is for transmission only; the
   archived original is untouched, which is what "preserve original audio format" protects. If no
   transcode tool is present, send the original and treat a decode refusal as a permanent error for that
   engine and file.
1. **Run the engines.** Sequentially by default, because two local engines on one laptop contend for the
   same cores and the workload has no latency requirement. Record wall time per engine.
1. **Normalize** both transcripts into comparison tokens.
1. **Align** the two token streams.
1. **Classify** every differing span, and every agreed span that falls in a risk class.
1. **Write** the transcript of record, the raw engine outputs and the review record.
1. **Notify** through pns, once, if anything needs review.

### Normalization

One normalizer, applied to both sides, whose output is used only for alignment and never written to any
artifact:

1. Unicode normalization form KC, then case folding.
1. Per token, drop every character that is not a letter or a digit. A token that becomes empty is
   dropped, and its index is remembered so the span can point back at the original words.
1. Digit groups lose their separators, so `12,400` and `12 ,400` both become `12400`. Ordinal suffixes
   are stripped, so `23rd` becomes `23`.
1. Nothing else. No stemming, no stopword removal, no spelling correction, and in particular no
   conversion of number words to digits: that needs a language model and would introduce the error class
   this whole design exists to catch.

This exact normalization was run against the measured transcripts. It collapsed `4173 -902` against
`4173-902`, `$12 ,400` against `$12,400` and `p .m.` against `p.m.` to equality, which is the difference
between six real spans and about twenty spans of tokenizer noise. The noise comes from whisply joining
word tokens with spaces, so it is a property of the adapter, not of the audio, and normalizing it away is
correct rather than lossy.

### Alignment

Word-level difference over the normalized streams, using a Myers-style algorithm rather than a full
longest-common-subsequence matrix. The reason is measured: the longest recording in the store is 52.6
minutes, roughly 7,900 words, so a naive matrix is about 62 million cells per recording. Myers runs in
time proportional to the edit distance, and two transcripts of the same audio have a small edit distance,
so it is effectively linear here. If the edit distance is large, that is itself a finding: it means one
engine failed or transcribed something else.

A guard makes that explicit. When the edit distance exceeds `max_divergence_ratio` of the token count,
default 0.35, vpt stops the span-level comparison, records the recording as `divergent`, and raises one
flag for the whole recording rather than thousands. A cloud engine that transcribed only the first thirty
seconds produces exactly this shape, and a thousand-span review list is a worse report of it than one
sentence.

Each surviving difference becomes a span carrying both sides' original words, both sides' time ranges,
and each side's confidence when the engine reported any.

### The flag classes

Every flag gets exactly one class, decided in this order. The order matters because the earlier classes
are the ones the operator should look at first.

1. **`formatting`.** One side's tokens are the other's with insertions drawn only from a small closed
   list (`the`, `of`, `a`, `an`, `and`, `to`, `at`, `is`), or the difference is an ordinal suffix that
   normalization already removed. Recorded, never surfaced, never notified. The measured `23 October`
   against `the 23rd of October` is this class, and it was one of six spans on a 34 second clip, so
   suppressing it is worth doing.
1. **`numeric`.** Either side contains a digit run and the digit runs differ. Always surfaced, always
   first. A wrong number in a note is the failure with the least chance of being noticed later.
1. **`proper-noun`.** Either side's span is capitalized in the original text and is not sentence-initial.
   Surfaced second. This is where every measured error landed.
1. **`low-confidence`.** No disagreement, but the transcript of record's words fall below
   `confidence_floor`. Surfaced, ranked below the two above. Finding 2 is why it is not ranked higher and
   why the default floor is deliberately low, 0.35 rather than the tempting 0.5: at 0.5 the measured run
   1 would have flagged "The", "Note" and "Call" while still missing "Muthakrishnan".
1. **`agreed-unverified`.** No disagreement and no low confidence, but the span is a proper noun, a
   number, a date, a time or a money amount. Surfaced last, under a heading that states plainly that
   agreement is not evidence. This is the class that contains "Muthakrishnan", and it is the only reason
   that error is visible at all.
1. **`other`.** A plain lexical disagreement in ordinary words. Surfaced last with `agreed-unverified`.

### Why class 5 is tolerable: the known-terms file

Left alone, `agreed-unverified` is too large. On the measured clip, personal names, numbers and dates
account for about nine of 67 words, roughly 13 percent; on a 1,500-word recording that is two hundred
tokens, which nobody reviews.

Two rules make it usable, and neither needs a model:

- **Aggregate by surface form.** The review record lists each distinct string once with its occurrence
  count and its first timecode, not once per occurrence. "Rajesh" appearing five times is one row.
- **Suppress confirmed terms permanently.** `~/.config/vpt/known-terms.txt` is a plain list of strings,
  one per line, that the operator has already confirmed. A term on that list never produces an
  `agreed-unverified` flag again. It still produces a `numeric` or `proper-noun` flag if an engine
  disagrees about it, because that is new information.

So the review list shrinks as the operator uses it, and it converges on the operator's actual vocabulary
of names and terms. `minutes` has a `vocabulary` command doing the same thing for the same reason, which
is corroboration that this is the right shape rather than an invention.

Deliberately not designed: automatic correction. A confirmed mapping from "Muthakrishnan" to
"Muthukrishnan" could rewrite future transcripts, and that is a tempting feature that changes the
transcript of record without a human in the loop. It is an open question, not a default.

### The transcript of record, and where alternatives live

One engine is the **transcript of record** and the other is the **auditor**. The role is configuration,
not a runtime quality judgement, because a runtime judgement would need a third opinion to make.

The record engine's text is what gets written as prose. The auditor's differing text is preserved as the
alternative on each flag. Both engines' complete raw outputs are kept verbatim, because they are the
evidence behind every flag and a flag with no reproducible evidence is an assertion.

```
agent-processing-pipeline/                       (the vault, per the discovery design)
  raw/audio/<id>.m4a                             the clone, gitignored
  transcripts/<id>.md                            the transcript of record, with a Review section
  analysis/<id>.md                               the note, written later by something else

~/.local/state/vpt/                              vpt's own state, mode 0700
  recordings/<id>.json                           the ingest sidecar, from the discovery design
  transcripts/<id>.<engine>.json                 each engine's raw output, verbatim, mode 0600
  review/<id>.json                               the flag list and its review state
```

The split is deliberate. The vault is a human's notes directory and holds what a human reads; the raw
engine outputs are large, machine-shaped and would clutter it, but they must be keepable, so they live in
state. The review record lives in state too because L-R5 says review state is kept in vpt.

`transcripts/<id>.md` follows the vault's own conventions, which the vault `CLAUDE.md` defines:
frontmatter properties in the documented order, a first-level heading matching the filename, wiki links
rather than Markdown links, and no colon in the filename. The discovery design's identity format already
satisfies the colon rule.

```markdown
---
reference: "[[2026-08-24T144736-4f3ab19c02de.m4a]]"
hub: "[[transcripts]]"
status: active
description: Transcript of a voice memo recorded 2026-08-24, 6 spans awaiting review
startDate: 2026-08-24
---

# 2026-08-24T144736-4f3ab19c02de

## Transcript

[00:00] ...the transcript of record, one paragraph per segment, each prefixed with its timecode...

## Review

> [!warning] 6 spans need review, 11 agreed spans unverified
> - `numeric` [04:12] record: "4173-902" | auditor: "4173-9002"
> - `proper-noun` [00:07] record: "Siobhan Kovalchuk" | auditor: "Shavon Kovalchik"
> - `agreed-unverified` [00:09] "Muthakrishnan", 5 occurrences, both engines agree
```

The timecode in square brackets is the source reference, and it is the same convention the note uses
below. It is readable outside Obsidian, trivially machine-checkable, and it tells a person where to
listen without being a clickable play affordance, which is the feature the operator rejected.

### Checking notes against the transcript

The ledger asks for unsupported notes to be flagged, and L-R5 phrases it as "check generated notes
against the transcript". This design defines the contract and the check. It does not define the
generator, which is a later bullet.

**The contract.** A note is Markdown. Every claim-bearing line, which means every list item and every
sentence in a summary paragraph, ends with one or more timecode ranges in square brackets, for example
`[12:04-12:19]`. A line with no timecode is making an unsourced claim.

**The check, `vpt verify-note`.** Four rules, in order, each producing its own flag class:

1. **`unsourced`.** A claim line carries no timecode range.
1. **`bad-reference`.** A timecode range is empty, inverted, or falls outside the transcript's own time
   bounds.
1. **`unsupported`.** The claim's numbers, money amounts, dates and proper nouns do not appear in the
   cited span, or within `grounding_window_secs` of it, default 15. This check is deliberately narrow: it
   verifies the classes of token that can be checked by string comparison, and it does not attempt to
   verify paraphrase. Verifying paraphrase needs a language model, and a language model checking a
   language model's summary would reintroduce exactly the error class this design exists to catch.
1. **`built-on-flagged-text`.** The claim cites a span that the disagreement pass already flagged. This
   is the cross-link between the two halves of the task and it is the one that matters most: a commitment
   attributed to a person whose name both engines got wrong is precisely the thing the neighboring ledger
   bullet means when it says "do not turn uncertain notes into confirmed commitments".

`verify-note` annotates. It never refuses to write a note, never edits one, and never gates publication.
Its output is appended to the note's own review callout and merged into the recording's review record.

### The pns notification

One submission per recording, after the comparison and the note check, and only when something is open.

```json
{
  "schema": "pns.request/1",
  "request_id": "<uuid v4>",
  "producer": "vpt",
  "event": "transcript_review_needed",
  "signal": {
    "kind": "needs_attention"
  },
  "occurred_at": 1757808000,
  "detail": "<id>: 6 spans to review, 11 agreed spans unverified",
  "context": {
    "project": "vpt"
  },
  "scope": "automatic",
  "route": "pns",
  "extensions": {
    "recording_id": "2026-08-24T144736-4f3ab19c02de",
    "review_path": "~/.local/state/vpt/review/<id>.json",
    "transcript_path": "<vault>/agent-processing-pipeline/transcripts/<id>.md",
    "counts": {
      "numeric": 3,
      "proper_noun": 2,
      "low_confidence": 4,
      "unsupported_claim": 1,
      "agreed_unverified": 11
    },
    "engines": [
      "whisply:large-v3-turbo",
      "elevenlabs:scribe_v2"
    ]
  }
}
```

Four rules govern it.

**No transcript text ever rides in the request.** Not the flagged span, not the alternative, not the
recording's title. The reason is structural: a flagged span is by definition a personal name, a number or
a date, which makes it the most sensitive fragment of the transcript, and pns delivers to Discord and to
a phone. The discovery design already set the same rule for titles. The notification says how many and
where; the operator opens the file.

**The route must exist on the gateway.** `pns` until the operator adds a `vpt` route, for the reason
measured above: posture's non-existent route is silently dead-lettering security pages right now.

**The bounds are respected with room to spare.** The example is a few hundred bytes against a 65,536 byte
cap, its deepest nesting is three against a cap of eight, and no array approaches 64 items. The flag list
itself never rides along, which is what keeps that true for a recording with two hundred flags.

**A backlog does not stampede.** A first run over the 28 recordings already on this Mac would otherwise
fire 28 notifications. When one run produces more than `max_notifications_per_run` recordings needing
review, default 3, vpt sends one aggregate request naming the count and the review directory instead of
one per recording.

If `pns` is absent or fails, vpt writes the same record to its log and exits non-zero. It never loses the
review record because a notifier is missing, and it never skips writing the transcript because a
notification failed.

### Review state

Kept in vpt, per L-R5, in `~/.local/state/vpt/review/<id>.json`. Each flag has an identifier stable
across re-runs, derived from its class and its time range, and a state: `open`, `confirmed` (the
transcript of record is right), `corrected` (with the operator's text), or `dismissed`.

The command surface is deliberately small:

```
vpt review <recording-id>                          print the open flags, ranked
vpt review <recording-id> --resolve <flag-id> --confirm | --correct <text> | --dismiss
```

`--confirm` on a `proper-noun` or `agreed-unverified` flag offers to append the term to
`known-terms.txt`, which is the mechanism that makes the list shrink. A resolution never edits the
transcript of record; `--correct` records the operator's text alongside the flag, and whether a later
stage applies it is the open question named above.

Re-running `vpt transcribe` on a recording preserves existing resolutions by flag identifier and adds
only new flags. That is what makes a re-run after an engine change safe.

### Configuration

One file, `~/.config/vpt/config.toml`, extending the discovery design's file. Following the repository's
ruling that defaulted keys ship uncommented at their default, so the shipped file shows the real posture:

```toml
[transcribe]
# The transcript of record. Its text is what gets written as prose.
record_engine = "whisply"
# The auditor. Its differing text is preserved as the alternative on each flag.
audit_engine = ""
# Refuse to run two engines of the same model family. Two runtimes of one model
# were measured to produce identical output, so pairing them detects nothing.
allow_same_family = false
# Word confidence below this is flagged, from engines that report confidence at
# all. Deliberately low: a higher floor flags ordinary words and still misses
# confident errors.
confidence_floor = 0.35
# Stop span comparison and raise one whole-recording flag past this edit-distance
# ratio. A truncated cloud transcript looks like this.
max_divergence_ratio = 0.35

[transcribe.whisply]
device = "mlx"
model = "large-v3-turbo"

[transcribe.elevenlabs]
model_id = "scribe_v2"
# Transcode to 16 kHz mono Opus before upload. Measured 15.6x smaller than the
# Apple Lossless original; the archived original is never modified.
transcode = true

[review]
known_terms_path = "~/.config/vpt/known-terms.txt"
grounding_window_secs = 15
# Aggregate into one notification past this many recordings in a single run.
max_notifications_per_run = 3

[notify]
producer = "vpt"
# Must name a route the hermes gateway actually declares.
route = "pns"
```

`audit_engine` ships **empty**, and an empty auditor means vpt transcribes once and runs the confidence
and risk-class signals only. That is the shape of Open Question 8 as a default: nothing leaves the
machine and nothing costs money until the operator names a second engine. It is not a recommendation that
redundancy be skipped; it is a refusal to make a cost decision on the operator's behalf by shipping one.

If the operator chooses the cloud engine, the API key is rendered from KeePassXC into this file the way
the other fifteen vault-backed targets are, which makes `~/.config/vpt/config.toml` the sixteenth and
means an apply needs KeePassXC unlocked. That is a real cost and it should be weighed against reading the
key from the environment instead.

### Failure modes

| Condition                                           | What vpt does                                           | Notification                           |
| --------------------------------------------------- | ------------------------------------------------------- | -------------------------------------- |
| Configured engine binary absent                     | refuse at startup, naming the engine and the config key | page once                              |
| Both engines report the same model family           | refuse at startup, naming both families                 | page once                              |
| Auditor engine fails, record engine succeeds        | write the transcript, flag it `single-engine`           | page, this is a degraded transcript    |
| Record engine fails                                 | defer the recording, retry next run                     | page after `deferral_page_threshold`   |
| An engine returns an empty transcript               | treat as failure, never write an empty transcript       | page, silence is the dangerous outcome |
| Cloud engine answers 401, 403, 404, 410 or 422      | permanent, stop using that engine, keep the local one   | page once, name the status             |
| Cloud engine answers 429 or 5xx, or does not answer | temporary, retry with backoff, defer the recording      | page after the deferral threshold      |
| Transcript duration disagrees with audio duration   | flag `truncated-transcript`, keep both outputs          | page                                   |
| Edit distance above `max_divergence_ratio`          | one whole-recording flag, no span list                  | page                                   |
| Normalization yields zero tokens on one side        | treat as an engine failure                              | as engine failure                      |
| Transcode tool absent with a cloud engine set       | send the original; a decode refusal is permanent        | page on the refusal only               |
| `pns` absent or failing                             | log the record, exit non-zero, keep every artifact      | none possible, by definition           |
| More than `max_notifications_per_run` recordings    | one aggregate notification                              | one page for the batch                 |

The permanent-versus-temporary split mirrors the one the sibling pns delivery design wrote for the same
reason: a refusal does not heal with time, and retrying it twenty times converts a typo into a week of
silence.

### Security and privacy

Voice memos are everyday personal audio and a transcript is more searchable than the audio it came from.

- **Local by default.** `audit_engine` ships empty and no cloud engine is configured. Audio leaves this
  machine only after the operator names one, and the configuration key that does it is explicit.
- **Only a transcode leaves, and only to the named engine.** The uploaded copy is a 16 kHz mono Opus
  derivative written into a 0700 directory and removed after the request, including on the error path.
  The archived original never moves.
- **Transcripts are as sensitive as the audio.** `~/.local/state/vpt/` is mode 0700 and every file in it
  is 0600. The vault transcript inherits the vault's own protections, and unlike the audio it **is**
  committed, because `.gitignore` excludes audio extensions and not Markdown. That means transcripts
  reach the vault's git history and therefore Obsidian's mobile sync. That is probably what the operator
  wants, since reading notes on a phone is the point, but it is a consequence worth stating rather than
  discovering.
- **Nothing sensitive is logged or notified.** No flagged span, no alternative, no title, no transcript
  excerpt appears in a log line or a pns request. Identities, counts, classes and paths only.
- **The cloud engine's retention is the operator's question, not vpt's.** vpt cannot promise what a
  vendor does with an upload. If that matters, the answer is the local pairing, and the design supports
  it.
- **Apple's container is never written.** Inherited from the discovery design and unchanged: this stage
  reads the clone in the vault, not the source.

### The behaviors to drive the implementation, test-first

Each is one failing test before one piece of code, in the repository's own style where a unit is a
behavior rather than a task. None needs an engine, a network or a recording: each takes two fixture
transcript structures.

1. Two transcripts that differ only by punctuation and token spacing produce zero spans after
   normalization.
1. `12,400` on one side and `12 ,400` on the other produce zero spans.
1. `23 October` against `the 23rd of October` produces one `formatting` flag, and `formatting` flags are
   excluded from the surfaced list and from the notification counts.
1. A differing span containing a digit run on either side is classed `numeric` and ranks above every
   other class.
1. A differing span that is capitalized mid-sentence on either side is classed `proper-noun`.
1. A word below `confidence_floor` in the transcript of record, with no disagreement, produces one
   `low-confidence` flag; the same word above the floor produces none.
1. An agreed proper noun that is not in `known-terms.txt` produces one `agreed-unverified` flag carrying
   its occurrence count and its first timecode, not one flag per occurrence.
1. The same proper noun, once present in `known-terms.txt`, produces no `agreed-unverified` flag, and
   still produces a `proper-noun` flag when the engines disagree about it.
1. Two engines whose adapters report the same model family cause a startup refusal that names both
   engines and the family, and no engine is spawned.
1. The same pair with `allow_same_family = true` runs.
1. An auditor transcript whose edit distance exceeds `max_divergence_ratio` produces exactly one
   whole-recording `divergent` flag and no span flags.
1. An auditor that fails while the record engine succeeds yields a written transcript carrying a
   `single-engine` flag, not a missing transcript.
1. An engine returning an empty transcript is a failure: nothing is written and the recording is
   deferred.
1. A note claim line with no timecode range yields `unsourced`.
1. A note claim citing a timecode outside the transcript's bounds yields `bad-reference`.
1. A note claim whose number does not appear within `grounding_window_secs` of its cited span yields
   `unsupported`; the same number inside the window yields nothing.
1. A note claim citing a span that already carries a `numeric` or `proper-noun` flag yields
   `built-on-flagged-text`.
1. The pns request contains no flagged text, no alternative and no title, and its encoded size, field
   count, array lengths and nesting depth are all inside the protocol bounds.
1. A run with more open recordings than `max_notifications_per_run` produces exactly one request.
1. Re-running the comparison on a recording preserves existing flag resolutions by identifier and adds
   only new flags.
1. The clone under `raw/audio/` has the same size, modification time and digest after a full run.

Behavior 9 is the one that pins Finding 1, behavior 7 the one that pins Finding 3, and behavior 21 the
one that pins the preservation constraint. Those three are worth writing first.

## Out of scope

Deliberately not designed here, and not to be smuggled in during implementation.

- **Which engines are chosen.** That is Open Question 8 and it is the operator's. This document prices
  the options and makes the choice a configuration value.
- **Note generation, summaries, tags, relationships, meeting briefs and redacted drafts.** Later bullets.
  `verify-note` checks a note; it does not write one.
- **Automatic correction of a confirmed term in future transcripts.** Named as an open question, not
  built. Rewriting the transcript of record without a human is a different feature with a different risk.
- **Speaker labels and diarization.** Listed in the ledger as unapproved candidates. Every engine here
  can do it and none of them is asked to.
- **A paraphrase or entailment check on note claims.** It needs a language model, and a language model
  auditing a language model's summary reintroduces the error class the design exists to catch.
- **The `minutes` disposition.** Untouched. `minutes transcribe` is one adapter among five and it is not
  privileged.
- **Retention of transcripts and raw engine outputs.** Retention is explicitly an operator choice made
  before implementation, and `minutes`' own 30-day delete-candidate classification is a reminder that a
  default here can quietly destroy originals.
- **Any change to the discovery design's boundary.** This stage consumes ingested recordings; it does not
  reach into Apple's container.
- **Bob, Forzare and Open Notebook.** Consumers, later.

## Assumptions made in the operator's place

Each of these was a choice this document had to make to be written at all. None is a decision, each names
its alternative, and reversing any of them changes the design without invalidating the measurements.

1. **Redundancy is kept, and a third signal is added rather than substituted.** The measurement shows
   redundancy alone misses the shared error. Alternative: deliver exactly approach A, two engines and a
   diff, and accept that the shared-error class ships silently. Rejected because the ledger's own
   sentence about agreement not proving correctness has to mean something in the design, and a class-5
   flag is what it means.
1. **The engine pair must come from different model families, and vpt refuses otherwise.** Alternative: a
   warning rather than a refusal, or no check at all. Taken as a refusal because the measured failure is
   silent: identical output looks like a clean recording, so a warning would be read as "nothing to
   review" rather than "this configuration detects nothing".
1. **`audit_engine` ships empty, so the default configuration runs one engine.** Alternative: ship a
   default pairing, which would either spend the operator's money or spend ten minutes of laptop compute
   per recording without being asked. Taken because Open Question 8 is explicitly the operator's and a
   shipped default would answer it by accident.
1. **`confidence_floor` defaults to 0.35 rather than 0.5.** Alternative: 0.5, the intuitive threshold.
   Rejected on the measurement: at 0.5 the weaker engine would have flagged "The", "Note" and "Call" as
   uncertain while still missing "Muthakrishnan". A low floor makes the signal quiet and honest rather
   than loud and wrong.
1. **Timecode ranges in square brackets are the note's source-reference convention.** Alternative:
   Obsidian block references, which are clickable inside Obsidian and meaningless outside it, or
   frontmatter tables, which separate the claim from its source. Timecodes were taken because they are
   readable everywhere, machine-checkable with one comparison, and because they tell a person where to
   listen without being the click-to-play affordance the operator rejected.
1. **The transcript is written to the vault and therefore committed and synced to mobile.** Alternative:
   keep transcripts in state too and put only the note in the vault. Taken because the vault layout the
   ledger names has a `transcripts/` directory and because reading them on a phone is plausibly the
   point, but the privacy consequence is stated rather than assumed away.
1. **Raw engine outputs are kept forever, in state, outside the vault.** Alternative: delete them after
   the comparison, since the flags carry the alternatives. Rejected because a flag whose evidence has
   been deleted is an assertion, and because re-running a comparison after an engine change needs them.
1. **The review list aggregates `agreed-unverified` by surface form and suppresses confirmed terms.**
   Alternative: list every occurrence, which is honest and unusable. The suppression list is the part
   that makes the honest signal survivable.
1. **A cloud engine receives a transcode, not the original.** Alternative: upload the Apple Lossless
   file, which is 15.6 times larger and gains nothing, since every one of these models resamples to 16
   kHz anyway. Taken on the measurement, and it does not touch the archived original.
1. **Engines run sequentially.** Alternative: in parallel, which halves wall time and doubles core
   contention on a laptop that may be on battery. Taken because the workload has no latency requirement;
   it is a one-value change.
1. **One notification per recording, aggregating past a threshold.** Alternative: one per flag, which is
   unusable, or one per run always, which buries a single urgent recording in a batch.
1. **`verify-note` annotates and never gates.** Alternative: refuse to publish a note with unsupported
   claims. Rejected because vpt does not own publication and because a check that blocks gets disabled.
1. **No engine was run against a real voice memo, and no cloud engine was called at all.** Alternative:
   transcribe one real recording to measure end to end. Not done because it would have written the
   operator's personal speech into a scratch directory, and because calling ElevenLabs would have spent
   the operator's money while they were asleep. The synthesized clip gives ground truth, which a real
   recording would not.

## Operator steps

Three. None of them is needed before the design can be read, and the first is the one the design is
waiting on.

**1. Answer Open Question 8, now that it is priced.** The choice is between three pairings, and the
numbers are measured rather than estimated:

| Pairing                                           | Cost per 10-minute recording        | Different families   | Confidence |
| ------------------------------------------------- | ----------------------------------- | -------------------- | ---------- |
| whisply MLX `large-v3-turbo` alone, no auditor    | about 5 minutes of laptop compute   | n/a                  | none       |
| whisply MLX plus openai-whisper `base` on the CPU | about 15 minutes of laptop compute  | **no**, both Whisper | partial    |
| whisply MLX plus ElevenLabs Scribe v2             | about 5 minutes plus $0.037         | yes                  | both sides |
| whisply MLX plus Apple SpeechAnalyzer             | about 5 minutes plus on-device time | yes                  | unknown    |

The second row is the trap: it looks like the free local answer and Finding 1 says two Whisper models are
a weak pair, with the small model's errors dominating the list. The whole existing back catalogue of 28
recordings through Scribe v2 costs **$1.01 once**, and the hypothesized three recordings a week costs
roughly **$0.50 a month**. The Apple row is the genuinely free different-family option and its cost is a
Swift helper binary in a Rust project, which is a real engineering cost and an unknown confidence story.

**2. Decide whether `minutes transcribe` is a candidate adapter, and if so run its setup.** It is a local
Whisper, so it is the same family as whisply and cannot be the auditor for it, but it could be the
transcript of record. It does not work today:

```bash
minutes setup --model small
```

**3. Add a `vpt` route to the hermes gateway, or accept that vpt posts on the `pns` route.** The gateway
declares `priority`, `pns` and `unattended-upgrades` today, and posture's pages are being dead-lettered
right now for naming a route that is not there. Whichever way this goes, it should be settled before vpt
sends its first notification rather than after.

## Open questions for the operator

**Triage, 2026-09-15:** every question below is closed. See
`docs/decisions/2026-09-15-vpt-question-triage.md` (rows T1-T7) and
`docs/decisions/2026-09-15-vpt-architecture-decisions.md` for the full reasoning; the short form is in the
per-item notes that follow. The architecture is broader than this design's own single-pair
recommendation: two engine slots, primary and optional fallback, both named in config, with the
fallback's trigger (on failure, or every recording) also configurable (decision 3). Reconciliation picks
a winner and flags uncertainty two ways only, inline markers plus `vpt review <id>`; a summary block and a
separate document were both considered and rejected (decision 4). Three further engine-pair behaviors are
approved for building (transcript-wins rule, both-engines-fail handling defaulting to keep-and-flag,
engine per language), and two adjacent ones are explicitly not built, a disagreement threshold and a
cloud spend ceiling, both because no defensible default exists yet (decision 5).

1. **Which engine pairing, from the priced table above?** This is Open Question 8 and everything else in
   the design is a configuration value once it is answered. **Closed:** no single pair. Primary plus
   optional fallback, both named in config; Apple Speech and whisply ship as the two starting first-class
   adapters, and the wider priced table above stays recorded as candidates reachable through the generic
   command adapter rather than a shipped pair.
1. **Is the Apple SpeechAnalyzer route worth a Swift helper inside a Rust project?** It is the only free
   different-family option on this Mac, it is confirmed available here, and it is the one choice that
   would need a second language in the build. Its confidence reporting is unknown and would need a probe.
   **Closed: yes, approved.** Apple Speech is one of the two starting first-class adapters (decision 10),
   and the confidence reporting is no longer unknown; see the corrected note above this section and
   `docs/decisions/2026-09-15-vpt-question-triage.md`.
1. **May a transcript be committed to the vault, and therefore synced to a phone?** The audio is
   gitignored and the transcript would not be. Everything in this design assumes yes, because that is
   what the vault's `transcripts/` directory is for, but it is the decision that puts searchable text of
   every voice memo into a git history. **Closed:** configurable destination, default outside any
   git-tracked tree; vault sync is the operator's deliberate opt-in.
1. **Should a confirmed correction rewrite future transcripts?** Recording that "Muthakrishnan" should be
   "Muthukrishnan" is cheap. Applying it automatically changes the transcript of record without a human
   reading the result. The design records and does not apply; the alternative is worth a sentence.
   **Confirmed 2026-09-15:** the design's own recommendation stands. Record only; a correction made
   through `vpt review` feeds forward into `vpt confirm --term` and improves future transcripts, never
   rewrites a shipped one (decision 4).
1. **How loud should `agreed-unverified` be?** It is the class that catches the error both engines
   shared, and it is also the largest class. The design ranks it last and aggregates it. Should it appear
   in the pns notification at all, or only in the file? **Closed:** counts only in the `[notify]` event;
   the flagged text stays in the file and the `vpt review` queue.
1. **One notification per recording, or one per run?** The default is per recording with aggregation past
   three. A weekly reviewer might prefer one summary always. **Closed:** per recording, aggregating past
   three, as designed; a config default, not a rule.
1. **Does `verify-note` belong in vpt, or in whatever writes the note?** It is designed here as a
   separate command precisely because the writer is undecided. If the writer turns out to be `minutes`,
   the check still works, but it would then be checking a third-party tool's output against a convention
   that tool does not follow, which needs the convention to be enforced somewhere else. **Closed: in
   vpt.** `minutes` is out entirely (decision 1), so vpt is the only thing that writes the note; the
   undecided writer this question depended on is now settled.
1. **Where does vpt's code live?** The boundaries design recommends its own repository; the discovery
   design assumed a fifth workspace here. Nothing in this document depends on the answer, but the two
   sibling designs should not stay in disagreement. **Closed: own repository.** See
   `docs/superpowers/specs/2026-09-14-vpt-project-boundaries-design.md`'s own open questions, below.
