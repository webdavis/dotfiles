# Run application ownership

The run command previously owned configuration loading, process construction, sequencing, presentation
and durable-state decisions in one function. `uu-application::Run` now owns the sequence and the record,
marker and staleness decisions. The command loads configuration, supplies declared names and resolved
deadlines, constructs adapters and maps the typed result to the existing exit behavior.

## Dependency direction

| Package               | Responsibility and direct dependencies                                                                                                                 |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `uu-domain`           | Reports, marker facts, deadlines and streak policy; no dependencies.                                                                                   |
| `uu-application`      | Run sequencing and consumer-owned ports; depends only on `uu-domain`.                                                                                  |
| `uu-protocol`         | Existing child-event and record encodings; independent of domain and application.                                                                      |
| `unattended-upgrades` | Transitional command and concrete adapters; depends on those three packages, `pns` and its existing infrastructure libraries. Its binary remains `uu`. |

`Marker` and `RunFacts` belong to the domain because they describe the previous successful run and the
facts supplied to a lane, without prescribing an encoding. `src/record/event.rs::event_for` maps them
into protocol types at the existing adapter boundary. No serializer or free-form configuration enters the
application. The child event, record payload and command arguments keep their existing formats; this row
introduces no protocol version or compatibility facade.

## Ports and their current adapters

| Consumer-owned port | Current adapter                        | Boundary                                                                                                 |
| ------------------- | -------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| `RunState`          | `run_adapters::FileRunState`           | Lock lifetime, marker and streak reads/writes, removed-lane pruning.                                     |
| `RunClock`          | `run_adapters::SystemRunClock`         | Fallible wall-clock samples and monotonic elapsed time.                                                  |
| `LaneExecutor`      | `run_adapters::ConfiguredLaneExecutor` | A runner per lane, supplied with actual budget and declared deadline; existing lane dispatch stays here. |
| `RunDelivery`       | `delivery::EngineRunDelivery`          | Configured alert process, record encoding/signing and signed POST through the existing clients.          |
| `RunPresentation`   | `run_adapters::ConsoleRunPresentation` | Header facts, detail rendering and exact diagnostic streams and wording.                                 |

The associated state guard keeps ownership explicit without exposing a filesystem lock to the use case.
Typed outcomes distinguish contention from unavailable locking, absent from unreadable history, missing
delivery configuration from failed delivery, and record rejection causes from success. Private policy
helpers may reduce a reported outcome to a marker decision; the port still preserves its cause.
Presentation supplies a header once for the lanes and printed detail. The existing delivery adapter
continues to sample the host for alerts and the outbound record; this is not one host lookup for every
output in a run.

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
- Keep record receipt separate from alert receipt. A failure alert cannot substitute for a missing weekly
  record, and a deferral cannot establish that every lane ran successfully. Both retain the old success
  marker. With no configured record channel nothing was owed, so a clean run may advance it.
- Keep name order and the existing budget policy. A configuration reorder must not reorder execution.
  Failure in one lane must not prevent later lanes from running. The six-hour default was chosen with
  margin for long package builds, not from measured lane runtimes; the 24-hour lane budget prevents a
  sequence of defaulted lanes from consuming the next weekly slot. It does not time every adapter call.
- Keep filesystem protocols in this row. The success marker remains `last-success`, and each lane's count
  remains `lanes/<name>/streak` under the existing state directory. Streak publication writes a sibling
  temporary file and renames it over the target, allowing replacement of a read-only old file. The
  temporary name includes target name and process identifier to distinguish targets in parallel fixtures.
  Marker publication still uses the existing direct write. There is no new permission, transaction or
  crash-durability guarantee.
- Preserve pruning's actual limit. Pruning requests the full declaration set under the run lock to avoid
  reusing removed-lane history. The filesystem implementation remains best effort and silent on failure.
  A failed removal can leave old history behind, so a claim that it can never affect a reused name would
  be stronger than the implementation.
- Preserve process ownership in the existing adapters. The signed POST remains bounded to ten seconds;
  alert process waiting remains unbounded. Lane watchdogs own process groups, pipe draining and their
  existing stuck-spawn and escaped-process limitations. The application neither claims those processes
  are gone nor introduces a second cleanup path.

Concrete adapters and command parsing still live in the root package. Row 0.4 owns their destination
crates and the external shipped-template fixture boundary. Row 0.5 owns lane registrations and removal of
the central technology-name switches. Row 0.6 closes remaining visibility, size and verification
obligations. This row does not claim those later boundaries are complete.

## Test ownership and evidence

[The name map](../test-name-map.tsv) records every one of the 287 baseline leaf names and its successful
successor, plus the new notification-order integration test and twelve application tests. No baseline
leaf is removed. Two record-body call-site tests move from `cli::run::tests` to `delivery::tests` with
new qualified names. Two delivery-policy tests keep their names but move from the binary's delivery
module into the application crate; their fixtures now supply typed outcomes rather than a transport. The
map records those ownership changes explicitly. These moved tests remain behavior contracts. The seven
inherited harness speed-guard leaves instead check test infrastructure; the map preserves them and
assigns their classification and timing closure to row 0.6. Their presence is not evidence of product
behavior or a guarantee that every test is fast.

The record-body tests still protect the adapter call site: dropping the deferred count there can post
`completed` while the pure verdict tests stay green. The moved delivery-policy tests protect refusal and
success directions through injected outcomes. The twelve new tests exercise the whole use case through
concrete in-memory ports, including guard lifetime, ordering, budget exhaustion, continued execution,
selection, lock refusals, finish-clock failure and the record-to-marker decision.

The combined package run recorded 300 expected successful outcomes with no missing or extra names. Rust
formatting, all-target checking, strict Clippy, workspace tests, strict documentation and a locked
release build passed on that candidate. Seventeen independently compiled application mutants failed the
intended assertions against green controls. This evidence does not replace repository gates or the
command-surface differential. The inherited test
`a_lane_that_outlives_its_deadline_fails_instead_of_holding_the_run_open` took 1.0611155 seconds in this
run, compared with 1.049154291 seconds in the baseline; its timing remains a row 0.6 closure item.
