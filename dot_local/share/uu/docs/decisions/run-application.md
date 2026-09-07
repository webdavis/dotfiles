# Run application ownership

The run command previously owned configuration loading, process construction, sequencing, presentation
and durable-state decisions in one function. `uu-application::Run` now owns the sequence and the record,
marker, staleness and pending escalation decisions. The command loads configuration, supplies declared
names, resolved deadlines and escalation thresholds, constructs adapters and maps the typed result to the
existing exit behavior.

## Dependency direction

| Package          | Responsibility and direct dependencies                                                                                                                              |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `uu-domain`      | Reports, marker facts, deadlines and streak policy; no dependencies.                                                                                                |
| `uu-application` | Run sequencing and consumer-owned ports; depends only on `uu-domain`.                                                                                               |
| `uu-protocol`    | Existing child-event and record encodings; independent of domain and application.                                                                                   |
| `uu-adapters`    | Configuration, state, process, clock and delivery adapters; depends on the three inner packages, `pns` and the existing infrastructure libraries.                   |
| `uu-cli`         | Arguments, command presentation and concrete composition; depends on application and adapters, plus libc for the existing signal disposition. Owns the `uu` binary. |

`uu-cli` also takes `uu-domain` as a test dependency for the value types in the existing application
ports.

`Marker` and `RunFacts` belong to the domain because they describe the previous successful run and the
facts supplied to a lane, without prescribing an encoding.
`crates/uu-adapters/src/record/event.rs::event_for` maps them into protocol types at the existing adapter
boundary. No serializer or free-form configuration enters the application. The child event, record
payload and command arguments keep their existing formats; this row introduces no protocol version or
compatibility facade.

## Ports and their current adapters

| Consumer-owned port | Current adapter                        | Boundary                                                                                                           |
| ------------------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `RunState`          | `run_adapters::FileRunState`           | Lock lifetime, marker and streak reads/writes, removed-lane pruning.                                               |
| `RunClock`          | `run_adapters::SystemRunClock`         | Fallible wall-clock samples and monotonic elapsed time.                                                            |
| `LaneExecutor`      | `run_adapters::ConfiguredLaneExecutor` | A runner per declared lane, supplied with actual budget and declared deadline; invokes its selected typed adapter. |
| `RunDelivery`       | `delivery::EngineRunDelivery`          | Configured alert process, signed records and optional failure-webhook posts through existing clients.              |
| `RunPresentation`   | `run_adapters::ConsoleRunPresentation` | Header facts, detail rendering and exact diagnostic streams and wording.                                           |

The associated state guard keeps ownership explicit without exposing a filesystem lock to the use case.
Typed outcomes distinguish contention from unavailable locking, absent from unreadable history, missing
delivery configuration from failed delivery, and record rejection causes from success. Private policy
helpers may reduce a reported outcome to a marker decision; the port still preserves its cause.
Presentation supplies a header once for the lanes, printed detail and alarms. The record adapter still
samples the host for the outbound weekly record; this is not one host lookup for every output in a run.

## Decisions and retained rationale

- Keep ordering in the application. The lock precedes pruning and all run effects. The previous marker
  must be sampled before work so the next record cannot describe a zero gap caused by its own marker
  write. The finish clock is sampled separately so the run's duration is not added to the next gap. An
  unreadable finish clock conservatively leaves that gap measured from the earlier run.
- Keep staleness separate from one-run sequencing. It answers whether a lane has remained unsuccessful
  across attempts, including repeated deferrals that otherwise produce no failure alert. Unreadable
  history must not reset that evidence. A failed trip notification retains two so it can retry; publish
  the count only after the attempt. The new ordering assertion makes the alert's view of old state an
  explicit contract.
- Keep pending history separate from non-success history. Pending work succeeded at its check, so it
  resets the non-success count and may advance the success marker. Its own counter trips at the
  configured positive threshold, resets on any non-pending verdict and retains one short of the threshold
  when an alarm is refused. The existing state port selects the two files with `StreakKind`; the reader,
  atomic writer and whole-directory pruning remain shared.
- Keep record receipt separate from alert receipt. A failure alert cannot substitute for a missing weekly
  record, and a deferral cannot establish that every lane ran successfully. Both retain the old success
  marker. With no configured record channel nothing was owed, so a clean run may advance it. Alarms
  attempt the pns engine and optional failure webhook independently. Either configured refusal keeps a
  one-shot alarm eligible for retry, even if the other destination received it; neither refusal changes
  the marker rule or run exit.
- Keep name order and the existing budget policy. A configuration reorder must not reorder execution.
  Failure in one lane must not prevent later lanes from running. The six-hour default was chosen with
  margin for long package builds, not from measured lane runtimes; the 24-hour lane budget prevents a
  sequence of defaulted lanes from consuming the next weekly slot. It does not time every adapter call.
- Keep filesystem protocols in this row. The success marker remains `last-success`, and each lane's count
  remains `lanes/<name>/streak` under the existing state directory, with pending history beside it at
  `lanes/<name>/pending`. Streak publication writes a sibling temporary file and renames it over the
  target, allowing replacement of a read-only old file. The temporary name includes target name and
  process identifier to distinguish targets in parallel fixtures. Marker publication still uses the
  existing direct write. There is no new permission, transaction or crash-durability guarantee.
- Preserve pruning's actual limit. Pruning requests the full declaration set under the run lock to avoid
  reusing removed-lane history. The filesystem implementation remains best effort and silent on failure.
  A failed removal can leave old history behind, so a claim that it can never affect a reused name would
  be stronger than the implementation.
- Preserve process ownership in the existing adapters. The signed POST remains bounded to ten seconds;
  alert process waiting remains unbounded. Lane watchdogs own process groups, pipe draining and their
  existing stuck-spawn and escaped-process limitations. The application neither claims those processes
  are gone nor introduces a second cleanup path.

Concrete adapters and command parsing belong to their own crates; the root is a virtual workspace. Parser
fixtures belong to the adapter package, and the outer repository verifies its actual configuration
template separately. The command's registration list binds the five existing type names to their typed
parsers, execution and diagnostics. Each typed parser owns its admitted keys. Configuration selects a
registration while retaining the operator's declared name, resolved deadline and pending threshold.
Execution and doctor use that selected adapter, so neither repeats a technology-name switch. Domain and
application retain their existing dependency direction and know no concrete lane technology.

Module trees are private, with deliberate exports at their package roots. Large unit-test modules live in
private child files. `config.rs` owns the shared file boundary and top-level configuration;
`config/records.rs` owns the record and failure-webhook settings. Size evidence is measured from the
formatted source with the Rust standard's physical-line command.

## Test ownership and evidence

[The name map](../test-name-map.tsv) retains the original 287 baseline contracts and the thirteen
application-extraction additions. All 300 cases entering registration and package closure have successful
named successors; four registration cases brought that row to 304. Pending records, escalation and
optional alarm delivery preserve those cases and add 32, for 336 successful named outcomes. The two
command timing cases move into the actual command composition, and the ten staleness cases split between
streak policy and persistence/delivery failures. The original record-body and delivery-policy moves
remain recorded in the map.

The seven inherited harness speed-guard cases remain classified as infrastructure coverage, not product
behavior. Their backdated fixture clocks test the guard itself. They do not excuse product tests from the
one-second limit; the timed workspace run measures each case's actual duration separately. No new
slow-test exception is added.

The record-body tests still protect the adapter call site: dropping the deferred count there can post
`completed` while the pure verdict tests stay green. The moved delivery-policy tests protect refusal and
success directions through injected outcomes. The twelve new tests exercise the whole use case through
concrete in-memory ports, including guard lifetime, ordering, budget exhaustion, continued execution,
selection, lock refusals, finish-clock failure and the record-to-marker decision.

The application-extraction run recorded 300 successful outcomes. Its formatting, checking, Clippy,
workspace tests, documentation and locked release build passed, and seventeen independently compiled
application mutants failed their intended assertions against green controls. That historical run measured
`a_lane_that_outlives_its_deadline_fails_instead_of_holding_the_run_open` at 1.0611155 seconds, compared
with 1.049154291 seconds in its baseline. The later 0.4 candidate measured 1.068 seconds. Those slow
observations remain evidence of the gap that package closure fixes.

The registration and package-closure timed workspace run passed all 304 cases, with a maximum of
0.653576125 seconds. The marker composition test supplies distinct start and finish clock readings while
using the real state adapter. The deadline composition test passes a parsed seven-second deadline through
the real executor and gives the pipe holder only the final 80 milliseconds of the overall run budget. Its
diagnostic must distinguish those two durations and its recorded process group must be gone. The existing
pure policy tests retain own-deadline and run-budget expiry directions. These controls avoid a wall-clock
wait without changing the configuration's whole-second units or the production clock.

The current evidence combines the workspace library, command composition and integration batches with the
focused delivery check that added the actual pending-count adapter case. All 336 cases passed below one
second; the maximum was 0.650372375 seconds. Each of the 32 added cases failed its intended assertion
under a real source fault, across 35 independently compiled fault arms. The signed predecessor and final
release also matched on all 21 argument vectors, while the retained faulty executable differed on 16.
