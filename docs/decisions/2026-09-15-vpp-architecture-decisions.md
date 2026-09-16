# vpp architecture decisions

Thirteen decisions the operator made on 2026-09-15, in conversation, outside the seven-document brief at
`docs/decisions/2026-09-15-vpp-decision-brief.md`. That brief still stands and is still unanswered; this
page settles a different, more fundamental layer underneath it: whether vpp depends on `minutes` at all,
how the engine pair and its notification path are shaped, and one piece of carried-forward posture work
these decisions expose. Where a decision here closes or moots a question the seven design documents or
the brief already asked, that document is marked in place and points back here.

## What was decided

| #   | Decision                                                                                                                                                                            | Closes / moots                                                                                      |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| 1   | vpp does not use or depend on `minutes`, in any form                                                                                                                                | reconciliation blocking question; boundaries Q5; filing Q7; briefs Q7; sharing Q8; transcription Q7 |
| 2   | Whether audio leaves the machine is the user's choice; local is the default                                                                                                         | reconciliation open question 5                                                                      |
| 3   | Two engines, primary plus optional fallback, both user-selected; fallback trigger (on failure, or always) is also configurable                                                      | transcription Q1 (architecture, not a fixed pairing)                                                |
| 4   | Reconciliation picks a winner, then flags uncertainty inline plus a `vpp review <id>` queue; no summary block, no separate document                                                 | transcription Q4 (confirms), design's existing recommendation                                       |
| 5   | Three more engine-pair options built (transcript winner rule, both-engines-fail behavior, engine per language); disagreement threshold and cloud spend ceiling explicitly not built | new, no prior open-question number                                                                  |
| 6   | `[notify]` gets exactly three modes: `desktop`, `command`, `off`; command mode does placeholder substitution AND writes vpp's JSON event on stdin, simultaneously                   | new, no prior open-question number                                                                  |
| 7   | vpp needs no copy of pns's wire contract; no `vpp-producer-wire` crate                                                                                                              | new, no prior open-question number                                                                  |
| 8   | Every vpp subcommand gets `--json` on its own output, in vpp's own shape                                                                                                            | new, no prior open-question number                                                                  |
| 9   | The configured command's own exit code, not merely whether it ran, decides whether vpp falls back to its own desktop notice                                                         | new, no prior open-question number                                                                  |
| 10  | Engines are named in config, not hardcoded; two first-class adapters to start (Apple Speech, whisply), plus a generic command escape hatch with no uncertainty flagging             | transcription Q1, Q2                                                                                |
| 11  | Two design ideas adopted from FluidVoice, credited: local-by-default with cloud strictly opt in; post-processing as its own layer, separate from transcription                      | new, no prior open-question number                                                                  |
| 12  | `whisply`, `openai-whisper` and `@elevenlabs/cli` stay declared; the `minutes` cask has no remaining use                                                                            | reconciliation open question 7 (partial: minutes half)                                              |
| 13  | File a new ledger task: drop `posture-producer-wire`, converge posture on the same three-mode `[notify]` shape                                                                      | new ledger task 91                                                                                  |

## 1. The `minutes` disposition

The reconciliation bullet in `docs/remaining-work.md` has carried this as vpp's own named blocking
question since before the design chain existed: is `minutes` kept, replaced, or run beside vpp. The
operator's answer is not a scope call so much as a taste call, and it is recorded in their own words
rather than softened: they consider `minutes` poorly designed, while acknowledging it has good features
worth learning from. vpp is therefore a standalone tool. No calling `minutes transcribe`, no
`minutes watch` front end, no runtime spawn of any `minutes` subcommand, ever.

This closes the blocking question outright, and it makes three of its own dependent questions moot rather
than answered, because the premise each one depended on (that vpp and `minutes` interact in some form) no
longer holds:

- Whether vpp calls `minutes` or runs beside it: moot. Neither.
- Whether the `PLAN-v12` L6 rule ("must not create a second automatic Voice Memos capture/transcription
  workflow", written for Open Notebook) binds `minutes` against vpp: moot. vpp does not touch `minutes`
  for L6 to bind against.
- Whether to fold the vault `minutes` symlink repair into vpp's own work: moot as a vpp question, and
  unchanged as its own item. The orphaned `agent-processing-pipeline/minutes` symlink and the false vault
  `CLAUDE.md` claim that it is managed by `minutes vault setup --subdir` are about `minutes`, not about
  vpp, and stay exactly where they were: their own small operator item, independent of this decision.

The same answer closes five further questions the design chain raised downstream, each on the same
premise: the project boundaries design's own question 5 ("is `minutes` in or out?") is answered out; the
metadata and filing design's question 7 ("does `minutes` stay?") no longer leaves two schemas sitting
side by side with nothing in common, because there is only one; the meeting briefs design's question 7
(whether `minutes`' `research` and `person` output could feed a brief) is moot with no `minutes` to draw
from; the redacted sharing design's question 8 (whether `minutes`' `vocabulary` should feed
`known-terms.txt` alongside vpp's own list) is moot for the same reason; and the redundant transcription
design's question 7 (whether `verify-note` belongs in vpp or "whatever writes the note") resolves in
vpp's favor, since vpp is now the only thing that writes the note.

## 2. Audio leaving the machine is a setting, not a rule

`~/.config/vpp/config.toml` selects the transcription engine per slot (primary and fallback). Local
processing is the DEFAULT: `vpp setup` prompts for an engine with the local option preselected, and the
operator's own configuration stays local. This document, and every other vpp document, must not write
"audio never leaves the machine" as a product rule, because that overstates what the capability is: it is
the operator's own default, not a ceiling on what another user or another day's configuration can do.

This is the same pattern this repository already carries elsewhere: `docs/remaining-work.md` task 90
ships Moshi image cards as a per-card-type capability, on by default, with the operator's own recap card
configured to keep images off; that is a setting in the operator's own config, not a limit on the
product. The lights features follow the same shape, a capability built and shipped, then configured per
machine. vpp's local-versus-cloud choice is the same move applied to a different subsystem: the
capability exists in full, the default is conservative, and the operator's own choice lives in
`~/.config/vpp/config.toml`, not in vpp's source.

This closes the reconciliation bullet's own open question 5, "do Voice Memos originals leave the machine
at all?": yes, when the operator's config names a cloud engine in either slot; no, under the shipped
default.

## 3. Two engines, and when the fallback runs

vpp supports exactly two transcription engine slots: a primary and an optional fallback, both named in
config, neither hardcoded. Whether the fallback runs at all, and when, is itself a configuration value:
on primary failure only, or on every recording, so the two outputs can be compared. The operator's stated
reasoning: published accuracy benchmarks let a user pick engines by measured word-error rate, and adding
a second model is how that number moves toward 100 percent rather than staying pinned to one engine's own
ceiling.

This does not select a fixed pairing (the redundant transcription design's own open question 1). It
answers a level above that question: there is no one correct pair the design should recommend, because
the pair, and even whether a second engine runs on every recording or only after a failure, is the
operator's call per machine. Decision 10 below supplies the two engines vpp ships adapters for on day
one; the pairing itself stays configuration.

## 4. Reconciliation: pick a winner, then flag uncertainty in place

When both engine slots ran, vpp picks a winning transcript and then surfaces where it was uncertain. Two
surfaces carry that uncertainty, and only two:

- Inline markers at the exact place in the transcript where the winning engine and the other engine (or
  the winning engine's own low confidence) disagreed or were unsure.
- A review queue, `vpp review <id>`, that walks the operator through each uncertain spot one at a time so
  they can correct it in context.

Two designs the redundant transcription document also considered were explicitly rejected: an uncertainty
summary block at the top of the transcript, and a separate document listing the uncertain spots. The
operator's reasoning: a transcript is a record of what was said, and a document that exists only to
announce problems still leaves the reader hunting through the transcript for where they are. A marker at
the exact word, plus a queue that walks to it, does the job neither rejected shape does. Every correction
made through `vpp review` feeds the term confirmation the chain already designed (`vpp confirm --term`),
so a fix improves every future transcript rather than only the one being reviewed. Examples of what gets
flagged: a speaker's name the engine could not make out, a word it could not settle between two
candidates.

This confirms, rather than reopens, the redundant transcription design's own question 4 ("should a
confirmed correction rewrite future transcripts?"): the design's recommendation, record and do not
rewrite the transcript of record, stands. `vpp confirm --term` is the forward-looking mechanism; nothing
here retroactively edits a shipped transcript.

## 5. Three more engine-pair options, and two deliberately not built

Beyond the pairing itself (decision 3) and the two starting adapters (decision 10), three further
behaviors on the engine pair are operator-chosen and get built:

- Which transcript wins when both engines ran.
- What happens when both engines fail: keep the audio and mark it for review is the default, because the
  audio is the one thing that cannot be regenerated once Apple evicts the original.
- An engine choice per language, so a bilingual operator is not stuck with one engine's weaker language.

Two adjacent options were considered and explicitly not built, both on the same reasoning: a good default
cannot be chosen yet.

- A disagreement threshold (how much two transcripts must diverge before it counts as a disagreement
  worth flagging) is not built, because no good number can be picked before real disagreements have been
  seen on real recordings; a number picked now would be a guess wearing a config key.
- A cloud spend ceiling is not built as a vpp feature, because spend belongs to whichever engine charges
  money, not to vpp's fallback logic. A ceiling implemented inside vpp would duplicate whatever limit the
  cloud engine's own account, API key or billing console already enforces, and would drift from it.

## 6. `[notify]`: exactly three modes, and command mode does both things

This is the decision with the most consequence for how vpp composes with everything else on this machine,
and it follows the shape `posture/crates/posture-adapters/src/producer.rs` states in its own header
comment: "THE PRODUCER API IS THE WHOLE COUPLING. A JSON request goes in on standard input, a JSON result
plus an exit code comes back, and the command and its arguments are both config." vpp's `[notify]` table
carries the same discipline into a three-value enum instead of posture's two-mode split:

- `desktop`: vpp raises its own local desktop notification. No external process, no config beyond the
  mode itself.
- `command`: vpp runs a program the operator named.
- `off`: vpp raises nothing.

In `command` mode, vpp does BOTH of the following at once, with no extra key in `[notify]` to choose
between them: it substitutes placeholders (identity, state, detail, and so on) into the argument list the
operator wrote, and it writes vpp's own event as JSON on that command's standard input. Both happen on
every invocation; there is no separate "argv only" or "stdin only" sub-mode. A user pointing the command
at `pns` writes the `pns` flags with a placeholder for the value that changes per call, and their command
never reads standard input, so it ignores the JSON entirely. A user who wants a translation layer instead
points `command` at their own script; that script reads the JSON on standard input and ignores whatever
arguments were substituted into its own argv. Both users are served by one mode with one contract. vpp's
source never contains the word `pns`, matching decision 7's reasoning below: vpp names no engine and no
downstream tool, the way posture and `uu` already commit to (`docs/remaining-work.md`'s own
"tools-are-pns-agnostic" ruling, 2026-09-14).

## 7. vpp needs no copy of pns's wire contract

The rejected alternative, recorded because it matters: publishing the producer API (JSON request in, JSON
result plus exit code out) as its own versioned crate, `vpp-producer-wire` or similar, that pns, posture
and vpp would each depend on. This was considered and rejected as unnecessary for vpp's actual use case.

The reason it is unnecessary rests on pns's own refactor plan, `pns/docs/pns-refactor.md`, which the
operator has approved (items cited by their position in that document):

- Item 73: "The command line (`pns send --<field>`) and the JSON API (`pns send --json`) share one field
  list." Once built, the pns command line carries the same fields the JSON API carries, so a caller that
  only ever shells out to `pns` with flags is already speaking the same contract a JSON integration would
  speak, without parsing or emitting JSON at all.
- Item 110: "Use the same exit codes on both paths, and a partial delivery is a failure: `0` only when
  every destination delivered, `1` when any destination failed... or nothing delivered at all, `2` bad
  input." This is what makes exit code alone sufficient signal (see decision 9).
- Item 115/116: "Handle bad input the same way on both paths, strictly: an unknown flag or field... is
  refused and named, with exit `2`." An unrecognized flag is a loud, named refusal rather than a silent
  skip, so a caller finds a contract mismatch immediately rather than discovering a dropped field months
  later.

Given those three, the pns command line alone is sufficient for a `[notify] mode = "command"` pointed
directly at `pns`: same fields, same exit-code meaning, same refusal behavior as the JSON API, with none
of the parsing. No `vpp-producer-wire` crate is needed, and vpp's source never names pns (decision 6).

The one condition under which this should be revisited: if vpp ever needs to send STRUCTURED data through
the command line rather than flags and free text, because pns's own `extensions` field is JSON-only (item
108 in the same refactor plan: "Keep two things JSON-only on purpose: `schema` ... and `extensions`
(free-form data, which has no clean flag form)"). vpp has no such need today; its notify payload is flags
and short strings, not nested structured data.

## 8. Every tool gets `--json` on its own output

Every vpp subcommand ships `--json` on its own output, in vpp's own shape, independent of the `[notify]`
decision above. This is what makes a translation layer (a user's own script pointed at by `command` mode,
or any other integration) actually writable: it can read structured vpp state rather than scraping
human-readable text meant for a terminal. This is a general vpp property, not limited to the notify path;
`vpp review`, `vpp handoff` and the other subcommands the design chain already specifies keep their own
`--json` forms as designed.

The config comment for `[notify] mode = "command"` states plainly, at the point of the toggle, that the
configured command receives VPP's JSON on standard input, not pns's. A user who wants pns's own JSON
shape gets it by running `pns` directly with `--json`, a separate concern from what vpp hands its own
configured command.

## 9. The exit code is load bearing

vpp decides whether to fall back to its own desktop notification (when `[notify] mode = "command"` is
configured) based on the CONFIGURED COMMAND'S OWN EXIT CODE, not on whether the command merely ran to
completion. A translation layer that wraps `pns` and exits 0 because the wrapper script finished, while
`pns` itself returned 1 for a partial or undelivered send (per item 110 above), tells vpp the error
notice was delivered when it was not. On an error path that is the worst outcome available: the one
channel that exists to catch a delivery failure reports success.

The rule this decision writes down, for both the documentation and the `[notify]` config comment: a
translation layer MUST pass through its final delivery's exit code as its own exit code, rather than
reporting success at merely having run. This is symmetric with decision 7's reliance on pns's own
refactored exit codes (0 delivered, 1 partial or undelivered, 2 bad input): the whole chain from vpp's
event to a phone notification only tells the truth if every link in it forwards the real result rather
than its own completion status.

## 10. Engines are configured, with one honest limit

Transcription engines are named in vpp's config, never hardcoded as a shipped list, matching decision 3's
architecture. The one honest limit: uncertainty flagging (decision 4) needs confidence scores and speaker
labels, and not every engine emits them. vpp therefore ships a small set of FIRST-CLASS adapters that can
read confidence and speaker labels, plus a generic command escape hatch that works with any engine and
says plainly, in its own documentation, that it gets no uncertainty flagging.

The two first-class adapters to start with:

- Apple Speech, built into macOS. No download, no cost, on device.
- `whisply`, already installed on this machine with Apple Silicon acceleration (`whisply[mlx,app]` in
  `.chezmoidata/system_packages_autoinstall.yaml`).

This closes the redundant transcription design's own question 2 ("is the Apple SpeechAnalyzer route worth
a Swift helper inside a Rust project?"): yes, it becomes one of the two starting first-class adapters. It
narrows question 1 ("which engine pairing?") from a single recommended pair to a documented starting menu
inside a configurable architecture (decision 3), rather than answering it with one fixed pair.

The wider engine menu the transcription design surveyed is recorded here as CANDIDATES, not commitments,
with the numbers found: Parakeet TDT v2 and v3, Parakeet Flash, Nemotron Speech 3.5, Cohere Transcribe
and the Whisper family (sizes 75 MB to 2.9 GB, 99 languages), NVIDIA Canary Qwen 2.5B at 5.63 percent
word error rate on the open speech recognition leaderboard, and `gpt-4o-transcribe` at roughly 0.006
United States dollars a minute. Sources: <https://github.com/altic-dev/FluidVoice>,
<https://artificialanalysis.ai/speech-to-text/non-streaming>, and
<https://northflank.com/blog/best-open-source-speech-to-text-stt-model-in-2026-benchmarks>. None of these
is adopted; they are the menu a later engine-adapter task chooses from.

## 11. Two design ideas adopted from FluidVoice, credited

Two ideas from the FluidVoice project (cited above) shaped this decision set and are adopted with credit
rather than silently absorbed:

- Local processing by default, with cloud strictly opt in. This is decision 2's shape, and FluidVoice is
  named as the source of the pattern, not just of the survey numbers in decision 10.
- Post-processing as its own layer, separate from transcription. The reasoning: whether the engine heard
  the word correctly, and whether the resulting text reads well, are different problems, and mixing them
  into one pass makes both harder to fix independently. A transcription engine's job ends at the most
  faithful text it can produce; anything that reshapes that text for readability is a distinct,
  separately configurable stage downstream of it.

## 12. The four unwired transcription installs

`whisply`, `openai-whisper` and `@elevenlabs/cli` STAY declared in
`.chezmoidata/system_packages_autoinstall.yaml`, because vpp is about to need local engines and these are
candidates decision 10 and the wider menu in decision 10 draw from. The `minutes` cask is the exception:
under decision 1 it has no remaining use.

This is a decision, not an edit: `.chezmoidata/system_packages_autoinstall.yaml` is untouched by this
pull request. This repository builds no removal mechanisms (`docs/remaining-work.md`'s own
"no-removal-mechanisms" ruling), so dropping the `minutes` cask declaration is an edit the operator makes
deliberately, on their own schedule, not something this decision record or any future vpp pull request
does on their behalf.

## 13. New ledger task: converge posture on vpp's `[notify]` shape

Approved by the operator on 2026-09-15, not started, filed as task 91 in `docs/remaining-work.md`.
Reading `posture/crates/posture-adapters/src/producer.rs` and `posture/crates/posture-producer-wire/` for
decision 7 surfaced a simplification in posture itself: it carries its own copy of a JSON request/result
envelope (`posture-producer-wire`) that duplicates the shape vpp's `[notify] mode = "command"` now covers
with a plain argv-and-stdin contract and no dedicated crate. Dropping `posture-producer-wire` and
converging posture on the same three-mode `[notify]` shape removes posture's own wire-contract crate in
favor of the same pattern decision 6 and decision 7 establish for vpp. Not started; filed as its own task
so it does not block anything in this pull request.
