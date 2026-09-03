# Persistence, delivery safety, and configuration

Where durable state lives and how it survives a crash. Reference for [`SKILL.md`](SKILL.md).

## Classify the state before choosing a store

Derive the state inventory **yourself, from the source**. An inventory you were handed is not
evidence: one circulated for `pns` carried four test-fixture names in it. Grep for every path the
code writes, then classify each as:

1. a public or external contract
2. an internal persistence detail
3. a temporary process-coordination mechanism
4. a compatibility artifact

Look for two things the classification usually surfaces. **Deletion targets**: names a sweep removes
on every tick and never reads, which are legacy debris rather than state. And **orphans**: a name
nothing in the source writes, or one nothing reads outside tests.

Implement migration for durable user state that should survive. Do not silently discard existing
state.

## SQLite or the filesystem

Replace internal durable multi-record state with a transactional SQLite adapter **unless the
filesystem path, name, metadata, or existence is itself an external integration contract**. SQLite is
the right store for a same-host, low-volume, multiprocess workload, in WAL mode, with **every caller
handling a busy database with a bounded timeout**. Prefer a synchronous driver over an asynchronous
database layer unless a runtime requirement proves otherwise.

Use versioned migrations, explicit transactions, WAL mode where appropriate, bounded busy timeouts,
restrictive database permissions, typed codecs, crash-recovery tests, and multiprocess contention
tests.

Create **semantic repositories, not one generic state store**: one per family of records, named for
what it holds. One SQLite type may implement several application-owned repository traits. Provide
in-memory implementations for fast application and contract tests, and run the same contract suite
against both.

### Two facts that shaped the existing filesystem protocols, and still bind

**Concurrent `unlink` does not arbitrate on this filesystem.** It reports success to every racer. Any
filesystem protocol that remains owns a file **by rename or by `O_EXCL` creation, never by removal**.

**Writers are many processes, not one.** Every hook is a short-lived process fired by a harness, a
daemon is one long-lived process, a shell notifier and a sibling tool's alert path are more. Name the
writers for each state family before choosing its store.

**A side-channel must never fail the work it reports on.** So on the delivery path a busy, locked,
missing or corrupt database is **fail-open**: deliver, and record the miss where recording is
possible. State mutation is **fail-closed**. Write this as a decision record before the first SQLite
code lands.

### Where the filesystem stays

Keep filesystem adapters where the filesystem genuinely is the interface: configuration files,
executable discovery, operating-system files and terminal metadata, markers a third party observes,
configuration publication and backups, and compatibility state external tools consume.

Do not collapse atomic publish, append, claim, lease and marker behavior into a vague `FileStore`
abstraction. Where a filesystem protocol remains, **name its semantic operation and test its actual
race behavior.**

## Delivery safety

For a tool that sends things outward, five rules. They are one mechanism, not five features.

**One idempotency key per event, minted at creation.** Carry it in the body and in an
`Idempotency-Key` header, and **a replay carries the ORIGINAL key, never a fresh one**. The argument
is not hypothetical retries: a replay path that re-sends journaled misses will re-send one that
actually got through, and nothing downstream can tell.

The key identifies **the caller's logical submission**, `(caller identity, caller request id)`, never
the content: two legitimate events can carry identical bodies. A unique delivery is
`(event id, destination instance)`.

**Say at-least-once, honestly.** Only a destination that persists and enforces the key can
deduplicate. A destination returning a boolean cannot; a spawned process proves only that a process
ran; an executable destination whose exit status is ignored proves nothing at all. Document those as
at-least-once rather than implying exactly-once.

**Do not retry inline.** A synchronous path is often synchronous deliberately, because that is what
makes a failure visible, and a notification must never fail or stall the work it reports on. Hand a
failed send to the component that already runs leased jobs, so the retry is off the hot path and
observable as a job rather than as a stall.

**Write the journal entry BEFORE attempting delivery, and clear it on confirmed success.** The hazard
is an unknown outcome, which is exactly what a timeout produces. Write-ahead turns "might lose an
event" into "might deliver twice", and the idempotency key makes the second harmless.

**The outbox row IS the queue.** A failed leg stays pending and the daemon leases that row. Do not
pair a journal row with a second job record: one mechanism, one place to look. Successful rows are
marked acknowledged and **retained as audit history, never deleted**. Label "acknowledged" honestly:
a gateway accepting a request is not the operator seeing it.

**A recording decorator wraps each destination at the composition root**, so adding a destination
stops requiring anyone to remember to record it. It is **fail-quiet**, meaning it does not change the
delivery outcome, *not* meaning it is invisible: record two separate facts, what the destination
acknowledged and whether that was recorded, and surface a recorder failure through the daemon log and
the diagnostic command. Do not flatten the tool's domain records (decision ring, journal, activity
ring, policy audit) into it; those have their own readers and retention.

**Order is best-effort.** Deferring a failed send while a later one succeeds inline reorders messages
by construction. Promise best-effort order, record a monotonic sequence, and do not pretend to
per-destination ordering, which would require every send to pass through the outbox.

### The delivery answer must be authoritative

**"Where did this event go" has exactly one answer, in one place, derived from OUTCOMES.** Asking the
*plan* whether something was intended is not that answer: a configured backend that is unavailable, a
contradictory override pair, or a destination that simply fails all deliver nothing while the journal
records a success, so the operator is never told and nothing is queued for replay.

Watch for competing authorities. Any code path that decides delivery from partial inputs before the
plan exists, or that runs independently of the tool's own gating, is a second authority that will
disagree with the first.

### Crash windows

Audit the ordering of every write against every send:

- Delivery happening before both the decision record and the journal loses both on a crash.
- A replay path that deletes its claim **before** delivering loses the event on a crash, and a test
  asserting the claim never survives the run pins that loss window as intended behavior.
- A drain that excludes claimed rows from its scan while the worker deletes its claim before spawning
  loses the job on either side of a restart.

## Configuration

Separate: raw deserialization types, validated application settings, adapter-specific settings,
schema and key metadata, template rendering, filesystem loading, setup answers, publication,
migrations, and operator diagnostics.

**Domain and application code must not depend on a dynamic configuration value type** or on free-form
tables.

Use strict decoding, and preserve tested rejection of unknown keys, unknown component names, invalid
bounds, malformed values, and unsafe paths.

Create **one authoritative definition** for every key name, default, bound, secret classification and
component identity. The renderer, the setup wizard, the decoder and the documentation are all checked
against those same definitions. Retain golden and anti-drift tests.

Do not create a large custom schema language unless it demonstrably reduces more complexity than it
introduces.

**Give secrets a type that cannot reveal its value through ordinary `Debug` or `Display`.** Keep
secrets out of diagnostics, traces, protocol responses, and test failure text. Where a shipped
template pulls secrets from a password manager, the exact template action is a contract pinned by
tests, with the test stub refusing any other action: preserve both.

Add an explicit configuration version and migrations. A version key changes the rendered template,
which the operator sees on `chezmoi diff` and applies themselves. Say so in the report.

Where a generated file is rendered from committed values, keep the renderer's own tests against
**fixtures the crate owns**, and pin `template == render(values)` from the outer repository instead.
A package that reaches out of its own folder, by a compile-time file include or a runtime path join, stops building
the day it moves repositories.
