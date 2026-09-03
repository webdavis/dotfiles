---
name: tool-architecture-swift
description: "The Swift bindings of this repository's tool-architecture standard. Use when building or restructuring a Swift package, framework or Xcode project: drawing module and target boundaries, choosing between a protocol, an enum and a concrete type, picking access levels, writing contract test suites and test doubles, or running the Swift quality gates. Read the language-neutral method alongside it."
---

# Tool architecture: Swift

**Read `~/.agents/skills/tool-architecture/SKILL.md` first.** That skill carries the method: the
ordered ladder, the five module roles and their dependency direction, SOLID, the test obligations,
the delivery ladder, the sol review, and the completion report. This file states only how those are
spelled and enforced in Swift, and it wins wherever the two disagree on a number or a mechanism.

The worked example is [`ESSENTIAL-FEED-EXAMPLE.md`](ESSENTIAL-FEED-EXAMPLE.md), read from
`essentialdevelopercom/essential-feed-case-study` on 2026-09-03. Facts below are marked
**(measured)** where they come from that repository or from a probe run on this machine.

Toolchain here (measured): Apple Swift 6.3.3; `swift`, `swiftc` and `xcodebuild` at `/usr/bin`;
`sourcekit-lsp`, `xcode-build-server`, `xcbeautify`, `swiftformat` and `swiftlint` installed and
declared in `.chezmoidata/system_packages_autoinstall.yaml`. Neovim drives builds and tests through
`xcodebuild.nvim`.

## Where the boundary goes, and what enforces it

**A module is the unit of enforcement**, whether it is a SwiftPM target or an Xcode framework
target. A module can only `import` what it declares as a dependency, so the inward dependency the
method forbids does not compile.

Measured on Swift 6.3.3, with SwiftPM targets:

- An undeclared cross-module import fails with `error: no such module '<Module>'`.
- A cycle declared in the manifest is refused before compilation:
  `error: cyclic dependency declaration found: A -> B -> A`.

That is the same guarantee Cargo gives, through a different file.

**The five roles do not need five modules, and in practice they should not be.** The case study
(measured) draws exactly three hard boundaries:

| Module          | Imports                            | Holds                                             |
| --------------- | ---------------------------------- | ------------------------------------------------- |
| `<Tool>`        | Foundation, a persistence framework | domain, use cases, protocols, adapters, presenters |
| `<Tool>UI`      | UIKit or SwiftUI, `<Tool>`         | views and view controllers                        |
| `<Tool>App`     | both, plus UIKit                   | the composition root, and nothing else            |

Inside `<Tool>`, the roles are **folders plus access control**, not targets: a `Feature/` folder for
the domain types and the ports, an `API/` and a `Cache/` folder for adapters, a `Presentation/`
folder for presenters. This works because Swift's `internal` default already scopes a symbol to its
module, so a folder split costs nothing and a target split would buy little.

**Draw a hard module boundary where the dependency direction must be enforced against a whole
category of code** (the UI must not be reachable from the domain; the app must not be reachable from
either). Draw a folder boundary inside a module for the rest. This is a deliberate adaptation of the
Rust five-crate rule, and the reason it is safe is that the one direction that actually goes wrong,
UI or infrastructure leaking inward, is exactly the one the module split still catches.

**Gotcha (measured).** After adding a target to `Package.swift`, an incremental `swift build` can
print `Build complete!` without compiling the new target at all. Run `swift package clean` before
trusting a green build that followed a manifest edit.

### What the domain excludes

The filesystem and networking APIs, any database framework, `ProcessInfo` environment reads,
`Process` spawning, AppKit, UIKit and SwiftUI, vendor SDKs, and any printing. Prefer the standard
library; reach into Foundation only for a genuine primitive such as `UUID`, `Date` or `URL`.

The case study holds to this exactly (measured): its framework module imports only Foundation and
CoreData across 28 files, and never UIKit. The UI module imports UIKit and the framework. Nothing
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

`internal` is what makes the folder-based role split safe: a type is invisible outside its module
unless you say otherwise. Make a type `public` only when it crosses a module boundary, which in
practice means the domain models, the ports, and the presenters the UI module consumes.

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

    swift build
    swift test
    swiftformat --lint .
    swiftlint

For an Xcode project rather than a package, `xcodebuild clean build test -project <p> -scheme <s>`,
piped through `xcbeautify`. `just lint-check` and `just ship` still gate the repository as a whole.

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

**The Rust numbers hold unchanged, and the case study is the evidence.** Measured across its 120
Swift files:

| | files | total lines | median | p90 | max | over 300 | over 500 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| production | 59 | 2,058 | 23 | 62 | 164 | 0 | 0 |
| tests | 61 | 4,574 | 46 | 153 | 590 | 1 | 1 |

Well-factored Swift lands nowhere near the caps: the largest production file in a 493-star teaching
repository is 164 lines, and the median is 23. So 200 implementation and 300 total remain targets that
never bind on good code, and the 500 hard cap stays a genuine backstop.

The one file over the cap is a **test** file (a 590-line UI integration suite), which is precisely the
neutral rule about decomposing large acceptance suites by behavior. Swift does not get an exemption
for that.

**Awaiting the operator:** these numbers are inherited from the Rust standard and confirmed
non-binding by the distribution above, not independently derived for Swift. If a real Swift tool of
ours later crowds the cap, that is the moment to revisit, not now.

## Open for Swift

Rules from the neutral skill with no answer in the case study, recorded rather than invented:

- **The versioned egress protocol and the executable-destination adapter.** The case study drives no
  external executables, so it demonstrates nothing here. The neutral rule stands unadapted.
- **Process ownership and process-group cleanup.** No subprocess spawning anywhere in it.
- **A busy-database policy.** It uses CoreData, not SQLite with concurrent writers, so it has no
  answer to the multiprocess contention rule. A Swift tool of ours with several writing processes
  needs that rule worked out before the first store lands.
