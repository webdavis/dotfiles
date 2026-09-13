# posture

`posture` ports the osquery security tools and ssh-hardening to Rust. Source implementation, caller
cutover and operator acceptance are tracked separately below. The original test inventory is an audit of
preserved behavior, with open gaps and retirement decisions retained explicitly.

| Crate                 | Responsibility                                               |
| --------------------- | ------------------------------------------------------------ |
| `posture-domain`      | Pure policy                                                  |
| `posture-application` | Use cases and their ports                                    |
| `posture-protocol`    | Existing cross-process digest record codec                   |
| `posture-pns-wire`    | This workspace's copy of the pns request and result contract |
| `posture-adapters`    | Files, processes, probes and protocol consumers              |
| `posture`             | Command decoding, composition and exit codes                 |

The member manifests enforce inward dependencies. Domain and application depend on neither protocol
crate. Adapters consume both local protocol crates; the command crate composes the tool. No build-time
dependency reaches another workspace. The six-field digest format remains unversioned. Notification
submission uses the deployed `pns submit --json` command and this workspace's versioned wire contract.

Rust work follows both `/Users/stephen/.agents/skills/clean-code/SKILL.md` and
`/Users/stephen/.agents/skills/clean-code-rust/SKILL.md`; the Rust binding wins all numbers and
mechanisms.

From the dotfiles checkout, build the installed target with:

```sh
cargo build --release --locked --quiet --bin posture --manifest-path posture/Cargo.toml
```

`just test-rust` runs the workspace tests, formatting check, and clippy. The builder installs
`~/.cargo/bin/posture` after recording the authorized artifact and refreshing its governing manifest
through the runner's pipeline-only option. It refuses empty artifacts and artifacts over 8 MiB. A
pre-publication refusal preserves the prior record, binary and tuple. A failed installation keeps the new
record so the next apply can retry. The interval between manifest publication and binary installation can
produce a detectable mismatch; it has no fixed time bound or interruption rollback.

[test-baseline.tsv](test-baseline.tsv) preserves the original 187 `target`, `test` and `result` values in
their original order. The added columns identify the assertion-bearing successor, related specification
statements, disposition, remaining work, source status and live acceptance. Original `ok` values are
historical Bash results. They are not new Rust test results.

This is task60's preparatory mapping at main `41423df8`, with the pending source snapshots below. It does
not close task60 or authorize Bash retirement. Of the 187 rows, 140 have mapped assertions, seven have
coverage distributed across components, five have an accepted transport replacement, 15 remain partial,
five propose a mechanism-specific disposition, 13 retain their legacy queue owner, and two have a direct
implementation gap. The categories describe this audit, not a coverage percentage.

`mapped-components` identifies the exact inputs each component test exercises. For example, a native
probe classifies an unreadable file and a separate generic repair test installs an absent file. That
combination is useful evidence, but it is not an unreadable-file repair drill. `partial` retains a
missing assertion or an unresolved behavioral difference. `proposed-disposition` needs review before its
Bash failure injection can be retired. `accepted-replacement` is limited to the plan's explicit
Observation and durable-receipt changes. A source reference beginning `merged` resolves under the main
snapshot; `*-pending` resolves under its named branch, not under the documentation worktree.

| Pending source                                     | Snapshot inspected                               | Assertion evidence and remaining boundary                                                                                                                                                                                                                                     |
| -------------------------------------------------- | ------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Task46 watchdog, `feat/posture-watchdog-health`    | `06a9d5df`, integrated branch                    | `watchdog_queue/tests.rs` pins missing/lazy/corrupt legacy counts; application watchdog tests pin alarm/delivery before state. Rows B064, B065 and B069-B071 include these pending successors. Queue retirement remains task49.                                               |
| Task47 poll, `feat/posture-poll`                   | `b3f39b5f`, includes main `41423df8`             | `posture/src/poll/tests.rs::an_exposure_is_stored_before_the_baseline_changes_and_refusal_retains_it` observes state at submission and after refusal/acceptance. This composition is outside the original 187-leaf corpus.                                                    |
| Task48 funnel, `feat/posture-funnel`               | `a777254f`                                       | `posture/tests/funnel.rs::oversized_exposure_reports_omission_without_advancing_the_baseline` pins the bounded omission correction; domain fixtures pin opening/steady/closed transitions and unknown baseline handling. Funnel had no leaf in the original corpus.           |
| Task50 converge, `fix/posture-converge-validation` | `92db6325`, includes poll and main `0ec1e22f`    | Local integration retains effective-access checks for both commands and passes private regressions and full `just ship`. B092 pins validation before stop. Parent publication and privileged cleanup/live repair acceptance remain open.                                      |
| Task58 secure shell, `feat/posture-ssh`            | `daad4344`, command/application work uncommitted | B187 includes committed directive/path policy and the pending `print_commands_and_help_do_not_construct_native_actions` assertion for exact output, zero exit and no native action. The original whole-process empty-directory assertion and operator acceptance remain open. |

The pending labels distinguish source from publication and live acceptance. Task50 has fresh integration
gates; the other entries here are assertion-source evidence, not new passing-test claims. A worker's
later merge or passing gate does not silently update this table. The private mapping receipts retain the
exact inspected file hashes and assertion bodies, including uncommitted source. Parent integration and
review must refresh the affected rows before publication.

Deployed status is unverified for every row. This audit reads repository source and private fixtures; it
does not establish an installed version or claim that a source caller cutover has reached the host.
Operator acceptance remains recorded separately and does not block this preparatory disposition.

The mapping found these unresolved behaviors:

- B041: S290 requires all four `DIGEST_MAX_*` scalar overrides. Native `digest.rs` has fixed constants
  and the digest command reads none of these knobs. Section 3.11's construction-only path policy does not
  retire scalar caps. B024 retains the distinction between ignoring an invalid value and testing a
  functioning override/fallback contract.
- B142: S122 requires a failure diagnostic naming the digest spool. `DigestAppendFile::append` returns
  false, its test asserts only that return, and `BatchJudge::spool_row` discards it. Detection should
  continue, with the named-spool diagnostic preserved and finding contents omitted.
- B020/B027: `DigestSpoolFile::claim` converts a failed `read_to_string` into empty text and removes the
  claim. A private fixture with valid rows around one invalid UTF-8 (Unicode Transformation Format,
  8-bit) byte returned no batch and left neither original nor claim. This is a demonstrated loss beyond
  the original torn-JSON fixtures. A failed read must retain recoverable bytes.
- B039: the existing restore fixture preserves an append made before restore starts. `fold` itself reads
  and rewrites the destination, so an append between those operations is still exposed to loss. The
  report does not claim a concurrent-fold test or a measured race reproduction.
- B001/B002: results-lock exclusion and checkpoint ordering have component tests. No retained native
  assertion repeats the original detached-child lock inheritance or two-process one-notification
  scenario. The exec-child test under `locks` exercises the different allowlist write lock.
- B012/B013, B028, B042, B141, B143, B148 and B187 retain their narrower configuration, composition,
  privacy or process assertions in the row's `remaining` column. B140 and B170-B173 propose dispositions
  for removed jq/pipe mechanisms; those proposals are not accepted retirements.

Existing research also exposes normalization differences outside the original leaf inputs.
[finding-boundaries](acceptance/finding-boundaries.md) captures an empty bundle path remaining empty and
a quoted `"0"` counter surviving. [The decision](decisions/finding-normalization.md) retains the
empty-path rule. Current results-row adapter tests instead assert fallback for an empty bundle and
suppression for quoted zero. Resolve those disagreements at the adapter boundary; the domain tests alone
do not prove the captured input behavior. The snapshot literal/action and interrupted-normalizer
contracts also remain recorded in [finding-normalization](specs/finding-normalization.md) and
[S022](acceptance/S022.md), pending an explicit final adapter disposition.

Source cutover and live acceptance have different owners. Main's heartbeat and digest cutovers do not
close task50a: the recorded remaining work still requires silent route/banner/ledger evidence and a
filled-spool digest with `.last` rotation. Alert, poll, funnel and watchdog retain their caller and
operator acceptance gates. Converge still needs the silent no-drift check and the approved mode repair
with a changed, stable daemon parent. Secure shell retains its command and operator gates. No live state
was inspected for this mapping. Before task49 retirement, every producer must migrate, all three legacy
queue tables must be empty, and the operator must review dead-letter disposition and approve any
exact-row removal. The 13 retained queue leaves have no native writer/retry successor by design.

The existing Rust file-size gate already covers posture. This work adds no guard or test suite. Task60
still needs the final post-port refresh, accepted dispositions, before/after implementation-size table,
and decision index required by plan step 9.2, after the ports and cutovers have met their own gates.

The dotfiles plan is `docs/superpowers/plans/2026-09-05-posture-port-plan.md`; its source inventory is
`docs/superpowers/specs/2026-09-05-posture-behavioral-specification.md`. These package documents remain
in Git and are excluded from deployment.
