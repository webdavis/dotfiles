# Architecture

The module roles, the extension model, SOLID, and the file-size standard. Reference for
[`SKILL.md`](SKILL.md). The language skill names the concrete construct behind every "module" here
(a crate, a target, a package) and the mechanism that enforces the direction.

## The five roles

Split the tool into five architectural boundaries, each a real build-system unit so the compiler,
not a convention, enforces the direction:

    <tool>-domain
    <tool>-application
    <tool>-protocol
    <tool>-adapters
    <tool>-cli

The dependency direction is one way:

    <tool>-domain
        <- <tool>-application
        <- <tool>-adapters
        <- <tool>-cli

`<tool>-protocol` is an independent external-contract unit used by adapters and the CLI. It must not
dictate the internal domain model.

**No dependency may point inward** from domain or application code to a concrete adapter. Declaring
the boundary in the manifest is what makes this real: an inward import then fails to build rather
than passing review by luck.

The executable keeps the tool's name, because every caller invokes it by that name. Update the
builder, the justfile and every dependent sibling in the same pull request as the conversion, and
prove all three by running their commands **before** moving any code into the new units.

A tool small enough that five units would outweigh the boundary they draw takes fewer. The boundaries
are the point, not the unit count: a tool with no external protocol needs no protocol unit. Say which
boundary you dropped and why.

### The domain unit

Keep it free of: filesystem access, databases, serialization formats, HTTP, environment variables,
process spawning, platform APIs, third-party service APIs, executable discovery, and command-line
output. Prefer standard-library-only domain code unless a small dependency represents a genuine
domain primitive.

Put pure policy here: the tool's neutral event and request types, its normalized signal types, its
decision and planning policy, its scheduling policy, its staleness and replay policy, its budgeting
policy, its invariants and value types.

**Make invalid states unrepresentable where practical.** Replace conflicting boolean combinations
with a closed set of alternatives. A scope expressed today as two independent booleans becomes one
typed value with one case per legal state.

A legacy flag combination that is illegal but *tested* does not become a domain state and gains no
case of its own. It is refused at the legacy adapter, with the tested wording.

Keep source-specific event names separate from the tool's normalized semantics. Policy must not
branch directly on every caller's arbitrary string.

### The application unit

Concrete use cases and consumer-owned ports.

Use **concrete use-case types by default**. Do not create an interface for a use case merely because
the use case exists.

Define interfaces only for meaningful external capabilities a use case consumes: destinations,
snapshot acquisition, external sources, clocks, and the repositories it reads and writes. Keep ports
narrow but cohesive: do not mechanically create one interface per method when several operations form
one transactional contract.

### The protocol unit

A stable, versioned, source-neutral contract for whoever talks to the tool. Where the callers are
separate processes, the transport is a documented text envelope (a JSON request on standard input,
for example), never a binary ABI a third party has to link against.

A request carries, at minimum: a schema identifier containing a **major version**, a caller-generated
request ID, caller identity, session and turn identity when available, the caller's own event name,
the **normalized** value the tool's policy actually keys on, occurrence time when available, bounded
detail text, typed context, and a bounded `extensions` object for caller-specific data.

Use tagged alternatives rather than boolean bags. The caller's own event name remains available as
source metadata but **must not be the value that directly controls the tool's core policy**.

Define a versioned result protocol alongside it: request ID, an accepted / degraded / rejected
status, a decision identifier, typed per-destination outcomes, and bounded diagnostic codes. Do not
echo private detail in the response by default.

Reject unknown major versions clearly. Define an explicit policy for unknown additive fields. Enforce
byte, field-count, text-length, collection-length and nesting limits at the protocol boundary.

Use request IDs to provide documented idempotency behavior. **Do not promise exactly-once external
delivery when a destination cannot support it**; document the behavior around retries, crashes and
duplicate IDs precisely (see [`PERSISTENCE.md`](PERSISTENCE.md)).

Keep the protocol transport-neutral. A future socket or daemon may carry the same envelopes, but do
not make a running daemon mandatory unless measured requirements justify that availability
dependency.

Create a **separate versioned egress protocol** for external destinations the tool drives. Do not
reuse the ingress request type as a delivery type, and do not reuse an already-rendered outbound
object as an ingress request.

### The adapters unit

All concrete infrastructure, organized by real capability: source adapters, destinations, each
external service, each platform reader, filesystem integrations, persistence, configuration loading
and rendering, bounded process execution, external data sources. **Do not create one broad
infrastructure module.**

### The CLI unit

Command decoding, standard input and output adaptation, exit-code translation, dependency
construction, and startup.

The entry-point file must contain no domain policy, state codec, filesystem transaction, request
construction for an external service, payload normalization, composition of an output document,
scheduling algorithm, or concrete delivery implementation. Target 50 to 150 lines, preferably under
100.

## Legacy compatibility

Retain the current flags and entry points as **adapters over the new use cases**, preserving their
tested behavior: a deliberately lenient argument parser, missing-value warnings, recognized flags not
being consumed as values, help behavior, typo refusal, the rule that a side-channel failure never
fails the work it reports on, standard output and error contracts, exit-code translation, exactly-once
behavior where currently specified, payload flattening and sanitization, observation events not
mutating state they do not own, and clearing events ending the right outstanding state.

Do not make the new domain model imitate the legacy parser's invalid states. Translate legacy
combinations explicitly at the adapter boundary.

## The extension model

**Do not create one universal `Plugin` interface** with optional methods or boolean capabilities. Use
separate interfaces and registries for separate roles, for example a destination, a stateful
indicator, a sensor, a diagnostic check, and a scheduled job.

A registry must contain actual implementations, not only names and metadata. A destination interface
carries an identity, a declared capability set, and one delivery operation returning a typed outcome.

Runtime polymorphism in the composition root's heterogeneous collection is appropriate. Do not spread
it through domain code without need.

**Remove central dispatch that switches on names.** Adding a built-in destination should require its
adapter, its registration in the composition root, contract tests, and adapter-specific tests. It
must not require a change to policy.

Executable destinations are one configurable adapter, driven by a command and declared capabilities,
speaking the versioned egress protocol on standard input.

A **stateful indicator is not a destination**. Something with reconciliation, held state, leases,
phases and quiet policy is its own role. A source of environmental facts is not a destination either.
Diagnostics and scheduled jobs are separate capabilities again.

## Environment acquisition and decision policy

Split the decision into three:

1. Pure determination of which environmental facts are required.
2. One snapshot acquisition operation.
3. One pure decision over the request, the settings, and the completed snapshot.

**The domain decision must not invoke probes** or coordinate probe startup.

Use a typed snapshot. **Each reading carries its own observation time**, not one shared epoch stamped
before the slow probes ran: a memoized clock makes two call sites agree on an epoch while the
readings themselves are taken at different moments, which is a different bug wearing the fix's
clothes. Preserve `unknown` separately from a confidently negative reading, and treat a reading from
the future as **bad input, not fresh input**: arithmetic that saturates a future timestamp to age
zero silently promotes garbage to the freshest evidence.

The concrete snapshot adapter may collect independent facts concurrently and memoize each reading.
Preserve these guarantees:

- a fact is observed at most once per snapshot
- slow independent probes do not serialize fast ones
- every decision in one submission uses one coherent snapshot
- an unavailable sensor does not silently become a false fact
- fail directions are explicit in policy, not hidden in parsers

**Failure direction is per input, not global.** "Unreadable means absent" applied across the board
produces a fresh reading holding a stale conclusion. Decide each input's direction on its own, and
treat a tie between two equally fresh inputs as uncertainty rather than as evidence for the default.
Urgency and interruption cost belong in the decision: "missing signals mean escalate" is right for a
request with a deadline and wrong for a routine completion at 3am.

Do not introduce a concurrency runtime solely to make the architecture look modern. A synchronous,
deadline-bounded model is acceptable. Use threads or synchronous operations where they remain simpler
and correct.

## SOLID

**Single responsibility.** Each file, module, type and function has one coherent responsibility and
one primary reason to change. A file must be describable in one sentence: "This module is responsible
for ______." If the sentence crosses policy, serialization, persistence, process execution and
presentation, split it.

**Open/closed.** Extend by registering implementations at the composition root, not by modifying core
policy or a central name switch. Do not create speculative extension points; add an abstraction for a
real variation or a real external boundary.

**Liskov substitution.** An interface declaration alone does not prove substitutability. For every
important interface with multiple implementations, write a reusable behavioral contract suite and run
it against each implementation. This applies most to in-memory versus durable repositories, built-in
versus executable destinations, and clocks and readers with a common contract.

**Interface segregation.** Do not recreate the current role distinctions as one interface with
optional behavior. Model each role separately.

**Dependency inversion.** Application use cases own the abstractions they consume; concrete adapters
implement them outward. No use case may construct an HTTP client, invoke a process, open a file, read
an environment variable, or decode configuration directly.

## Choosing the abstraction

Use a concrete type when there is one implementation and no substitution need; a function value for
one injected operation such as a clock or a mapper; an interface for a stable external capability or
a meaningful contract; a closed set of cases for a closed set of alternatives; a wrapper type for
validated identifiers and sensitive values; compile-time composition when it improves clarity;
runtime polymorphism at heterogeneous composition boundaries.

Do not create an interface for every type, create one-method wrapper types merely for injection,
spread runtime polymorphism through domain code, introduce type parameters that obscure the use case,
build a service locator or dependency-injection container, translate object-oriented patterns
mechanically, or hide branching inside macros to make files look shorter.

Prefer composition and explicit construction.

## Error and outcome design

Do not use a bare optional where several materially different failure or unknown states must be
distinguished for diagnostics or policy. Use typed outcomes for probe results, delivery results,
configuration loading, persistence claims, protocol acceptance, interaction results, and diagnostic
checks.

Keep infrastructure errors at adapter boundaries and map them into stable application outcomes
without erasing information required for diagnosis.

Do not crash on ordinary external failures. An assertion that aborts is acceptable only for a
compiled-in invariant whose violation is a programmer error and cannot depend on operator input or
runtime conditions. Never force-unwrap untrusted input.

## Concurrency, processes, and unsafe code

Every spawned thread or process has explicit ownership, a deadline, cancellation or termination
behavior, error observation, a cleanup path, and shutdown behavior. Do not leave detached processes
without a documented owner. Preserve process-group cleanup where children can spawn descendants.

Do not reach for a shared mutable lock by default. First consider ownership, immutable sharing,
message passing, task confinement, or a transaction.

Isolate unsafe platform and terminal operations in the smallest possible adapter modules, document
the safety invariants, add focused tests, and use the toolchain's sanitizers where applicable.

## Public API discipline

Private by default, widening one step at a time: internal to a type, then to a module, then to the
package, and public only for an intentional external API. Do not expose the internal module tree so
integration tests can reach it. Curate public exports.

The primary stable public boundaries are the versioned protocols, narrow application-facing request
and outcome types where needed, and any unit a sibling package depends on.

Do not expose raw configuration tables, adapter internals, state codecs, or concrete platform types
as general public API.

## The file-size standard

Applies to every handwritten source file, including test files. Track two numbers:

1. **Implementation lines**: the lines before the file's trailing test section. A file that is
   entirely tests has zero. For this count to be honest the test marker may appear only where the
   trailing test section begins: **a test-only item above production code is itself a finding**, not
   a way to shrink the number.
2. **Total physical lines**, including inline tests, documentation, comments and blanks.

Targets, after the formatter has run:

- no more than 200 implementation lines, and no more than 300 total
- 201 to 250 implementation lines require a responsibility review
- 301 to 400 total lines require a responsibility and test-organization review
- above 250 implementation lines normally requires decomposition
- above 400 total lines normally requires decomposition
- **no handwritten source file may exceed 500 total lines. There is no waiver.**

At completion the entry-point file is below 150 lines; module-declaration files primarily declare or
curate modules and public exports; no existing large file is grandfathered; large acceptance suites
are decomposed by behavior.

The language skill carries the exact counting command, because the marker for "where the tests begin"
is language-specific and a generic line counter mis-parses these trees.

Nothing enforces this automatically. The completion report runs the counting command over every
handwritten source file and lists every result. **A file-size check is not a test**: tests here pin
the behavior of tools we wrote, and a meta-test about code shape gets deleted on sight.

Generated code, machine-generated bindings, and fixture or data files are exempt. Large declarative
data belongs in a fixture or data format rather than in executable code.

**Do not game the count** by compressing statements, deleting useful rationale, removing
documentation, using cryptic names, hiding logic in macros, creating arbitrary submodules, creating
`part_1`, moving unrelated code into `utils`, `helpers`, `common` or `misc`, or introducing needless
interfaces and wrappers. Small files must result from meaningful responsibility boundaries.
