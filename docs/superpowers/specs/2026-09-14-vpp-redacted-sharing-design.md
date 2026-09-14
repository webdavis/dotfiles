# vpp redacted drafts for sharing, with a review gate before release

Status: design, written 2026-09-14 while the operator was asleep. Not approved, not built. No code was
written or changed, and nothing was shared, sent or published while measuring. Every choice made in the
operator's place is listed under "Assumptions made in the operator's place" with its alternative, and the
questions that need an answer are at the end.

Scope: `docs/remaining-work.md`, the `vpp (Voice Processing Pipeline)` section, sixth bullet.

> Support a separate redacted draft for sharing, reviewed before release, preserving private originals.
> Choose the summary format, retention, transcription engines and local/cloud processing before
> implementation. Speaker labels and dated digests remain unapproved candidates.

The fuller statement of the same feature is homelab `PLAN-v12-experiments-backlog.md`, L-R5, read today
rather than remembered. It is two bullets there, and the second sentence of the first one is the sharpest
requirement in the whole item:

> Produce a separate redacted draft for sharing, with human review before release. Preserve the private
> original; generating a draft does not authorize sending it.

> Choose transcription engines, local/cloud processing, summary format and retention before
> implementation. Speaker labels and dated digests remain candidates.

The done-means for this item, from the triage record: "A design in which no draft can be shared without
review and the private original is never the shared artifact." Both halves are mechanical claims, so this
document's job is to make them properties of the design rather than promises about behavior, and to state
plainly the one thing that cannot be made mechanical.

## What this builds on

This is the sixth document in the vpp chain and it assumes the five before it.

**The boundaries design** put vpp's application code in its own project, macOS installation and service
configuration in dotfiles, and the user's content in a configured output directory. Nothing here changes
that split; a draft is content, its policy is configuration, and the code is vpp's.

**The discovery design** produced the recording identity, `2026-08-24T144736-4f3ab19c02de`, the sidecar
record at `~/.local/state/vpp/recordings/<id>.json`, and the rule that Apple's originals are never
modified.

**The redundant transcription design** produced the transcript of record, the review record at
`~/.local/state/vpp/review/<id>.json` with its flag classes and four resolution states, the in-line
timecode convention `[04:12]`, `known-terms.txt` (the list the operator grows by confirming a term once),
`vpp verify-note`, and the rule that no transcript text ever rides in a notification.

**The tags, schema and filing design** produced the three-layer model (the record is authoritative, the
note is a rendering, the index is a cache rebuilt in 0.245 seconds over 753 files), the managed-marker
block with its refusal rules, the two output profiles (`portable` and `obsidian`), the slug sanitizer
with its path refusals, and `vpp path` as the single implementation of the filing rules.

**The meeting-briefs design** produced the occasion identity, the four-selector selection rule, the
`vpp.brief/1` machine form in which every item carries `certainty` and `sources` with no default, and the
rule that uncertainty is marked in place rather than in a footer, because a consumer that lifts one
bullet drops a footer and keeps the sentence. It also stated, in its own security section, the sentence
this document has to make true: "A brief is not a shareable draft. The redacted-draft bullet is a
separate ledger item with its own human review gate."

Two disagreements inside the chain remain open and are not resolved here: the audio's home, and vpp's
shipping name (`VPP` collides with FD.io's Vector Packet Processing). Neither affects this bullet.

## Constraints this design is bound by

From the two ledger statements:

1. The draft is **separate**. It is a distinct artifact, not a mutated original and not a flag on one.
1. **Human review happens before release**, which means the review is a gate on an act, not a suggestion
   attached to a file.
1. **Private originals are preserved.** Nothing in this feature edits, moves or consumes an original.
1. **Generating a draft does not authorize sending it.** Two separate acts, and the second one needs the
   first plus a review. This forbids the common shape where producing the redacted copy is the release.
1. **Speaker labels and dated digests are unapproved candidates**, so neither is designed here.
1. Summary format, retention, engines and local or cloud processing are the **operator's choices, before
   implementation**. They are restated as open questions below rather than assumed.

From the repository's standing rules, carried over from the earlier documents with the same labels:

- **R1.** No workspace may depend on another, and a tool never assumes this checkout exists.
- **R2.** Rust files target 300 lines and never exceed 500, unit tests included.
- **R3.** Never patch, fork or modify a third-party tool. `minutes`, Obsidian and `gitleaks` are third
  party here; each is configured and called, never modified.
- **R4.** Tests cover the behavior of tools we wrote and nothing else.
- **R5.** The operator runs applies. An agent proposes.
- **R6.** This repository builds no removal mechanisms, and the chain has read that as: vpp deletes
  nothing.

And from the vault's own `CLAUDE.md`, with the labels the previous documents used: **V3** wiki links for
internal references, **V5** no colon in a filename, **V6** a folder note per directory that should
surface in a listing, **V7** Obsidian Git auto-commits the vault on a timer and vpp never runs git.

One more constraint, which is this document's own and is not in any ledger: **a design document about
redaction lands in a public repository.** `gh-axi repo view` reports `visibility: public` for
`webdavis/dotfiles`. Every example below is invented, every measurement is reported as counts and
classes, and no name, number or sentence from the operator's vault is reproduced anywhere in this file.

## What was measured, and how

### How fast a file written into the vault becomes published

Read from `~/workspaces/Ivy/.obsidian/plugins/obsidian-git/data.json`:

| Setting                     | Value  | What it means here                                         |
| --------------------------- | ------ | ---------------------------------------------------------- |
| `autoSaveInterval`          | 10     | a commit every 10 minutes when anything changed             |
| `autoBackupAfterFileChange` | `true` | the timer restarts from a file change, so a write is caught |
| `autoPushInterval`          | 15     | a push to `git@github.com:webdavis/Ivy.git` every 15 minutes |
| `disablePush`               | `false` | pushing is on                                              |
| `pullBeforePush`            | `true` | it fetches first, so the push is unlikely to fail and stop  |

And from the vault's `.gitignore`: audio extensions are excluded (`*.m4a`, `*.wav`, `*.mp3` and five
more), Markdown is not. So a Markdown file written anywhere inside the vault is committed within about
ten minutes and pushed within fifteen, with no further human act.

That single measurement decides the largest structural question in this design. If a draft were built
inside the vault and then corrected, every intermediate version would already be in the repository's
history, and history is not something a later edit repairs. A redaction that happens after the first
write has already failed.

### What a real note in this vault actually contains

Measured over one genuine meeting note in the vault, the kind of artifact a person would most plausibly
want to share a version of. Reported as counts and classes only, for the public-repository reason above.

| Measure                                             | Count |
| --------------------------------------------------- | ----- |
| Words                                               | 566   |
| Unique capitalized tokens, not sentence-initial      | 33    |
| Of those, personal given names                       | 13    |
| Of those, ordinary words, headings and weekday names | 16    |
| Of those, organization or program names              | 4     |
| Email addresses                                      | 0     |
| Digit runs of four or more                           | 3     |
| Currency amounts                                     | 0     |

Three conclusions come straight out of that table.

**A capitalized-token heuristic cannot be a redactor.** Sixteen of the thirty-three are ordinary English
words that happen to be capitalized. Masking them would turn the draft into nonsense, and a redactor that
produces nonsense gets turned off.

**It is an excellent review prompt.** Thirty-three lines for a six-hundred-word note is a review a person
finishes in a minute, and thirteen of those lines are exactly what must not be shared. That is the
difference between "read the whole thing again carefully" and "check this list."

**The names are the payload.** Zero emails, zero currency amounts, three number runs, thirteen people.
Whatever pattern classes a redactor carries, the class that matters in this operator's material is
personal names, which is precisely the class no pattern can find. The chain already has the answer for
that: `known-terms.txt`, the list the operator grows by confirming a term once, built by the
transcription design for exactly this population of errors.

### What an agent shell looks like from inside

Measured inside this session, which is an agent running commands on this machine:

| Signal                       | Observed                          |
| ---------------------------- | --------------------------------- |
| `[ -t 0 ]`                   | false, standard input is not a terminal |
| `CLAUDECODE`                 | `1`                               |
| `AI_AGENT`                   | `claude-code_2-1-270_agent`       |
| `CLAUDE_CODE_SESSION_ID`     | set                               |
| `CLAUDE_CODE_ENTRYPOINT`     | `cli`                             |

This is what an approval gate has to work against, and it is worth being honest about what it proves. An
agent's ordinary command execution has no terminal, so requiring one is a real obstacle rather than a
decorative one. It is not a guarantee: an agent that decided to could allocate a pseudo-terminal. What
the environment markers give is evidence after the fact, not enforcement, and the design uses them that
way.

### Where a released file would land

- `~/Documents` is a local directory on this machine (device 16777229, inode 368989423). It is not the
  same directory as `~/Library/Mobile Documents/com~apple~CloudDocs/Documents` (same device, inode
  27245724, different contents), so the macOS Desktop and Documents iCloud sync is not redirecting it.
- An iCloud Drive container does exist and is active: `brctl status` reports 98 containers and the
  CloudDocs container last synced 2026-09-08.
- The vault is a git working tree that pushes to GitHub on a timer, as measured above.

So there are two categories of destination that quietly publish a file, and they are both present on this
machine: a git working tree with an automatic push, and a cloud-sync root. A release check can test for
both with no network and no configuration: walk up for a `.git` entry, and compare against the known sync
roots.

### What is already installed that could do any of this

- **`minutes` 0.26.1** has no redaction, sharing or publishing command. Its `sensitive` subcommand is a
  capture-time control ("Start or stop a no-capture sensitive meeting"), which prevents a recording
  rather than sanitizing one. Its `export` writes CSV (comma-separated values) of meeting metadata. Its
  `vocabulary` command manages local names and terms, which is the same idea as `known-terms.txt` and is
  named again in the open questions because the keep-or-replace ruling on `minutes` is still open.
- **`gitleaks`** is installed at `/opt/homebrew/bin/gitleaks` and already runs at pre-commit in this
  repository. It detects credentials by pattern. It has nothing to say about a person's name, which is
  the class that matters here, so it is not a candidate for this check. It is worth one sentence only to
  record that it was considered and why it does not fit.
- Nothing else on this machine performs entity detection, and adding a model-based detector would mean
  feeding the whole private transcript to a model, which is an egress decision, not a redaction feature.

## Approaches

Three, and the recommendation is a hybrid of the second and third.

### A. Redact the note in place, in the vault

A `shared` copy beside the original inside the output root, or a frontmatter flag that marks a note as
cleared for sharing. Cheapest to build, and it is what most note systems do.

Rejected on the first measurement. A copy inside the vault is committed within ten minutes and pushed
within fifteen, so the unredacted intermediate state is published before any human looks at it, and a
later correction does not remove it from history. The second problem is subtler and worse: with the
private note and the shareable note in the same tree under the same naming convention, "the private
original is never the shared artifact" becomes a matter of which path somebody typed. The done-means asks
for a structural property, and this shape cannot provide one.

### B. Derive by removal into a staging tree, and gate release on an approval bound to the bytes

vpp reads the source read-only, writes a candidate into its own state tree (mode 0700, outside the vault,
outside git), runs a removal pass, and refuses to produce a released copy until an approval record exists
whose digest matches the candidate's bytes exactly.

This gets both done-means properties structurally. The shared artifact is a different file, in a
different tree, with a different identity, produced by a command that refuses without an approval; and
the approval is bound to bytes, so an edit after review invalidates it rather than riding along.

Its weakness is the removal pass itself: removal can only remove what it can recognize, and the
measurement says the dominant class is personal names, which patterns do not find. A design that stops
here would be quietly trading on a completeness it does not have.

### C. Derive by selection: the draft contains only what was explicitly chosen

Invert the default. Nothing is in the draft unless the operator selected it, span by span, the way the
brief design assembles a pack from selected notes. Nothing unknown can leak, because nothing arrives
without a choice.

The strongest privacy property available, and too expensive to be the only mode: choosing every span of
every draft is the kind of cost that makes a feature go unused, and an unused sharing feature means the
operator shares the private original instead, which is the outcome this item exists to prevent.

### Recommendation

**B for the gate and the boundary, C for the structure of the artifact.** Concretely: everything the
released file may contain is an allowlist (frontmatter keys, sections, links, timecodes, provenance are
all deny-by-default), the text inside the selected sections goes through a removal pass over confirmed
terms and closed pattern classes, and the residue of that pass is checked again at release time. The
human review sits between the two, and it is told exactly what to look at.

The honest sentence, stated once here and repeated in the security section because it is the thing a
future session will be tempted to forget: **the machine's job is not to prove the draft is safe. It is to
make the unreviewed release impossible, to narrow what a human has to look at, and to refuse when its own
work does not check out.** The safety claim rests on the human read, which is what "reviewed before
release" means.

## The recommended design

### The boundary

Four verbs, in order, each of which does exactly one thing. Everything else is reused from the chain.

```
vpp share draft <source> [--sections ...] [--prose -] [--json]
vpp share review <draft-id> [--diff] [--show-values]
vpp share approve <draft-id>
vpp share release <draft-id> --to <path> [--as <name>]
```

What vpp does not do, and this is the load-bearing half: **it never transmits anything.** No mail, no
message, no upload, no clipboard, no `open`. `release` writes a file into a directory the operator
configured, and the act of sending that file is the operator's, performed with their own tools. An egress
channel inside vpp would make the approval gate the only thing standing between an agent with shell
access and a send, and a gate in that position is a single point of failure rather than a boundary.

The internal seam is the chain's: a domain crate holding the policy, the removal pass, the residue scan
and the approval rules, with no input or output; an adapters crate owning the filesystem, the state tree
and the terminal. Under R2 the domain crate splits by stage: `policy`, `redact`, `residue`, `approve`.

### The draft, and why it lives where it lives

```
~/.local/state/vpp/share/<draft-id>/        mode 0700
  draft.md          the candidate bytes, mode 0600
  draft.json        sources, policy, removals with their real values, candidates, digest, mode 0600
  approval.json     written by `vpp share approve`, absent until then
  releases.json     append-only, one entry per released copy: path, digest, time
```

Not in the vault, and not in any git working tree. On the measurement above, a draft in the vault is
published before it is reviewed, which inverts the requirement. Outside the vault it is also outside
Obsidian's mobile sync, which is correct: a half-reviewed draft on a phone is a draft one tap from a
share sheet.

The draft identity is stable, following the chain's rule that re-running a command rewrites one artifact
rather than producing a second:

```
draft id = <source date>-<source slug>-<8 hex characters>
```

The eight hexadecimal characters are the leading digits of a SHA-256 (secure hash algorithm 256-bit)
digest over the source identities plus the policy digest. Same source, same policy, same draft directory,
rewritten. A policy change produces a different directory, which is deliberate: a draft built under an
older policy is not silently upgraded, and its approval does not transfer.

**The source slug appears in the private directory name and never in a released filename.** The release
name comes from `--as <name>` or defaults to the draft's eight hexadecimal characters. A title like
"invoice call with the landlord" is exactly the kind of thing that survives a perfect body redaction by
riding in the filename, and a default that carried the slug would do that every time.

### Deny by default: what a released file may contain

The released file is assembled, not filtered. Four allowlists, and anything not named is absent.

**Frontmatter** carries at most three keys, none of them vpp's:

```markdown
---
title: Project sync, redacted extract
date: 2026-08-24
source: redacted extract, not a verbatim record
---
```

`title` is operator-supplied at draft time or absent. `date` is present only when
`[share].include_date = true`. `source` is a fixed sentence naming the artifact's nature, configurable in
wording and on by default, because a recipient who does not know a document is a redacted derivative will
read it as a verbatim record, and the chain's whole position on uncertainty is that a reader must be able
to tell.

Everything vpp normally writes is absent by construction: `vppRecording`, `vppOccasion`, `vppStage`,
`vppCapturedAt`, `vppDurationSecs`, `vppOpenFlags`, `vppEngines`, `vppSchema`, `tags`,
`vppSuggestedTags`, and every one of the vault's eight keys. Two of those deserve their own sentence.
`vppRecording` embeds the capture date and time to the second, so it is a timestamp disguised as an
identifier. `tags` is a compact statement of what a private recording was about, which the tags design
already called out as sometimes worse than the transcript.

**Sections.** The body is the source's sections minus a fixed deny list minus anything the operator did
not select. The fixed deny list is `Review` (the flag callout), any managed `vpp:` marker block (link
blocks carry paths and note names), and any section the source marked as generated-but-unverified.
`--sections` narrows further; it never widens.

**Links.** Every link target is removed. Link text survives only if it survives the removal pass. A wiki
link to a contact note leaks a name in the target even when the body masked it, and a relative Markdown
link leaks a directory layout.

**Timecodes and provenance.** Removed by default. `[04:12]` is meaningless to a recipient and tells them
a recording exists and roughly how long it ran. The mapping from every released line back to its source
span stays in `draft.json`, so the operator keeps full traceability and the recipient gets none. Set
`keep_timecodes = true` if a recipient is someone the recording is being shared with in full context.

### The removal pass

Deterministic, no model, no network, and the whole of it is expressible as a list.

| Class        | What it matches                                                              | Source of truth        |
| ------------ | ---------------------------------------------------------------------------- | ---------------------- |
| `person`     | a confirmed term from `known-terms.txt`, whole token, case-insensitive        | the operator's own list |
| `note`       | a note name or alias from the output index, whole token                       | the index, rebuilt per run |
| `email`      | an address                                                                    | pattern                |
| `url`        | a link or bare address                                                        | pattern                |
| `phone`      | E.164 and North American shapes                                               | pattern                |
| `number`     | a digit run of four or more, including grouped forms                          | pattern                |
| `money`      | a currency amount                                                             | pattern                |

Four rules govern it:

1. **Placeholders are stable inside one draft and never across drafts.** The first person becomes
   `[person 1]` everywhere in that draft, so the text stays readable and a reader can follow who said
   what without learning who they are. Across drafts the numbering restarts, because a stable global
   pseudonym would let two separately-approved drafts be joined into one picture by a recipient who has
   both. That is a real trade and it is an open question, not a settled preference.
1. **A flagged span is omitted, not masked.** The transcription design's review record says which spans
   two engines disagreed about, and those are by definition the names, numbers and dates the machine got
   wrong. Shipping a wrong name to an outsider is worse than shipping no sentence, so the default is
   `flagged_spans = "omit"`, with the omission counted in the report. `"mark"` keeps the span with its
   `[unverified]` marker attached, and the marker is protected: the removal pass may not strip it and the
   residue scan refuses a release where a kept flagged span lost it.
1. **Case, possessives and hyphenation are handled; fuzz is not.** `[person 1]'s` renders correctly, and
   a near-miss spelling of a confirmed term is not matched. The chain's reason carries over unchanged:
   every measured engine error was a personal name, and a fuzzy matcher applied to names produces
   confident wrong matches. Here the consequence runs the other way from the brief's, and it is worse: a
   fuzzy redactor that silently masks the wrong token teaches the operator to trust a pass that is
   guessing.
1. **The pass is pure.** Given the same source bytes, the same term list, the same index and the same
   policy, it produces the same draft bytes. That is what makes the digest meaningful, and it is one
   test.

### The residue scan and the candidate report, which are the two halves of review

**The residue scan** is the machine checking its own work. Every real value recorded in `draft.json`,
plus every confirmed term in `known-terms.txt`, plus every note name in the index, must not appear in the
draft bytes as a whole-token case-insensitive match. It runs at draft time and **again at release time**,
because the bytes can change between the two and a stale pass is not a check.

Residue is a refusal, never a repair. The repository already uses refuse-not-repair in the osquery
converge, for the same reason: a pass that quietly fixes what it finds hides the fact that its first
attempt was wrong, and here the first attempt being wrong is the operator's signal that the policy needs
work.

**The candidate report** is the machine admitting what it cannot do. It lists every capitalized token in
the draft that is not a masked value, not a sentence opener, not a heading word and not in the vocabulary
of ordinary words. On the measurement above that is on the order of thirty lines for a six-hundred-word
note, about thirteen of which are the names that matter.

The report is never applied automatically. It is printed by `vpp share review`, and confirming one of its
tokens as a term through the chain's existing `vpp confirm --term` both masks it in the next draft and
adds it to `known-terms.txt` forever, so the thirty-line report shrinks with use. That is the same
confirm-once mechanism the transcription, tagging and brief designs all use, and this is the fourth place
it pays for itself.

This is a deliberately naive heuristic with a named ceiling: it finds capitalized tokens, so it does not
find a lowercase surname, a street address written in words, or an identifying detail carried by a
sentence rather than a token ("the guy who broke his wrist at the Christmas party"). Nothing mechanical
finds that last class. The human read is what finds it, and the report exists to leave the human enough
attention to do so.

### Review

```
vpp share review <draft-id> [--diff] [--show-values]
```

Prints, and writes nothing:

1. the draft as it would be released, byte for byte;
1. the removal report: counts per class, each placeholder, how many occurrences, and where the term came
   from. Real values are **not** printed unless `--show-values` is given, so the report can be read with
   somebody looking over a shoulder;
1. the omissions: how many flagged spans were dropped and of which class;
1. the candidate report;
1. the digest, in full and in the short form the approval will ask for.

`--diff` shows the source and the draft side by side. That is the only view where the private original
and the draft are on the screen together, and it is a read-only command in a tool that cannot send
anything.

### Approval, bound to the bytes

```
vpp share approve <draft-id>
```

Five rules:

1. **It refuses when standard input is not an interactive terminal.** Measured above: an agent's ordinary
   command execution has none. There is no `--yes`, no `--force` and no environment variable that
   bypasses it, because every one of those exists to be used by the thing the gate is for.
1. **It requires the operator to type the draft's short digest**, which is printed by `review` and by
   `approve` itself. Typing eight characters that are derived from the content is a small act that cannot
   be performed by a pipe, and it fails after any edit, because the digest changes.
1. **The approval record stores the digest of `draft.md`**, the policy version and digest, the moment,
   and evidence about its environment: whether a terminal was present, the parent process name, and which
   of the agent environment markers measured above were set. The evidence is recorded, not enforced.
   A future session reading an approval taken with `CLAUDECODE=1` in the environment learns something
   true and useful.
1. **Approval does not release anything.** It records that a human read specific bytes. Separating the
   two is the ledger's own "generating a draft does not authorize sending it", one step further along.
1. **Any change invalidates it.** An edited draft, a re-run that produced different bytes, or a policy
   change all produce a digest mismatch, and `release` refuses and says which of the three it was.

### Release

```
vpp share release <draft-id> --to <path> [--as <name>]
```

It refuses, before writing anything, when any of these hold:

| Refusal                                                                  | Why                                             |
| ------------------------------------------------------------------------ | ----------------------------------------------- |
| No approval record                                                       | the gate                                        |
| Approval digest does not match the current `draft.md`                     | the bytes changed after review                  |
| Policy digest in the approval differs from the current policy             | the rules changed after review                  |
| The residue scan does not pass on the current bytes                       | the machine's own check, re-run, never cached   |
| The destination resolves inside a git working tree                        | a push publishes it, on this machine on a timer |
| The destination resolves inside a known cloud-sync root                   | same, without even a commit                     |
| The destination resolves inside the output root, the audio destination, or vpp's state tree | that is the private side of the boundary |
| The destination exists and was not produced by this draft                 | never overwrite somebody else's file            |
| The source is audio, or any non-text artifact                             | a voice is identifying and no redaction removes it |

The git working-tree test is a walk up the destination's resolved parents looking for a `.git` entry, and
the cloud-sync test is a prefix comparison against `~/Library/Mobile Documents` plus any root named in
configuration. Both are local, cheap and testable with a temporary directory. `allow_git_destination` and
`allow_sync_destination` exist and default to `false`; an operator who deliberately publishes into a
repository can say so once, in a file, rather than at the moment of release.

On success it writes the approved bytes, appends an entry to `releases.json`, and prints the path. It
sends nothing, opens nothing and notifies nothing: the operator typed the command and is reading the
output, which is the same rule the brief design applied to a requested brief.

### If the operator wants generated prose

The ledger's "summary format" question may well be answered with "a written summary, not an extract." The
design accommodates that without owning it, using the chain's existing shape for agent-produced content:

```
vpp share draft <source> --prose -
```

The prose arrives on standard input from whatever wrote it, exactly as a tag proposal does. vpp then
applies the same pipeline to it: the removal pass, the residue scan, the candidate report, the review,
the byte-bound approval. In addition it runs `vpp verify-note`, which the transcription design already
built, so a generated sentence whose numbers, dates and proper nouns do not appear in the cited span is
flagged before a human reads it.

One property degrades and it must be said plainly: **a paraphrase can reintroduce a redacted fact in
words the removal pass never saw.** "The landlord" is not a masked term. No mechanical check catches
that, `verify-note` included, since the paraphrase is faithful to the source. For generated prose, the
human read is not one of several protections, it is the only one, and the review output says so on the
screen.

### Configuration

Extending the file the earlier designs define, and following the repository's ruling that defaulted keys
ship uncommented at their default so the shipped file shows the real posture:

```toml
[share]
# Where `vpp share release` may write. Must not be inside a git working tree or a
# cloud-sync root; both are refused unless explicitly allowed below.
release_dir = "~/Documents/vpp-shared"
allow_git_destination = false
allow_sync_destination = false
# Extra sync roots to refuse, beyond ~/Library/Mobile Documents.
sync_roots = []
# The one line that tells a recipient what they are holding. Empty disables it.
source_line = "redacted extract, not a verbatim record"
# Put the source's capture date in the released frontmatter.
include_date = false
# Keep [mm:ss] source references in the released text. They mean nothing to a
# recipient and reveal that a recording exists.
keep_timecodes = false

[share.redact]
# Pattern classes applied to the selected text. Confirmed terms and note names
# are always masked and are not a class you can turn off.
classes = ["email", "url", "phone", "number", "money"]
# Rendered in place of a removed value. {class} and {n} are substituted.
placeholder = "[{class} {n}]"
# What to do with a span the transcription review flagged: "omit" drops it and
# counts it, "mark" keeps it with its [unverified] marker, which cannot be
# stripped.
flagged_spans = "omit"
# Sections never copied into a draft, in addition to every vpp: marker block.
deny_sections = ["Review"]
```

Nothing here holds a secret, so `~/.config/vpp/config.toml` does not become a KeePassXC-backed target on
account of this feature. Whether it becomes one for the cloud transcription engine is the transcription
design's open question and is unchanged.

### Failure modes

| Condition                                                    | What vpp does                                        | Notification |
| ------------------------------------------------------------ | ---------------------------------------------------- | ------------ |
| Source artifact missing or unreadable                        | refuse, name the path                                 | none, exit 2 |
| Source is audio or a non-text artifact                       | refuse, name the kind                                 | none, exit 2 |
| `--sections` names a section the source does not have        | refuse, name it, list the sections that exist         | none, exit 2 |
| Source has managed markers that are missing, doubled or unbalanced | refuse to read it, name the path                | page         |
| `known-terms.txt` missing or empty                           | build the draft, and say in the report that the person class matched nothing | none, log |
| Residue scan fails at draft time                             | write the draft, mark it unreleasable, name every residual placeholder | none, exit 3 |
| Residue scan fails at release time                           | refuse, name every residual placeholder               | page         |
| `approve` with no interactive terminal                       | refuse, naming the requirement                        | none, exit 2 |
| `approve` with a mistyped digest                             | refuse, no record written                             | none, exit 2 |
| `release` with no approval                                   | refuse, print the approve command                     | none, exit 2 |
| `release` when the digest does not match                     | refuse, say the bytes changed since the approval      | none, exit 2 |
| `release` when the policy digest does not match              | refuse, say the policy changed since the approval     | none, exit 2 |
| Destination inside a git working tree or a sync root         | refuse, name the root that matched                    | none, exit 2 |
| Destination inside the output root, audio destination or state tree | refuse, name which                             | page         |
| Destination exists and was not produced by this draft        | refuse, name both                                     | none, exit 2 |
| A re-run produces different bytes for an approved draft      | rewrite the draft, drop the approval, say so          | none, log    |
| Draft directory unreadable or wrong mode                     | refuse, name the path and the mode found              | page         |

The split is the chain's: a configuration mistake, a broken boundary or an ambiguity that could destroy
or expose work refuses loudly, and a routine absence is a log line. The pages here are the ones that mean
either "the boundary between private and shareable is not where it should be" or "vpp's own check did not
hold", and both deserve interrupting for.

### Security and privacy

**The draft tree is the most sensitive directory in the whole chain.** `draft.json` holds the map from
each placeholder to the real value it replaced, which is a compact index of exactly the material the
operator considered too sensitive to share, next to the sentences it came from. It is mode 0600 inside a
0700 directory, it is never in the vault, never in git, never in a sync root, and `vpp share review`
hides its values unless asked.

**The gate does not protect against an agent with shell access, and must not be described as if it did.**
Such an agent can read the transcripts, the vault and the draft tree directly, without touching this
feature. What the gate protects against is a release that nobody read: an unattended job, a well-meant
script, a tired operator, an agent that was asked to "share the meeting notes" and would otherwise have
copied a private note somewhere. Those are the realistic failures and they are the ones the byte-bound
approval and the destination refusals actually stop.

**Redaction is not a security control and the tool says so.** The report prints what was removed and what
it could not check, and the `source_line` in the released file tells the recipient it is a derivative. A
tool that printed "redacted" and nothing else would be making a claim it cannot support.

**Nothing about a draft rides a pns request, and nothing is notified.** Not a placeholder, not a count,
not a path, not a title. The chain set that rule for flagged spans and tags; a draft is the same class of
data with an audience attached.

**vpp never runs git**, per V7, and this feature adds the inverse rule: it refuses to write where git
would find its output.

**Two fingerprints are not designed, and both are real.** A per-recipient watermark (a distinct
placeholder numbering or an invisible marker per copy) would tell the operator which copy leaked, and a
released file's own metadata (a later PDF export carries a producer, a creation timestamp and sometimes a
source path) can identify the machine. The first is out of scope; the second is named in the open
questions because the vault already has a PDF export recipe and an operator would reach for it.

### The behaviors to drive the implementation, test-first

Each is one failing test before one piece of code, in this repository's style where a unit is a behavior
rather than a task. None needs a network, a model, a recording, an agent or a vault: each takes a source
file, a term list and a temporary directory.

1. The same source and the same policy produce the same draft identity and rewrite one directory rather
   than creating a second.
1. A policy change produces a different draft identity, and the old draft's approval does not transfer.
1. The removal pass is pure: two runs over the same bytes, term list and index produce identical draft
   bytes and identical digests.
1. A confirmed term is masked everywhere it appears as a whole token, in any case, including its
   possessive form; a near-miss spelling is not masked and appears in the candidate report.
1. Two different people map to two different placeholders, and one person maps to one placeholder across
   every occurrence in the draft.
1. Two drafts built from two sources that share a person use placeholder numbering that does not let the
   two drafts be joined.
1. A released file's frontmatter contains none of `vppRecording`, `vppOccasion`, `vppStage`,
   `vppCapturedAt`, `vppOpenFlags`, `vppEngines`, `vppSchema`, `tags` or `vppSuggestedTags`, and none of
   the vault's eight keys.
1. Every link target is absent from the released bytes, and a wiki link to a contact note leaves neither
   the target nor the name behind.
1. With `keep_timecodes = false` the released bytes contain no `[mm:ss]` reference, and `draft.json`
   still maps every released line to its source span.
1. A section on the deny list, and every `vpp:` marker block, is absent from the draft even when the
   operator selected the whole document.
1. A span flagged in the review record is omitted by default and counted in the report; with
   `flagged_spans = "mark"` it is kept and its `[unverified]` marker survives the removal pass.
1. The residue scan fails when a masked value survives anywhere in the draft bytes, including inside a
   heading, and the failure names the placeholder rather than the value.
1. `release` re-runs the residue scan on the current bytes: a draft that passed at draft time and was
   then edited to reintroduce a term is refused.
1. `approve` refuses when standard input is not an interactive terminal, and there is no flag that makes
   it proceed.
1. `approve` refuses a mistyped digest and writes no record.
1. `release` with no approval refuses and prints the approve command.
1. Editing one byte of an approved draft makes `release` refuse, and the message says the bytes changed
   rather than the policy.
1. Changing the policy after approval makes `release` refuse, and the message says the policy changed.
1. `release` refuses a destination inside a git working tree, naming the tree, and accepts the same
   relative path outside one.
1. `release` refuses a destination inside a configured sync root and inside the output root, naming which
   rule matched.
1. The released filename never contains the source slug unless `--as` supplied it.
1. `review` writes nothing, prints no real values without `--show-values`, and prints the candidate
   report.
1. The source file is byte-identical before and after every command in the flow, including `release`.
1. A source with no `known-terms.txt` at all still produces a draft, and the report states that the
   person class matched nothing rather than implying a clean result.

Behaviors 3, 12 and 13 pin the machine's own check, 14 through 18 pin "no draft can be shared without
review", 19 through 21 and 23 pin "the private original is never the shared artifact", and 24 is the one
a later change is most likely to erode, because a report that looks clean when the term list is empty is
exactly the false comfort this feature must not produce.

## Out of scope

Deliberately not designed here, and not to be smuggled in during implementation.

- **Sending anything.** vpp writes a file. Mail, messaging, upload and clipboard are all absent, and
  their absence is what keeps the gate meaningful.
- **Speaker labels.** Named in the ledger as an unapproved candidate. They would also add a dimension
  this design has no answer for: a label is either a real name (which the removal pass would mask into
  `[person 1]`, making the label pointless) or a pseudonym (which is a per-draft identity decision the
  operator has not made).
- **Dated digests.** Named in the ledger as an unapproved candidate. A digest is a multi-source draft
  over a time window, and multi-source drafts are the shape where a redaction that is correct per source
  leaks by correlation across sources.
- **Encryption, passwords or expiry on a released file.** A released file is an ordinary file. Delivery
  security belongs to the channel the operator chooses.
- **Per-recipient watermarking or leak attribution.** Real, useful, and a different feature with its own
  privacy questions.
- **Model-based entity detection.** It would require feeding the private transcript to a model, which is
  an egress decision rather than a redaction feature, and it cannot be tested deterministically.
- **Redacting audio.** Audio is never releasable here. A voice identifies its speaker and no text pass
  changes that.
- **Any change to the discovery, transcription, tagging or brief designs.** This stage reads their
  artifacts and writes into its own tree.
- **The `minutes` keep-or-replace ruling.** Its `vocabulary` command is named as a possible source of
  terms and nothing here depends on the ruling.
- **Retention and deletion.** vpp deletes nothing, per R6 and the chain. `vpp storage` gains the draft
  tree as a reported class, and any pruning is the operator's own command.

## Assumptions made in the operator's place

Each of these was a choice this document had to make to be written at all. None is a decision, each names
its alternative, and reversing any of them changes a configuration value or one section rather than
invalidating the measurements.

1. **The draft lives outside the vault and outside git until released.** Alternative: a gitignored path
   inside the vault, which keeps everything in one tree. Taken on the measured commit and push timers,
   and because a `.gitignore` entry is a vault-repository edit that the operator owns and that one later
   change could undo without anyone noticing.
1. **Release requires an approval bound to the exact bytes.** Alternative: an approval per draft
   identity, so small edits after review do not require re-approval. Rejected because it makes "reviewed
   before release" false the moment the file changes, and because the cheap version of that failure is
   the operator fixing a typo after approving.
1. **Approval requires an interactive terminal and a typed digest, with no bypass flag.** Alternative: a
   `--yes` for scripted use. Rejected: measured on this machine, an agent's ordinary command execution
   has no terminal, so the requirement is a real barrier, and any bypass flag would be the first thing an
   unattended job reached for. The cost is that a legitimate scripted release is impossible, which is
   the intended cost.
1. **vpp never transmits.** Alternative: a send integration, which is convenient and is what would make
   this feature genuinely one-step. Rejected because it puts the gate in series with an egress channel
   inside the same binary, and because the operator's existing tools already send files.
1. **Redaction is deterministic over confirmed terms plus closed pattern classes, with a candidate report
   for everything else.** Alternative: a model-based detector, which would catch unlisted names.
   Rejected on egress and testability, and because a detector that is right most of the time produces
   exactly the false confidence that makes a human review perfunctory.
1. **Flagged spans are omitted from a draft by default.** Alternative: keep them with their markers,
   which preserves more of the content. Taken because a span two engines disagreed about is a name, a
   number or a date that the machine probably got wrong, and shipping a wrong name outside the machine is
   the sharpest version of the failure the Forzare bullet names. Configurable either way.
1. **Placeholders restart at 1 in every draft.** Alternative: a stable pseudonym map, so `[person 3]` is
   the same person across drafts, which is friendlier for a recipient reading several. Taken because a
   stable map lets two drafts be joined; this is a genuine trade and it is an open question.
1. **Timecodes and every link target are removed by default.** Alternative: keep them, since they help a
   recipient who has context. Taken because they leak the existence and shape of the private material,
   and because the operator's own traceability is preserved in `draft.json` either way.
1. **The released frontmatter is an allowlist of at most three keys, and carries a line saying the file
   is a redacted derivative.** Alternative: no frontmatter at all, which leaks least. Taken because a
   recipient who cannot tell a derivative from a record will treat it as a record, and that
   misunderstanding is a harm the chain already refuses elsewhere.
1. **A draft is built from one source artifact.** Alternative: multi-source packs. Taken because the
   ledger lists dated digests as unapproved, and because correlation across sources is a redaction
   failure mode with no cheap answer.
1. **Destinations inside a git working tree or a cloud-sync root are refused, not warned.** Alternative:
   warn and proceed. Taken because the warning arrives in a terminal the operator may not be reading and
   the publication arrives on a timer they cannot recall.
1. **A brief is a releasable source, on the same terms as a transcript or an analysis note.**
   Alternative: forbid drafting from a brief. Flagged rather than forbidden, because a brief
   concentrates participants and context, which makes it the most dangerous source in the chain; it is an
   open question.
1. **vpp deletes nothing, drafts included.** Alternative: a retention policy that prunes draft
   directories. Taken because R6 and the chain say so, and because the draft tree holds the mapping that
   would be worst to lose track of. Retention is in the operator's half of this ledger bullet.
1. **Nothing was written, shared or published while measuring.** No file was written into the vault, no
   draft was produced, no release was performed, and the one real note that was measured is reported as
   counts and classes only, because this document lands in a public repository.

## Operator steps

**1. Answer the four choices the ledger names, because they gate implementation.** They are one bullet in
the ledger and four separate decisions:

- **Summary format.** Extract from the source, or generated prose, or both. This design supports both and
  states plainly that the mechanical protections are much weaker for prose.
- **Retention.** Of drafts (which hold the placeholder map), of released copies, and of the originals.
  vpp deletes nothing, so this is a decision about what the operator prunes by hand and when.
- **Transcription engines** and **local or cloud processing**. Both are the transcription design's open
  questions, and they matter here for a reason that document did not raise: a recording that was
  transcribed by a cloud engine has already left this machine once, before any redaction existed. The
  sharing gate protects the second egress, never the first.

**2. Choose the release directory, and confirm it is not synced anywhere.** The default proposed here is
`~/Documents/vpp-shared`. Measured: `~/Documents` on this machine is local and not redirected into
iCloud, but an iCloud Drive container is active, and this is the kind of setting that changes years later
without anyone remembering which directories it affects.

**3. Say which source kinds may be drafted from.** Transcript, analysis note, brief, or all three. The
brief is the one worth a moment's thought: it names the people who will be in a room.

**4. Confirm the approval ritual.** An interactive terminal plus typing eight characters, with no bypass
flag. If that is too heavy for how the operator actually shares things, the honest answer is to change
the ritual now rather than to add a bypass later.

**5. Seed `known-terms.txt` by using the review flow.** Redaction quality is a direct function of that
list, and it fills itself: every `vpp confirm --term` during transcript review makes every future draft
better. Nothing extra to do, but worth knowing that the first few drafts will have long candidate
reports.

**Also, when the vault layout is adopted:** nothing in this feature writes into the vault, so it adds no
folder note to the three the tagging design named and the fourth the brief design added.

## Open questions for the operator

1. **What is a shared draft made of: an extract, or written prose?** This is the ledger's "summary
   format" question. The design supports both, and the difference is not cosmetic: for an extract the
   residue scan is a real check, and for prose the human read is the only protection, because a
   paraphrase can reintroduce a redacted fact in words the pass never saw.
1. **What is the retention of drafts and of released copies?** A draft directory holds the map from each
   placeholder to the real value, which makes the draft tree more sensitive than the transcript it came
   from. vpp deletes nothing, so this is a question about what the operator prunes and on what rhythm.
1. **Should pseudonyms be stable across drafts?** Stable numbering is easier for a recipient who reads
   several drafts, and it lets two drafts be correlated by anyone holding both. Per-draft numbering is
   the proposed default and it is a real trade in the other direction.
1. **May a brief be drafted from?** It concentrates participants and context, which is what makes it
   useful and what makes it the most exposing source in the chain.
1. **Should an approval expire?** An approval taken today and released in three weeks was a review of the
   same bytes but not of the same situation. A time-to-live is one configuration key and it is the
   operator's call whether it is protection or friction.
1. **Does the released file say it came from vpp?** The proposed `source_line` says only that the file is
   a redacted extract and not a verbatim record. Naming the tool would be more honest about provenance
   and would tell a recipient that a recording exists.
1. **Is the PDF path in scope later?** The vault already has a PDF export recipe, an operator will reach
   for it, and a PDF carries producer, timestamp and sometimes path metadata that a Markdown file does
   not. If shared drafts are usually going to be PDFs, the metadata question needs its own answer.
1. **If `minutes` stays, should its `vocabulary` be a source of terms alongside `known-terms.txt`?** It
   manages the same kind of list, and two lists that disagree would produce a draft that masks a name in
   one pipeline and not the other. This is downstream of the `minutes` keep-or-replace ruling.
1. **Where does vpp's code live, and what is it called?** Carried forward unresolved from the boundaries
   design, because the chain should not stay in disagreement with itself.
