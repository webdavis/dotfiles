# posture

`posture` ports the osquery security tools and ssh-hardening to Rust. Source implementation, caller
cutover and operator acceptance are tracked separately below. The original test inventory is an audit of
preserved behavior, with open gaps and retirement decisions retained explicitly.

| Crate                 | Responsibility                                                                                        |
| --------------------- | ----------------------------------------------------------------------------------------------------- |
| `posture-domain`      | Pure policy                                                                                           |
| `posture-application` | Use cases and their ports                                                                             |
| `posture-protocol`    | Existing cross-process digest record codec                                                            |
| `posture-adapters`    | Files, processes, probes and protocol consumers, and this workspace's own reading of the producer API |
| `posture`             | Command decoding, composition and exit codes                                                          |

The member manifests enforce inward dependencies. Domain and application depend on no protocol crate.
Adapters consume the one there is; the command crate composes the tool. No build-time dependency reaches
another workspace. The six-field digest format remains unversioned. Notification takes one of three
modes, chosen in `~/.config/posture/config.toml`: a signed POST straight to a hermes webhook route, a
page handed to a configured command on its standard input, or nothing leaving the machine and the local
banner alone. The two delivering modes carry this workspace's own reading of the versioned producer
documents, which lives in `crates/posture-adapters/src/wire/` beside the golden fixtures that pin it; see
[the producer API](producer-api.md).

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

| Pending source                                     | Snapshot inspected                                                                                                  | Assertion evidence and remaining boundary                                                                                                                                                                                                                                     |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Task46 watchdog, `feat/posture-watchdog-health`    | `06a9d5df`, integrated branch                                                                                       | `watchdog_queue/tests.rs` pins missing/lazy/corrupt legacy counts; application watchdog tests pin alarm/delivery before state. Rows B064, B065 and B069-B071 include these pending successors. Queue retirement remains task49.                                               |
| Task47 poll, `feat/posture-poll`                   | `b3f39b5f`, includes main `41423df8`                                                                                | `posture/src/poll/tests.rs::an_exposure_is_stored_before_the_baseline_changes_and_refusal_retains_it` observes state at submission and after refusal/acceptance. This composition is outside the original 187-leaf corpus.                                                    |
| Task48 funnel, `feat/posture-funnel`               | `a777254f`                                                                                                          | `posture/tests/funnel.rs::oversized_exposure_reports_omission_without_advancing_the_baseline` pins the bounded omission correction; domain fixtures pin opening/steady/closed transitions and unknown baseline handling. Funnel had no leaf in the original corpus.           |
| Task50 converge, `fix/posture-converge-validation` | `92db6325`, includes poll and main `0ec1e22f`                                                                       | Local integration retains effective-access checks for both commands and passes private regressions and full `just ship`. B092 pins validation before stop. Parent publication and privileged cleanup/live repair acceptance remain open.                                      |
| Task58 secure shell, `feat/posture-ssh`            | `daad4344`; the command and application work is committed at `539ecbb0` (PR #549), where the B187 citations resolve | B187 includes committed directive/path policy and the pending `print_commands_and_help_do_not_construct_native_actions` assertion for exact output, zero exit and no native action. The original whole-process empty-directory assertion and operator acceptance remain open. |

The pending labels distinguish source from publication and live acceptance. Task50 has fresh integration
gates; the other entries here are assertion-source evidence, not new passing-test claims. A worker's
later merge or passing gate does not silently update this table. The private mapping receipts retain the
exact inspected file hashes and assertion bodies, including uncommitted source. Parent integration and
review must refresh the affected rows before publication.

Deployed status is unverified for every row. This audit reads repository source and private fixtures; it
does not establish an installed version or claim that a source caller cutover has reached the host.
Operator acceptance remains recorded separately and does not block this preparatory disposition.

The mapping found these unresolved behaviors:

- B041: S290 requires all four `DIGEST_MAX_*` scalar overrides. Native `posture-domain/src/digest.rs`
  fixes `GROUP_LIMIT`, `BULLETS_PER_GROUP` and `BODY_LIMIT`, `posture-domain/src/sanitize.rs` fixes
  `FIELD_LIMIT`, and the digest command reads none of these knobs. The four overrides are restored on
  `fix/posture-digest-limits` (PR #550), pending merge, so this row stays a gap until that lands. Section
  3.11's construction-only path policy does not retire scalar caps. B024 retains the distinction between
  ignoring an invalid value and testing a functioning override/fallback contract.
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

The existing Rust file-size gate already covers posture. This work adds no guard or test suite. The
before/after implementation-size table and the decision index plan step 9.2 requires are below. Accepted
dispositions and the deployed-status refresh remain open, and wait on the ports and cutovers meeting
their own gates.

## Implementation size, before and after

Measured on 2026-09-14 against main `f24aba51`. Bash counts are physical lines from `wc -l`. Rust counts
come from the `clean-code-rust` file-size command, which reads implementation lines as those before a
file's first `#[cfg(test)]`, and zero for a `tests.rs` or a file under `tests/`. `tokei` is not used
anywhere here: it mis-parses this tree and its totals fall thousands of lines short of `wc -l`.

```sh
# Bash still tracked: entry points and the modules they source.
git ls-files 'dot_local/libexec/osquery/*.sh' 'dot_local/bin/executable_ssh-hardening.sh' |
  xargs wc -l | sort -rn

# Bash already deleted, each read at the parent of its deleting commit.
for path in 8f211044^:dot_local/libexec/osquery/executable_enrich-finding.sh \
  e8048748^:dot_local/libexec/osquery/executable_allowlist.sh \
  45e18321^:dot_local/libexec/osquery/executable_heartbeat.sh \
  d2a88b4c^:dot_local/libexec/osquery/executable_digest.sh; do
  printf '%6d %s\n' "$(git show "$path" | wc -l)" "$path"
done

# Rust, per crate.
git ls-files 'posture/*.rs' | while IFS= read -r f; do
  awk -v F="$f" '
    /^[[:space:]]*#\[cfg\(test\)\]/ && !seen { seen = 1 }
    !seen { impl++ }
    { total++ }
    END {
      if (F ~ /(^|\/)tests(\.rs|\/)/) impl = 0
      printf "%d %d %s\n", impl, total, F
    }' "$f"
done | awk '{ split($3, part, "/"); crate = part[3]
    files[crate]++; implementation[crate] += $1; whole[crate] += $2 }
  END { for (crate in files)
    printf "%-20s %4d %6d %6d\n", crate, files[crate], implementation[crate], whole[crate] }' |
  sort | awk '{ print; f += $2; i += $3; t += $4 }
    END { printf "%-20s %4d %6d %6d\n", "Workspace", f, i, t }'

wc -c < ~/.cargo/bin/posture
```

| Bash before                                                         | Lines | posture after                                | Bash source state            |
| ------------------------------------------------------------------- | ----- | -------------------------------------------- | ---------------------------- |
| `executable_ssh-hardening.sh`                                       | 2826  | `posture ssh`                                | tracked, operator-typed Bash |
| `executable_results-alerter.sh` plus its seven sourced stages       | 1945  | `posture alert`                              | tracked, Bash caller         |
| `executable_alert-dispatch.sh`                                      | 1263  | none, delivery moves to pns (spec section 5) | retired at `f645f50d`        |
| `executable_firewall-gatekeeper-monitor.sh`                         | 998   | `posture poll`                               | tracked, Bash caller         |
| `executable_osquery-converge.sh` plus `drift-verdict.sh`            | 986   | `posture converge`                           | tracked, caller cut over     |
| `executable_uptime-watchdog.sh` plus `executable_pipeline-audit.sh` | 826   | `posture watchdog`                           | tracked, Bash caller         |
| `executable_allowlist.sh`                                           | 359   | `posture allowlist`                          | retired at `e8048748`        |
| `executable_tailscale-monitor.sh`                                   | 287   | `posture funnel`                             | tracked, Bash caller         |
| `executable_digest.sh`                                              | 238   | `posture digest`                             | retired at `d2a88b4c`        |
| `executable_enrich-finding.sh`                                      | 139   | `posture enrich`                             | retired at `8f211044`        |
| `executable_drain-undelivered-alerts.sh`                            | 114   | none (spec section 6, D1)                    | retired at `f645f50d`        |
| `executable_heartbeat.sh`                                           | 109   | `posture heartbeat`                          | retired at `45e18321`        |
| `executable_canary-freshness.sh`                                    | 47    | inside `posture heartbeat` and `watchdog`    | tracked, Bash caller         |
| Bash in the port's scope                                            | 10137 |                                              | 2222 retired, 7915 tracked   |

The figures below were measured on 2026-09-13 and have not been re-measured since; the
`posture-producer-wire` row is gone with the crate, whose 16 files became 6 inside `posture-adapters`.

| Crate                 | Files | Implementation lines | Total lines |
| --------------------- | ----- | -------------------- | ----------- |
| `posture-adapters`    | 145   | 6351                 | 14975       |
| `posture-domain`      | 95    | 4420                 | 10578       |
| `posture`             | 51    | 1888                 | 5863        |
| `posture-application` | 49    | 2364                 | 7088        |
| `posture-protocol`    | 2     | 111                  | 269         |
| Workspace             | 358   | 16017                | 40824       |

The installed binary is 3,792,416 bytes (3.6 MiB) at `~/.cargo/bin/posture`, written by the apply of
2026-09-13 20:50 and well under the builder's 8 MiB refusal bound. Its usage text matches main's `USAGE`
constant, which is evidence about the deployed subcommand set and not a build identity.

Read the two tables as a size comparison, not a deletion record. 16,017 Rust implementation lines stand
against 10,137 Bash lines, and 24,807 of the 40,824 total Rust lines are tests the Bash pipeline never
had, where five tools carried no coverage at all (spec section 9). Only 845 Bash lines have actually left
the tracked set; the other 9,292 remain until their own cutover and retirement pull requests land.

## Delivery

[The producer API](producer-api.md) is the contract behind posture's two delivery paths: the envelopes,
what posture puts in a request, what it requires of a result, the severity-to-route table and the config
that chooses between them.

## Decision index

Each record fixes one boundary before its Rust implementation, and pairs with a policy specification and,
where behavior was captured from the running Bash, an acceptance map. The normalization boundary has
three acceptance documents because its captures were taken per statement.

| Decision record                                             | Boundary it fixes                                                                                                    | Specification                           | Bash-derived acceptance                                                                                          |
| ----------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | --------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| [allowlist-integrity](decisions/allowlist-integrity.md)     | `posture allowlist` curation, publication bytes, and the two known-good manifest consumers                           | [specs](specs/allowlist-integrity.md)   | [acceptance](acceptance/allowlist-integrity.md)                                                                  |
| [enrichment](decisions/enrichment.md)                       | `posture enrich` inspection order, command-output channels and the shared spawn budget                               | [specs](specs/enrichment.md)            | [acceptance](acceptance/enrichment.md)                                                                           |
| [finding-normalization](decisions/finding-normalization.md) | the alerter's normalization stage: detector admission, pack prefixes, baseline exemptions, enrichment-path selection | [specs](specs/finding-normalization.md) | [boundaries](acceptance/finding-boundaries.md), [S022](acceptance/S022.md), [S025-S360](acceptance/S025-S360.md) |
| [heartbeat](decisions/heartbeat.md)                         | `posture heartbeat` epoch admission, two-sided freshness and message vocabulary                                      | [specs](specs/heartbeat.md)             | [acceptance](acceptance/heartbeat.md)                                                                            |
| [poll-funnel](decisions/poll-funnel.md)                     | `posture poll` and `posture funnel` controls admission, gap-before-exposure order and baseline proposals             | [specs](specs/poll-funnel.md)           | [acceptance](acceptance/poll-funnel.md)                                                                          |
| [severity-gate](decisions/severity-gate.md)                 | the alerter's severity resolution, detector gate and signing verdict text                                            | [specs](specs/severity-gate.md)         | [acceptance](acceptance/severity-gate.md)                                                                        |
| [watchdog](decisions/watchdog.md)                           | `posture watchdog` manifest audit, streamed finding order and two-tick confirmation                                  | [specs](specs/watchdog.md)              | [acceptance](acceptance/watchdog.md)                                                                             |

Two decision sets stay outside this index and outside this package. The operator's four delivery,
vouching, file-location and mute decisions are plan section 8, and the numbered drops D1 to D15, with the
deliberate behavior changes in section 6.1, are the specification's.

The dotfiles plan is `docs/superpowers/plans/2026-09-05-posture-port-plan.md`; its source inventory is
`docs/superpowers/specs/2026-09-05-posture-behavioral-specification.md`. These package documents remain
in Git and are excluded from deployment.
