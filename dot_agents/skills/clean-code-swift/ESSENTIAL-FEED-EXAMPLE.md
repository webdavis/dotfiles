# Worked example: the Essential Feed case study

`essentialdevelopercom/essential-feed-case-study`, read at commit `706a0da` on 2026-09-03. A teaching
repository for an iOS image feed. It tracks 124 Swift files; the 120 measured below are its three
architecture targets, excluding the standalone `Prototype/` throwaway app. Everything here was
measured from the source, not recalled.

It is a worked example of the method, not a specification. Where its shape and the method differ, the
difference is noted rather than smoothed over.

## The specification, which matches the method exactly

The README is the behavioral specification, written before the code, in the shape
[`clean-code/SKILL.md`](../clean-code/SKILL.md) step 2 asks for.

Narratives:

    As an offline customer
    I want the app to show the latest saved version of my image feed
    So I can always enjoy images of my friends

Acceptance criteria as scenarios:

    Given the customer doesn't have connectivity
      And there's a cached version of the feed
      And the cache is seven days old or more
     When the customer requests to see the feed
     Then the app should display an error message

And each use case as a primary course plus every named sad path:

    ### Validate Feed Cache Use Case
    #### Primary course:
    1. Execute "Validate Cache" command with above data.
    2. System retrieves feed data from cache.
    3. System validates cache is less than seven days old.
    #### Retrieval error course (sad path):
    1. System deletes cache.
    #### Expired cache course (sad path):
    1. System deletes cache.

Note what the sad paths carry: not just "it fails", but the required side effect on failure. That is
the method's "every meaningful failure source" and "required side effects" in the same three lines.

The README also pins the model as a property table and the wire format as a literal payload, which is
the protocol contract stated where a reader will find it.

## The modules and the enforcement

Three modules, three Xcode framework targets:

| Module            | Files | Imports (measured, whole module)       |
| ----------------- | ----- | -------------------------------------- |
| `EssentialFeed`   | 36    | `Foundation` (28), `CoreData` (5)      |
| `EssentialFeediOS`| 15    | `UIKit` (15), `EssentialFeed` (4)      |
| `EssentialApp`    | 8     | `EssentialFeed` (7), `UIKit` (6), `EssentialFeediOS` (4), `os`, `CoreData` |

The direction is one way and it is enforced by the target graph: the framework cannot reach the UI
because it does not link it, and nothing links the app. No UIKit import appears anywhere in
`EssentialFeed`.

**Inside `EssentialFeed` the roles are folders, not targets:**

    Feed Feature/            domain models and the ports
    Feed API/                remote adapters and mappers
    Feed Cache/              the local use case, its policy, its store port
    Feed Cache/Infrastructure/CoreData/    the durable adapter
    Feed Cache/Infrastructure/InMemory/    the fast adapter
    Feed Presentation/       presenters and view models
    Shared API/  Shared API Infra/  Shared Presentation/

This is the deliberate difference from the Rust five-crate layout, and it is a trade, not a free
win. The three TARGET boundaries are enforced by the build. The folder boundaries inside
`EssentialFeed` are not: `internal` confines a symbol to its module, and all of these folders are one
module, so nothing stops a `Feed Feature/` file importing CoreData and calling into
`Feed Cache/Infrastructure/` (measured on Swift 6.3.3, exit 0 clean and after `swift package clean`).
The folders record the intent and reviewers hold it. What the case study demonstrates is that the
trade works at this size, not that the compiler is keeping the line.

## The domain is ports and values

`Feed Feature/` is four files, and it is the whole domain surface:

    public protocol FeedCache {
        func save(_ feed: [FeedImage]) throws
    }

    public protocol FeedImageDataLoader {
        func loadImageData(from url: URL) throws -> Data
    }

    public struct FeedImage: Hashable, Sendable {
        public let id: UUID
        public let description: String?
        public let location: String?
        public let url: URL
    }

One method per protocol. Interface segregation taken literally: a collaborator that only saves never
sees a load method. Where a caller needs several capabilities it composes them at the point of use,
as the scene delegate does with `FeedStore & FeedImageDataStore & Scheduler & Sendable`.

The policy is a stateless namespace:

    final class FeedCachePolicy {
        private init() {}
        private static let calendar = Calendar(identifier: .gregorian)
        private static var maxCacheAgeInDays: Int { 7 }

        static func validate(_ timestamp: Date, against date: Date) -> Bool {
            guard let maxCacheAge = calendar.date(byAdding: .day, value: maxCacheAgeInDays, to: timestamp) else {
                return false
            }
            return date < maxCacheAge
        }
    }

Note it is `internal`, not `public`: the seven-day rule is the module's business and no caller gets to
depend on it. Note also that it takes both instants as parameters rather than reading a clock, which
is what makes it a pure function and what lets the test suite walk one second either side of the
boundary.

The ports are `async throws` where the work is asynchronous:

    public protocol HTTPClient {
        func get(from url: URL) async throws -> (Data, HTTPURLResponse)
    }

## The composition root

`EssentialApp`, 8 files, 493 lines. The largest is `FeedService.swift` at 164 lines, which composes
the remote-with-local-fallback strategy. Neither loader knows about the fallback; the root does.

`WeakRefVirtualProxy` is a decorator that holds its target weakly, conformed by conditional
extensions:

    final class WeakRefVirtualProxy<T: AnyObject> {
        private weak var object: T?
        init(_ object: T) { self.object = object }
    }

    extension WeakRefVirtualProxy: ResourceLoadingView where T: ResourceLoadingView {
        func display(_ viewModel: ResourceLoadingViewModel) { object?.display(viewModel) }
    }

One type, several conditional conformances, all of it in the root. This is the same move the method
asks for with a recording decorator: wrap at composition so no collaborator has to remember.

The test seam is a second initializer on the scene delegate:

    convenience init(httpClient: HTTPClient, store: FeedStore & FeedImageDataStore & Scheduler & Sendable) {
        self.init()
        self.feedService = FeedService(httpClient: httpClient, store: store)
    }

which is what lets the acceptance suite drive the entire assembled app with stubbed infrastructure and
no network.

## The testing methodology

The five practices worth copying are written up in [`SKILL.md`](SKILL.md) with their code. In
summary, as they appear here:

1. **`FeedStoreSpecs`**: a protocol whose method names are the test names, so a conforming test class
   fails to compile if it omits a case. Split into `FailableRetrieve`, `FailableInsert` and
   `FailableDelete` variants composed with `&`, with shared `assertThat...(on sut:)` free functions
   holding the bodies. Run against both `CoreDataFeedStoreTests` and `InMemoryFeedStoreTests`.
2. **`makeSUT`**: a private factory per test class returning a labeled tuple, defaulting the clock to
   `Date.init`, and leak-tracking every instance it builds.
3. **`trackForMemoryLeaks`**: an `XCTestCase` extension using `addTeardownBlock` with a weak capture
   and `XCTAssertNil`.
4. **`file:`/`line:` threading**: every helper takes `file: StaticString = #filePath, line: UInt = #line`
   and forwards it, so failures report at the calling test.
5. **Message-recording spies**: `enum ReceivedMessage: Equatable` plus
   `private(set) var receivedMessages`, asserted as a whole array.

Suites, all separate targets: `EssentialFeedTests`, `EssentialFeediOSTests` (including snapshot
tests), `EssentialFeedCacheIntegrationTests`, `EssentialFeedAPIEndToEndTests`, and
`EssentialAppTests` (which holds `FeedAcceptanceTests`).

CI runs `xcodebuild clean build test` with `-enableThreadSanitizer YES`.

## Measured file sizes

| | files | total lines | median | p90 | max | over the 200 production cap |
| --- | --- | --- | --- | --- | --- | --- |
| production | 59 | 2,058 | 24 | 62 | 164 | 0 |
| tests | 61 | 4,574 | 50 | 156 | 590 | n/a |

Largest production files: `FeedService.swift` 164, `ListViewController.swift` 119, `ErrorView.swift`
109, `FeedImageCellController.swift` 96, `FeedViewAdapter.swift` 90.

Every production file is under the 200-line cap. The largest file overall,
`EssentialAppTests/FeedUIIntegrationTests.swift` at 590 lines, is a test and sits inside the 700-line
test allowance, so it is compliant on size; split it only if it covers several behaviors.

## What it does not demonstrate

No executable destinations or subprocess spawning, so nothing about egress protocols, process
ownership or process-group cleanup. No SQLite with multiple writing processes, so nothing about
busy-database handling or multiprocess contention. Those rules carry over from the neutral skill
unadapted, and a Swift tool of ours that needs them works them out fresh.
