# pns documentation

These documents live inside the crate so they travel with it when it moves to its own repository.

They were derived from the source and its tests on 2026-09-02, at `origin/main` commit `413eb8d0`. They
are not a design proposal and they do not describe intended behavior: where a document and the code
disagree, the CODE is right and the document is the defect. Every non-obvious claim cites the file and
symbol it came from, so a claim can be rechecked without trusting the writer.

Where evidence for an expected behavior could not be found, the document says `NOT ESTABLISHED:` and
names what was looked for and where. Those lines are findings, not filler. Search for them.

## `specs/`

One behavioral specification per area, written as `Given / When / Then` scenarios. Each behavior records
its success path, every failure source, its fail-open or fail-closed direction, exact thresholds with the
step either side, required and forbidden side effects, timeout and cancellation, idempotency, privacy,
process ownership and cleanup, and which outputs or exit codes are compatibility contracts.

| Document                               | Area                                                                                                   |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| `glossary.md`                          | The vocabulary, verified against `src/`, including the words in circulation that the code does NOT use |
| `producer-submission.md`               | Argv to attempts made, and the records the first delivery writes                                       |
| `legacy-producer-flags.md`             | The frozen command-line surface, flag by flag                                                          |
| `hook-compatibility.md`                | The eleven harness hook events and their stdin, stdout and exit-code contracts                         |
| `blocking-approval.md`                 | The moshi gate, the bounded wait, and exit-code translation                                            |
| `routing-and-delivery.md`              | Legs, destinations, executable channels, and delivery outcomes                                         |
| `presence-and-visibility.md`           | The probes, their fail directions, and how a delivery plan is reached                                  |
| `quiet-behavior.md`                    | Every mechanism that silences pns, and exactly what each one silences                                  |
| `missed-notifications.md`              | Journalling a notification the operator could not perceive, and replaying it                           |
| `return-recap.md`                      | Composing and posting the account of an absence                                                        |
| `nagging.md`                           | The repeat card about an approval nobody answered                                                      |
| `home-probe.md`                        | Asking the router whether the operator's devices are home                                              |
| `lighting-policy.md`                   | Pulses, the unread state, leases, phases, quiet windows and dim windows                                |
| `daemon-jobs.md`                       | The clock, the job model, and process ownership                                                        |
| `doctor-diagnostics.md`                | The diagnostic census, and which parts of it have live external effects                                |
| `setup-and-publication.md`             | The first-run walk and safe publication of the configuration file                                      |
| `configuration.md`                     | Loading, strict decoding, bounds, secrets, and template rendering                                      |
| `persistence-and-process-lifecycle.md` | The state directory, the file protocols, and every spawned child                                       |
| `privacy-and-hostile-input.md`         | Sanitization, ceilings, and where a secret may and may not go                                          |

## `decisions/`

Numbered records for decisions whose reasoning is worth more than one comment site, especially the
measured investigations behind them. A production comment states the invariant and links here for the
history, rather than restating the measurement at every site.

| Record | Decision                                                                                           |
| ------ | -------------------------------------------------------------------------------------------------- |
| `0001` | A file protocol owns a file by rename, never by removal                                            |
| `0002` | The binary's spawn roster is a closed, operator-approved list                                      |
| `0003` | A numeric reading is refused rather than coerced, mirroring the shell pns replaced                 |
| `0004` | The lamp state is `unread`, and the legacy `glow` state is deleted rather than migrated            |
| `0005` | `doctor` and a bare `pulse` change the real world, so no test may run them                         |
| `0006` | A word that names no command is refused, even though the producer parser is lenient                |
| `0007` | Passing both delivery-scope flags is refused at the legacy adapter                                 |
| `0008` | `pns <harness>-hook` is a compatibility spelling for a field that holds one pathname               |
| `0009` | A compiled-in destination beats an executable of the same name, unless the directory is overridden |
| `0010` | A notification never fails the work it reports on                                                  |
| `0011` | The shipped configuration template is pinned from outside the crate, and that pin has to leave     |

## `test-baseline.tsv` and `test-baseline.md`

The suite recorded as a SET OF NAMES with results, never a count, plus the procedure for diffing a later
suite against it and the classification of the existing tests. A count passes when one test is dropped
and another added, and it passes a rename, which is how a behavioral contract goes missing during a
refactor.
