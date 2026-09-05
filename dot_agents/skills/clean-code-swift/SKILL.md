---
name: clean-code-swift
description: "The Swift bindings of this repository's clean-code standard. Use when building or restructuring a Swift package, framework or Xcode project: drawing module and target boundaries, choosing between a protocol, an enum and a concrete type, picking access levels, writing contract test suites and test doubles, or running the Swift quality gates. Read the language-neutral method alongside it."
---

# Clean code: Swift

**Read `~/.agents/skills/clean-code/SKILL.md` first.** That skill carries the method: the
ordered ladder, the five module roles and their dependency direction, SOLID, the test obligations,
the delivery ladder, the sol review, and the completion report. This file states only how those are
spelled and enforced in Swift, and it wins wherever the two disagree on a number or a mechanism.

The worked example is [`ESSENTIAL-FEED-EXAMPLE.md`](ESSENTIAL-FEED-EXAMPLE.md), read from
`essentialdevelopercom/essential-feed-case-study` on 2026-09-03. Facts below are marked
**(measured)** where they come from that repository or from a probe run on this machine.

Toolchain here (measured): Apple Swift 6.3.3; `swift`, `swiftc`, `xcodebuild` and `sourcekit-lsp` at
`/usr/bin`, shipped with the toolchain and declared nowhere; `xcode-build-server`, `xcbeautify`,
`swiftformat` and `swiftlint` from Homebrew, declared in
`.chezmoidata/system_packages_autoinstall.yaml`. Neovim drives builds and tests through
`xcodebuild.nvim`.

## Where the boundary goes, and what enforces it

**A module is the unit of enforcement, and it is the only unit of enforcement**, whether it is a
SwiftPM target or an Xcode framework target. But a plain `swift build` DOES NOT ENFORCE IT, and the
gate has to be asked for explicitly. Everything else this skill calls a boundary is review, not a
gate.

Measured on Swift 6.3.3, reproduced 2026-09-04 with SwiftPM targets `A` and `C` declared independent
and a file in `A` doing `import C`:

| command                                                       | undeclared import |
| ------------------------------------------------------------- | ----------------- |
| `swift build`, fresh tree, default parallelism                | REFUSED, 8 of 8   |
| `swift build -j 1`, fresh tree                                | **accepted**, 6 of 6 |
| `swift build`, second run of the same tree, nothing fixed     | **accepted**      |
| `swift package clean && swift build`                          | REFUSED           |
| `swift build --target A --scratch-path <empty dir>`           | REFUSED, `-j 1` too |
| `swift build --explicit-target-dependency-import-check error` | REFUSED, any tree |

**THE DEFAULT BUILD'S ANSWER IS A SCHEDULING ARTIFACT, NOT A CHECK.** A target compiles against
whatever modules are on its search path when IT is compiled. With no dependency edge between `A` and
`C`, nothing orders `C` first, so the parallel scheduler usually emits `A` before `C` and `A` fails
to find it. Serialize the build and the order flips:

    swift build -j 1     # exit 0, Build complete! -- the SAME undeclared import
    swift build          # exit 1, error: no such module 'C'

Six fresh trees under `-j 1`, six acceptances, under both manifest orderings. So the refusal a clean
whole-package build gives you is luck, and it evaporates on a serial builder, a different scheduler,
or simply a second run: once `C`'s module is in the scratch path, every later build finds it there
and **a retry of a red build goes green with the boundary still broken**. A check that answers by
build order is worse than no check, because it reads green while it lies.

Two things ARE gates, and one of them is the one to use:

- **`--explicit-target-dependency-import-check error`.** Ask SwiftPM the question directly:

      swift build --explicit-target-dependency-import-check error

  It refuses regardless of build order or scratch state, populated tree included:
  `error: Target A imports another target (C) in the package without declaring it a dependency.`,
  exit 1. This is the one to put in CI, because it is the only form that does not depend on how the
  build happened to be scheduled.
- **An isolated target build against an empty scratch path**, which also held under `-j 1`:
  `swift build --target <Target> --scratch-path "$(mktemp -d)"`. Useful for checking one protected
  target in isolation; the explicit check covers the whole package in one command.

A cycle declared in the manifest needs neither, and is refused before compilation:
`error: cyclic dependency declaration found: A -> B -> A`.

**NEITHER GATE REACHES PAST THE DECLARED CHAIN.** Measured 2026-09-04: with `A` declaring `D`, `D`
declaring `C`, and a file in `A` importing `C` and calling its functions, `swift build`,
`swift build --target A --scratch-path <empty dir>` and
`swift build --explicit-target-dependency-import-check error` ALL exit 0. Cargo refuses the same
shape outright (`error[E0433]`, measured), so this is NOT the guarantee Cargo gives. It is the weaker
one: a module may reach anything its declared neighbours pull in, and no build flag recovers the
difference. A transitive import is caught by reading the import lines, or not at all.

The architecture survives that, because the direction it protects runs the other way. The domain
declares no module of ours, so nothing of ours is transitively reachable from it, and the UI still
cannot leak inward. Infrastructure is a different question: it shares `<Tool>` with the domain, so
that direction never crosses a target boundary and stays REVIEW-ONLY. What does not survive is
reviewing a MIDDLE module off the build alone: `<Tool>UI` declares `<Tool>`, so it can import
whatever `<Tool>` imports, a persistence framework included, and compile. Read that module's import
lines by hand; the compiler is not checking them for you.

**The five roles do not need five modules, and in practice they should not be.** The case study
(measured) draws exactly three hard boundaries:

| Module          | Imports                            | Holds                                             |
| --------------- | ---------------------------------- | ------------------------------------------------- |
| `<Tool>`        | Foundation, a persistence framework | domain, use cases, protocols, adapters, presenters |
| `<Tool>UI`      | UIKit or SwiftUI, `<Tool>`         | views and view controllers                        |
| `<Tool>App`     | both, plus UIKit                   | the composition root, and nothing else            |

Inside `<Tool>`, the roles are **folders**: a `Feature/` folder for the domain types and the ports,
an `API/` and a `Cache/` folder for adapters, a `Presentation/` folder for presenters.

**A FOLDER ENFORCES NOTHING. IT IS ORGANIZATION FOR A HUMAN READER.** Measured 2026-09-04:
`Feature/Policy.swift` importing CoreData and calling an `internal` type declared in
`Infrastructure/Adapter.swift` compiles to `Build complete!`, exit 0, both on a fresh build and again
after `swift package clean`. `internal` scopes a symbol to its MODULE, and the two folders are the
same module, so every symbol in it is already visible to every other file in it. Access control adds
nothing across a folder line, because there is no line there to cross.

**Draw a hard module boundary where the dependency direction must be checked by
`--explicit-target-dependency-import-check error`** (the UI must not be reachable from the domain;
the app must not be reachable from either), and run that check, because a plain build does not.
Inside a module, a folder records the intent and a REVIEWER is the only thing holding it: read the
import lines and the call sites by hand, per pull request. If a direction inside a module matters
more than review can carry, it is not a folder, it is a target.

This is a deliberate adaptation of the Rust five-crate rule, and **it is narrower than it looks.**
The module split catches ONE of the two directions that go wrong: UI leaking inward, because `<Tool>`
and `<Tool>UI` are separate targets. It catches NOTHING of infrastructure leaking into the domain,
because the table above puts both inside `<Tool>`, and both the fresh and the post-clean build accept
that leak (measured above). Infrastructure-to-domain is REVIEW-ONLY here. If that direction has to be
enforced rather than reviewed, the adapters need their own target.

### What the domain excludes

The filesystem and networking APIs, any database framework, `ProcessInfo` environment reads,
`Process` spawning, AppKit, UIKit and SwiftUI, vendor SDKs, and any printing. Prefer the standard
library; reach into Foundation only for a genuine primitive such as `UUID`, `Date` or `URL`.

The case study holds to this exactly (measured): its framework module imports only Foundation and
CoreData across its 36 files, and never UIKit. The UI module imports UIKit and the framework. Nothing
imports the app.

## The composition root

One module whose only job is to build the object graph, and the only place that knows every other
module. In the case study it is 8 files and 493 lines (measured), holding the scene delegate, two
composers, two adapters, and a weak-reference proxy.

Rules it must keep:

- It is the only place that constructs concrete adapters and wires them to ports.
- It is where decorators and proxies live, so a cross-cutting concern (recording a delivery, holding
  a weak reference, falling back from remote to cache) is composed in rather than remembered by each
  collaborator. The case study composes its remote-with-local-fallback strategy here, not inside
  either loader.
- It carries an injection seam for tests. The case study's `SceneDelegate` has a
  `convenience init(httpClient:store:)` used only by the acceptance tests, so the whole app can be
  driven with stubbed infrastructure.
- It contains no policy. If a decision about *what should happen* is being made there, it belongs in
  the domain.

## Swift abstraction choices

- a **struct** when there is one implementation and no substitution need; value semantics are the
  default, and a reference type needs a reason
- a **closure** for one injected operation. The case study injects the clock as
  `currentDate: @escaping () -> Date = Date.init`, which is the neutral rule's "a closure for one
  injected operation" spelled exactly
- a **protocol** for a stable external capability. Keep them **one method wide** unless several
  operations form one transactional contract: the case study's ports are
  `func save(_ feed: [FeedImage]) throws`, `func loadImageData(from url: URL) throws -> Data`, and
  `func get(from url: URL) async throws -> (Data, HTTPURLResponse)`
- **protocol composition** (`FeedStore & FeedImageDataStore & Scheduler & Sendable`) to require
  several narrow capabilities of one collaborator without inventing a wide protocol
- an **enum with associated values** for a closed set of alternatives. This is how "make invalid
  states unrepresentable" is spelled in Swift, and exhaustive `switch` without a catch-all is what
  makes it stronger than a boolean pair
- a **wrapper struct** for validated identifiers and sensitive values, with a failable or throwing
  initializer doing the validation
- a **`final class` with a `private init()`** for a stateless policy namespace, as the case study's
  cache-expiry policy does. Not a pattern to reach for often, but correct where a policy has no state
- an **existential (`any Protocol`)** at the composition root's heterogeneous collections only.
  Write `any` explicitly; do not let an implicit existential hide a boxing decision

Do not create a protocol for every type, create one-method wrapper types merely for injection, spread
`any` through domain code, or reach for a class where a struct works. Prefer a protocol extension's
default implementation over inheritance, and mark a class `final` unless subclassing is the design.

## Access levels

Narrowest to widest: `private`, `fileprivate`, `internal` (the default, so write nothing), `package`,
`public`, `open`.

`internal` keeps a type inside its MODULE. It does not keep it inside its folder, so it is not what
makes the folder-based role split safe (measured above); the module split plus review is. Make a
type `public` only when it crosses a module boundary, which in practice means the domain models, the
ports, and the presenters the UI module consumes.

`package` (measured working across two targets on 6.3.3) is for cross-target collaboration inside one
package that must not escape it. On a three-module layout it rarely comes up; reach for it when a
module split forces a symbol wider than `internal` for no external reason.

`open` is almost never right here: nothing in these tools is designed for external subclassing.

## Errors and outcomes

**`async throws` is the idiomatic port shape**, not a modernism to resist. Every port in the case
study is a throwing function, and asynchronous where the work is. Use `Result` where an outcome must
be stored, compared or replayed rather than propagated immediately.

Do not use a bare `Optional` where several materially different failure or unknown states must be
distinguished: `nil` collapses "absent", "unreadable" and "unknown" into one value, which is the
failure-direction bug the method warns about.

**Never force-unwrap (`!`) untrusted input**, and never `try!` on anything that depends on operator
input or runtime conditions. `fatalError` is acceptable only for a compiled-in invariant whose
violation is a programmer error.

Adopt strict concurrency (`swiftLanguageModes: [.v6]`), which makes data-race safety a compile error.
Mark value types `Sendable` deliberately; `@unchecked Sendable` is the Swift equivalent of an
unexplained lint suppression. Every task you spawn still needs an owner, a deadline and a
cancellation path.

## Tests

The neutral obligations are unchanged: test-first for new behavior, proof-of-no-change for a move,
mutation verification by hand, a deadline on anything that can hang. What follows is how the case
study spells them, and it is worth copying wholesale.

**Contract suites are protocols whose method names are the test names.** This is the strongest idea in
the case study and Rust has no equivalent:

    protocol FeedStoreSpecs {
        func test_retrieve_deliversEmptyOnEmptyCache() async throws
        func test_retrieve_hasNoSideEffectsOnEmptyCache() async throws
        func test_insert_overridesPreviouslyInsertedCacheValues() async throws
        // ...
    }

    protocol FailableInsertFeedStoreSpecs: FeedStoreSpecs {
        func test_insert_deliversErrorOnInsertionError() async throws
        func test_insert_hasNoSideEffectsOnInsertionError() async throws
    }

A test class declares `class CoreDataFeedStoreTests: XCTestCase, FeedStoreSpecs`, and **the compiler
refuses to build if any case of the contract is missing**. The capability protocols compose with `&`,
so an implementation that cannot fail on insert is not forced to fake that test. The shared
assertions live as free functions taking `on sut:`, so the bodies are written once and each
implementation's class holds only the wiring. The case study runs the same contract against its
CoreData store and its in-memory store.

That is the neutral rule "for every important interface with multiple implementations, write a
reusable behavioral contract suite" turned from a convention into a build error. Use it.

**`makeSUT` is the only way a test builds its subject:**

    private func makeSUT(
        currentDate: @escaping () -> Date = Date.init,
        file: StaticString = #filePath, line: UInt = #line
    ) -> (sut: LocalFeedLoader, store: FeedStoreSpy) {
        let store = FeedStoreSpy()
        let sut = LocalFeedLoader(store: store, currentDate: currentDate)
        trackForMemoryLeaks(store, file: file, line: line)
        trackForMemoryLeaks(sut, file: file, line: line)
        return (sut, store)
    }

Three things at once: one place to change when a constructor moves, the clock injected as a closure
with a real default, and every instance leak-checked.

**Every instance is checked for leaks**, which has no Rust equivalent because ownership is static
there:

    extension XCTestCase {
        func trackForMemoryLeaks(_ instance: AnyObject, file: StaticString = #filePath, line: UInt = #line) {
            addTeardownBlock { [weak instance] in
                XCTAssertNil(instance, "Instance should have been deallocated. Potential memory leak.", file: file, line: line)
            }
        }
    }

A retain cycle in a composed graph is exactly the defect a decorator-heavy composition root
introduces, and this catches it in every test rather than in Instruments later.

**Thread `file:` and `line:` through every shared helper.** Without it a failure inside
`assertThatRetrieveDeliversEmptyOnEmptyCache` reports at the helper's line and tells you nothing
about which of the twelve conforming cases broke. With it the failure lands on the caller.

**A spy records messages, it does not count calls:**

    class FeedStoreSpy: FeedStore {
        enum ReceivedMessage: Equatable { case deleteCachedFeed, insert([LocalFeedImage], Date) }
        private(set) var receivedMessages = [ReceivedMessage]()
    }

One equatable array pins which messages arrived, with what arguments, in what order. A call-count
assertion pins none of those.

**Test layers, all present in the case study:** unit tests per module; a cache integration suite that
runs the real store; an API end-to-end suite against the real endpoint; an acceptance suite that
drives the whole app through the composition root's injection seam with stubbed infrastructure; and
snapshot tests for the UI. Keep the layers separate, and keep the network out of everything except
the end-to-end suite.

**Tooling.** No mutation-testing tool for Swift is installed here, so mutation verification is by
hand, per behavior, against an unmutated control, exactly as in Rust. Thread and address sanitizers
are available; the case study's own CI runs `-enableThreadSanitizer YES`, which is worth copying for
anything with concurrency.

**No `#[cfg(test)]` equivalent.** Swift tests live in a separate target, so a file is either
production or test and the implementation-versus-total split is structural. Test-only code inside a
production file behind `#if DEBUG` counts as implementation and is a finding: it belongs in the test
target.

## Quality gates

    swift build --explicit-target-dependency-import-check error
    swift test
    swiftformat --lint .
    swiftlint

For an Xcode project rather than a package, `xcodebuild clean build test -project <p> -scheme <s>`,
piped through `xcbeautify`. `just lint-check` and `just ship` still gate the repository as a whole.

The neutral method requires the build to run from a committed lockfile. In Swift that is
`Package.resolved`, committed, with `--disable-automatic-resolution` on `swift build` and `swift test`
as the analogue of cargo's `--locked`. **Not measured**: no Swift package of ours exists to run it
against yet, so confirm the flag on the first one rather than trusting this line.

**No `just test-swift` recipe exists yet (measured: neither the justfile nor
`.github/workflows/lint.yml` mentions Swift).** The first Swift tool this repository owns adds one
alongside its entry in CI's gate list, and the two must be edited together by hand.

Do not add broad suppressions. A `swiftlint:disable` must name the rule, cover the narrowest possible
region, and explain why the rule is wrong there.

## File size

Count lines per file, split by target:

    git ls-files '*.swift' | tr '\n' '\0' | xargs -0 wc -l | grep -v ' total' | sort -rn

The `tr`/`-0` is load-bearing: Xcode project folders routinely contain spaces, and a plain pipe to
`xargs` splits those paths and silently drops files.

**Swift has its own two numbers (operator ruling 2026-09-03), and they are NOT the Rust ones:**

- **A production file must not exceed 200 lines. That is a hard cap, not a target.**
- **A test file may run to 700 lines.**

The Rust skill's 300 and 500 do not apply here. Do not carry them across.

The case study is the evidence that the production cap is a backstop rather than a squeeze. Measured
across the 120 Swift files of its three architecture targets (the repository tracks 124; the four
remaining sit in the standalone `Prototype/` throwaway app, which is not part of the architecture):

| | files | total lines | median | p90 | max | over 200 |
| --- | --- | --- | --- | --- | --- | --- |
| production | 59 | 2,058 | 24 | 62 | 164 | 0 |
| tests | 61 | 4,574 | 50 | 156 | 590 | n/a |

Every production file in a 493-star teaching repository sits under the cap, with a median of 24 and a
largest file of 164. Well-factored Swift does not come close, so a file approaching 200 is a
responsibility problem long before it is a length problem.

**The 590-line test file is compliant**, and if you remember an earlier draft calling it a violation,
that draft was written against the Rust numbers. Under the 700-line test allowance it needs no
exemption and no decomposition on size grounds alone. Split it if it covers several behaviors, which
is the neutral rule about acceptance suites, not because of its length.

## Open for Swift

Rules from the neutral skill with no answer in the case study, recorded rather than invented:

- **The versioned egress protocol and the executable-destination adapter.** The case study drives no
  external executables, so it demonstrates nothing here. The neutral rule stands unadapted.
- **Process ownership and process-group cleanup.** No subprocess spawning anywhere in it.
- **A busy-database policy.** It uses CoreData, not SQLite with concurrent writers, so it has no
  answer to the multiprocess contention rule. A Swift tool of ours with several writing processes
  needs that rule worked out before the first store lands.
- **The committed-lockfile gate.** `Package.resolved` plus `--disable-automatic-resolution` is the
  intended analogue of `--locked`, but its behavior was not measured. Confirm it against the first
  Swift package this repository owns before treating it as a gate. The file-size numbers above are
  no longer open: the operator ruled them on 2026-09-03.
