# pns served pages Implementation Plan

> Implement task by task, in order. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Seven read-only pages on pns's loopback site (Waiting on you, Health, Recap, Right now,
Sessions, Deliveries, Config as loaded), each the phone form of a terminal command whose value builder it
shares.

**Architecture:** Every page is three parts: a value builder beside its terminal command that returns a
view of finished display strings, a terminal renderer over that view, and a page renderer over the same
view in the failures page's dark visual language. The site reads through a store that SQLite itself
refuses to write through, reads the clock once per request, and adds one route and one index row per
page. Each page is one pull request, in the order below, after the failures page pull request.

**Tech Stack:** Rust 2024 in the `pns/` cargo workspace (`pns`, `pns-adapters`, `pns-application`,
`pns-domain`), `rusqlite`, `serde_json` for herdr's answers, `toml` for config. No new dependency.
Gates: `just test-rust`, `just lint-check`, `just test-unit`.

**Spec:** `docs/superpowers/specs/2026-09-23-pns-served-pages-design.md`

## Global Constraints

- ONE FORMATTER: every value a page shows is produced by the function the matching terminal command uses;
  the page only lays values out. Where no terminal command exists, the same pull request adds one, and
  both surfaces render from one extracted value builder, pinned by golden tests.
- SECRETS: page 7 never renders a secret value. Redaction is by construction (typed fields spelled by
  hand, plugin keys by allowlist, no error text on a failed load), and a test plants a sentinel in every
  secret-bearing key and asserts it never appears.
- Read-only, loopback-only (`127.0.0.1`), no JavaScript, no external assets, dark only, every producer
  string escaped, `Content-Length` correct, the single-threaded server loop unchanged. Nothing
  irreversible behind the tunnel.
- The site reads through `SqliteStore::viewer` only (spec D3).
- Rust files: 300 lines target, 500 hard cap, tests included. Count with the file-size command in
  `~/.agents/skills/clean-code-rust/SKILL.md`, never `tokei`.
- Tests first: each task states its red test, and the test is run and seen failing for the stated
  reason before the implementation is written. A pure move owes the existing tests green before and
  after, listed by name.
- No cargo workspace depends on another; pns ships no bash.
- Each page is its own small pull request. Pages 1 and 2 go first. Every pull request lands after the
  failures page pull request (`feat/pns-failures-page-timeline`), which owns the shell, the router and the
  index row pattern. Page 1 also lands after pull request 924 (`fix/pns-return-card-lists-open-waits`),
  whose `open_waits` it reads.
- No em-dashes anywhere. Docs never mention agents, reviews or how a change was produced. Comments say
  what the code does or why.
- Conventional commits, `SKIP_AI_COMMIT=1`, no AI co-author trailer. No `rm` (use `trash`), no
  `git checkout .`, no `git checkout -- <file>`, no reset, no stash, no force push, no `chezmoi apply`.
- Never run `pns` against the real state directory. Served-page tests bind an OS-assigned port and read
  a sandbox state directory built by the test.
- Every pull request ends with `just test-rust`, `just lint-check` and `just test-unit` green, exit codes
  reported.
- The failures page pull request adds three UTC formatters beside `utc_timestamp` in
  `pns-adapters/src/macos/clock.rs`. This plan calls them `utc_clock(epoch) -> String` (`03:20`),
  `utc_day(epoch, now) -> String` (`Today`, `Yesterday`, `Sep 20`) and `utc_long(epoch) -> String`
  (`September 23, 2026 at 03:20 UTC`). Task 1.1 exports them under exactly these names, renaming in its
  pure-move commit if the failures pull request spelled them otherwise.
- The fixture clock every test in this plan uses is `NOW = 1_790_139_720`, which is
  2026-09-23T05:02:00Z.

## The inputs most likely to bite

- A producer string carrying markup (`<`, `>`, `&`, a quote) in any field of any page: it must arrive
  escaped once, never raw and never double-escaped. Pinned per page by an escaping test.
- A state directory whose database is missing, locked, or one schema version ahead: every page prints its
  unreadable sentence with `200 OK`, the index shows "unreadable" for that row only, and nothing is
  created on disk. Pinned by Task 1.2's viewer tests and each page's unreadable test.
- A request for `/waiting/`, `/waiting?x=1`, `/WAITING` or a POST to any page: 404. Pinned per page by
  the route test each page's last task extends.
- A secret pasted where the schema expects one but under a key the allowlist does not know (a new plugin
  key tomorrow): it renders `hidden`. Pinned by Task 7.2's classification test.
- herdr not running, or answering a shape pns does not know: the Sessions page says herdr did not answer
  and lists nothing, rather than listing pns's never-swept session rows as live. Pinned by Task 5.1 and
  Task 5.3.

---

## File Structure

New files across all seven pull requests, in `pns/crates/`:

| File | Pull request | Responsibility |
| --- | --- | --- |
| `pns/src/site.rs` (moved) | 1 | bind, serve loop, request timeout, `Target`, `answer` |
| `pns/src/site/shell.rs` (moved) | 1 | the CSS constant, `page()`, the footer, `escaped()`, tone classes |
| `pns/src/site/index.rs` (moved) | 1 | the index rows and their markup |
| `pns/src/site/failures.rs` (moved) | 1 | the failures listing and record pages |
| `pns/src/view.rs` | 1 | `Sources` (what a view reads) and `Summary` (what the index shows) |
| `pns/src/command_waiting.rs` | 1 | `pns waiting`: value builder, terminal renderer, summary |
| `pns/src/site/waiting.rs` | 1 | the Waiting on you page |
| `pns/src/site/health.rs` | 2 | the Health page |
| `pns/src/command_recap/history.rs` | 3 | `pns recap history`: value builder and terminal renderer |
| `pns/src/site/recap.rs` | 3 | the Recap page |
| `pns/src/command_now.rs`, `command_now/preview.rs` | 4 | `pns now` and the per-state delivery preview |
| `pns/src/live_overrides.rs` | 4 | the override assembly the event path and `pns now` share |
| `pns/src/site/now.rs` | 4 | the Right now page |
| `pns-adapters/src/protocols/markers/leases.rs` | 4 | `live_leases`, the read-only lease listing |
| `pns-adapters/src/herdr/panes.rs` | 5 | `parse_agent_panes` over `herdr pane list` |
| `pns-adapters/src/persistence/sqlite/session_facts.rs` | 5 | `session_facts` |
| `pns-domain/src/sessions.rs` | 5 | herdr status words, their order and chips |
| `pns/src/command_sessions.rs` | 5 | `pns sessions` |
| `pns/src/site/sessions.rs` | 5 | the Sessions page |
| `pns-adapters/src/persistence/sqlite/ledger/deliveries.rs` | 6 | `deliveries_since` |
| `pns-domain/src/retry/delivered.rs` | 6 | `LegOutcome`, the event verdict, the destination summary |
| `pns/src/command_deliveries.rs` | 6 | `pns deliveries` |
| `pns/src/site/deliveries.rs` | 6 | the Deliveries page |
| `pns-adapters/src/config/secrets.rs` | 7 | `SECRET_KEYS`, `SECRET_TABLES` |
| `pns-adapters/src/config/disclosure.rs` | 7 | `disclosed`, `SHOWN_PLUGIN_KEYS` |
| `pns/src/command_config.rs` | 7 | `pns config show` |
| `pns/src/site/config.rs` | 7 | the Config as loaded page |

Every new test module sits beside its file as `<file>/tests.rs` under `#[cfg(test)] mod tests;`.

---

## Pull request 1: Waiting on you

Branch `feat/pns-page-waiting`. Worktree:
`herdr worktree create --cwd /Users/stephen/workspaces/Ivy/webdavis/dotfiles --branch feat/pns-page-waiting --no-focus`

### Task 1.1: Move the server to `site` (pure move)

**Files:**
- Move: `pns/crates/pns/src/failures_page.rs` to `pns/crates/pns/src/site.rs`
- Move: `pns/crates/pns/src/failures_page/parent_watch.rs` to `pns/crates/pns/src/site/parent_watch.rs`
- Move: `pns/crates/pns/src/failures_page/tests.rs` to `pns/crates/pns/src/site/tests.rs`
- Split: `pns/crates/pns/src/failures_page/html.rs` into `site/shell.rs`, `site/index.rs`,
  `site/failures.rs`
- Modify: `pns/crates/pns/src/lib.rs` (`mod failures_page;` becomes `mod site;`)
- Modify: `pns/crates/pns/src/command_failures.rs` (the `serve` arm calls `crate::site::serve`)
- Modify: `pns/crates/pns-adapters/src/macos/clock.rs` and `pns-adapters/src/lib.rs` only if the UTC
  formatters need renaming (Global Constraints)

**Interfaces:**
- Consumes: the failures pull request's server and HTML.
- Produces: `crate::site::serve(port: u16)`, `site::shell::{CSS, page, escaped, footer}`,
  `site::index`, `site::failures::{listing_page, record_page}`; `pns_adapters::{utc_clock, utc_day,
  utc_long}`.

This is a pure move: no behavior changes, so it owes the existing tests green before and after, compared
by name.

- [ ] **Step 1: Record the baseline test names**

```bash
cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib -- --list 2>/dev/null \
  | grep ': test$' | sed 's/^failures_page::/site::/' | sort > "$SCRATCH/site-before.txt"
```

- [ ] **Step 2: Move the files with git so history follows them**

```bash
git mv pns/crates/pns/src/failures_page.rs pns/crates/pns/src/site.rs
git mv pns/crates/pns/src/failures_page pns/crates/pns/src/site
```

- [ ] **Step 3: Split `site/html.rs` by moving items, editing none**

Move the CSS constant, the page wrapper, the footer and `escaped()` into `site/shell.rs`; the index
function into `site/index.rs`; the listing and record renderers into `site/failures.rs`. `git mv
site/html.rs site/shell.rs` first so the largest part keeps its history, then cut the other two out of
it. In `site.rs`, replace `mod html;` with:

```rust
mod failures;
mod index;
mod parent_watch;
mod shell;
```

and change every `html::` path to `shell::`, `index::` or `failures::`. Change the `#[path]` attribute on
the tests module to `#[path = "site/tests.rs"]`. In `lib.rs`, `mod failures_page;` becomes `mod site;`.
In `command_failures.rs`, `crate::failures_page::serve(port)` becomes `crate::site::serve(port)`.

- [ ] **Step 4: Rename the UTC formatters if needed**

If the failures pull request exported its three formatters under other names, rename them to
`utc_clock`, `utc_day` and `utc_long` in `pns-adapters/src/macos/clock.rs`, its re-export in
`pns-adapters/src/lib.rs`, and every caller. No other edit.

- [ ] **Step 5: Compare the test names**

```bash
cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib -- --list 2>/dev/null \
  | grep ': test$' | sort > "$SCRATCH/site-after.txt"
diff "$SCRATCH/site-before.txt" "$SCRATCH/site-after.txt"
cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib site::
```

Expected: `diff` prints nothing apart from the three formatter tests if Step 4 renamed them; every
`site::` test passes.

- [ ] **Step 6: Commit**

```bash
git add -A pns/crates/pns/src pns/crates/pns-adapters/src
SKIP_AI_COMMIT=1 git commit -m "refactor(pns): move the served page into a site module"
```

### Task 1.2: The site reads through a store that cannot write

**Files:**
- Modify: `pns/crates/pns-adapters/src/persistence/sqlite/store.rs`
- Create: `pns/crates/pns-adapters/src/persistence/sqlite/tests/viewer.rs`
- Modify: `pns/crates/pns-adapters/src/persistence/sqlite/tests.rs` (add `mod viewer;`)
- Create: `pns/crates/pns/src/view.rs`
- Modify: `pns/crates/pns/src/lib.rs` (`mod view;`)
- Modify: `pns/crates/pns/src/wait_runtime.rs` (`stale_settings` reads through `stale_settings_at(home)`)
- Modify: `pns/crates/pns/src/site.rs`
- Test: `pns/crates/pns/src/site/tests.rs`

**Interfaces:**
- Consumes: `SqliteStore::read_only()` (`store.rs`), `pns_adapters::state_dir()`.
- Produces:
  - `pub fn SqliteStore::viewer(state: PathBuf) -> SqliteStore`
  - `pub(crate) struct view::Sources { pub(crate) store: SqliteStore, pub(crate) state: PathBuf,
    pub(crate) home: String }` with
    `pub(crate) fn live() -> Sources`, `#[cfg(test)] pub(crate) fn at(state: PathBuf, home: &str) ->
    Sources` and `pub(crate) fn stale_window(&self) -> u64`
  - `pub(crate) fn wait_runtime::stale_settings_at(home: &str) -> StaleSettings`
  - `pub(crate) struct view::Summary { pub(crate) text: String, pub(crate) tone: style::Tone }`
  - `site::serve_on_within(listener: TcpListener, sources: &Sources, request_timeout: Duration)`

- [ ] **Step 1: Write the failing viewer tests**

`pns/crates/pns-adapters/src/persistence/sqlite/tests/viewer.rs`:

```rust
use super::{SqliteStore, state};

#[test]
fn a_viewer_reads_what_a_writer_wrote() {
    let state = state();
    SqliteStore::new(state.clone())
        .begin_wait("s1", 100, true)
        .expect("the writer records a wait");
    let viewer = SqliteStore::viewer(state);
    assert_eq!(viewer.newest_wait().map(|wait| wait.session), Some("s1".to_string()));
}

#[test]
fn a_viewer_cannot_write() {
    let state = state();
    let writer = SqliteStore::new(state.clone());
    writer.begin_wait("s1", 100, true).expect("the writer records a wait");
    let viewer = SqliteStore::viewer(state);
    assert!(viewer.begin_wait("s2", 200, true).is_err(), "a viewer wrote");
    assert!(viewer.end_wait("s1", Some(300)).is_err(), "a viewer wrote");
    assert_eq!(writer.newest_wait().map(|wait| wait.session), Some("s1".to_string()));
}

#[test]
fn a_viewer_creates_nothing_where_no_database_exists() {
    let state = state();
    let viewer = SqliteStore::viewer(state.clone());
    assert!(viewer.open_waits(0, 100).is_err());
    assert!(!state.exists(), "a viewer created {state:?}");
}
```

Add `mod viewer;` to the test module list in `persistence/sqlite/tests.rs`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters viewer`
Expected: compile error, `no function or associated item named viewer found for struct SqliteStore`.

- [ ] **Step 3: Implement the viewer**

In `store.rs`, add the field and the constructor, and gate the two openers:

```rust
/// Whether this store's connections may write.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Access {
    ReadWrite,
    ReadOnly,
}

pub struct SqliteStore {
    pub(super) state: PathBuf,
    pub(super) claim: std::sync::Mutex<Option<i64>>,
    pub(super) busy_timeout: Duration,
    pub(super) log: PathBuf,
    legacy_records: bool,
    access: Access,
}

impl SqliteStore {
    /// A store every connection of which is `read_only()`: nothing is created,
    /// migrated or imported, and SQLite refuses every write made through it.
    pub fn viewer(state: PathBuf) -> Self {
        Self {
            access: Access::ReadOnly,
            ..Self::new(state)
        }
    }
}
```

`new` sets `access: Access::ReadWrite`. At the top of `open()` and of `open_existing()`:

```rust
if self.access == Access::ReadOnly {
    return self.read_only();
}
```

- [ ] **Step 4: Run the viewer tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters viewer`
Expected: 3 passed.

- [ ] **Step 5: Write the failing site test**

In `pns/crates/pns/src/site/tests.rs`, a real request against a sandbox state directory:

```rust
/// THE SITE'S STORE IS A VIEWER: a request answers from a sandbox the test
/// built, and the database it read is byte-identical afterwards.
#[test]
fn a_request_reads_the_sandbox_and_writes_nothing() {
    use std::io::Read;
    let state = sandbox_state();
    pns_adapters::SqliteStore::new(state.clone())
        .begin_wait("s1", 100, true)
        .expect("seed a wait");
    let before = std::fs::read(state.join("pns.db")).expect("the database");
    let sources = crate::view::Sources::at(state.clone(), "/nonexistent-home");
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let port = listener.local_addr().expect("its address").port();
    std::thread::spawn(move || serve_on_within(listener, &sources, REQUEST_TIMEOUT));

    let mut client = std::net::TcpStream::connect(("127.0.0.1", port)).expect("a connection");
    client.write_all(b"GET /failures HTTP/1.1\r\n\r\n").expect("the request");
    let mut answered = String::new();
    client.read_to_string(&mut answered).expect("the response");

    assert!(answered.starts_with("HTTP/1.1 200 OK\r\n"), "{answered}");
    assert_eq!(std::fs::read(state.join("pns.db")).expect("the database"), before);
}

fn sandbox_state() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "pns-site-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos())
    ));
    std::fs::create_dir_all(&root).expect("the sandbox");
    root
}
```

The two existing socket tests (`a_real_request_gets_a_real_response`,
`a_stalled_connection_does_not_block_the_next_request`) change their spawn line to pass
`&crate::view::Sources::at(sandbox_state(), "/nonexistent-home")`, so no served-page test reads the real
state directory any more.

- [ ] **Step 6: Run it to see it fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib site::tests::a_request_reads_the_sandbox`
Expected: compile error, `failed to resolve: could not find view in the crate root`.

- [ ] **Step 7: Implement `view.rs` and thread `Sources` through the site**

`pns/crates/pns/src/view.rs`:

```rust
//! What a read-only view reads, and the one line the site's index shows for it.

use pns_adapters::SqliteStore;
use pns_adapters::style::Tone;
use std::path::PathBuf;

/// The sources every read-only view reads, opened once per process.
///
/// `state` is the directory `store` was opened on, for the readings that are
/// files beside the database (the daemon's heartbeat, the lamp leases).
pub(crate) struct Sources {
    pub(crate) store: SqliteStore,
    pub(crate) state: PathBuf,
    pub(crate) home: String,
}

impl Sources {
    /// The operator's own state, read through a store that cannot write.
    pub(crate) fn live() -> Self {
        let state = pns_adapters::state_dir();
        Self {
            store: SqliteStore::viewer(state.clone()),
            state,
            home: std::env::var("HOME").unwrap_or_default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn at(state: PathBuf, home: &str) -> Self {
        Self {
            store: SqliteStore::viewer(state.clone()),
            state,
            home: home.to_string(),
        }
    }

    /// The stale escalation window the config under `home` names, 0 when the
    /// escalation is off or the config cannot be read.
    pub(crate) fn stale_window(&self) -> u64 {
        crate::wait_runtime::stale_settings_at(&self.home).window
    }
}

/// One index row's right-hand side: the page's own headline value and its tone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Summary {
    pub(crate) text: String,
    pub(crate) tone: Tone,
}
```

In `wait_runtime.rs`, `stale_settings()` keeps its body under a new name that takes the home directory,
and becomes one line over it, so a test's sandbox home is what a view reads:

```rust
pub(crate) fn stale_settings() -> StaleSettings {
    stale_settings_at(&std::env::var("HOME").unwrap_or_default())
}

pub(crate) fn stale_settings_at(home: &str) -> StaleSettings {
    // the former body of `stale_settings`, reading `config_path(home)`
}
```

In `site.rs`: `serve(port)` builds `let sources = crate::view::Sources::live();` and calls
`serve_on_within(bind(port), &sources, REQUEST_TIMEOUT)`; `serve_on` is deleted (its one caller now
passes sources); `serve_on_within` and `answer` take `sources: &Sources` in place of the store they
opened themselves, and every handler reads `sources.store` and `sources.home`.

- [ ] **Step 8: Run the site tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib site::`
Expected: every `site::` test passes.

- [ ] **Step 9: Commit**

```bash
git add pns/crates/pns-adapters/src/persistence/sqlite pns/crates/pns/src/view.rs \
  pns/crates/pns/src/lib.rs pns/crates/pns/src/site.rs pns/crates/pns/src/site/tests.rs
SKIP_AI_COMMIT=1 git commit -m "feat(pns): serve every page through a store SQLite refuses to write"
```

### Task 1.3: An open wait's age and its chip

**Files:**
- Modify: `pns/crates/pns-domain/src/missed/recap.rs` (`OpenWait.since`; `waiting` becomes `pub`)
- Modify: `pns/crates/pns-domain/src/missed.rs` (re-export `waiting`)
- Modify: `pns/crates/pns-domain/src/stale.rs` (`WaitChip`, `wait_chip`, `escalates_at`)
- Test: `pns/crates/pns-domain/src/stale.rs` tests module
- Modify: `pns/crates/pns-adapters/src/persistence/sqlite/sessions.rs` (`open_waits` selects
  `s.blocked_since`)
- Test: `pns/crates/pns-adapters/src/persistence/sqlite/tests/open_waits.rs`

**Interfaces:**
- Consumes: `SqliteStore::open_waits(since, until)` from pull request 924.
- Produces:
  - `pub struct OpenWait { pub agent, pub state, pub project, pub asks: String, pub count: usize, pub
    since: u64 }`
  - `pub fn pns_domain::missed::waiting(wait: &OpenWait) -> String`
  - `pub enum pns_domain::stale::WaitChip { Waiting, Overdue }` with `pub fn word(self) -> &'static str`
  - `pub fn pns_domain::stale::wait_chip(since: u64, now: u64, window: u64) -> WaitChip`
  - `pub fn pns_domain::stale::escalates_at(since: u64, window: u64) -> Option<u64>`

- [ ] **Step 1: Write the failing domain tests**

Append to the tests of `pns-domain/src/stale.rs`:

```rust
#[test]
fn a_wait_is_overdue_from_the_second_the_window_closes() {
    assert_eq!(wait_chip(1_000, 1_000 + 3_599, 3_600), WaitChip::Waiting);
    assert_eq!(wait_chip(1_000, 1_000 + 3_600, 3_600), WaitChip::Overdue);
    assert_eq!(wait_chip(1_000, 1_000 + 3_601, 3_600), WaitChip::Overdue);
}

#[test]
fn with_the_escalation_off_no_wait_is_ever_overdue() {
    assert_eq!(wait_chip(0, u64::MAX, 0), WaitChip::Waiting);
    assert_eq!(escalates_at(1_000, 0), None);
}

#[test]
fn a_clock_behind_the_wait_is_not_overdue() {
    assert_eq!(wait_chip(5_000, 4_000, 3_600), WaitChip::Waiting);
}

#[test]
fn the_escalation_fires_one_window_after_the_wait_opened() {
    assert_eq!(escalates_at(1_000, 3_600), Some(4_600));
}

#[test]
fn the_chip_words_are_the_pages_words() {
    assert_eq!(WaitChip::Waiting.word(), "Waiting");
    assert_eq!(WaitChip::Overdue.word(), "Overdue");
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-domain stale::tests`
Expected: compile error, `cannot find function wait_chip in this scope`.

- [ ] **Step 3: Implement the chip**

In `pns-domain/src/stale.rs`:

```rust
/// How an open wait reads on the page and at the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitChip {
    /// Younger than the escalation window, or the escalation is off.
    Waiting,
    /// At or past the window, the same test `stale_blocks` puts to the row.
    Overdue,
}

impl WaitChip {
    pub fn word(self) -> &'static str {
        match self {
            Self::Waiting => "Waiting",
            Self::Overdue => "Overdue",
        }
    }
}

/// OVERDUE IS `blocked_since <= now - window`, which is the escalation's own
/// predicate, so the chip turns red at the second the page becomes due. A
/// window of zero is the escalation switched off.
pub fn wait_chip(since: u64, now: u64, window: u64) -> WaitChip {
    match escalates_at(since, window) {
        Some(due) if now >= due => WaitChip::Overdue,
        _ => WaitChip::Waiting,
    }
}

/// When the escalation pages about a wait that opened at `since`, or `None`
/// when the escalation is off.
pub fn escalates_at(since: u64, window: u64) -> Option<u64> {
    (window > 0).then(|| since.saturating_add(window))
}
```

- [ ] **Step 4: Run the domain tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-domain stale::tests`
Expected: every `stale::tests` test passes.

- [ ] **Step 5: Write the failing adapter test**

Append to `pns-adapters/src/persistence/sqlite/tests/open_waits.rs`:

```rust
#[test]
fn an_open_wait_carries_the_second_it_opened() {
    let store = SqliteStore::new(state());
    row(&store, "open", "blocked", 110, "Bash: ls");
    store.begin_wait("open", 110, true).unwrap();
    let waits = store.open_waits(0, 200).unwrap();
    assert_eq!(waits.len(), 1);
    assert_eq!(waits[0].since, 110);
}
```

- [ ] **Step 6: Run it to see it fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters an_open_wait_carries`
Expected: compile error, `no field since on type OpenWait`.

- [ ] **Step 7: Add the field and select it**

In `pns-domain/src/missed/recap.rs`, add to `OpenWait`:

```rust
    /// The second the session's wait opened, off its `sessions` row.
    pub since: u64,
```

and change `fn waiting(wait: &OpenWait) -> String` to `pub fn waiting(wait: &OpenWait) -> String`,
re-exported from `pns-domain/src/missed.rs` beside `recap_card`. In `open_waits`, append
`, s.blocked_since` to the `SELECT` list after the `MAX(1, ...)` count and read it:

```rust
            Ok(pns_domain::missed::OpenWait {
                agent: row.get(0)?,
                state: row.get(1)?,
                project: row.get(2)?,
                asks: row.get(3)?,
                count: row.get(4)?,
                since: row.get(5)?,
            })
```

Every `OpenWait { .. }` literal in the existing tests gains `since: 0` (the card's tests do not read it).

- [ ] **Step 8: Run the adapter and domain suites**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters -p pns-domain -p pns-application`
Expected: all pass, the new test included.

- [ ] **Step 9: Commit**

```bash
git add pns/crates/pns-domain/src pns/crates/pns-adapters/src/persistence/sqlite pns/crates/pns-application/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): give an open wait its age and an overdue chip"
```

### Task 1.4: `pns waiting`

**Files:**
- Create: `pns/crates/pns/src/command_waiting.rs`
- Create: `pns/crates/pns/src/command_waiting/tests.rs`
- Modify: `pns/crates/pns/src/lib.rs` (`mod command_waiting;`, `pub(crate) use
  command_waiting::waiting_mode;`)
- Modify: `pns/crates/pns/src/invocation.rs` (dispatch `waiting`)
- Modify: `pns/crates/pns/src/subcommand_usage.rs` (`("waiting", crate::command_waiting::WAITING_USAGE)`)
- Modify: `pns/crates/pns/src/legacy/usage.rs` (one `USAGE` line)

**Interfaces:**
- Consumes: `OpenWait.since`, `missed::waiting`, `stale::{wait_chip, escalates_at, WaitChip}`,
  `remind::waited`, `render::title`, `pns_adapters::{utc_clock, utc_day, utc_long}`, `view::{Sources,
  Summary}`, `Sources::stale_window`.
- Produces:

```rust
pub(crate) const WAITING_USAGE: &str;
pub(crate) const UNREADABLE: &str = "pns: the session store could not be read";
pub(crate) struct WaitingView { pub(crate) waits: Vec<WaitRow> }
pub(crate) struct WaitRow {
    pub(crate) title: String,      // render::title(agent, state, project)
    pub(crate) asks: String,
    pub(crate) count: String,      // "8 unanswered", or empty for one
    pub(crate) age: String,        // remind::waited(now - since)
    pub(crate) chip: WaitChip,
    pub(crate) line: String,       // missed::waiting(&wait)
    pub(crate) clock: String,      // utc_clock(since)
    pub(crate) day: String,        // utc_day(since, now)
    pub(crate) opened: String,     // utc_long(since)
    pub(crate) escalation: String, // "after 1h, at 05:41 UTC", or "off"
}
pub(crate) fn view(waits: &[OpenWait], now: u64, window: u64) -> WaitingView;
pub(crate) fn read(sources: &Sources, now: u64) -> Result<WaitingView, &'static str>;
pub(crate) fn render(paint: Paint, view: &WaitingView) -> String;
pub(crate) fn summarize(view: &WaitingView) -> Summary;
pub(crate) fn summary(sources: &Sources, now: u64) -> Summary;
pub(crate) fn tone(chip: WaitChip) -> Tone;
pub(crate) fn waiting_mode() -> i32;
```

- [ ] **Step 1: Write the failing tests**

`pns/crates/pns/src/command_waiting/tests.rs`:

```rust
use super::*;
use pns_domain::missed::OpenWait;

const NOW: u64 = 1_790_139_720;
const HOUR: u64 = 3_600;

fn wait(agent: &str, state: &str, project: &str, asks: &str, count: usize, since: u64) -> OpenWait {
    OpenWait {
        agent: agent.into(),
        state: state.into(),
        project: project.into(),
        asks: asks.into(),
        count,
        since,
    }
}

/// open_waits answers oldest first; the view is newest first.
fn fixture() -> Vec<OpenWait> {
    vec![
        wait("codex", "blocked", "dotfiles", "Bash: git push", 8, NOW - HOUR - 42 * 60),
        wait("claude", "asked", "pns", "Which branch?", 1, NOW - 3 * 60),
    ]
}

#[test]
fn the_terminal_listing_is_pinned() {
    assert_eq!(
        render(Paint::Plain, &view(&fixture(), NOW, HOUR)),
        "pns: 2 sessions waiting on you\n\
         \x20 ● 3m   Waiting  claude · asked · pns: Which branch?\n\
         \x20 ● 1h   Overdue  codex · blocked · dotfiles ×8: Bash: git push\n"
    );
}

#[test]
fn every_row_carries_the_values_the_page_shows() {
    let view = view(&fixture(), NOW, HOUR);
    let codex = &view.waits[1];
    assert_eq!(codex.title, "codex · blocked · dotfiles");
    assert_eq!(codex.asks, "Bash: git push");
    assert_eq!(codex.count, "8 unanswered");
    assert_eq!(codex.age, "1h");
    assert_eq!(codex.chip, WaitChip::Overdue);
    assert_eq!(codex.clock, "03:20");
    assert_eq!(codex.day, "Today");
    assert_eq!(codex.opened, "September 23, 2026 at 03:20 UTC");
    assert_eq!(codex.escalation, "after 1h, at 04:20 UTC");
    assert_eq!(view.waits[0].count, "", "one wait says no count");
}

#[test]
fn with_the_escalation_off_the_row_says_so() {
    let view = view(&fixture(), NOW, 0);
    assert!(view.waits.iter().all(|row| row.escalation == "off"));
    assert!(view.waits.iter().all(|row| row.chip == WaitChip::Waiting));
}

#[test]
fn nothing_waiting_is_one_sentence() {
    assert_eq!(render(Paint::Plain, &view(&[], NOW, HOUR)), "pns: nothing is waiting on you\n");
}

#[test]
fn the_summary_is_the_count_and_red_when_any_is_overdue() {
    let summarized = summarize(&view(&fixture(), NOW, HOUR));
    assert_eq!(summarized.text, "2, 1 overdue");
    assert_eq!(summarized.tone, Tone::Bad);
    assert_eq!(summarize(&view(&[], NOW, HOUR)).text, "none");
    assert_eq!(summarize(&view(&fixture()[1..], NOW, HOUR)).tone, Tone::Warn);
}

#[test]
fn an_unreadable_store_is_the_sentence() {
    let absent = std::env::temp_dir().join(format!("pns-waiting-absent-{}", std::process::id()));
    let sources = crate::view::Sources::at(absent, "/nonexistent-home");
    assert_eq!(read(&sources, NOW).err(), Some(UNREADABLE));
    assert_eq!(summary(&sources, NOW).text, "unreadable");
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib command_waiting`
Expected: compile error, `file not found for module command_waiting` until the module is declared, then
`cannot find function view`.

- [ ] **Step 3: Implement the builder, the renderer and the mode**

`pns/crates/pns/src/command_waiting.rs`:

```rust
//! `pns waiting`: every session waiting on an answer, newest first.
//!
//! THE VALUE BUILDER FOR TWO SURFACES. The terminal prints `render` and the
//! site's `/waiting` page lays out the same `WaitingView`, so the page shows
//! nothing this module did not spell.

use crate::style::{self, Paint, Tone};
use crate::view::{Sources, Summary};
use pns_domain::missed::OpenWait;
use pns_domain::stale::WaitChip;

pub(crate) const WAITING_USAGE: &str = "\
pns: usage:
  pns waiting                      every session waiting on an answer, newest first
";

pub(crate) const UNREADABLE: &str = "pns: the session store could not be read";

pub(crate) struct WaitingView {
    pub(crate) waits: Vec<WaitRow>,
}

pub(crate) struct WaitRow {
    pub(crate) title: String,
    pub(crate) asks: String,
    pub(crate) count: String,
    pub(crate) age: String,
    pub(crate) chip: WaitChip,
    pub(crate) line: String,
    pub(crate) clock: String,
    pub(crate) day: String,
    pub(crate) opened: String,
    pub(crate) escalation: String,
}

pub(crate) fn view(waits: &[OpenWait], now: u64, window: u64) -> WaitingView {
    let waits = waits
        .iter()
        .rev()
        .map(|wait| WaitRow {
            title: pns_domain::render::title(&wait.agent, &wait.state, &wait.project),
            asks: wait.asks.clone(),
            count: if wait.count > 1 {
                format!("{} unanswered", wait.count)
            } else {
                String::new()
            },
            age: pns_domain::remind::waited(now.saturating_sub(wait.since)),
            chip: pns_domain::stale::wait_chip(wait.since, now, window),
            line: pns_domain::missed::waiting(wait),
            clock: pns_adapters::utc_clock(wait.since),
            day: pns_adapters::utc_day(wait.since, now),
            opened: pns_adapters::utc_long(wait.since),
            escalation: escalation(wait.since, window),
        })
        .collect();
    WaitingView { waits }
}

fn escalation(since: u64, window: u64) -> String {
    match pns_domain::stale::escalates_at(since, window) {
        None => "off".to_string(),
        Some(due) => format!(
            "after {}, at {} UTC",
            pns_domain::duration::spelled(std::time::Duration::from_secs(window)),
            pns_adapters::utc_clock(due)
        ),
    }
}

pub(crate) fn read(sources: &Sources, now: u64) -> Result<WaitingView, &'static str> {
    let window = sources.stale_window();
    sources
        .store
        .open_waits(0, now)
        .map(|waits| view(&waits, now, window))
        .map_err(|_| UNREADABLE)
}

pub(crate) fn tone(chip: WaitChip) -> Tone {
    match chip {
        WaitChip::Waiting => Tone::Warn,
        WaitChip::Overdue => Tone::Bad,
    }
}

pub(crate) fn render(paint: Paint, view: &WaitingView) -> String {
    if view.waits.is_empty() {
        return "pns: nothing is waiting on you\n".to_string();
    }
    let noun = if view.waits.len() == 1 { "session" } else { "sessions" };
    let mut out = format!("pns: {} {noun} waiting on you\n", view.waits.len());
    for row in &view.waits {
        let text = format!("{:<4} {:<7}  {}", row.age, row.chip.word(), row.line);
        out.push_str(&style::row(paint, tone(row.chip), "●", 2, &text));
        out.push('\n');
    }
    out
}

pub(crate) fn summarize(view: &WaitingView) -> Summary {
    let overdue = view.waits.iter().filter(|row| row.chip == WaitChip::Overdue).count();
    match (view.waits.len(), overdue) {
        (0, _) => Summary { text: "none".into(), tone: Tone::Quiet },
        (count, 0) => Summary { text: count.to_string(), tone: Tone::Warn },
        (count, overdue) => Summary { text: format!("{count}, {overdue} overdue"), tone: Tone::Bad },
    }
}

pub(crate) fn summary(sources: &Sources, now: u64) -> Summary {
    match read(sources, now) {
        Ok(view) => summarize(&view),
        Err(_) => Summary { text: "unreadable".into(), tone: Tone::Quiet },
    }
}

pub(crate) fn waiting_mode() -> i32 {
    if !crate::arguments_after_subcommand().is_empty() {
        eprintln!("{WAITING_USAGE}");
        return 2;
    }
    let Some(now) = pns_adapters::now_secs() else {
        eprintln!("pns waiting: this machine has no clock to measure a wait against");
        return 1;
    };
    match read(&Sources::live(), now) {
        Ok(view) => {
            print!("{}", render(Paint::for_stdout(), &view));
            0
        }
        Err(sentence) => {
            eprintln!("{sentence}");
            1
        }
    }
}

#[cfg(test)]
mod tests;
```

In `invocation.rs`, beside the `resume` arm:

```rust
    // Every open wait, printed. A MODE for the reason the others are: it reads
    // the store through a viewer, prints, and delivers nothing.
    if first == "waiting" {
        std::process::exit(crate::waiting_mode());
    }
```

In `legacy/usage.rs`, after the `pns resume` line:

```
  pns waiting                      every session waiting on an answer
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib command_waiting subcommand_usage`
Expected: all pass, including `every_subcommand_answers_both_help_flags_with_its_own_usage` and
`the_tool_wide_listing_names_every_subcommand`.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): add pns waiting, every open wait with its age"
```

### Task 1.5: The Waiting on you page, its route and its index row

**Files:**
- Create: `pns/crates/pns/src/site/waiting.rs`
- Create: `pns/crates/pns/src/site/waiting/tests.rs`
- Modify: `pns/crates/pns/src/site.rs` (`Target::Waiting`, the `answer` arm)
- Modify: `pns/crates/pns/src/site/shell.rs` (the neutral tone and the record card's red and neutral
  chips, spec D7)
- Modify: `pns/crates/pns/src/site/index.rs` (the index as a list of rows; the Waiting row)
- Test: `pns/crates/pns/src/site/tests.rs`

**Interfaces:**
- Consumes: `command_waiting::{read, summary, tone, WaitingView, UNREADABLE}`, `shell::{page, escaped,
  footer}`.
- Produces:
  - `pub(super) fn site::waiting::page(view: Result<&WaitingView, &'static str>, rendered_at: u64) ->
    String`
  - `pub(super) fn shell::row_class(tone: Tone) -> &'static str` (`"fh-row fh-active"`, `"fh-row"`,
    `"fh-row fh-quiet"`)
  - `pub(super) struct index::Row { pub(super) href: &'static str, pub(super) name: &'static str,
    pub(super) blurb: &'static str, pub(super) summary: Summary }` and `pub(super) fn index::rows(sources:
    &Sources, now: u64) -> Vec<Row>`

- [ ] **Step 1: Write the failing page tests**

`pns/crates/pns/src/site/waiting/tests.rs`:

```rust
use super::*;
use crate::command_waiting::view;
use pns_domain::missed::OpenWait;

const NOW: u64 = 1_790_139_720;

fn fixture(asks: &str) -> Vec<OpenWait> {
    vec![
        OpenWait {
            agent: "codex".into(),
            state: "blocked".into(),
            project: "dotfiles".into(),
            asks: asks.into(),
            count: 8,
            since: NOW - 6_120,
        },
        OpenWait {
            agent: "claude".into(),
            state: "asked".into(),
            project: "pns".into(),
            asks: "Which branch?".into(),
            count: 1,
            since: NOW - 180,
        },
    ]
}

#[test]
fn every_value_the_builder_spelled_is_on_the_page() {
    let built = view(&fixture("Bash: git push"), NOW, 3_600);
    let served = page(Ok(&built), NOW);
    for row in &built.waits {
        for value in [&row.title, &row.asks, &row.age, &row.clock, &row.day, &row.opened, &row.escalation] {
            assert!(served.contains(&escaped(value)), "{value:?} is not on the page");
        }
        assert!(served.contains(row.chip.word()));
    }
    assert!(served.contains("8 unanswered"));
}

#[test]
fn an_overdue_wait_is_red_and_a_waiting_one_amber() {
    let served = page(Ok(&view(&fixture("x"), NOW, 3_600)), NOW);
    let codex = served.find("codex · blocked").expect("the codex row");
    let claude = served.find("claude · asked").expect("the claude row");
    let opening = |at: usize| served[..at].rfind("<article class=\"").map(|start| &served[start..at]);
    assert!(opening(claude).expect("an article").contains("fh-row fh-active"));
    assert!(!opening(codex).expect("an article").contains("fh-active"));
}

#[test]
fn producer_text_is_escaped_once() {
    let served = page(Ok(&view(&fixture("<b>rm & go</b>"), NOW, 3_600)), NOW);
    assert!(served.contains("&lt;b&gt;rm &amp; go&lt;/b&gt;"), "{served}");
    assert!(!served.contains("<b>rm"), "{served}");
}

#[test]
fn nothing_waiting_is_one_line_inside_the_shell() {
    let served = page(Ok(&view(&[], NOW, 3_600)), NOW);
    assert!(served.contains("Nothing is waiting on you."), "{served}");
    assert!(served.contains("All times UTC"), "{served}");
}

#[test]
fn an_unreadable_store_is_the_terminal_sentence() {
    let served = page(Err(crate::command_waiting::UNREADABLE), NOW);
    assert!(served.contains("pns: the session store could not be read"), "{served}");
}
```

In `site/tests.rs`, extend the route tests:

```rust
#[test]
fn the_waiting_page_is_its_exact_path_only() {
    assert_eq!(target("GET /waiting HTTP/1.1\r\n"), Some(Target::Waiting));
    for line in [
        "GET /waiting/ HTTP/1.1\r\n",
        "GET /waiting?x=1 HTTP/1.1\r\n",
        "GET /WAITING HTTP/1.1\r\n",
        "POST /waiting HTTP/1.1\r\n",
    ] {
        assert_eq!(target(line), None, "{line:?} was served");
    }
}

#[test]
fn the_index_links_the_waiting_page_first_with_its_summary() {
    let sources = crate::view::Sources::at(sandbox_state(), "/nonexistent-home");
    let rows = index::rows(&sources, NOW_FOR_TESTS);
    assert_eq!(rows[0].href, "/waiting");
    assert_eq!(rows[0].name, "Waiting on you");
    assert_eq!(rows[0].summary.text, "unreadable", "an empty sandbox has no database");
}
```

with `const NOW_FOR_TESTS: u64 = 1_790_139_720;` at the top of `site/tests.rs`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib site::`
Expected: compile errors, `no variant Waiting` and `cannot find function page in module waiting`.

- [ ] **Step 3: Implement the page**

`pns/crates/pns/src/site/waiting.rs`:

```rust
//! The Waiting on you page: `pns waiting`'s view, laid out as the listing card.

use super::shell::{escaped, footer, page as shell_page, row_class};
use crate::command_waiting::{WaitRow, WaitingView, tone};

pub(super) fn page(view: Result<&WaitingView, &'static str>, rendered_at: u64) -> String {
    let body = match view {
        Err(sentence) => format!("<p>{}</p>", escaped(sentence)),
        Ok(view) if view.waits.is_empty() => "<p>Nothing is waiting on you.</p>".to_string(),
        Ok(view) => entries(view),
    };
    shell_page(
        "pns waiting",
        &format!(
            "<section id=\"failure-history\"><a class=\"fh-back\" href=\"/\">pns</a>\
             <header class=\"fh-header\"><h2>Waiting on you</h2></header>\n{body}\n{}</section>",
            footer(rendered_at)
        ),
    )
}

fn entries(view: &WaitingView) -> String {
    let mut out = String::new();
    let mut day = None;
    let last = view.waits.len() - 1;
    for (index, row) in view.waits.iter().enumerate() {
        if day != Some(&row.day) {
            out.push_str(&format!("<div class=\"fh-day\"><strong>{}</strong></div>\n", escaped(&row.day)));
            day = Some(&row.day);
        }
        out.push_str(&entry(row, index == last));
    }
    out
}

fn entry(row: &WaitRow, last: bool) -> String {
    let class = row_class(tone(row.chip));
    let last = if last { " fh-last" } else { "" };
    let count = if row.count.is_empty() {
        String::new()
    } else {
        format!(" <span class=\"fh-muted\">· {}</span>", escaped(&row.count))
    };
    format!(
        "<article class=\"{class}{last}\"><time class=\"fh-clock\">{clock}</time>\
         <div class=\"fh-content\"><span class=\"fh-dot\" aria-hidden=\"true\"></span>\
         <div class=\"fh-title\"><strong>{title}</strong><span class=\"fh-state\">{chip}</span></div>\
         <div class=\"fh-sub\">{asks}</div><div class=\"fh-next\">Waiting {age}{count}</div>\
         <details><summary>Details</summary><div class=\"fh-detail\"><dl>\
         <dt>Waiting since</dt><dd>{opened}</dd><dt>Escalation</dt><dd>{escalation}</dd>\
         </dl></div></details></div></article>\n",
        clock = escaped(&row.clock),
        title = escaped(&row.title),
        chip = row.chip.word(),
        asks = escaped(&row.asks),
        age = escaped(&row.age),
        opened = escaped(&row.opened),
        escalation = escaped(&row.escalation),
    )
}

#[cfg(test)]
mod tests;
```

`shell::page(title, card)` and `shell::footer(rendered_at)` are the shell functions Task 1.1 moved; if
the failures pull request's shell takes the card body differently, adapt the call, never the shell. In
`shell.rs`, add the tone-to-class map and append the D7 rules to the CSS constant:

```rust
pub(super) fn row_class(tone: Tone) -> &'static str {
    match tone {
        Tone::Warn => "fh-row fh-active",
        Tone::Bad => "fh-row",
        Tone::Good | Tone::Quiet => "fh-row fh-quiet",
    }
}
```

```css
#failure-history .fh-quiet .fh-dot { background:var(--muted); box-shadow:0 0 0 1px var(--muted); }
#failure-history .fh-quiet .fh-state { color:var(--muted); }
#failure-record .fr-status.fr-red { color:#ff8993; }
#failure-record .fr-status.fr-red::before { background:currentColor; }
#failure-record .fr-status.fr-quiet { color:var(--fr-muted); }
#failure-record .fr-status.fr-quiet::before { background:currentColor; }
```

In `site.rs`, add `Waiting` to `Target`, `"waiting" => Some(Target::Waiting)` to the path match, and the
arm:

```rust
        Some(Target::Waiting) => {
            let view = crate::command_waiting::read(sources, now);
            ok(&waiting::page(view.as_ref().map_err(|sentence| *sentence), now))
        }
```

where `now` is read once at the top of `answer` with `pns_adapters::now_secs().unwrap_or(0)`. In
`index.rs`, the index becomes a list of `Row` values rendered in order, and the Waiting row is first:

```rust
pub(super) fn rows(sources: &Sources, now: u64) -> Vec<Row> {
    vec![
        Row {
            href: "/waiting",
            name: "Waiting on you",
            blurb: "sessions waiting on an answer",
            summary: crate::command_waiting::summary(sources, now),
        },
        Row {
            href: "/failures",
            name: "Failures",
            blurb: "delivery legs that did not arrive",
            summary: crate::command_failures::summary(sources),
        },
    ]
}
```

`command_failures::summary` wraps the count the failures pull request already shows ("none" or the
number) in a `Summary`, amber when any leg is retrying and red when any has given up; it is a move of that
computation, not a new one. The row markup is the failures pull request's; the summary's tone picks the
class (`ix-red`, `ix-amber` or none).

- [ ] **Step 4: Run the site tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib site::`
Expected: all pass.

- [ ] **Step 5: Run the gates**

```bash
just test-rust; echo "test-rust exit $?"
just lint-check; echo "lint-check exit $?"
just test-unit; echo "test-unit exit $?"
```

Expected: three zero exit codes. `just lint-check` writes fixes into the tree when it fails; stage them
and rerun it.

- [ ] **Step 6: Check file sizes**

Run the file-size command from `~/.agents/skills/clean-code-rust/SKILL.md` over `pns/crates/pns/src`.
Expected: every file this pull request touched is under 500 total lines.

- [ ] **Step 7: Commit**

```bash
git add pns/crates/pns/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): serve the Waiting on you page"
```

---

## Pull request 2: Health

Branch `feat/pns-page-health`. Worktree:
`herdr worktree create --cwd /Users/stephen/workspaces/Ivy/webdavis/dotfiles --branch feat/pns-page-health --no-focus`

This pull request builds `pns doctor --no-send` (item Q7, approved 2026-09-23) and the page over it.

### Task 2.1: A withheld outcome in the doctor's domain

**Files:**
- Modify: `pns/crates/pns-domain/src/doctor/outcome.rs`
- Create: `pns/crates/pns-domain/src/doctor/outcome/tests.rs`

**Interfaces:**
- Produces: `Outcome::Withheld(Option<String>)`, carrying the rendered time of the destination's last
  delivery, or `None` when it never delivered. `line`, `summary`, `exit_code` and `outcome_mark` answer it.

- [ ] **Step 1: Write the failing tests**

`pns/crates/pns-domain/src/doctor/outcome/tests.rs`:

```rust
use super::*;

fn check(plugin: &'static str, kind: CheckKind) -> Check {
    Check { plugin, kind }
}

fn unpaired() -> PairingReport {
    PairingReport { pairing: Pairing::Unpaired, server: None }
}

fn uninstalled() -> PairingReport {
    PairingReport { pairing: Pairing::NotInstalled, server: None }
}

#[test]
fn a_withheld_send_names_its_last_delivery() {
    let when = Some("September 23, 2026 at 03:20 UTC, 1h ago".to_string());
    assert_eq!(
        line(&check("phone", CheckKind::Send), &Outcome::Withheld(when)),
        "phone: not sent (--no-send); last delivered September 23, 2026 at 03:20 UTC, 1h ago"
    );
    assert_eq!(
        line(&check("phone", CheckKind::Send), &Outcome::Withheld(None)),
        "phone: not sent (--no-send); never delivered"
    );
}

#[test]
fn a_withheld_pulse_says_it_was_not_pulsed() {
    assert_eq!(
        line(&check("lights", CheckKind::Pulse), &Outcome::Withheld(None)),
        "lights: not pulsed (--no-send)"
    );
}

#[test]
fn a_withheld_check_is_a_note_and_never_fails_the_run() {
    assert_eq!(outcome_mark(&Outcome::Withheld(None)), Mark::Note);
    assert_eq!(exit_code(&[Outcome::Withheld(None)], &uninstalled()), 0);
    assert_eq!(exit_code(&[Outcome::Withheld(None)], &unpaired()), 1);
    assert_eq!(
        exit_code(&[Outcome::Withheld(None), Outcome::Failed("no".into())], &uninstalled()),
        1
    );
}

#[test]
fn the_summary_counts_withheld_only_when_there_are_any() {
    assert_eq!(
        summary(&[Outcome::Sent("ok".into())]),
        "pns doctor: 1 sent, 0 failed, 0 skipped"
    );
    assert_eq!(
        summary(&[Outcome::Withheld(None), Outcome::Skipped("off")]),
        "pns doctor: 0 sent, 0 failed, 1 skipped, 1 withheld"
    );
}
```

Add `#[cfg(test)] mod tests;` at the bottom of `outcome.rs`. `Pairing::NotInstalled` is whichever variant
`pairing.rs` names for a machine without moshi-hook; use that variant's real name.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-domain doctor::outcome::tests`
Expected: compile error, `no variant named Withheld found for enum Outcome`.

- [ ] **Step 3: Implement the outcome**

In `outcome.rs`, add the variant:

```rust
    /// Nothing was sent, because the run was asked to send nothing. The text is
    /// when this destination last delivered, rendered by the caller, or `None`
    /// when the ledger holds no delivery for it.
    Withheld(Option<String>),
```

In `line`, before the `Skipped` arm:

```rust
        Outcome::Withheld(_) if check.kind == CheckKind::Pulse => {
            format!("{plugin}: not pulsed (--no-send)")
        }
        Outcome::Withheld(Some(when)) => {
            format!("{plugin}: not sent (--no-send); last delivered {when}")
        }
        Outcome::Withheld(None) => format!("{plugin}: not sent (--no-send); never delivered"),
```

Add `Withheld` to `Verdict`, map `Outcome::Withheld(_) => Verdict::Withheld` in `verdict`, and
`Outcome::Withheld(_)` to `Mark::Note` in `outcome_mark`. `summary` appends `, {n} withheld` only when
`n > 0`, so a plain run's line is byte-identical. `exit_code`'s last rule counts a withheld check as a
check that had something to check:

```rust
    i32::from(!outcomes.iter().any(|outcome| {
        matches!(verdict(outcome), Verdict::Sent | Verdict::Withheld)
    }))
```

- [ ] **Step 4: Run the domain suite**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-domain doctor`
Expected: all pass, the four new tests included.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-domain/src/doctor
SKIP_AI_COMMIT=1 git commit -m "feat(pns): give the doctor a withheld outcome for a run that sends nothing"
```

### Task 2.2: `RunDoctor` withholds its sends

**Files:**
- Modify: `pns/crates/pns-application/src/doctor.rs`
- Modify: `pns/crates/pns-application/src/lib.rs` (re-export `Sending`)
- Create: `pns/crates/pns-application/src/doctor/tests/withheld.rs`
- Modify: `pns/crates/pns-application/src/doctor/tests.rs` (the `report` helper passes
  `sending: Sending::Send` and the two new actions; `mod withheld;`)

**Interfaces:**
- Consumes: `Outcome::Withheld`.
- Produces:
  - `pub enum pns_application::Sending { Send, Withhold }`
  - `RunDoctor { .., pub sending: Sending }`
  - `DoctorActions { .., pub last_delivered: Box<dyn FnOnce() -> Vec<(String, String)>> }`, the
    rendered last delivery per destination name, read only under `Withhold`
  - `DoctorActions.home` becomes `Box<dyn FnOnce() -> Vec<pns_domain::doctor::Item>>`, so a caller can
    hand it a home directory

- [ ] **Step 1: Write the failing test**

`pns/crates/pns-application/src/doctor/tests/withheld.rs`:

```rust
use super::*;

/// UNDER `Withhold` THE THREE SPENDING ACTIONS ARE NEVER CALLED: each panics,
/// so the run completing at all is the proof.
#[test]
fn a_withholding_run_sends_nothing_and_reads_the_last_delivery() {
    let history = History::default();
    let checks = [
        Check { plugin: "alpha", kind: CheckKind::Send },
        Check { plugin: "beta", kind: CheckKind::Send },
        Check { plugin: "lights", kind: CheckKind::Pulse },
    ];
    let mut lines = Vec::new();
    let code = RunDoctor {
        decisions: pns_domain::doctor::Detail::Spoken,
        sending: Sending::Withhold,
        checks: &checks,
        records: &history,
        clock: &|| Some(100),
        replay_card: false,
        remind_delay_secs: 0,
    }
    .run(
        DoctorActions {
            deliver: |_: &[Leg], _: &EventArgs| -> Vec<(Leg, Delivery)> {
                panic!("a withholding run delivered")
            },
            pulse: || -> Outcome { panic!("a withholding run pulsed") },
            summarizer: Box::new(|| panic!("a withholding run ran the summarizer")),
            last_delivered: Box::new(|| {
                vec![("alpha".to_string(), "September 23, 2026 at 03:20 UTC, 1h ago".to_string())]
            }),
            presence: || (PresenceStatus::Nowhere { poll_age_secs: 2 }, None),
            pairing: || PairingReport { pairing: Pairing::Paired, server: None },
            tap: || pns_domain::doctor::Item::note("tap fixture"),
            focus: || "focus fixture".into(),
            daemon: || "daemon fixture".into(),
            home: Box::new(Vec::new),
            lamps: || LightsReport::Off,
            certificate: || {
                pns_domain::doctor::certificate_row(&pns_domain::doctor::PinState::Unconfigured)
            },
            delivery_health: || history.health.clone(),
            routes: || history.routes.clone(),
            imports: || Ok(Vec::new()),
        },
        |item| {
            if let pns_domain::doctor::Item::Row { .. } = item {
                lines.push(item.text().to_string());
            }
        },
    );
    assert!(lines.contains(
        &"alpha: not sent (--no-send); last delivered September 23, 2026 at 03:20 UTC, 1h ago"
            .to_string()
    ));
    assert!(lines.contains(&"beta: not sent (--no-send); never delivered".to_string()));
    assert!(lines.contains(&"lights: not pulsed (--no-send)".to_string()));
    assert!(lines.contains(&"summarizer: not run (--no-send)".to_string()));
    assert_eq!(code, 0);
}
```

`Pairing::Paired` is `pairing.rs`'s variant for a paired host; use its real name.

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-application doctor::tests::withheld`
Expected: compile error, `cannot find type Sending in this scope`.

- [ ] **Step 3: Implement the withholding**

In `doctor.rs`:

```rust
/// Whether this run may spend: a test send per destination, a lamp pulse and
/// one summarizer run. `Withhold` is `pns doctor --no-send` and every served
/// page; it reads everything the report reads and spends nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sending {
    Send,
    Withhold,
}
```

Add `pub sending: Sending` to `RunDoctor` and `pub last_delivered: Box<dyn FnOnce() -> Vec<(String,
String)>>` to `DoctorActions`; change `home`'s type to `Box<dyn FnOnce() -> Vec<pns_domain::doctor::Item>>`
and call it as `(actions.home)()`. In `run`, replace the deliver call and the two outcome arms:

```rust
        let delivered = match self.sending {
            Sending::Send => (actions.deliver)(&legs, &event),
            Sending::Withhold => Vec::new(),
        };
        let last = match self.sending {
            Sending::Send => Vec::new(),
            Sending::Withhold => (actions.last_delivered)(),
        };
```

```rust
                pns_domain::doctor::CheckKind::Pulse => match self.sending {
                    Sending::Send => (actions.pulse)(),
                    Sending::Withhold => pns_domain::doctor::Outcome::Withheld(None),
                },
                pns_domain::doctor::CheckKind::Send if self.sending == Sending::Withhold => {
                    pns_domain::doctor::Outcome::Withheld(
                        last.iter()
                            .find(|(name, _)| name == check.plugin)
                            .map(|(_, when)| when.clone()),
                    )
                }
```

and where the summarizer row is emitted:

```rust
        emit(match self.sending {
            Sending::Send => (actions.summarizer)(),
            Sending::Withhold => Item::note("summarizer: not run (--no-send)"),
        });
```

In `tests.rs`'s `report` helper, pass `sending: Sending::Send`, `last_delivered: Box::new(Vec::new)` and
`home: Box::new(Vec::new)`. Re-export `Sending` from `pns-application/src/lib.rs` beside `RunDoctor`.

- [ ] **Step 4: Run the application suite**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-application doctor`
Expected: all pass; every existing doctor test is unchanged in its assertions.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-application/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): let the doctor run without sending, pulsing or summarizing"
```

### Task 2.3: Each destination's last delivery, from the ledger

**Files:**
- Modify: `pns/crates/pns-adapters/src/persistence/sqlite/ledger.rs` (`last_delivered_each`)
- Modify: `pns/crates/pns-adapters/src/persistence/sqlite/ledger/failing.rs` (the query)
- Create: `pns/crates/pns-adapters/src/persistence/sqlite/ledger/tests/last_delivered.rs`
- Modify: `pns/crates/pns-adapters/src/persistence/sqlite/ledger/tests.rs` (`mod last_delivered;`)

**Interfaces:**
- Produces: `pub fn SqliteStore::last_delivered_each(&self) -> Result<Vec<(String, u64)>,
  LedgerFailure>`, destination name and the newest `finished` of an attempt with `outcome = 1`, sorted by
  name.

- [ ] **Step 1: Write the failing tests**

`ledger/tests/last_delivered.rs`, using the seeding helpers `ledger/tests.rs` already has
(`remote_input`, `created`, `lease`, `state`; `remote_input`'s one leg is `hermes`, as
`a_failing_leg_reports_its_route_producer_and_answer` shows):

```rust
use super::*;

#[test]
fn an_acknowledged_attempt_is_its_destinations_last_delivery() {
    let store = SqliteStore::new(state());
    let claims = created(&store, &remote_input());
    store
        .record(&claims[0].claim, &pns_domain::Delivery::Delivered("ok".into()), 13, Default::default())
        .unwrap();
    assert_eq!(store.last_delivered_each().unwrap(), vec![("hermes".to_string(), 13)]);
}

#[test]
fn a_failed_attempt_is_no_delivery() {
    let store = SqliteStore::new(state());
    let claims = created(&store, &remote_input());
    store
        .record(&claims[0].claim, &pns_domain::Delivery::Failed("no".into()), 13, Default::default())
        .unwrap();
    assert_eq!(store.last_delivered_each().unwrap(), vec![]);
}

#[test]
fn a_viewer_reads_it() {
    let state = state();
    let store = SqliteStore::new(state.clone());
    let claims = created(&store, &remote_input());
    store
        .record(&claims[0].claim, &pns_domain::Delivery::Delivered("ok".into()), 13, Default::default())
        .unwrap();
    assert_eq!(
        SqliteStore::viewer(state).last_delivered_each().unwrap(),
        vec![("hermes".to_string(), 13)]
    );
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters last_delivered`
Expected: compile error, `no method named last_delivered_each`.

- [ ] **Step 3: Implement the read**

In `ledger/failing.rs`:

```rust
/// The newest acknowledged attempt per destination. Keyed on `outcome = 1`,
/// never on `acknowledged`, which the dead-letter drain also sets.
pub(super) fn last_delivered(connection: &Connection) -> Result<Vec<(String, u64)>, StoreError> {
    let mut query = connection.prepare(
        "SELECT l.destination, MAX(a.finished) FROM ledger_legs l
           JOIN ledger_attempts a ON a.leg = l.id
          WHERE a.outcome = 1 AND a.finished IS NOT NULL
          GROUP BY l.destination ORDER BY l.destination",
    )?;
    let rows = query
        .query_map([], |row| {
            let finished: Vec<u8> = row.get(1)?;
            let bytes: [u8; 8] = finished.try_into().unwrap_or([0; 8]);
            Ok((row.get::<_, String>(0)?, u64::from_be_bytes(bytes)))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}
```

In `ledger.rs`, beside `failing_legs`:

```rust
    pub fn last_delivered_each(&self) -> Result<Vec<(String, u64)>, LedgerFailure> {
        self.failing(failing::last_delivered)
    }
```

`MAX` over the 8-byte big-endian BLOB orders numerically, which is why the ledger stores times that way.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters last_delivered`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-adapters/src/persistence/sqlite/ledger.rs pns/crates/pns-adapters/src/persistence/sqlite/ledger
SKIP_AI_COMMIT=1 git commit -m "feat(pns): read each destination's last delivery from the ledger"
```

### Task 2.4: `pns doctor --no-send`

**Files:**
- Modify: `pns/crates/pns/src/command_doctor.rs` (argv parse; `run_report` extracted from `doctor_mode`)
- Create: `pns/crates/pns/src/command_doctor/tests.rs`
- Modify: `pns/crates/pns/src/doctor_home.rs` (`rows_at(home, state)` and `read_rows(home)`)
- Modify: `pns/crates/pns/src/doctor_style.rs` (`unattributed` and `closing` become `pub(crate)`
  functions `Report` calls)
- Modify: `pns/crates/pns/src/legacy/usage.rs` (the doctor line)

**Interfaces:**
- Consumes: `Sending`, `last_delivered_each`, `pns_adapters::{utc_long}`, `pns_domain::doctor::ago`,
  `view::Sources`.
- Produces:

```rust
pub(crate) fn parse(arguments: &[String]) -> Option<(pns_domain::doctor::Detail, pns_application::Sending)>;
pub(crate) fn run_report(
    sources: &Sources,
    detail: pns_domain::doctor::Detail,
    sending: pns_application::Sending,
    emit: impl FnMut(pns_domain::doctor::Item),
) -> i32;
pub(crate) fn last_delivered(sources: &Sources, now: u64) -> Vec<(String, String)>;
pub(crate) fn summary(sources: &Sources, now: u64) -> Summary;
// doctor_style.rs
pub(crate) fn unattributed(text: &str) -> &str;
pub(crate) struct Closing { pub(crate) headline: String, pub(crate) tone: Tone, pub(crate) entries: Vec<String> }
pub(crate) fn closing(issues: &[String], warnings: &[String]) -> Closing;
// doctor_home.rs
pub(crate) fn read_rows(home: &str) -> Vec<pns_domain::doctor::Item>;
```

- [ ] **Step 1: Write the failing tests**

`pns/crates/pns/src/command_doctor/tests.rs`:

```rust
use super::*;
use pns_application::Sending;
use pns_domain::doctor::Detail;

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|word| (*word).to_string()).collect()
}

#[test]
fn the_two_flags_parse_in_either_order() {
    assert_eq!(parse(&words(&[])), Some((Detail::Spoken, Sending::Send)));
    assert_eq!(parse(&words(&["--no-send"])), Some((Detail::Spoken, Sending::Withhold)));
    assert_eq!(parse(&words(&["--raw", "--no-send"])), Some((Detail::Raw, Sending::Withhold)));
    assert_eq!(parse(&words(&["--no-send", "--raw"])), Some((Detail::Raw, Sending::Withhold)));
}

#[test]
fn anything_else_is_refused() {
    for tail in [&["--nosend"][..], &["--no-send", "--no-send"], &["--raw", "x"], &["send"]] {
        assert_eq!(parse(&words(tail)), None, "{tail:?}");
    }
}

#[test]
fn a_withholding_report_over_a_sandbox_names_every_send_as_not_sent() {
    let state = std::env::temp_dir().join(format!("pns-doctor-nosend-{}", std::process::id()));
    std::fs::create_dir_all(&state).unwrap();
    let sources = crate::view::Sources::at(state, "/nonexistent-home");
    let mut rows = Vec::new();
    run_report(&sources, Detail::Spoken, Sending::Withhold, |item| {
        if let pns_domain::doctor::Item::Row { text, .. } = item {
            rows.push(text);
        }
    });
    assert!(rows.iter().any(|row| row.contains("not sent (--no-send)")), "{rows:#?}");
    assert!(rows.iter().all(|row| !row.contains(": sent,")), "{rows:#?}");
    assert!(rows.contains(&"summarizer: not run (--no-send)".to_string()), "{rows:#?}");
}
```

Append to `doctor_style/tests.rs`:

```rust
#[test]
fn the_closing_counts_are_one_function() {
    let none = closing(&[], &[]);
    assert_eq!(none.headline, "nothing to act on");
    assert_eq!(none.tone, Tone::Good);
    let mixed = closing(&["a".into()], &["b".into(), "c".into()]);
    assert_eq!(mixed.headline, "1 issue to fix, 2 warnings to look at:");
    assert_eq!(mixed.tone, Tone::Bad);
    assert_eq!(mixed.entries, ["a", "b", "c"]);
    assert_eq!(closing(&[], &["b".into()]).tone, Tone::Warn);
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib command_doctor doctor_style`
Expected: compile errors, `cannot find function parse` and `cannot find function closing`.

- [ ] **Step 3: Extract and implement**

In `command_doctor.rs`:

```rust
pub(crate) const DOCTOR_USAGE: &str = "pns: usage: pns doctor [--raw] [--no-send]";

const NO_SEND_FLAG: &str = "--no-send";

/// The doctor's argv: each of the two flags at most once, in either order.
pub(crate) fn parse(
    arguments: &[String],
) -> Option<(pns_domain::doctor::Detail, pns_application::Sending)> {
    let mut detail = pns_domain::doctor::Detail::Spoken;
    let mut sending = pns_application::Sending::Send;
    let mut seen = Vec::new();
    for argument in arguments {
        if seen.contains(&argument.as_str()) {
            return None;
        }
        match argument.as_str() {
            RAW_FLAG => detail = pns_domain::doctor::Detail::Raw,
            NO_SEND_FLAG => sending = pns_application::Sending::Withhold,
            _ => return None,
        }
        seen.push(argument.as_str());
    }
    Some((detail, sending))
}
```

`doctor_mode` becomes the parse, the header, one `run_report` call with a `Sources` it builds exactly as
today's code opens its stores, and the close:

```rust
pub(crate) fn doctor_mode() -> i32 {
    let Some((detail, sending)) = parse(&crate::arguments_after_subcommand()) else {
        eprintln!("{DOCTOR_USAGE}");
        return 2;
    };
    let mut report = doctor_style::Report::new(style::Paint::for_stdout());
    print_lines(report.open());
    let state = state_dir();
    let sources = crate::view::Sources {
        store: pns_adapters::SqliteStore::for_records(state.clone()),
        state,
        home: std::env::var("HOME").unwrap_or_default(),
    };
    let code = run_report(&sources, detail, sending, |item| print_lines(report.item(&item)));
    print_lines(report.close());
    code
}
```

`run_report` holds today's body of `doctor_mode` from the config load to the `.run(...)` call, moved
without edits except these substitutions: `std::env::var("HOME")` becomes `sources.home`, `state_dir()`
becomes `sources.state`, every `SqliteStore::for_records(state_dir())` and `SqliteStore::new(state_dir())`
becomes `&sources.store`, `RunDoctor` gains `sending`, and the actions gain the two boxed readings. Both
boxes are `'static`, so each owns what it reads: the last deliveries are read before the run, and only
when withholding.

```rust
    let last = match sending {
        pns_application::Sending::Send => Vec::new(),
        pns_application::Sending::Withhold => last_delivered(sources, now_secs().unwrap_or(0)),
    };
```

```rust
            last_delivered: Box::new(move || last),
            home: match sending {
                pns_application::Sending::Send => {
                    let home = sources.home.clone();
                    let state = sources.state.clone();
                    Box::new(move || crate::doctor_home::rows_at(&home, &state))
                }
                pns_application::Sending::Withhold => {
                    let home = sources.home.clone();
                    Box::new(move || crate::doctor_home::read_rows(&home))
                }
            },
```

`last_delivered` renders each ledger time with the site's formatter and the doctor's own ago:

```rust
pub(crate) fn last_delivered(sources: &Sources, now: u64) -> Vec<(String, String)> {
    sources
        .store
        .last_delivered_each()
        .unwrap_or_default()
        .into_iter()
        .map(|(destination, at)| {
            let when = format!(
                "{}, {}",
                pns_adapters::utc_long(at),
                pns_domain::doctor::ago(now.saturating_sub(at))
            );
            (destination, when)
        })
        .collect()
}
```

`summary` is the Health index row, from the heartbeat and the dead-letter count only:

```rust
pub(crate) fn summary(sources: &Sources, now: u64) -> Summary {
    let beating = pns_adapters::daemon_heartbeat(&sources.state)
        .and_then(|beat| now.checked_sub(beat.at))
        .is_some_and(|age| age <= pns_domain::jobs::HEARTBEAT_STALE_SECS);
    let dead = sources.store.dead_lettered_leg_count().unwrap_or(0);
    let daemon = if beating { "daemon running" } else { "daemon not running" };
    let text = match dead {
        0 => daemon.to_string(),
        count => format!("{daemon}, {count} not delivered"),
    };
    let tone = if beating && dead == 0 { Tone::Good } else { Tone::Bad };
    Summary { text, tone }
}
```

In `doctor_home.rs`, `rows()` becomes `rows_at(home, state)` (the same body reading `config_path(home)`
and `SqliteStore::for_records(state.to_path_buf())`), and `read_rows(home)` is the same config walk ending
in the pure read, which never alerts and never writes:

```rust
pub(crate) fn read_rows(home: &str) -> Vec<pns_domain::doctor::Item> {
    // the same early returns as `rows_at`, down to `let router = ...`
    let reading = pns_application::read_home(&router, &settings.device);
    crate::home_report::rows(&reading, None)
}
```

The config walk both share is one private function returning `Result<(UniFiRouter, DeviceIdentity),
Vec<Item>>`, so the two differ only in their last two lines.

In `doctor_style.rs`, lift the two helpers out of `Report` and have `Report::item` and `Report::close`
call them:

```rust
pub(crate) fn unattributed(text: &str) -> &str {
    text.strip_prefix("pns doctor: ").unwrap_or(text)
}

pub(crate) struct Closing {
    pub(crate) headline: String,
    pub(crate) tone: Tone,
    pub(crate) entries: Vec<String>,
}

pub(crate) fn closing(issues: &[String], warnings: &[String]) -> Closing {
    if issues.is_empty() && warnings.is_empty() {
        return Closing { headline: "nothing to act on".into(), tone: Tone::Good, entries: Vec::new() };
    }
    let headline = counted(issues, "issue to fix")
        .into_iter()
        .chain(counted(warnings, "warning to look at"))
        .collect::<Vec<_>>()
        .join(", ");
    Closing {
        headline: format!("{headline}:"),
        tone: if issues.is_empty() { Tone::Warn } else { Tone::Bad },
        entries: issues.iter().chain(warnings).cloned().collect(),
    }
}
```

`Report::close` renders a `Closing`: the good glyph and the headline when there are no entries, else the
headline painted by its tone and the numbered entries, byte-identical to today (the existing
`doctor_style` tests pin it). The usage line in `legacy/usage.rs` becomes:

```
  pns doctor [--raw] [--no-send]   one test send through every channel, or none
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib command_doctor doctor_style subcommand_usage`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): add pns doctor --no-send, the report with nothing sent"
```

### Task 2.5: The Health page, its route and its index row

**Files:**
- Create: `pns/crates/pns/src/site/health.rs`
- Create: `pns/crates/pns/src/site/health/tests.rs`
- Modify: `pns/crates/pns/src/site.rs` (`Target::Health`, the arm)
- Modify: `pns/crates/pns/src/site/shell.rs` (the doctor row dot rules)
- Modify: `pns/crates/pns/src/site/index.rs` (the Health row)
- Test: `pns/crates/pns/src/site/tests.rs`

**Interfaces:**
- Consumes: `command_doctor::{run_report, summary}`, `doctor_style::{unattributed, closing, Closing}`,
  `Sending::Withhold`.
- Produces: `pub(super) fn site::health::page(items: &[pns_domain::doctor::Item], rendered_at: u64) ->
  String`.

- [ ] **Step 1: Write the failing tests**

`pns/crates/pns/src/site/health/tests.rs`:

```rust
use super::*;
use pns_domain::doctor::{Item, Mark};

const NOW: u64 = 1_790_139_720;

fn report(text: &str) -> Vec<Item> {
    vec![
        Item::section("Channels", "whether each destination is reachable"),
        Item::row(Mark::Note, "phone: not sent (--no-send); last delivered September 23, 2026 at 03:20 UTC, 1h ago"),
        Item::row(Mark::Bad, text),
        Item::row(Mark::Detail, "run pns failures for the record"),
        Item::section("Daemon and gates", "the clock and what can silence a notification"),
        Item::row(Mark::Good, "pns doctor: the daemon is running, pid 812, 4 jobs scheduled"),
        Item::row(Mark::Warn, "the lamp dim window is inside its hours"),
    ]
}

#[test]
fn every_row_is_on_the_page_without_its_prefix() {
    let served = page(&report("hermes route uu-runs: HTTP 404"), NOW);
    for text in [
        "Channels",
        "phone: not sent (--no-send); last delivered September 23, 2026 at 03:20 UTC, 1h ago",
        "hermes route uu-runs: HTTP 404",
        "run pns failures for the record",
        "the daemon is running, pid 812, 4 jobs scheduled",
        "the lamp dim window is inside its hours",
    ] {
        assert!(served.contains(&escaped(text)), "{text:?} is not on the page");
    }
    assert!(!served.contains("pns doctor: the daemon"), "{served}");
}

#[test]
fn the_headline_and_meaning_come_from_the_closing_summary() {
    let served = page(&report("hermes route uu-runs: HTTP 404"), NOW);
    assert!(served.contains("1 to fix"), "{served}");
    assert!(served.contains("1 issue to fix, 1 warning to look at:"), "{served}");
    assert!(served.contains("fr-status fr-red"), "{served}");
}

#[test]
fn a_clean_report_is_all_good_in_the_neutral_tone() {
    let clean = vec![Item::section("Daemon and gates", "the clock"), Item::row(Mark::Good, "running")];
    let served = page(&clean, NOW);
    assert!(served.contains("All good"), "{served}");
    assert!(served.contains("nothing to act on"), "{served}");
    assert!(served.contains("fr-status fr-quiet"), "{served}");
}

#[test]
fn producer_text_in_a_row_is_escaped_once() {
    let served = page(&report("route <b>&</b>"), NOW);
    assert!(served.contains("route &lt;b&gt;&amp;&lt;/b&gt;"), "{served}");
}
```

In `site/tests.rs`:

```rust
#[test]
fn the_health_page_is_its_exact_path_only() {
    assert_eq!(target("GET /health HTTP/1.1\r\n"), Some(Target::Health));
    for line in ["GET /health/ HTTP/1.1\r\n", "GET /health?send=1 HTTP/1.1\r\n", "POST /health HTTP/1.1\r\n"] {
        assert_eq!(target(line), None, "{line:?} was served");
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib site::`
Expected: compile errors, `no variant Health` and `cannot find function page in module health`.

- [ ] **Step 3: Implement the page**

`pns/crates/pns/src/site/health.rs`:

```rust
//! The Health page: `pns doctor --no-send`'s items, laid out as the record card.

use super::shell::{escaped, footer, page as shell_page};
use crate::doctor_style::{closing, unattributed};
use crate::style::Tone;
use pns_domain::doctor::{Item, Mark};

pub(super) fn page(items: &[Item], rendered_at: u64) -> String {
    let (issues, warnings) = graded(items);
    let closed = closing(&issues, &warnings);
    let (chip, class) = match closed.tone {
        Tone::Bad => (format!("{} to fix", issues.len()), "fr-status fr-red"),
        Tone::Warn => (format!("{} to look at", warnings.len()), "fr-status"),
        Tone::Good | Tone::Quiet => ("All good".to_string(), "fr-status fr-quiet"),
    };
    let entries: String = closed
        .entries
        .iter()
        .map(|entry| format!("<li>{}</li>", escaped(entry)))
        .collect();
    let list = if entries.is_empty() { String::new() } else { format!("<ol class=\"dr-closing\">{entries}</ol>") };
    let card = format!(
        "<article id=\"failure-record\"><a class=\"fr-back\" href=\"/\">pns</a>\
         <header><div class=\"fr-heading\"><h2>Health</h2><span class=\"{class}\">{chip}</span></div>\
         <p class=\"fr-meaning\">{meaning}</p></header>{list}\n{sections}\n{}</article>",
        footer(rendered_at),
        meaning = escaped(&closed.headline),
        sections = sections(items),
    );
    shell_page("pns health", &card)
}

fn graded(items: &[Item]) -> (Vec<String>, Vec<String>) {
    let mut issues = Vec::new();
    let mut warnings = Vec::new();
    for item in items {
        if let Item::Row { mark, text } = item {
            match mark {
                Mark::Bad => issues.push(unattributed(text).to_string()),
                Mark::Warn => warnings.push(unattributed(text).to_string()),
                _ => {}
            }
        }
    }
    (issues, warnings)
}

fn sections(items: &[Item]) -> String {
    let mut out = String::new();
    let mut open = false;
    for item in items {
        match item {
            Item::Section { title, blurb } => {
                if open {
                    out.push_str("</ul></section>");
                }
                out.push_str(&format!(
                    "<section class=\"dr-section\"><h3>{}</h3><p class=\"fr-meta\">{}</p><ul class=\"dr-rows\">",
                    escaped(title),
                    escaped(blurb)
                ));
                open = true;
            }
            Item::Row { mark, text } => out.push_str(&format!(
                "<li class=\"{}\">{}</li>",
                mark_class(*mark),
                escaped(unattributed(text))
            )),
        }
    }
    if open {
        out.push_str("</ul></section>");
    }
    out
}

fn mark_class(mark: Mark) -> &'static str {
    match mark {
        Mark::Good => "dr-good",
        Mark::Bad => "dr-bad",
        Mark::Warn => "dr-warn",
        Mark::Note => "dr-note",
        Mark::Detail => "dr-detail",
        Mark::Aside => "dr-aside",
    }
}

#[cfg(test)]
mod tests;
```

Append the row rules to the CSS constant in `shell.rs`:

```css
#failure-record .dr-section { border-top:1px solid var(--fr-line); padding-top:14px; margin-top:18px; }
#failure-record .dr-section h3 { font-size:14px; font-weight:600; margin:0; }
#failure-record .dr-rows { list-style:none; margin:10px 0 0; padding:0; font-size:13px; }
#failure-record .dr-rows li { position:relative; padding:3px 0 3px 18px; overflow-wrap:anywhere; }
#failure-record .dr-rows li::before { content:''; position:absolute; left:2px; top:10px; width:8px; height:8px; border-radius:50%; }
#failure-record .dr-good::before { background:var(--fr-muted); }
#failure-record .dr-warn::before { border:2px solid var(--fr-amber); width:4px; height:4px; }
#failure-record .dr-bad::before { background:#ff8993; }
#failure-record .dr-detail, #failure-record .dr-aside { color:var(--fr-muted); padding-left:34px; }
#failure-record .dr-closing { margin:0 0 8px; padding-left:20px; font-size:13px; }
```

In `site.rs`, `Target::Health` on `/health`, and the arm collects the withheld report:

```rust
        Some(Target::Health) => {
            let mut items = Vec::new();
            crate::command_doctor::run_report(
                sources,
                pns_domain::doctor::Detail::Spoken,
                pns_application::Sending::Withhold,
                |item| items.push(item),
            );
            ok(&health::page(&items, now))
        }
```

The arm passes `Sending::Withhold` as a literal: nothing a request carries can reach it, and no query
string is parsed (D2). In `index.rs`, insert the Health row after Deliveries' place in the final order
(for now, after Failures):

```rust
        Row {
            href: "/health",
            name: "Health",
            blurb: "the destinations, the gateway and the daemon",
            summary: crate::command_doctor::summary(sources, now),
        },
```

- [ ] **Step 4: Run the site tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib site::`
Expected: all pass.

- [ ] **Step 5: Run the gates and check file sizes**

```bash
just test-rust; echo "test-rust exit $?"
just lint-check; echo "lint-check exit $?"
just test-unit; echo "test-unit exit $?"
```

Expected: three zero exit codes. Then the file-size command over `pns/crates`; `command_doctor.rs` must
stay under 500 total lines, and if the extraction pushed it past 400, move `run_report` into
`command_doctor/report.rs` in the same commit.

- [ ] **Step 6: Commit**

```bash
git add pns/crates/pns/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): serve the Health page, the doctor with nothing sent"
```

---

## Pull request 3: Recap

Branch `feat/pns-page-recap`. Worktree:
`herdr worktree create --cwd /Users/stephen/workspaces/Ivy/webdavis/dotfiles --branch feat/pns-page-recap --no-focus`

### Task 3.1: The recap history's wording, in the domain

**Files:**
- Create: `pns/crates/pns-domain/src/recap/history.rs`
- Create: `pns/crates/pns-domain/src/recap/history/tests.rs`
- Modify: `pns/crates/pns-domain/src/recap.rs` (`pub mod history;`)

**Interfaces:**
- Consumes: `recap::activity::{Project, Session}`, `recap::window::LocalCivilTime`.
- Produces:

```rust
pub fn tally_line(projects: &[Project]) -> String;           // "42 events, 5 sessions in 3 projects"
pub fn project_line(project: &Project) -> String;            // "dotfiles: 3 sessions, 30 events"
pub fn covers(summary_covers: u64, since: u64, until: u64) -> bool;
pub fn day_label(day: LocalCivilTime, today: LocalCivilTime) -> String; // "Today", "Yesterday", "Sep 21"
pub fn chip(until: u64, now: u64) -> &'static str;           // "In progress" or "Complete"
```

- [ ] **Step 1: Write the failing tests**

`pns/crates/pns-domain/src/recap/history/tests.rs`:

```rust
use super::*;
use crate::recap::activity::{Event, by_project};

fn event(project: &str, session: &str, at: u64) -> Event {
    Event {
        at,
        agent: "claude".into(),
        state: "done".into(),
        project: project.into(),
        session: session.into(),
        ..Default::default()
    }
}

fn civil(year: i32, month: u32, day: u32) -> LocalCivilTime {
    LocalCivilTime { year, month, day, hour: 9, minute: 0, second: 0 }
}

#[test]
fn the_tally_counts_events_sessions_and_projects() {
    let projects = by_project(&[
        event("dotfiles", "a", 1),
        event("dotfiles", "a", 2),
        event("dotfiles", "b", 3),
        event("pns", "c", 4),
    ]);
    assert_eq!(tally_line(&projects), "4 events, 3 sessions in 2 projects");
    assert_eq!(project_line(&projects[0]), "dotfiles: 2 sessions, 3 events");
}

#[test]
fn one_of_each_is_singular_and_nothing_says_so() {
    let projects = by_project(&[event("pns", "c", 4)]);
    assert_eq!(tally_line(&projects), "1 event, 1 session in 1 project");
    assert_eq!(project_line(&projects[0]), "pns: 1 session, 1 event");
    assert_eq!(tally_line(&[]), "nothing recorded");
}

#[test]
fn a_summary_belongs_to_the_instance_its_newest_event_falls_in() {
    assert!(covers(150, 100, 200));
    assert!(covers(200, 100, 200));
    assert!(!covers(100, 100, 200));
    assert!(!covers(201, 100, 200));
}

#[test]
fn days_read_as_the_operator_says_them() {
    let today = civil(2026, 9, 23);
    assert_eq!(day_label(civil(2026, 9, 23), today), "Today");
    assert_eq!(day_label(civil(2026, 9, 22), today), "Yesterday");
    assert_eq!(day_label(civil(2026, 9, 21), today), "Sep 21");
    assert_eq!(day_label(civil(2026, 8, 31), civil(2026, 9, 1)), "Yesterday");
}

#[test]
fn an_instance_is_in_progress_until_its_end_passes() {
    assert_eq!(chip(200, 199), "In progress");
    assert_eq!(chip(200, 200), "Complete");
}
```

`Event` derives `Default` if it does not already; add `#[derive(Default)]` beside its existing derives
if the compiler says it is missing. `LocalCivilTime`'s fields are the six `window::epoch` reads.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-domain recap::history`
Expected: compile error, `file not found for module history`.

- [ ] **Step 3: Implement the wording**

`pns/crates/pns-domain/src/recap/history.rs`:

```rust
//! What `pns recap history` and the Recap page say about one window instance.

use super::activity::Project;
use super::window::LocalCivilTime;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn counted(count: usize, noun: &str) -> String {
    if count == 1 { format!("1 {noun}") } else { format!("{count} {noun}s") }
}

fn events_in(project: &Project) -> usize {
    project.sessions.iter().map(|session| session.events.len()).sum()
}

pub fn tally_line(projects: &[Project]) -> String {
    if projects.is_empty() {
        return "nothing recorded".to_string();
    }
    let events: usize = projects.iter().map(events_in).sum();
    let sessions: usize = projects.iter().map(|project| project.sessions.len()).sum();
    format!(
        "{}, {} in {}",
        counted(events, "event"),
        counted(sessions, "session"),
        counted(projects.len(), "project")
    )
}

pub fn project_line(project: &Project) -> String {
    format!(
        "{}: {}, {}",
        project.project,
        counted(project.sessions.len(), "session"),
        counted(events_in(project), "event")
    )
}

/// The stored summary's newest event falls inside this instance, half open
/// at the start the way every recap window is.
pub fn covers(summary_covers: u64, since: u64, until: u64) -> bool {
    summary_covers > since && summary_covers <= until
}

pub fn day_label(day: LocalCivilTime, today: LocalCivilTime) -> String {
    let same = |a: LocalCivilTime, b: LocalCivilTime| (a.year, a.month, a.day) == (b.year, b.month, b.day);
    if same(day, today) {
        return "Today".to_string();
    }
    if same(day, today.at(0).plus_days(-1)) {
        return "Yesterday".to_string();
    }
    let month = MONTHS.get(day.month.saturating_sub(1) as usize).unwrap_or(&"???");
    format!("{month} {}", day.day)
}

pub fn chip(until: u64, now: u64) -> &'static str {
    if now < until { "In progress" } else { "Complete" }
}

#[cfg(test)]
mod tests;
```

`at(0)` and `plus_days` are `LocalCivilTime`'s own methods, which `window::resolve` already uses.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-domain recap::history`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-domain/src/recap.rs pns/crates/pns-domain/src/recap
SKIP_AI_COMMIT=1 git commit -m "feat(pns): word a recap window instance for the history view"
```

### Task 3.2: `pns recap history`

**Files:**
- Create: `pns/crates/pns/src/command_recap/history.rs`
- Create: `pns/crates/pns/src/command_recap/history/tests.rs`
- Modify: `pns/crates/pns/src/command_recap.rs` (`mod history;`, the `history` arm in `recap_mode`)
- Modify: `pns/crates/pns/src/subcommand_usage.rs` (`("recap history", ...HISTORY_USAGE)`)
- Modify: `pns/crates/pns/src/legacy/usage.rs` (one line)

**Interfaces:**
- Consumes: `recap::history::*`, `command_recap::window::named`, `SqliteStore::{activity_between,
  recap_summary}`, `pns_adapters::{local_civil, local_minutes_since_midnight}`,
  `pns_application::recap_wall_clock`, `view::{Sources, Summary}`.
- Produces:

```rust
pub(crate) const HISTORY_USAGE: &str;
pub(crate) const UNREADABLE: &str =
    "pns: the activity store could not be opened, so there is no recap to give";
pub(crate) struct Instance {
    pub(crate) window: pns_domain::recap::window::Window,
    pub(crate) since: u64,
    pub(crate) until: u64,
    pub(crate) day: LocalCivilTime,
    pub(crate) bounds: String,       // "08:00-12:00 local"
    pub(crate) clock: String,        // "08:00", the start
    pub(crate) ends: String,         // "12:00", the end
    pub(crate) events: Vec<pns_domain::recap::activity::Event>,
    pub(crate) stored: Option<pns_adapters::StoredSummary>,
    pub(crate) written: String,      // "written 12:01 by claude", empty without a summary
}
pub(crate) struct HistoryView { pub(crate) instances: Vec<InstanceRow> }
pub(crate) struct InstanceRow {
    pub(crate) title: String, pub(crate) day: String, pub(crate) clock: String,
    pub(crate) bounds: String, pub(crate) chip: &'static str, pub(crate) tally: String,
    pub(crate) summary: Option<String>, pub(crate) written: String, pub(crate) projects: Vec<String>,
    pub(crate) window: &'static str, pub(crate) ends: String,
}
pub(crate) fn view(instances: &[Instance], today: LocalCivilTime, now: u64) -> HistoryView;
pub(crate) fn read(sources: &Sources, now: u64) -> Result<HistoryView, &'static str>;
pub(crate) fn render(view: &HistoryView) -> String;
pub(crate) fn summary(sources: &Sources, now: u64) -> Summary;
pub(crate) fn history_mode() -> i32;
```

- [ ] **Step 1: Write the failing tests**

`pns/crates/pns/src/command_recap/history/tests.rs`:

```rust
use super::*;
use pns_domain::recap::activity::Event;
use pns_domain::recap::window::{LocalCivilTime, Window};

const NOW: u64 = 1_790_139_720;

fn civil(day: u32) -> LocalCivilTime {
    LocalCivilTime { year: 2026, month: 9, day, hour: 8, minute: 0, second: 0 }
}

fn event(project: &str, session: &str, at: u64) -> Event {
    Event { at, agent: "claude".into(), state: "done".into(), project: project.into(), session: session.into(), ..Default::default() }
}

fn instances() -> Vec<Instance> {
    vec![
        Instance {
            window: Window::Morning,
            since: NOW - 7_200,
            until: NOW + 7_200,
            day: civil(23),
            bounds: "08:00-12:00 local".into(),
            clock: "08:00".into(),
            ends: "12:00".into(),
            events: vec![event("pns", "a", NOW - 60)],
            stored: None,
            written: String::new(),
        },
        Instance {
            window: Window::Nightshift,
            since: NOW - 36_000,
            until: NOW - 7_200,
            day: civil(23),
            bounds: "00:00-08:00 local".into(),
            clock: "00:00".into(),
            ends: "08:00".into(),
            events: vec![event("dotfiles", "b", NOW - 9_000), event("dotfiles", "b", NOW - 8_000)],
            stored: Some(pns_adapters::StoredSummary {
                at: NOW - 7_100,
                covers: NOW - 8_000,
                source: "claude".into(),
                text: "The nightshift <b>renamed</b> the window.".into(),
            }),
            written: "written 03:01 by claude".into(),
        },
    ]
}

#[test]
fn the_terminal_history_is_pinned() {
    assert_eq!(
        render(&view(&instances(), civil(23), NOW)),
        "pns: the last two instances of each recap window\n\
         \x20 Morning     Today      08:00-12:00 local  In progress  1 event, 1 session in 1 project\n\
         \x20 Nightshift  Today      00:00-08:00 local  Complete     2 events, 1 session in 1 project\n\
         \x20             written 03:01 by claude: The nightshift <b>renamed</b> the window.\n"
    );
}

#[test]
fn a_summary_attaches_only_to_the_instance_it_covers() {
    let mut moved = instances();
    moved[1].stored.as_mut().unwrap().covers = NOW - 60;
    let view = view(&moved, civil(23), NOW);
    assert_eq!(view.instances[1].summary, None);
    assert_eq!(view.instances[1].written, "");
}

#[test]
fn every_instance_row_carries_its_projects() {
    let view = view(&instances(), civil(23), NOW);
    assert_eq!(view.instances[1].projects, ["dotfiles: 1 session, 2 events"]);
    assert_eq!(view.instances[0].chip, "In progress");
}

#[test]
fn the_summary_names_the_newest_complete_instance() {
    let view = view(&instances(), civil(23), NOW);
    assert_eq!(summarize(&view).text, "nightshift, ended 08:00");
    assert_eq!(summarize(&HistoryView { instances: Vec::new() }).text, "nothing recorded");
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib command_recap::history`
Expected: compile error, `file not found for module history`.

- [ ] **Step 3: Implement the builder, the renderer and the verb**

`pns/crates/pns/src/command_recap/history.rs`:

```rust
//! `pns recap history`: the latest two instances of each recap window, with
//! their counts and the summary kept for each, newest first.
//!
//! IT READS THE ACTIVITY STORE AND THE STORED SUMMARIES ONLY. A full recap
//! runs `[recap.sources]` commands and can run the summarizer; this spawns
//! nothing, which is what lets the site serve it.

use crate::style::Tone;
use crate::view::{Sources, Summary};
use pns_domain::recap::history::{chip, covers, day_label, project_line, tally_line};
use pns_domain::recap::window::{LocalCivilTime, Window};

pub(crate) const HISTORY_USAGE: &str = "\
pns: usage:
  pns recap history                the last two instances of each recap window
";

pub(crate) const UNREADABLE: &str =
    "pns: the activity store could not be opened, so there is no recap to give";

const WINDOWS: [Window; 4] = [Window::Nightshift, Window::Morning, Window::Afternoon, Window::Evening];

pub(crate) struct Instance {
    pub(crate) window: Window,
    pub(crate) since: u64,
    pub(crate) until: u64,
    pub(crate) day: LocalCivilTime,
    pub(crate) bounds: String,
    pub(crate) clock: String,
    pub(crate) ends: String,
    pub(crate) events: Vec<pns_domain::recap::activity::Event>,
    pub(crate) stored: Option<pns_adapters::StoredSummary>,
    pub(crate) written: String,
}

pub(crate) struct HistoryView {
    pub(crate) instances: Vec<InstanceRow>,
}

pub(crate) struct InstanceRow {
    pub(crate) title: String,
    pub(crate) day: String,
    pub(crate) clock: String,
    pub(crate) bounds: String,
    pub(crate) chip: &'static str,
    pub(crate) tally: String,
    pub(crate) summary: Option<String>,
    pub(crate) written: String,
    pub(crate) projects: Vec<String>,
    pub(crate) window: &'static str,
    pub(crate) ends: String,
}

pub(crate) fn view(instances: &[Instance], today: LocalCivilTime, now: u64) -> HistoryView {
    let mut rows: Vec<(u64, InstanceRow)> = instances
        .iter()
        .map(|instance| {
            let projects = pns_domain::recap::activity::by_project(&instance.events);
            let kept = instance
                .stored
                .as_ref()
                .filter(|stored| covers(stored.covers, instance.since, instance.until));
            let word = instance.window.as_str();
            let row = InstanceRow {
                title: capitalized(word),
                day: day_label(instance.day, today),
                clock: instance.clock.clone(),
                bounds: instance.bounds.clone(),
                chip: chip(instance.until, now),
                tally: tally_line(&projects),
                summary: kept.map(|stored| stored.text.clone()),
                written: kept.map(|_| instance.written.clone()).unwrap_or_default(),
                projects: projects.iter().map(project_line).collect(),
                window: word,
                ends: instance.ends.clone(),
            };
            (instance.since, row)
        })
        .collect();
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    HistoryView { instances: rows.into_iter().map(|(_, row)| row).collect() }
}

fn capitalized(word: &str) -> String {
    let mut letters = word.chars();
    letters
        .next()
        .map(|first| first.to_uppercase().chain(letters).collect())
        .unwrap_or_default()
}

pub(crate) fn render(view: &HistoryView) -> String {
    let mut out = "pns: the last two instances of each recap window\n".to_string();
    for row in &view.instances {
        out.push_str(&format!(
            "  {:<10}  {:<9}  {:<17}  {:<11}  {}\n",
            row.title, row.day, row.bounds, row.chip, row.tally
        ));
        if let Some(summary) = &row.summary {
            out.push_str(&format!("  {:<10}  {}: {summary}\n", "", row.written));
        }
    }
    out
}

pub(crate) fn summarize(view: &HistoryView) -> Summary {
    match view.instances.iter().find(|row| row.chip == "Complete") {
        Some(row) => Summary { text: format!("{}, ended {}", row.window, row.ends), tone: Tone::Quiet },
        None => Summary { text: "nothing recorded".into(), tone: Tone::Quiet },
    }
}

pub(crate) fn read(sources: &Sources, now: u64) -> Result<HistoryView, &'static str> {
    let recap = match pns_adapters::load_config(&pns_adapters::config_path(&sources.home)) {
        Ok(pns_adapters::LoadOutcome::Loaded(config)) => config.recap.clone(),
        _ => pns_adapters::Recap::default(),
    };
    let (today, _) = pns_adapters::local_civil(now).ok_or(UNREADABLE)?;
    let wall = |at: u64| {
        pns_application::recap_wall_clock(Some(at), pns_adapters::local_minutes_since_midnight)
    };
    let mut instances = Vec::new();
    for window in WINDOWS {
        let stored = sources.store.recap_summary(window.as_str()).map_err(|_| UNREADABLE)?;
        for previous in [false, true] {
            let resolved =
                super::window::named(window, previous, &recap, now).map_err(|_| UNREADABLE)?;
            let events = sources
                .store
                .activity_between(resolved.since, resolved.until)
                .map_err(|_| UNREADABLE)?;
            let (day, _) = pns_adapters::local_civil(resolved.since).ok_or(UNREADABLE)?;
            instances.push(Instance {
                window,
                since: resolved.since,
                until: resolved.until,
                day,
                bounds: format!("{}-{} local", wall(resolved.since), wall(resolved.until)),
                clock: wall(resolved.since),
                ends: wall(resolved.until),
                events,
                written: stored
                    .as_ref()
                    .map(|held| format!("written {} by {}", wall(held.at), held.source))
                    .unwrap_or_default(),
                stored: stored.clone(),
            });
        }
    }
    Ok(view(&instances, today, now))
}

pub(crate) fn summary(sources: &Sources, now: u64) -> Summary {
    match read(sources, now) {
        Ok(view) => summarize(&view),
        Err(_) => Summary { text: "unreadable".into(), tone: Tone::Quiet },
    }
}

pub(crate) fn history_mode() -> i32 {
    if !crate::arguments_after_verb().is_empty() {
        eprintln!("{HISTORY_USAGE}");
        return 2;
    }
    let Some(now) = pns_adapters::now_secs() else {
        eprintln!("pns recap history: this machine has no clock to place a window with");
        return 1;
    };
    match read(&Sources::live(), now) {
        Ok(view) => {
            print!("{}", render(&view));
            0
        }
        Err(sentence) => {
            eprintln!("{sentence}");
            1
        }
    }
}

#[cfg(test)]
mod tests;
```

In `command_recap.rs`, beside `AGENT` and `GIT`:

```rust
const HISTORY: &str = "history";
```

and the arm `Some(HISTORY) => history::history_mode(),` in `recap_mode`. In `subcommand_usage.rs`, add
`("recap history", crate::command_recap::HISTORY_USAGE)` after the `recap` row (re-export the constant
from `command_recap.rs`). In `legacy/usage.rs`, after `pns recap git`:

```
  pns recap history                the last two instances of each recap window
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib command_recap subcommand_usage`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): add pns recap history, each window's last two instances"
```

### Task 3.3: The Recap page, its route and its index row

**Files:**
- Create: `pns/crates/pns/src/site/recap.rs`
- Create: `pns/crates/pns/src/site/recap/tests.rs`
- Modify: `pns/crates/pns/src/site.rs` (`Target::Recap`, the arm)
- Modify: `pns/crates/pns/src/site/shell.rs` (`footer_noting`)
- Modify: `pns/crates/pns/src/site/index.rs` (the Recap row)
- Test: `pns/crates/pns/src/site/tests.rs`

**Interfaces:**
- Consumes: `command_recap::history::{read, summary, HistoryView, InstanceRow}`.
- Produces:
  - `pub(super) fn site::recap::page(view: Result<&HistoryView, &'static str>, rendered_at: u64) ->
    String`
  - `pub(super) fn shell::footer_noting(note: &str, rendered_at: u64) -> String`, which `footer` calls
    with `"All times UTC"`

- [ ] **Step 1: Write the failing tests**

`pns/crates/pns/src/site/recap/tests.rs`:

```rust
use super::*;
use crate::command_recap::history::{HistoryView, InstanceRow};

const NOW: u64 = 1_790_139_720;

fn row(title: &str, day: &str, chip: &'static str, summary: Option<&str>) -> InstanceRow {
    InstanceRow {
        title: title.into(),
        day: day.into(),
        clock: "08:00".into(),
        bounds: "08:00-12:00 local".into(),
        chip,
        tally: "42 events, 5 sessions in 3 projects".into(),
        summary: summary.map(str::to_string),
        written: if summary.is_some() { "written 12:01 by claude".into() } else { String::new() },
        projects: vec!["dotfiles: 3 sessions, 30 events".into()],
        window: "morning",
        ends: "12:00".into(),
    }
}

fn fixture() -> HistoryView {
    HistoryView {
        instances: vec![
            row("Afternoon", "Today", "In progress", None),
            row("Morning", "Today", "Complete", Some("The morning shipped <b>two</b> fixes.")),
            row("Evening", "Yesterday", "Complete", None),
        ],
    }
}

#[test]
fn every_value_the_builder_spelled_is_on_the_page() {
    let view = fixture();
    let served = page(Ok(&view), NOW);
    for row in &view.instances {
        for value in [&row.title, &row.day, &row.clock, &row.bounds, &row.tally, &row.written] {
            assert!(served.contains(&escaped(value)), "{value:?} is not on the page");
        }
        assert!(served.contains(row.chip));
        for project in &row.projects {
            assert!(served.contains(&escaped(project)));
        }
    }
}

#[test]
fn the_summary_is_escaped_and_an_instance_without_one_says_so() {
    let served = page(Ok(&fixture()), NOW);
    assert!(served.contains("The morning shipped &lt;b&gt;two&lt;/b&gt; fixes."), "{served}");
    assert!(served.contains("no summary kept"), "{served}");
}

#[test]
fn in_progress_is_amber_and_complete_is_neutral() {
    let served = page(Ok(&fixture()), NOW);
    assert!(served.contains("fh-row fh-active"), "{served}");
    assert!(served.contains("fh-row fh-quiet"), "{served}");
}

#[test]
fn the_footer_says_the_window_times_are_local() {
    let served = page(Ok(&fixture()), NOW);
    assert!(served.contains("Window times local"), "{served}");
    assert!(!served.contains("All times UTC"), "{served}");
}

#[test]
fn an_unreadable_store_is_the_terminal_sentence() {
    let served = page(Err(crate::command_recap::history::UNREADABLE), NOW);
    assert!(served.contains("the activity store could not be opened"), "{served}");
}
```

In `site/tests.rs`:

```rust
#[test]
fn the_recap_page_is_its_exact_path_only() {
    assert_eq!(target("GET /recap HTTP/1.1\r\n"), Some(Target::Recap));
    for line in ["GET /recap/ HTTP/1.1\r\n", "GET /recap?window=morning HTTP/1.1\r\n", "POST /recap HTTP/1.1\r\n"] {
        assert_eq!(target(line), None, "{line:?} was served");
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib site::`
Expected: compile errors, `no variant Recap` and `cannot find function page in module recap`.

- [ ] **Step 3: Implement the page**

`pns/crates/pns/src/site/recap.rs`:

```rust
//! The Recap page: `pns recap history`'s view, laid out as the listing card.

use super::shell::{escaped, footer_noting, page as shell_page};
use crate::command_recap::history::{HistoryView, InstanceRow};

pub(super) fn page(view: Result<&HistoryView, &'static str>, rendered_at: u64) -> String {
    let body = match view {
        Err(sentence) => format!("<p>{}</p>", escaped(sentence)),
        Ok(view) => entries(view),
    };
    shell_page(
        "pns recap",
        &format!(
            "<section id=\"failure-history\"><a class=\"fh-back\" href=\"/\">pns</a>\
             <header class=\"fh-header\"><h2>Recap</h2></header>\n{body}\n{}</section>",
            footer_noting("Window times local, updated time UTC", rendered_at)
        ),
    )
}

fn entries(view: &HistoryView) -> String {
    let mut out = String::new();
    let mut day = None;
    let last = view.instances.len().saturating_sub(1);
    for (index, row) in view.instances.iter().enumerate() {
        if day != Some(&row.day) {
            out.push_str(&format!("<div class=\"fh-day\"><strong>{}</strong></div>\n", escaped(&row.day)));
            day = Some(&row.day);
        }
        out.push_str(&entry(row, index == last));
    }
    out
}

fn entry(row: &InstanceRow, last: bool) -> String {
    let class = if row.chip == "In progress" { "fh-row fh-active" } else { "fh-row fh-quiet" };
    let last = if last { " fh-last" } else { "" };
    let summary = match &row.summary {
        Some(text) => format!(
            "<p class=\"fh-next\">{}</p><div class=\"fh-sub fh-small\">{}</div>",
            escaped(text),
            escaped(&row.written)
        ),
        None => "<div class=\"fh-next fh-muted\">no summary kept</div>".to_string(),
    };
    let projects: String = row
        .projects
        .iter()
        .map(|project| format!("<dd>{}</dd>", escaped(project)))
        .collect();
    format!(
        "<article class=\"{class}{last}\"><time class=\"fh-clock\">{clock}</time>\
         <div class=\"fh-content\"><span class=\"fh-dot\" aria-hidden=\"true\"></span>\
         <div class=\"fh-title\"><strong>{title}</strong><span class=\"fh-state\">{chip}</span></div>\
         <div class=\"fh-sub\">{bounds} · {tally}</div>{summary}\
         <details><summary>Details</summary><div class=\"fh-detail\"><dl>\
         <dt>Projects</dt>{projects}</dl></div></details></div></article>\n",
        clock = escaped(&row.clock),
        title = escaped(&row.title),
        chip = row.chip,
        bounds = escaped(&row.bounds),
        tally = escaped(&row.tally),
    )
}

#[cfg(test)]
mod tests;
```

In `shell.rs`, `footer(rendered_at)` becomes `footer_noting("All times UTC", rendered_at)`, and
`footer_noting` holds the footer's markup with `note` escaped in its left span. In `site.rs`,
`Target::Recap` on `/recap`, and the arm:

```rust
        Some(Target::Recap) => {
            let view = crate::command_recap::history::read(sources, now);
            ok(&recap::page(view.as_ref().map_err(|sentence| *sentence), now))
        }
```

In `index.rs`, the Recap row after Health:

```rust
        Row {
            href: "/recap",
            name: "Recap",
            blurb: "the latest recap for each window",
            summary: crate::command_recap::history::summary(sources, now),
        },
```

- [ ] **Step 4: Run the site tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib site::`
Expected: all pass.

- [ ] **Step 5: Run the gates and check file sizes**

```bash
just test-rust; echo "test-rust exit $?"
just lint-check; echo "lint-check exit $?"
just test-unit; echo "test-unit exit $?"
```

Expected: three zero exit codes; every touched `.rs` under 500 total lines.

- [ ] **Step 6: Commit**

```bash
git add pns/crates/pns/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): serve the Recap page, each window's last two instances"
```

---

## Pull request 4: Right now

Branch `feat/pns-page-now`. Worktree:
`herdr worktree create --cwd /Users/stephen/workspaces/Ivy/webdavis/dotfiles --branch feat/pns-page-now --no-focus`

This is the largest of the seven. Tasks 4.1 to 4.3 are extractions and one read-only lister, each
standing on its own; if the pull request proves too large, it splits cleanly after Task 4.3.

### Task 4.1: The Right now page's wording, in the domain

**Files:**
- Modify: `pns/crates/pns-domain/src/surface.rs` (`Surface::{word, headline, meaning}`)
- Modify: `pns/crates/pns/src/command_tap.rs` (its inline `match` becomes `reading.surface.word()`)
- Modify: `pns/crates/pns-domain/src/mute.rs` (`status`, which `status_line` prefixes)
- Modify: `pns/crates/pns-domain/src/stale.rs` (`pending_line`)
- Modify: `pns/crates/pns-domain/src/lamps/window.rs` (`dim_line`)
- Modify: `pns/crates/pns-domain/src/routing.rs` (`legs_line`, `PREVIEW_STATES`)
- Test: the tests module beside each

**Interfaces:**
- Produces:

```rust
impl Surface {
    pub fn word(self) -> &'static str;      // "desk", "mobile", "away" (today's tap spelling)
    pub fn headline(self) -> &'static str;  // "At the desk", "On the phone", "Away"
    pub fn meaning(self) -> &'static str;   // "Delivery treats you as at the desk."
}
pub fn pns_domain::mute::status(expiry: Option<u64>, now: Option<u64>) -> String; // "not muted"
pub fn pns_domain::stale::pending_line(blocked: &Blocked, due_clock: &str) -> String;
pub fn pns_domain::lamps::window::dim_line(text: Option<&str>, minutes_now: Option<u16>) -> String;
pub const pns_domain::routing::PREVIEW_STATES: [&str; 5];
pub fn pns_domain::routing::legs_line(legs: &[Leg], pulse: bool, route: &str) -> String;
```

- [ ] **Step 1: Write the failing tests**

In the tests module of `surface.rs`:

```rust
#[test]
fn each_surface_has_one_word_one_headline_and_one_meaning() {
    assert_eq!(Surface::Desk.word(), "desk");
    assert_eq!(Surface::Mobile.word(), "mobile");
    assert_eq!(Surface::Away.word(), "away");
    assert_eq!(Surface::Desk.headline(), "At the desk");
    assert_eq!(Surface::Mobile.headline(), "On the phone");
    assert_eq!(Surface::Away.headline(), "Away");
    assert_eq!(Surface::Desk.meaning(), "Delivery treats you as at the desk.");
    assert_eq!(Surface::Mobile.meaning(), "Delivery treats you as on the phone.");
    assert_eq!(Surface::Away.meaning(), "Delivery treats you as away.");
}
```

In the tests module of `mute.rs`:

```rust
#[test]
fn the_status_is_the_line_without_its_prefix() {
    assert_eq!(status(None, Some(10)), "not muted");
    assert_eq!(status(Some(10 + 120), Some(10)), "muted for another 2 minutes");
    assert_eq!(status_line(Some(10 + 120), Some(10)), "pns: muted for another 2 minutes");
}
```

In the tests module of `stale.rs`:

```rust
#[test]
fn a_pending_escalation_names_its_session_and_when_it_pages() {
    let blocked = Blocked {
        session: "s1".into(),
        harness: "claude".into(),
        project: "pns".into(),
        branch: "main".into(),
        title: "ship it".into(),
        since: 100,
    };
    assert_eq!(pending_line(&blocked, "05:41"), "claude in pns pages at 05:41 UTC");
}
```

In the tests module of `lamps/window.rs`:

```rust
#[test]
fn the_dim_line_says_the_window_and_whether_now_is_inside_it() {
    assert_eq!(dim_line(None, Some(600)), "none");
    assert_eq!(dim_line(Some("22:00-07:00"), Some(600)), "22:00-07:00 local, outside now");
    assert_eq!(dim_line(Some("22:00-07:00"), Some(23 * 60)), "22:00-07:00 local, inside now");
    assert_eq!(dim_line(Some("22:00-07:00"), None), "22:00-07:00 local, clock unreadable");
    assert_eq!(dim_line(Some("nonsense"), Some(600)), "nonsense local, not a window pns can read");
}
```

In the tests module of `routing.rs`:

```rust
fn leg(name: &'static str) -> Leg {
    Leg { name, mode: ReportMode::Silent, decorative: false }
}

#[test]
fn a_legs_line_names_each_destination_and_the_durable_route() {
    assert_eq!(
        legs_line(&[leg("banner"), leg("hermes")], true, "priority"),
        "banner, hermes (priority), lights"
    );
    assert_eq!(legs_line(&[leg("phone")], false, "pns-events"), "phone");
    assert_eq!(legs_line(&[], false, "pns-events"), "nothing");
}

#[test]
fn the_preview_states_are_the_five_the_page_shows() {
    assert_eq!(PREVIEW_STATES, ["done", "failed", "blocked", "asked", "observation"]);
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-domain surface mute stale lamps::window routing`
Expected: compile errors naming `headline`, `status`, `pending_line`, `dim_line` and `legs_line`.

- [ ] **Step 3: Implement the wording**

`surface.rs`:

```rust
impl Surface {
    pub fn word(self) -> &'static str {
        match self {
            Self::Desk => "desk",
            Self::Mobile => "mobile",
            Self::Away => "away",
        }
    }
    pub fn headline(self) -> &'static str {
        match self {
            Self::Desk => "At the desk",
            Self::Mobile => "On the phone",
            Self::Away => "Away",
        }
    }
    pub fn meaning(self) -> &'static str {
        match self {
            Self::Desk => "Delivery treats you as at the desk.",
            Self::Mobile => "Delivery treats you as on the phone.",
            Self::Away => "Delivery treats you as away.",
        }
    }
}
```

`command_tap.rs`'s three-arm `match` becomes `let surface = reading.surface.word();` (a pure move; the tap
command's own tests pin the word). `mute.rs`: the body of `status_line` moves into `status` without its
`pns: ` prefix, and `status_line` becomes `format!("pns: {}", status(expiry, now))`. `stale.rs`:

```rust
/// One wait the escalation has not paged about yet, and when it will.
pub fn pending_line(blocked: &Blocked, due_clock: &str) -> String {
    format!("{} in {} pages at {due_clock} UTC", blocked.harness, blocked.project)
}
```

`lamps/window.rs`:

```rust
/// The configured dim window as the Right now page says it.
pub fn dim_line(text: Option<&str>, minutes_now: Option<u16>) -> String {
    let Some(text) = text else {
        return "none".to_string();
    };
    let Some(window) = parse_window(text) else {
        return format!("{text} local, not a window pns can read");
    };
    match minutes_now {
        None => format!("{text} local, clock unreadable"),
        Some(_) if quiet_now(Some(&window), minutes_now) => format!("{text} local, inside now"),
        Some(_) => format!("{text} local, outside now"),
    }
}
```

`routing.rs`:

```rust
/// The states the Right now page previews, in the order it lists them.
pub const PREVIEW_STATES: [&str; 5] = ["done", "failed", "blocked", "asked", "observation"];

/// Where one decision delivers, as one line: each leg by name, the durable
/// legs with the route they post to, and the lamps when the plan pulses.
pub fn legs_line(legs: &[Leg], pulse: bool, route: &str) -> String {
    let mut named: Vec<String> = legs
        .iter()
        .map(|leg| match leg.name {
            "hermes" | "discord" => format!("{} ({route})", leg.name),
            name => name.to_string(),
        })
        .collect();
    if pulse {
        named.push("lights".to_string());
    }
    if named.is_empty() { "nothing".to_string() } else { named.join(", ") }
}
```

- [ ] **Step 4: Run the domain and pns suites**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-domain` and
`cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib command_tap`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-domain/src pns/crates/pns/src/command_tap.rs
SKIP_AI_COMMIT=1 git commit -m "feat(pns): word the surface, the mute, the dim window and a leg plan once"
```

### Task 4.2: One override assembly for the event path and `pns now`

**Files:**
- Create: `pns/crates/pns/src/live_overrides.rs`
- Create: `pns/crates/pns/src/live_overrides/tests.rs`
- Modify: `pns/crates/pns/src/event_flow/execution.rs` (calls it)
- Modify: `pns/crates/pns/src/command_mute.rs` (`muted_now_in(store, now)`, which `muted_now` calls)
- Modify: `pns/crates/pns/src/lib.rs` (`mod live_overrides;`)

**Interfaces:**
- Produces:

```rust
pub(crate) fn command_mute::muted_now_in(store: &SqliteStore, now_secs: Option<u64>) -> bool;
pub(crate) fn live_overrides::live_overrides(
    store: &SqliteStore,
    home: &str,
    focus_silence: &[String],
    now_secs: Option<u64>,
) -> pns_domain::Overrides;
```

This is a move of the block in `execution.rs` that builds `Overrides { muted, focus_active,
..overrides_from_env() }`, with the store and home made parameters. It owes a test pinning the mute input
before the move.

- [ ] **Step 1: Write the pinning test**

`pns/crates/pns/src/live_overrides/tests.rs`:

```rust
use super::*;

fn sandbox() -> pns_adapters::SqliteStore {
    let state = std::env::temp_dir().join(format!(
        "pns-live-overrides-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos())
    ));
    pns_adapters::SqliteStore::new(state)
}

#[test]
fn a_standing_mute_is_an_input_and_an_expired_one_is_not() {
    let store = sandbox();
    store.set_mute_expiry(Some(1_000)).unwrap();
    assert!(live_overrides(&store, "/nonexistent-home", &[], Some(999)).muted);
    assert!(!live_overrides(&store, "/nonexistent-home", &[], Some(1_000)).muted);
}

#[test]
fn no_focus_list_is_never_focus_active() {
    assert!(!live_overrides(&sandbox(), "/nonexistent-home", &[], Some(1)).focus_active);
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib live_overrides`
Expected: compile error, `file not found for module live_overrides`.

- [ ] **Step 3: Move the block**

`pns/crates/pns/src/live_overrides.rs`:

```rust
//! The overrides every live decision is taken with: the operator's mute, a
//! named macOS Focus, and the environment's own switches.
//!
//! ONE ASSEMBLY for the event path and `pns now`, so the preview on the Right
//! now page is the decision an event would get.

use pns_adapters::SqliteStore;

pub(crate) fn live_overrides(
    store: &SqliteStore,
    home: &str,
    focus_silence: &[String],
    now_secs: Option<u64>,
) -> pns_domain::Overrides {
    pns_domain::Overrides {
        muted: crate::command_mute::muted_now_in(store, now_secs),
        focus_active: pns_adapters::focus_now(home, focus_silence)
            .is_ok_and(|reading| reading.silenced),
        ..crate::overrides_from_env()
    }
}

#[cfg(test)]
mod tests;
```

The two comment paragraphs above the block in `execution.rs` (THE MUTE IS AN INPUT, THE OPERATING
SYSTEM'S MUTE) move with it onto `live_overrides`. `execution.rs` becomes:

```rust
    let overrides = crate::live_overrides::live_overrides(
        &pns_adapters::SqliteStore::for_records(state_dir()),
        &home,
        &focus_silence,
        now_secs,
    );
```

In `command_mute.rs`:

```rust
pub(crate) fn muted_now(now_secs: Option<u64>) -> bool {
    muted_now_in(&SqliteStore::for_records(state_dir()), now_secs)
}

pub(crate) fn muted_now_in(store: &SqliteStore, now_secs: Option<u64>) -> bool {
    pns_domain::mute::is_muted(read_mute_expiry(store), now_secs)
}
```

- [ ] **Step 4: Run the tests, the event flow's included**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib live_overrides event_flow command_mute`
Expected: all pass, the event flow's unchanged.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns/src
SKIP_AI_COMMIT=1 git commit -m "refactor(pns): assemble a live decision's overrides in one place"
```

### Task 4.3: The loop lamp's leases, listed without sweeping

**Files:**
- Create: `pns/crates/pns-adapters/src/protocols/markers/leases.rs`
- Create: `pns/crates/pns-adapters/src/protocols/markers/leases/tests.rs`
- Modify: `pns/crates/pns-adapters/src/protocols/markers/mod.rs` (`mod leases; pub use
  leases::live_leases;`) and `pns-adapters/src/lib.rs` (re-export)

**Interfaces:**
- Consumes: `marker_files::lease_dir`, `read::read_epoch`, `pns_domain::lights::working_owner`,
  `pns_domain::lights::held::marker_is_live`.
- Produces: `pub fn live_leases(state: &Path, now: u64, timeout_secs: u64) -> Vec<(String, u64)>`, each
  live lease's pane and epoch, sorted by pane.

- [ ] **Step 1: Write the failing tests**

`leases/tests.rs`:

```rust
use super::*;

fn state() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "pns-leases-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos())
    ));
    std::fs::create_dir_all(crate::marker_files::lease_dir(&root)).unwrap();
    root
}

fn lease(root: &Path, pane: &str, epoch: u64) {
    std::fs::write(crate::marker_files::lease_dir(root).join(pane), epoch.to_string()).unwrap();
}

#[test]
fn a_live_lease_is_listed_with_its_pane() {
    let root = state();
    lease(&root, "wW:p21", 1_000);
    assert_eq!(live_leases(&root, 1_100, 3_600), vec![("wW:p21".to_string(), 1_000)]);
}

#[test]
fn an_expired_lease_is_not_listed_and_is_left_on_disk() {
    let root = state();
    lease(&root, "wW:p9", 1_000);
    assert_eq!(live_leases(&root, 1_000 + 3_601, 3_600), vec![]);
    assert!(crate::marker_files::lease_dir(&root).join("wW:p9").exists(), "the lister swept");
}

#[test]
fn a_working_file_is_not_a_lease() {
    let root = state();
    lease(&root, "wW:p3.new.4321", 1_000);
    assert_eq!(live_leases(&root, 1_100, 3_600), vec![]);
}

#[test]
fn no_directory_is_no_leases() {
    assert_eq!(live_leases(Path::new("/nonexistent-state"), 1, 1), vec![]);
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters leases`
Expected: compile error, `file not found for module leases`.

- [ ] **Step 3: Implement the lister**

`leases.rs`:

```rust
//! The loop lamp's leases, read and never touched.
//!
//! THE SWEEP BELONGS TO THE TICK (`sweep_leases`), which renames expired
//! leases away. This lists what the sweep would keep, by the sweep's own
//! liveness rule, and moves nothing, so a page can read it.

use std::path::Path;

pub fn live_leases(state: &Path, now: u64, timeout_secs: u64) -> Vec<(String, u64)> {
    let mut live: Vec<(String, u64)> = std::fs::read_dir(crate::marker_files::lease_dir(state))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if pns_domain::lights::working_owner(&name).is_some() {
                return None;
            }
            let at = super::read_epoch(&entry.path())?;
            pns_domain::lights::held::marker_is_live(at, now, timeout_secs).then_some((name, at))
        })
        .collect();
    live.sort();
    live
}

#[cfg(test)]
mod tests;
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters leases`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-adapters/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): list the loop lamp's live leases without sweeping them"
```

### Task 4.4: `pns now`

**Files:**
- Create: `pns/crates/pns/src/command_now.rs`
- Create: `pns/crates/pns/src/command_now/preview.rs`
- Create: `pns/crates/pns/src/command_now/tests.rs`
- Modify: `pns/crates/pns/src/lib.rs`, `invocation.rs`, `subcommand_usage.rs`, `legacy/usage.rs`

**Interfaces:**
- Consumes: Task 4.1's wording, `live_overrides`, `live_leases`, `profile_runtime::active`,
  `profiles::{because, surfaces_line}`, `pns_application::{operator_surface_reading, decide,
  doctor_focus, select_plugins}`, `doctor_home::read_rows`, `doctor_style::unattributed`,
  `lights::mute::muted_report`, `SqliteStore::{mute_expiry, read_muted, stale_blocks}`,
  `routes::route_for`, `view::{Sources, Summary}`.
- Produces:

```rust
pub(crate) const NOW_USAGE: &str;
pub(crate) const HOME_NOT_ROUTED: &str = "not used for delivery yet";
pub(crate) struct NowRow { pub(crate) label: &'static str, pub(crate) lines: Vec<String> }
pub(crate) struct NowInputs {
    pub(crate) reading: pns_domain::SurfaceReading,
    pub(crate) overrides: pns_domain::Overrides,
    pub(crate) rows: Vec<NowRow>,                      // the labelled rows, already worded
    pub(crate) preview: Vec<(&'static str, String)>,   // state, legs_line
}
pub(crate) struct NowView {
    pub(crate) headline: &'static str, pub(crate) lock: &'static str, pub(crate) lock_tone: Tone,
    pub(crate) meaning: &'static str,
    pub(crate) desk: String, pub(crate) desk_meta: String,
    pub(crate) phone: String, pub(crate) phone_meta: String,
    pub(crate) rows: Vec<NowRow>,
    pub(crate) preview: Vec<(&'static str, String)>,
    pub(crate) technical: Vec<(&'static str, String)>,
}
pub(crate) fn view(inputs: NowInputs) -> NowView;
pub(crate) fn read(sources: &Sources, now: u64, probe_router: bool) -> NowView;
pub(crate) fn render(view: &NowView) -> String;
pub(crate) fn summary(sources: &Sources, now: u64) -> Summary;
pub(crate) fn now_mode() -> i32;
// preview.rs
pub(crate) fn preview(
    decide: impl Fn(&str) -> (Vec<pns_domain::routing::Leg>, bool),
    class_route: Option<&str>,
    default_route: &str,
) -> Vec<(&'static str, String)>;
```

`decide` answers one state's legs and whether its plan pulses, which is all of a `Decision` the preview
reads.

- [ ] **Step 1: Write the failing tests**

`pns/crates/pns/src/command_now/tests.rs`:

```rust
use super::*;
use pns_domain::SurfaceReading;
use pns_domain::surface::Surface;

fn reading(locked: Option<bool>) -> SurfaceReading {
    SurfaceReading {
        surface: Surface::Desk,
        phone_input_fresh: false,
        desk_input_age: Some(8),
        phone_input_age: None,
        marker_age: Some(2_460),
        screen_locked: locked,
        desk_fresh_secs: Some(120),
    }
}

fn inputs(locked: Option<bool>) -> NowInputs {
    NowInputs {
        reading: reading(locked),
        overrides: pns_domain::Overrides::default(),
        rows: vec![
            NowRow { label: "profile", lines: vec!["work (rule 2: days, hours)".into(),
                "quiet on; banner all, Discord priority, phone priority, lights none".into()] },
            NowRow { label: "mute", lines: vec!["not muted".into()] },
            NowRow { label: "escalations", lines: vec!["claude in pns pages at 05:41 UTC".into()] },
        ],
        preview: vec![("done", "banner, hermes (pns-events)".into()), ("observation", "banner".into())],
    }
}

#[test]
fn the_terminal_form_is_pinned() {
    assert_eq!(
        render(&view(inputs(Some(false)))),
        "pns: At the desk, unlocked\n\
         \x20 Delivery treats you as at the desk.\n\
         \x20 desk input     8s ago, fresh within 2m\n\
         \x20 phone input    none, Back Tap 41m ago\n\
         \x20 profile        work (rule 2: days, hours)\n\
         \x20                quiet on; banner all, Discord priority, phone priority, lights none\n\
         \x20 mute           not muted\n\
         \x20 escalations    claude in pns pages at 05:41 UTC\n\
         \x20 would deliver\n\
         \x20   done         banner, hermes (pns-events)\n\
         \x20   observation  banner\n\
         \x20 for an event with no pane; a pane on screen also suppresses its own banner\n"
    );
}

#[test]
fn the_lock_chip_reads_the_lock() {
    assert_eq!(view(inputs(Some(true))).lock, "Locked");
    assert_eq!(view(inputs(Some(true))).lock_tone, Tone::Warn);
    assert_eq!(view(inputs(Some(false))).lock, "Unlocked");
    assert_eq!(view(inputs(None)).lock, "Lock unknown");
}

#[test]
fn the_technical_rows_carry_the_raw_inputs() {
    let technical = view(inputs(None)).technical;
    assert!(technical.contains(&("desk idle", "8 s".to_string())));
    assert!(technical.contains(&("Back Tap marker age", "2460 s".to_string())));
    assert!(technical.contains(&("visibility", "unknown (no pane)".to_string())));
}

#[test]
fn the_preview_resolves_each_states_route() {
    let decide = |state: &str| {
        let hermes = pns_domain::routing::Leg {
            name: "hermes",
            mode: pns_domain::routing::ReportMode::ReportOutcome,
            decorative: false,
        };
        (vec![hermes], state == "failed")
    };
    let rows = preview::preview(decide, Some("priority"), "pns-events");
    assert_eq!(rows[0], ("done", "hermes (pns-events)".to_string()));
    assert_eq!(rows[1], ("failed", "hermes (priority), lights".to_string()));
    assert_eq!(rows.len(), 5);
}

#[test]
fn the_summary_is_the_surface_and_the_profile() {
    let mut given = inputs(Some(false));
    given.rows.retain(|row| row.label == "profile");
    assert_eq!(summarize(&view(given)).text, "desk, work");
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib command_now`
Expected: compile error, `file not found for module command_now`.

- [ ] **Step 3: Implement `preview.rs`**

```rust
//! Where a notification of each state would go at this moment: the real
//! delivery decision, run once per state.

use pns_domain::routing::{PREVIEW_STATES, legs_line};

pub(crate) fn preview(
    decide: impl Fn(&str) -> (Vec<pns_domain::routing::Leg>, bool),
    class_route: Option<&str>,
    default_route: &str,
) -> Vec<(&'static str, String)> {
    PREVIEW_STATES
        .iter()
        .map(|state| {
            let (legs, pulse) = decide(state);
            let route = pns_domain::routes::route_for(class_route, state).unwrap_or(default_route);
            (*state, legs_line(&legs, pulse, route))
        })
        .collect()
}
```

- [ ] **Step 4: Implement `command_now.rs`**

```rust
//! `pns now`: what pns believes at this second, and why.
//!
//! THE VALUE BUILDER FOR TWO SURFACES: the terminal prints `render`, and the
//! site's `/now` page lays out the same `NowView`.

mod preview;

use crate::style::Tone;
use crate::view::{Sources, Summary};

pub(crate) const NOW_USAGE: &str = "\
pns: usage:
  pns now                          what pns believes now, and where each state would go
";

pub(crate) const HOME_NOT_ROUTED: &str = "not used for delivery yet";
const NO_PANE: &str = "for an event with no pane; a pane on screen also suppresses its own banner";

pub(crate) struct NowRow {
    pub(crate) label: &'static str,
    pub(crate) lines: Vec<String>,
}

pub(crate) struct NowInputs {
    pub(crate) reading: pns_domain::SurfaceReading,
    pub(crate) overrides: pns_domain::Overrides,
    pub(crate) rows: Vec<NowRow>,
    pub(crate) preview: Vec<(&'static str, String)>,
}

pub(crate) struct NowView {
    pub(crate) headline: &'static str,
    pub(crate) lock: &'static str,
    pub(crate) lock_tone: Tone,
    pub(crate) meaning: &'static str,
    pub(crate) desk: String,
    pub(crate) desk_meta: String,
    pub(crate) phone: String,
    pub(crate) phone_meta: String,
    pub(crate) rows: Vec<NowRow>,
    pub(crate) preview: Vec<(&'static str, String)>,
    pub(crate) technical: Vec<(&'static str, String)>,
    pub(crate) surface_word: &'static str,
}

fn age(seconds: Option<u64>, absent: &str) -> String {
    seconds.map_or_else(|| absent.to_string(), pns_domain::doctor::ago)
}

fn seconds(value: Option<u64>) -> String {
    value.map_or_else(|| "none".to_string(), |secs| format!("{secs} s"))
}

pub(crate) fn view(inputs: NowInputs) -> NowView {
    let reading = inputs.reading;
    let (lock, lock_tone) = match reading.screen_locked {
        Some(true) => ("Locked", Tone::Warn),
        Some(false) => ("Unlocked", Tone::Quiet),
        None => ("Lock unknown", Tone::Quiet),
    };
    let window = reading.desk_fresh_secs.unwrap_or(pns_domain::DEFAULT_DESK_IDLE_SECS);
    let spelled = pns_domain::duration::spelled(std::time::Duration::from_secs(window));
    let overrides = &inputs.overrides;
    NowView {
        headline: reading.surface.headline(),
        lock,
        lock_tone,
        meaning: reading.surface.meaning(),
        desk: age(reading.desk_input_age, "unknown"),
        desk_meta: format!("fresh within {spelled}"),
        phone: age(reading.phone_input_age, "none"),
        phone_meta: reading
            .marker_age
            .map_or_else(|| "no Back Tap".to_string(), |at| format!("Back Tap {}", pns_domain::doctor::ago(at))),
        rows: inputs.rows,
        preview: inputs.preview,
        technical: vec![
            ("desk idle", seconds(reading.desk_input_age)),
            ("phone input age", seconds(reading.phone_input_age)),
            ("Back Tap marker age", seconds(reading.marker_age)),
            ("screen locked", reading.screen_locked.map_or("unknown", |locked| if locked { "yes" } else { "no" }).to_string()),
            ("freshness window", spelled),
            ("muted", if overrides.muted { "yes" } else { "no" }.to_string()),
            ("focus active", if overrides.focus_active { "yes" } else { "no" }.to_string()),
            ("visibility", "unknown (no pane)".to_string()),
        ],
        surface_word: reading.surface.word(),
    }
}

pub(crate) fn render(view: &NowView) -> String {
    let mut out = format!("pns: {}, {}\n", view.headline, view.lock.to_lowercase());
    out.push_str(&format!("  {}\n", view.meaning));
    out.push_str(&format!("  {:<13}  {}, {}\n", "desk input", view.desk, view.desk_meta));
    out.push_str(&format!("  {:<13}  {}, {}\n", "phone input", view.phone, view.phone_meta));
    for row in &view.rows {
        for (index, line) in row.lines.iter().enumerate() {
            let label = if index == 0 { row.label } else { "" };
            out.push_str(&format!("  {label:<13}  {line}\n"));
        }
    }
    out.push_str("  would deliver\n");
    for (state, line) in &view.preview {
        out.push_str(&format!("    {state:<11}  {line}\n"));
    }
    out.push_str(&format!("  {NO_PANE}\n"));
    out
}

pub(crate) fn summarize(view: &NowView) -> Summary {
    let profile = view
        .rows
        .iter()
        .find(|row| row.label == "profile")
        .and_then(|row| row.lines.first())
        .and_then(|line| line.split(' ').next())
        .unwrap_or("default");
    Summary { text: format!("{}, {profile}", view.surface_word), tone: Tone::Quiet }
}
```

`read(sources, now, probe_router)` gathers the inputs: one `system_probes()` set; the reading from
`pns_application::operator_surface_reading(&probes, &crate::overrides_from_env(), Some(now))`; the config
from `load_config(&config_path(&sources.home))`; then the rows in this order, each worded by the function
the spec's Page 4 table names:

```rust
    let mut rows = Vec::new();
    if probe_router {
        let mut home: Vec<String> = crate::doctor_home::read_rows(&sources.home)
            .iter()
            .map(|item| crate::doctor_style::unattributed(item.text()).to_string())
            .collect();
        home.push(HOME_NOT_ROUTED.to_string());
        rows.push(NowRow { label: "home network", lines: home });
    }
    if let Some(config) = config.as_ref() {
        let active = crate::profile_runtime::active(&sources.store, config);
        rows.push(NowRow {
            label: "profile",
            lines: vec![
                format!(
                    "{} ({})",
                    active.resolved.profile,
                    pns_domain::profiles::because(
                        &active.resolved.chose,
                        &active.resolved.matched,
                        active.until_clock.as_deref()
                    )
                ),
                pns_domain::profiles::surfaces_line(&active.profile),
            ],
        });
    }
    let expiry = sources.store.mute_expiry().ok().flatten();
    rows.push(NowRow { label: "mute", lines: vec![pns_domain::mute::status(expiry, Some(now))] });
    let (focus_enabled, focus_modes) = config
        .as_ref()
        .map_or((false, Vec::new()), |config| (config.focus_enabled, config.focus_modes.clone()));
    let focus = pns_application::doctor_focus(focus_enabled, !focus_modes.is_empty(), || {
        pns_adapters::focus_now(&sources.home, &focus_modes).map_err(|error| error.kind())
    });
    rows.push(NowRow { label: "focus", lines: vec![crate::doctor_style::unattributed(&focus).to_string()] });
    let lights = config.as_ref().and_then(|config| config.lights.as_deref());
    rows.push(NowRow {
        label: "lamp dimming",
        lines: vec![pns_domain::lamps::window::dim_line(
            lights.and_then(|lights| lights.dim_window.as_deref()),
            pns_adapters::local_minutes_since_midnight(now),
        )],
    });
    let muted = sources.store.read_muted().unwrap_or_default();
    rows.push(NowRow { label: "lamp mutes", lines: pns_domain::lights::mute::muted_report(&muted, Some(now)) });
    if let Some(lights) = lights {
        let leases = pns_adapters::live_leases(&sources.state, now, lights.looping.lease_expiry_secs);
        let lines = if leases.is_empty() {
            vec!["not held".to_string()]
        } else {
            leases
                .iter()
                .map(|(pane, at)| format!("held by {pane} for {}", pns_domain::remind::waited(now.saturating_sub(*at))))
                .collect()
        };
        rows.push(NowRow { label: "loop lamp", lines });
    }
    let window = sources.stale_window();
    let pending = if window == 0 {
        vec!["off".to_string()]
    } else {
        sources
            .store
            .stale_blocks(now)
            .unwrap_or_default()
            .iter()
            .map(|blocked| {
                let due = pns_domain::stale::escalates_at(blocked.since, window).unwrap_or(blocked.since);
                pns_domain::stale::pending_line(blocked, &pns_adapters::utc_clock(due))
            })
            .collect()
    };
    rows.push(NowRow { label: "escalations", lines: if pending.is_empty() { vec!["none pending".into()] } else { pending } });
```

The preview reuses the event path's own pieces: `live_overrides(&sources.store, &sources.home,
&focus_silence, Some(now))`, `select_plugins(&roster(), loaded)`, the default class's
`silence_policy("default")` and route, and `[routes] default`:

```rust
    let decide = |state: &str| {
        let decision = pns_application::decide(
            &probes,
            &selection,
            &overrides,
            pns_domain::DecisionRequest {
                observation: state == "observation",
                scope: pns_domain::DeliveryScope::Automatic,
                pane: "",
                now_secs: Some(now),
                long_running: false,
                mobile_watch_card: mobile.watch_card,
                silence_policy,
            },
        );
        (decision.legs, decision.plan.pulse)
    };
    let preview = preview::preview(decide, class_route.as_deref(), routes.default_route());
    view(NowInputs { reading, overrides, rows, preview })
```

`summary(sources, now)` calls `read(sources, now, false)` and `summarize`, so the index never probes the
router. `now_mode` refuses any argument (exit 2 with `NOW_USAGE`), reads the clock (exit 1 without one),
prints `render(&read(&Sources::live(), now, true))`, and exits 0. Register `now` in `invocation.rs`,
`SUBCOMMAND_USAGE` and `USAGE` (`  pns now                          what pns believes now`) the way Task
1.4 registered `waiting`.

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib command_now subcommand_usage`
Expected: all pass.

- [ ] **Step 6: Check the file size and commit**

`command_now.rs` holds the view and the renderer; if the file-size command puts it past 400 total lines
with `read`, move `read` into `command_now/read.rs` before committing.

```bash
git add pns/crates/pns/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): add pns now, what pns believes and where each state would go"
```

### Task 4.5: The Right now page, its route and its index row

**Files:**
- Create: `pns/crates/pns/src/site/now.rs`
- Create: `pns/crates/pns/src/site/now/tests.rs`
- Modify: `pns/crates/pns/src/site.rs`, `site/index.rs`, `site/tests.rs`

**Interfaces:**
- Consumes: `command_now::{read, summary, NowView}`.
- Produces: `pub(super) fn site::now::page(view: &NowView, rendered_at: u64) -> String`.

- [ ] **Step 1: Write the failing tests**

`pns/crates/pns/src/site/now/tests.rs`:

```rust
use super::*;
use crate::command_now::{NowRow, NowView};
use crate::style::Tone;

const NOW: u64 = 1_790_139_720;

fn fixture(profile: &str) -> NowView {
    NowView {
        headline: "At the desk",
        lock: "Unlocked",
        lock_tone: Tone::Quiet,
        meaning: "Delivery treats you as at the desk.",
        desk: "8s ago".into(),
        desk_meta: "fresh within 2m".into(),
        phone: "none".into(),
        phone_meta: "Back Tap 41m ago".into(),
        rows: vec![
            NowRow { label: "profile", lines: vec![profile.into()] },
            NowRow { label: "home network", lines: vec!["home: on the home network".into(), "not used for delivery yet".into()] },
        ],
        preview: vec![("done", "banner, hermes (pns-events)".into()), ("blocked", "banner, hermes (pns-events), lights".into())],
        technical: vec![("visibility", "unknown (no pane)".into())],
        surface_word: "desk",
    }
}

#[test]
fn every_value_the_builder_spelled_is_on_the_page() {
    let view = fixture("work (rule 2: days, hours)");
    let served = page(&view, NOW);
    for value in [view.headline, view.lock, view.meaning] {
        assert!(served.contains(value), "{value:?}");
    }
    for value in [&view.desk, &view.desk_meta, &view.phone, &view.phone_meta] {
        assert!(served.contains(&escaped(value)), "{value:?}");
    }
    for row in &view.rows {
        for line in &row.lines {
            assert!(served.contains(&escaped(line)), "{line:?}");
        }
    }
    for (state, line) in &view.preview {
        assert!(served.contains(state) && served.contains(&escaped(line)), "{state}");
    }
    assert!(served.contains("unknown (no pane)"));
}

#[test]
fn producer_text_in_a_row_is_escaped_once() {
    let served = page(&fixture("<i>work</i> & more"), NOW);
    assert!(served.contains("&lt;i&gt;work&lt;/i&gt; &amp; more"), "{served}");
}

#[test]
fn a_locked_desk_wears_the_amber_chip() {
    let mut view = fixture("work");
    view.lock = "Locked";
    view.lock_tone = Tone::Warn;
    let served = page(&view, NOW);
    assert!(served.contains("<span class=\"fr-status\">Locked</span>"), "{served}");
}
```

In `site/tests.rs`, the route test for `/now` in the same shape as the others (`/now/`, `/now?x=1`,
`POST /now` refused).

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib site::`
Expected: compile errors, `no variant Now` and `cannot find function page in module now`.

- [ ] **Step 3: Implement the page**

`pns/crates/pns/src/site/now.rs`:

```rust
//! The Right now page: `pns now`'s view, laid out as the record card.

use super::shell::{escaped, footer, page as shell_page};
use crate::command_now::NowView;
use crate::style::Tone;

pub(super) fn page(view: &NowView, rendered_at: u64) -> String {
    let chip_class = match view.lock_tone {
        Tone::Warn => "fr-status",
        Tone::Bad => "fr-status fr-red",
        Tone::Good | Tone::Quiet => "fr-status fr-quiet",
    };
    let rows: String = view
        .rows
        .iter()
        .map(|row| {
            let mut lines = row.lines.iter();
            let first = lines.next().map(|line| escaped(line)).unwrap_or_default();
            let rest: String = lines.map(|line| format!("<span class=\"fr-note\">{}</span>", escaped(line))).collect();
            format!("<dt>{}</dt><dd>{first}{rest}</dd>", escaped(&capitalized(row.label)))
        })
        .collect();
    let preview: String = view
        .preview
        .iter()
        .map(|(state, line)| format!("<dt>{state}</dt><dd>{}</dd>", escaped(line)))
        .collect();
    let technical: String = view
        .technical
        .iter()
        .map(|(label, value)| format!("<dt>{}</dt><dd>{}</dd>", escaped(&capitalized(label)), escaped(value)))
        .collect();
    let card = format!(
        "<article id=\"failure-record\"><a class=\"fr-back\" href=\"/\">pns</a>\
         <header><div class=\"fr-heading\"><h2>{headline}</h2><span class=\"{chip_class}\">{lock}</span></div>\
         <p class=\"fr-meaning\">{meaning}</p></header>\
         <dl class=\"fr-timing\"><div><dt>Desk input</dt><dd><span class=\"fr-value\">{desk}</span>\
         <span class=\"fr-meta\">{desk_meta}</span></dd></div><div><dt>Phone input</dt><dd>\
         <span class=\"fr-value\">{phone}</span><span class=\"fr-meta\">{phone_meta}</span></dd></div></dl>\
         <dl class=\"fr-fix\">{rows}</dl>\
         <details open><summary>Would deliver now</summary><dl class=\"fr-fields\">{preview}</dl>\
         <p class=\"fr-meaning\">For an event with no pane. A pane on screen also suppresses its own banner.</p></details>\
         <details><summary>Technical details</summary><dl class=\"fr-fields\">{technical}</dl></details>\n{}</article>",
        footer(rendered_at),
        headline = view.headline,
        lock = view.lock,
        meaning = view.meaning,
        desk = escaped(&view.desk),
        desk_meta = escaped(&view.desk_meta),
        phone = escaped(&view.phone),
        phone_meta = escaped(&view.phone_meta),
    );
    shell_page("pns now", &card)
}

fn capitalized(word: &str) -> String {
    let mut letters = word.chars();
    letters.next().map(|first| first.to_uppercase().chain(letters).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests;
```

In `site.rs`, `Target::Now` on `/now`, arm `Some(Target::Now) => ok(&now::page(&crate::command_now::read(sources,
now, true), now)),` (the variable `now` is the request's clock second and shadows nothing; rename the
module import to `now_page` if the compiler objects). In `index.rs`, the Right now row after Failures:

```rust
        Row {
            href: "/now",
            name: "Right now",
            blurb: "what pns believes and where a notification would go",
            summary: crate::command_now::summary(sources, now),
        },
```

Add `.fr-note` to the CSS constant in `shell.rs`:

```css
#failure-record .fr-note { color:var(--fr-muted); font-size:12px; display:block; margin-top:3px; }
```

- [ ] **Step 4: Run the site tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib site::`
Expected: all pass.

- [ ] **Step 5: Run the gates and check file sizes**

```bash
just test-rust; echo "test-rust exit $?"
just lint-check; echo "lint-check exit $?"
just test-unit; echo "test-unit exit $?"
```

Expected: three zero exit codes; every touched `.rs` under 500 total lines.

- [ ] **Step 6: Commit**

```bash
git add pns/crates/pns/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): serve the Right now page"
```

---

## Pull request 5: Sessions

Branch `feat/pns-page-sessions`. Worktree:
`herdr worktree create --cwd /Users/stephen/workspaces/Ivy/webdavis/dotfiles --branch feat/pns-page-sessions --no-focus`

### Task 5.1: herdr's live agent panes

**Files:**
- Create: `pns/crates/pns-adapters/src/herdr/panes.rs`
- Create: `pns/crates/pns-adapters/src/herdr/panes/tests.rs`
- Modify: `pns/crates/pns-adapters/src/herdr/mod.rs` (`mod panes; pub use panes::{AgentPane,
  parse_agent_panes};`) and `pns-adapters/src/lib.rs` (re-export)

**Interfaces:**
- Produces:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentPane {
    pub pane: String,           // pane_id
    pub workspace: String,      // workspace_id
    pub agent: String,          // agent
    pub session: String,        // agent_session.value
    pub status: String,         // agent_status
    pub terminal_title: String, // terminal_title_stripped
}
pub fn parse_agent_panes(pane_list_json: &str) -> Option<Vec<AgentPane>>;
```

- [ ] **Step 1: Write the failing tests**

`panes/tests.rs`, over the shape `herdr pane list` answers:

```rust
use super::*;

const LIST: &str = r#"{"id":"cli:pane:list","result":{"panes":[
  {"agent":"claude","agent_session":{"agent":"claude","kind":"id","source":"herdr:claude","value":"s-claude"},
   "agent_status":"blocked","pane_id":"wW:p21","workspace_id":"wW","terminal_title_stripped":"ship the plan | dotfiles"},
  {"agent":"codex","agent_session":{"value":"s-codex"},"agent_status":"working","pane_id":"wW:pDB",
   "workspace_id":"wW","terminal_title_stripped":"Check VPT"},
  {"agent_status":"unknown","pane_id":"wW:p1","workspace_id":"wW"},
  {"agent":"claude","agent_session":{"value":""},"agent_status":"idle","pane_id":"wW:p2","workspace_id":"wW"}
]}}"#;

#[test]
fn every_pane_with_an_agent_session_is_listed() {
    let panes = parse_agent_panes(LIST).expect("herdr's shape");
    assert_eq!(panes.len(), 2);
    assert_eq!(
        panes[0],
        AgentPane {
            pane: "wW:p21".into(),
            workspace: "wW".into(),
            agent: "claude".into(),
            session: "s-claude".into(),
            status: "blocked".into(),
            terminal_title: "ship the plan | dotfiles".into(),
        }
    );
    assert_eq!(panes[1].session, "s-codex");
}

#[test]
fn a_document_that_is_not_herdrs_is_no_answer() {
    assert_eq!(parse_agent_panes("not json"), None);
    assert_eq!(parse_agent_panes(r#"{"result":{}}"#), None);
}

#[test]
fn herdr_with_no_panes_is_an_empty_answer() {
    assert_eq!(parse_agent_panes(r#"{"result":{"panes":[]}}"#), Some(Vec::new()));
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters herdr::panes`
Expected: compile error, `file not found for module panes`.

- [ ] **Step 3: Implement the parse**

`panes.rs`:

```rust
//! The panes herdr says an agent is running in, from `herdr pane list`.
//!
//! HERDR IS WHAT KNOWS A SESSION IS LIVE: a pane it lists with an agent
//! session exists now, and its `agent_status` is herdr's own reading of that
//! agent. A document of any other shape is no answer rather than an empty one.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentPane {
    pub pane: String,
    pub workspace: String,
    pub agent: String,
    pub session: String,
    pub status: String,
    pub terminal_title: String,
}

pub fn parse_agent_panes(pane_list_json: &str) -> Option<Vec<AgentPane>> {
    let document: Value = serde_json::from_str(pane_list_json).ok()?;
    let panes = document.pointer("/result/panes")?.as_array()?;
    let text = |pane: &Value, pointer: &str| {
        pane.pointer(pointer).and_then(Value::as_str).unwrap_or_default().to_string()
    };
    Some(
        panes
            .iter()
            .filter_map(|pane| {
                let session = text(pane, "/agent_session/value");
                (!session.is_empty()).then(|| AgentPane {
                    pane: text(pane, "/pane_id"),
                    workspace: text(pane, "/workspace_id"),
                    agent: text(pane, "/agent"),
                    session,
                    status: text(pane, "/agent_status"),
                    terminal_title: text(pane, "/terminal_title_stripped"),
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests;
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters herdr::panes`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-adapters/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): read herdr's live agent panes"
```

### Task 5.2: What the store knows about those sessions

**Files:**
- Create: `pns/crates/pns-adapters/src/persistence/sqlite/session_facts.rs`
- Create: `pns/crates/pns-adapters/src/persistence/sqlite/tests/session_facts.rs`
- Modify: `pns/crates/pns-adapters/src/persistence/sqlite/mod.rs` (`mod session_facts; pub use
  session_facts::SessionFacts;`), `persistence/sqlite/tests.rs` (`mod session_facts;`),
  `pns-adapters/src/lib.rs` (re-export)

**Interfaces:**
- Produces:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionFacts {
    pub id: String,
    pub harness: String,
    pub project: String,
    pub branch: String,
    pub title: String,
    pub last_at: Option<u64>,
    pub last_state: Option<String>,
}
pub fn SqliteStore::session_facts(&self, ids: &[String]) -> Result<Vec<SessionFacts>, StoreError>;
```

- [ ] **Step 1: Write the failing tests**

`tests/session_facts.rs`:

```rust
use super::{SqliteStore, state};
use crate::persistence::sqlite::sessions::SessionNote;

fn activity(store: &SqliteStore, session: &str, state: &str, at: u64) {
    store
        .record_activity_event(&pns_domain::recap::activity::Event {
            at,
            agent: "claude".into(),
            state: state.into(),
            session: session.into(),
            project: "dotfiles".into(),
            ..Default::default()
        })
        .unwrap();
}

#[test]
fn a_known_session_carries_its_row_and_its_newest_event() {
    let store = SqliteStore::new(state());
    store
        .note_session(&SessionNote { id: "s1", harness: "claude", project: "dotfiles", branch: "main", title: "ship it", now: 100 })
        .unwrap();
    activity(&store, "s1", "prompt", 110);
    activity(&store, "s1", "asked", 120);
    let facts = store.session_facts(&["s1".to_string()]).unwrap();
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].title, "ship it");
    assert_eq!(facts[0].last_at, Some(120));
    assert_eq!(facts[0].last_state.as_deref(), Some("asked"));
}

#[test]
fn an_unknown_session_is_simply_absent() {
    let store = SqliteStore::new(state());
    store
        .note_session(&SessionNote { id: "s1", harness: "claude", project: "p", branch: "b", title: "t", now: 1 })
        .unwrap();
    assert_eq!(store.session_facts(&["nobody".to_string()]).unwrap(), vec![]);
}

#[test]
fn a_session_with_no_events_has_no_last_event() {
    let store = SqliteStore::new(state());
    store
        .note_session(&SessionNote { id: "s2", harness: "codex", project: "p", branch: "b", title: "t", now: 1 })
        .unwrap();
    let facts = store.session_facts(&["s2".to_string()]).unwrap();
    assert_eq!((facts[0].last_at, facts[0].last_state.clone()), (None, None));
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters session_facts`
Expected: compile error, `no method named session_facts`.

- [ ] **Step 3: Implement the read**

`session_facts.rs`:

```rust
//! What the store knows about sessions herdr says are live: their row, and
//! the newest event each one sent.

use super::{SqliteStore, StoreError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionFacts {
    pub id: String,
    pub harness: String,
    pub project: String,
    pub branch: String,
    pub title: String,
    pub last_at: Option<u64>,
    pub last_state: Option<String>,
}

impl SqliteStore {
    pub fn session_facts(&self, ids: &[String]) -> Result<Vec<SessionFacts>, StoreError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT s.id, s.harness, s.project, s.branch, s.title, a.at, a.state
               FROM sessions s
               LEFT JOIN activity_events a ON a.seq = (
                    SELECT n.seq FROM activity_events n WHERE n.session = s.id
                     ORDER BY n.at DESC, n.seq DESC LIMIT 1)
              WHERE s.id = ?1",
        )?;
        let mut facts = Vec::new();
        for id in ids {
            let mut rows = statement.query_map([id], |row| {
                Ok(SessionFacts {
                    id: row.get(0)?,
                    harness: row.get(1)?,
                    project: row.get(2)?,
                    branch: row.get(3)?,
                    title: row.get(4)?,
                    last_at: row.get(5)?,
                    last_state: row.get(6)?,
                })
            })?;
            if let Some(found) = rows.next() {
                facts.push(found?);
            }
        }
        Ok(facts)
    }
}
```

`connect()` is the viewer's read-only connection when the site calls it (Task 1.2).

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters session_facts`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-adapters/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): read a live session's row and its newest event"
```

### Task 5.3: `pns sessions`

**Files:**
- Create: `pns/crates/pns-domain/src/sessions.rs` (+ `sessions/tests.rs`), `pub mod sessions;` in
  `pns-domain/src/lib.rs`
- Create: `pns/crates/pns/src/command_sessions.rs`
- Create: `pns/crates/pns/src/command_sessions/tests.rs`
- Modify: `pns/crates/pns/src/lib.rs`, `invocation.rs`, `subcommand_usage.rs`, `legacy/usage.rs`

**Interfaces:**
- Consumes: `AgentPane`, `SessionFacts`, `render::{clipped, SESSION_TITLE_MAX_CHARS}`, `doctor::ago`,
  `utc_clock`, `view::{Sources, Summary}`, `pns_adapters::SystemCommandRunner`.
- Produces:

```rust
// pns-domain/src/sessions.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status { Blocked, Working, Done, Idle, Unknown }  // declared in the page's order
impl Status {
    pub fn parse(herdr_word: &str) -> Status;
    pub fn chip(self) -> &'static str;                      // "Blocked", "Working", ...
}
// pns/src/command_sessions.rs
pub(crate) const HERDR_SILENT: &str =
    "pns: herdr did not answer, so which sessions are live is unknown";
pub(crate) struct SessionRow {
    pub(crate) status: Status, pub(crate) title: String, pub(crate) agent: String,
    pub(crate) project: String, pub(crate) branch: String, pub(crate) pane: String,
    pub(crate) workspace: String, pub(crate) session: String, pub(crate) clock: String,
    pub(crate) last: String,
}
pub(crate) struct SessionsView { pub(crate) rows: Vec<SessionRow> }
pub(crate) fn view(panes: &[AgentPane], facts: Option<&[SessionFacts]>, now: u64) -> SessionsView;
pub(crate) fn read(sources: &Sources, pane_list: Option<String>, now: u64) -> Result<SessionsView, &'static str>;
pub(crate) fn herdr_pane_list() -> Option<String>;
pub(crate) fn render(paint: Paint, view: &SessionsView) -> String;
pub(crate) fn tone(status: Status) -> Tone;
pub(crate) fn summary(sources: &Sources, now: u64) -> Summary;
pub(crate) fn sessions_mode() -> i32;
```

- [ ] **Step 1: Write the failing tests**

`pns-domain/src/sessions/tests.rs`:

```rust
use super::*;

#[test]
fn herdrs_words_parse_and_anything_else_is_unknown() {
    assert_eq!(Status::parse("blocked"), Status::Blocked);
    assert_eq!(Status::parse("working"), Status::Working);
    assert_eq!(Status::parse("done"), Status::Done);
    assert_eq!(Status::parse("idle"), Status::Idle);
    assert_eq!(Status::parse("unknown"), Status::Unknown);
    assert_eq!(Status::parse("sleeping"), Status::Unknown);
}

#[test]
fn the_order_puts_what_needs_the_operator_first() {
    assert!(Status::Blocked < Status::Working);
    assert!(Status::Working < Status::Done);
    assert!(Status::Done < Status::Idle);
    assert!(Status::Idle < Status::Unknown);
    assert_eq!(Status::Blocked.chip(), "Blocked");
}
```

`pns/crates/pns/src/command_sessions/tests.rs`:

```rust
use super::*;
use pns_adapters::{AgentPane, SessionFacts};

const NOW: u64 = 1_790_139_720;

fn pane(session: &str, status: &str, pane: &str) -> AgentPane {
    AgentPane {
        pane: pane.into(),
        workspace: "wW".into(),
        agent: "claude".into(),
        session: session.into(),
        status: status.into(),
        terminal_title: "terminal title".into(),
    }
}

fn facts(id: &str, title: &str, last_at: Option<u64>, last_state: Option<&str>) -> SessionFacts {
    SessionFacts {
        id: id.into(),
        harness: "claude".into(),
        project: "dotfiles".into(),
        branch: "main".into(),
        title: title.into(),
        last_at,
        last_state: last_state.map(str::to_string),
    }
}

fn fixture() -> SessionsView {
    view(
        &[pane("a", "working", "wW:p1"), pane("b", "blocked", "wW:p2"), pane("c", "idle", "wW:p3")],
        Some(&[
            facts("a", "fix the guard", Some(NOW - 60), Some("prompt")),
            facts("b", "ship the plan", Some(NOW - 1_260), Some("asked")),
        ][..]),
        NOW,
    )
}

#[test]
fn rows_are_ordered_by_status_then_newest_event() {
    let order: Vec<_> = fixture().rows.iter().map(|row| row.session.clone()).collect();
    assert_eq!(order, ["b", "a", "c"]);
}

#[test]
fn a_row_carries_the_stored_facts_and_herdrs_pane() {
    let view = fixture();
    let blocked = &view.rows[0];
    assert_eq!(blocked.title, "ship the plan");
    assert_eq!(blocked.pane, "wW:p2");
    assert_eq!(blocked.last, "21m ago, asked");
    assert_eq!(blocked.clock, "04:41");
}

#[test]
fn a_pane_pns_never_heard_from_is_still_listed() {
    let view = fixture();
    let idle = &view.rows[2];
    assert_eq!(idle.title, "terminal title");
    assert_eq!(idle.last, "no events recorded");
}

#[test]
fn an_unreadable_store_still_lists_herdrs_panes() {
    let view = view(&[pane("a", "working", "wW:p1")], None, NOW);
    assert_eq!(view.rows[0].last, "session store unreadable");
}

#[test]
fn the_terminal_listing_is_pinned() {
    assert_eq!(
        render(Paint::Plain, &fixture()),
        "pns: 3 live sessions\n\
         \x20 ● Blocked  claude  dotfiles  wW:p2  21m ago, asked  ship the plan\n\
         \x20 ● Working  claude  dotfiles  wW:p1  1m ago, prompt  fix the guard\n\
         \x20 ● Idle     claude  -  wW:p3  no events recorded  terminal title\n"
    );
}

#[test]
fn herdr_silent_is_the_sentence_and_not_an_empty_list() {
    let sources = crate::view::Sources::at(std::env::temp_dir().join("pns-sessions-absent"), "/x");
    assert_eq!(read(&sources, None, NOW).err(), Some(HERDR_SILENT));
    assert_eq!(read(&sources, Some("{}".into()), NOW).err(), Some(HERDR_SILENT));
}

#[test]
fn the_summary_counts_live_and_blocked() {
    let summarized = summarize(&fixture());
    assert_eq!(summarized.text, "3 live, 1 blocked");
    assert_eq!(summarized.tone, Tone::Bad);
}
```

A pane with no stored facts carries herdr's agent and an empty project, which the renderer prints as
`-`, as the third line shows.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-domain sessions` and
`cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib command_sessions`
Expected: compile errors, `file not found for module sessions` and `for module command_sessions`.

- [ ] **Step 3: Implement the domain status**

`pns-domain/src/sessions.rs`:

```rust
//! herdr's agent status words, in the order the Sessions page lists them.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Blocked,
    Working,
    Done,
    Idle,
    Unknown,
}

impl Status {
    pub fn parse(herdr_word: &str) -> Status {
        match herdr_word {
            "blocked" => Status::Blocked,
            "working" => Status::Working,
            "done" => Status::Done,
            "idle" => Status::Idle,
            _ => Status::Unknown,
        }
    }

    pub fn chip(self) -> &'static str {
        match self {
            Status::Blocked => "Blocked",
            Status::Working => "Working",
            Status::Done => "Done",
            Status::Idle => "Idle",
            Status::Unknown => "Unknown",
        }
    }
}

#[cfg(test)]
mod tests;
```

- [ ] **Step 4: Implement the builder, the renderer and the mode**

`pns/crates/pns/src/command_sessions.rs`:

```rust
//! `pns sessions`: one row per live agent session.
//!
//! LIVE IS HERDR'S WORD. pns keeps no liveness of its own (its session rows
//! are never swept), so a session is listed when herdr lists its pane, with
//! herdr's own status, and the store only adds what herdr does not know.

use crate::style::{self, Paint, Tone};
use crate::view::{Sources, Summary};
use pns_adapters::{AgentPane, SessionFacts};
use pns_application::CommandRunner;
use pns_domain::sessions::Status;

pub(crate) const SESSIONS_USAGE: &str = "\
pns: usage:
  pns sessions                     one row per live agent session
";

pub(crate) const HERDR_SILENT: &str =
    "pns: herdr did not answer, so which sessions are live is unknown";

pub(crate) struct SessionRow {
    pub(crate) status: Status,
    pub(crate) title: String,
    pub(crate) agent: String,
    pub(crate) project: String,
    pub(crate) branch: String,
    pub(crate) pane: String,
    pub(crate) workspace: String,
    pub(crate) session: String,
    pub(crate) clock: String,
    pub(crate) last: String,
    last_at: u64,
}

pub(crate) struct SessionsView {
    pub(crate) rows: Vec<SessionRow>,
}

pub(crate) fn view(panes: &[AgentPane], facts: Option<&[SessionFacts]>, now: u64) -> SessionsView {
    let mut rows: Vec<SessionRow> = panes
        .iter()
        .map(|pane| {
            let known = facts.and_then(|facts| facts.iter().find(|fact| fact.id == pane.session));
            let title = known
                .map(|fact| fact.title.as_str())
                .filter(|title| !title.is_empty())
                .unwrap_or(&pane.terminal_title);
            let last = match (facts, known.and_then(|fact| fact.last_at.zip(fact.last_state.clone()))) {
                (None, _) => "session store unreadable".to_string(),
                (Some(_), None) => "no events recorded".to_string(),
                (Some(_), Some((at, state))) => {
                    format!("{}, {state}", pns_domain::doctor::ago(now.saturating_sub(at)))
                }
            };
            let last_at = known.and_then(|fact| fact.last_at).unwrap_or(0);
            SessionRow {
                status: Status::parse(&pane.status),
                title: pns_domain::render::clipped(title, pns_domain::render::SESSION_TITLE_MAX_CHARS),
                agent: pane.agent.clone(),
                project: known.map(|fact| fact.project.clone()).unwrap_or_default(),
                branch: known.map(|fact| fact.branch.clone()).unwrap_or_default(),
                pane: pane.pane.clone(),
                workspace: pane.workspace.clone(),
                session: pane.session.clone(),
                clock: if last_at == 0 { String::new() } else { pns_adapters::utc_clock(last_at) },
                last,
                last_at,
            }
        })
        .collect();
    rows.sort_by(|a, b| a.status.cmp(&b.status).then(b.last_at.cmp(&a.last_at)));
    SessionsView { rows }
}

pub(crate) fn herdr_pane_list() -> Option<String> {
    pns_adapters::SystemCommandRunner.run("herdr", &["pane", "list"])
}

pub(crate) fn read(sources: &Sources, pane_list: Option<String>, now: u64) -> Result<SessionsView, &'static str> {
    let panes = pane_list
        .as_deref()
        .and_then(pns_adapters::parse_agent_panes)
        .ok_or(HERDR_SILENT)?;
    let ids: Vec<String> = panes.iter().map(|pane| pane.session.clone()).collect();
    let facts = sources.store.session_facts(&ids).ok();
    Ok(view(&panes, facts.as_deref(), now))
}

pub(crate) fn tone(status: Status) -> Tone {
    match status {
        Status::Blocked => Tone::Bad,
        Status::Working => Tone::Warn,
        Status::Done | Status::Idle | Status::Unknown => Tone::Quiet,
    }
}

pub(crate) fn render(paint: Paint, view: &SessionsView) -> String {
    if view.rows.is_empty() {
        return "pns: no agent is running in herdr\n".to_string();
    }
    let noun = if view.rows.len() == 1 { "session" } else { "sessions" };
    let mut out = format!("pns: {} live {noun}\n", view.rows.len());
    for row in &view.rows {
        let project = if row.project.is_empty() { "-" } else { &row.project };
        let text = format!(
            "{:<7}  {}  {project}  {}  {}  {}",
            row.status.chip(),
            row.agent,
            row.pane,
            row.last,
            row.title
        );
        out.push_str(&style::row(paint, tone(row.status), "●", 2, &text));
        out.push('\n');
    }
    out
}

pub(crate) fn summarize(view: &SessionsView) -> Summary {
    let blocked = view.rows.iter().filter(|row| row.status == Status::Blocked).count();
    match (view.rows.len(), blocked) {
        (0, _) => Summary { text: "none".into(), tone: Tone::Quiet },
        (live, 0) => Summary { text: format!("{live} live"), tone: Tone::Quiet },
        (live, blocked) => Summary { text: format!("{live} live, {blocked} blocked"), tone: Tone::Bad },
    }
}

pub(crate) fn summary(sources: &Sources, now: u64) -> Summary {
    match read(sources, herdr_pane_list(), now) {
        Ok(view) => summarize(&view),
        Err(_) => Summary { text: "herdr silent".into(), tone: Tone::Quiet },
    }
}

pub(crate) fn sessions_mode() -> i32 {
    if !crate::arguments_after_subcommand().is_empty() {
        eprintln!("{SESSIONS_USAGE}");
        return 2;
    }
    let Some(now) = pns_adapters::now_secs() else {
        eprintln!("pns sessions: this machine has no clock to age an event with");
        return 1;
    };
    match read(&Sources::live(), herdr_pane_list(), now) {
        Ok(view) => {
            print!("{}", render(Paint::for_stdout(), &view));
            0
        }
        Err(sentence) => {
            eprintln!("{sentence}");
            1
        }
    }
}

#[cfg(test)]
mod tests;
```

Register `sessions` in `invocation.rs`, `SUBCOMMAND_USAGE` and `USAGE` (`  pns sessions                     one
row per live agent session`) the way Task 1.4 registered `waiting`.

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-domain sessions` and
`cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib command_sessions subcommand_usage`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add pns/crates/pns-domain/src pns/crates/pns/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): add pns sessions, one row per live agent session"
```

### Task 5.4: The Sessions page, its route and its index row

**Files:**
- Create: `pns/crates/pns/src/site/sessions.rs`
- Create: `pns/crates/pns/src/site/sessions/tests.rs`
- Modify: `pns/crates/pns/src/site.rs`, `site/index.rs`, `site/tests.rs`

**Interfaces:**
- Consumes: `command_sessions::{read, herdr_pane_list, summary, tone, SessionsView, SessionRow}`,
  `shell::row_class`.
- Produces: `pub(super) fn site::sessions::page(view: Result<&SessionsView, &'static str>, rendered_at:
  u64) -> String`.

- [ ] **Step 1: Write the failing tests**

`pns/crates/pns/src/site/sessions/tests.rs`:

```rust
use super::*;
use crate::command_sessions::view;
use pns_adapters::{AgentPane, SessionFacts};

const NOW: u64 = 1_790_139_720;

fn fixture(title: &str) -> crate::command_sessions::SessionsView {
    view(
        &[
            AgentPane { pane: "wW:p2".into(), workspace: "wW".into(), agent: "claude".into(), session: "b".into(), status: "blocked".into(), terminal_title: String::new() },
            AgentPane { pane: "wW:p1".into(), workspace: "wW".into(), agent: "codex".into(), session: "a".into(), status: "working".into(), terminal_title: String::new() },
        ],
        Some(&[
            SessionFacts { id: "b".into(), harness: "claude".into(), project: "dotfiles".into(), branch: "main".into(), title: title.into(), last_at: Some(NOW - 1_260), last_state: Some("asked".into()) },
            SessionFacts { id: "a".into(), harness: "codex".into(), project: "pns".into(), branch: "fix".into(), title: "check the build".into(), last_at: Some(NOW - 60), last_state: Some("prompt".into()) },
        ][..]),
        NOW,
    )
}

#[test]
fn every_value_the_builder_spelled_is_on_the_page() {
    let view = fixture("ship the plan");
    let served = page(Ok(&view), NOW);
    for row in &view.rows {
        for value in [&row.title, &row.agent, &row.project, &row.branch, &row.pane, &row.workspace, &row.session, &row.clock, &row.last] {
            assert!(served.contains(&escaped(value)), "{value:?} is not on the page");
        }
        assert!(served.contains(row.status.chip()));
    }
}

#[test]
fn each_status_group_has_its_heading_in_order() {
    let served = page(Ok(&fixture("t")), NOW);
    let blocked = served.find("<strong>Blocked</strong>").expect("the Blocked heading");
    let working = served.find("<strong>Working</strong>").expect("the Working heading");
    assert!(blocked < working);
}

#[test]
fn a_session_title_is_escaped_once() {
    let served = page(Ok(&fixture("<script>x</script>")), NOW);
    assert!(served.contains("&lt;script&gt;x&lt;/script&gt;"), "{served}");
    assert!(!served.contains("<script>x"), "{served}");
}

#[test]
fn herdr_silent_is_the_terminal_sentence() {
    let served = page(Err(crate::command_sessions::HERDR_SILENT), NOW);
    assert!(served.contains("herdr did not answer"), "{served}");
}

#[test]
fn no_live_session_is_one_line() {
    let empty = view(&[], Some(&[][..]), NOW);
    assert!(page(Ok(&empty), NOW).contains("No agent is running in herdr."));
}
```

In `site/tests.rs`, the route test for `/sessions` in the same shape as the others.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib site::`
Expected: compile errors, `no variant Sessions` and `cannot find function page in module sessions`.

- [ ] **Step 3: Implement the page**

`pns/crates/pns/src/site/sessions.rs`:

```rust
//! The Sessions page: `pns sessions`' view, laid out as the listing card
//! with one heading per status.

use super::shell::{escaped, footer, page as shell_page, row_class};
use crate::command_sessions::{SessionRow, SessionsView, tone};

pub(super) fn page(view: Result<&SessionsView, &'static str>, rendered_at: u64) -> String {
    let body = match view {
        Err(sentence) => format!("<p>{}</p>", escaped(sentence)),
        Ok(view) if view.rows.is_empty() => "<p>No agent is running in herdr.</p>".to_string(),
        Ok(view) => entries(view),
    };
    shell_page(
        "pns sessions",
        &format!(
            "<section id=\"failure-history\"><a class=\"fh-back\" href=\"/\">pns</a>\
             <header class=\"fh-header\"><h2>Sessions</h2></header>\n{body}\n{}</section>",
            footer(rendered_at)
        ),
    )
}

fn entries(view: &SessionsView) -> String {
    let mut out = String::new();
    let mut group = None;
    let last = view.rows.len() - 1;
    for (index, row) in view.rows.iter().enumerate() {
        if group != Some(row.status) {
            out.push_str(&format!("<div class=\"fh-day\"><strong>{}</strong></div>\n", row.status.chip()));
            group = Some(row.status);
        }
        out.push_str(&entry(row, index == last));
    }
    out
}

fn entry(row: &SessionRow, last: bool) -> String {
    let last = if last { " fh-last" } else { "" };
    let detail = |label: &str, value: &str| format!("<dt>{label}</dt><dd>{}</dd>", escaped(value));
    format!(
        "<article class=\"{class}{last}\"><time class=\"fh-clock\">{clock}</time>\
         <div class=\"fh-content\"><span class=\"fh-dot\" aria-hidden=\"true\"></span>\
         <div class=\"fh-title\"><strong>{title}</strong><span class=\"fh-state\">{chip}</span></div>\
         <div class=\"fh-sub\">{agent} · {project} · {pane}</div><div class=\"fh-next\">Last event {last_event}</div>\
         <details><summary>Details</summary><div class=\"fh-detail\"><dl>{details}</dl></div></details>\
         </div></article>\n",
        class = row_class(tone(row.status)),
        clock = escaped(&row.clock),
        title = escaped(&row.title),
        chip = row.status.chip(),
        agent = escaped(&row.agent),
        project = escaped(&row.project),
        pane = escaped(&row.pane),
        last_event = escaped(&row.last),
        details = [
            detail("Session", &row.session),
            detail("Agent", &row.agent),
            detail("Project", &row.project),
            detail("Branch", &row.branch),
            detail("Pane", &row.pane),
            detail("Workspace", &row.workspace),
        ]
        .concat(),
    )
}

#[cfg(test)]
mod tests;
```

In `site.rs`, `Target::Sessions` on `/sessions`, and the arm:

```rust
        Some(Target::Sessions) => {
            let view = crate::command_sessions::read(sources, crate::command_sessions::herdr_pane_list(), now);
            ok(&sessions::page(view.as_ref().map_err(|sentence| *sentence), now))
        }
```

In `index.rs`, the Sessions row after Right now, with `summary: crate::command_sessions::summary(sources,
now)`, name "Sessions", blurb "live agent sessions".

- [ ] **Step 4: Run the site tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib site::`
Expected: all pass.

- [ ] **Step 5: Run the gates and check file sizes**

```bash
just test-rust; echo "test-rust exit $?"
just lint-check; echo "lint-check exit $?"
just test-unit; echo "test-unit exit $?"
```

Expected: three zero exit codes; every touched `.rs` under 500 total lines.

- [ ] **Step 6: Commit**

```bash
git add pns/crates/pns/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): serve the Sessions page"
```

---

## Pull request 6: Deliveries

Branch `feat/pns-page-deliveries`. Worktree:
`herdr worktree create --cwd /Users/stephen/workspaces/Ivy/webdavis/dotfiles --branch feat/pns-page-deliveries --no-focus`

### Task 6.1: A leg's outcome, an event's verdict and a destination's summary, in the domain

**Files:**
- Modify: `pns/crates/pns-domain/src/retry.rs` (`TransportOutcome::short`; `mod delivered; pub use
  delivered::*;`)
- Create: `pns/crates/pns-domain/src/retry/delivered.rs`
- Create: `pns/crates/pns-domain/src/retry/delivered/tests.rs`
- Modify: `pns/crates/pns/src/command_failures.rs` (`short_status` calls `failure.outcome.short()`)

**Interfaces:**
- Produces:

```rust
impl TransportOutcome { pub fn short(self) -> String; }          // "HTTP 404", "no response", "bad URL"
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegOutcome { Delivered, Pending, Failed(TransportOutcome) }
pub fn leg_word(outcome: LegOutcome, deadlettered: bool) -> String; // "delivered", "pending", "gave up", "HTTP 404"
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict { Delivered, Retrying, NotDelivered }
impl Verdict { pub fn word(self) -> &'static str; }                // "Delivered", "Retrying", "Not delivered"
pub fn event_verdict(legs: &[(LegOutcome, bool)]) -> Verdict;    // (outcome, deadlettered) per leg
pub fn median(values: &[u64]) -> Option<u64>;
pub fn destination_summary(delivered: usize, total: usize, median: Option<u64>) -> String;
```

- [ ] **Step 1: Write the failing tests**

`retry/delivered/tests.rs`:

```rust
use super::*;

#[test]
fn a_failure_is_spelled_the_way_the_failures_listing_spells_it() {
    assert_eq!(TransportOutcome::Status(404).short(), "HTTP 404");
    assert_eq!(TransportOutcome::NoResponse.short(), "no response");
    assert_eq!(TransportOutcome::NoStatus.short(), "bad URL");
}

#[test]
fn a_leg_says_what_became_of_it() {
    assert_eq!(leg_word(LegOutcome::Delivered, false), "delivered");
    assert_eq!(leg_word(LegOutcome::Pending, false), "pending");
    assert_eq!(leg_word(LegOutcome::Pending, true), "gave up");
    assert_eq!(leg_word(LegOutcome::Failed(TransportOutcome::Status(404)), true), "HTTP 404");
}

#[test]
fn an_event_is_as_bad_as_its_worst_leg() {
    use LegOutcome::*;
    let failed = Failed(TransportOutcome::NoResponse);
    assert_eq!(event_verdict(&[(Delivered, false), (Delivered, false)]), Verdict::Delivered);
    assert_eq!(event_verdict(&[(Delivered, false), (failed, false)]), Verdict::Retrying);
    assert_eq!(event_verdict(&[(Delivered, false), (Pending, false)]), Verdict::Retrying);
    assert_eq!(event_verdict(&[(failed, false), (failed, true)]), Verdict::NotDelivered);
    assert_eq!(Verdict::NotDelivered.word(), "Not delivered");
}

#[test]
fn the_median_is_the_middle_value_and_the_lower_of_two() {
    assert_eq!(median(&[]), None);
    assert_eq!(median(&[3]), Some(3));
    assert_eq!(median(&[9, 1, 2]), Some(2));
    assert_eq!(median(&[4, 1, 2, 3]), Some(2));
}

#[test]
fn a_destination_summary_counts_and_times() {
    assert_eq!(destination_summary(41, 43, Some(1)), "41 of 43 delivered, median 1s");
    assert_eq!(destination_summary(0, 3, None), "0 of 3 delivered");
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-domain retry::delivered`
Expected: compile error, `file not found for module delivered`.

- [ ] **Step 3: Implement**

In `retry.rs`, beside `TransportOutcome`:

```rust
impl TransportOutcome {
    /// The failure as a listing's status column says it.
    pub fn short(self) -> String {
        match self {
            Self::Status(code) => format!("HTTP {code}"),
            Self::NoResponse => "no response".to_string(),
            Self::NoStatus => "bad URL".to_string(),
        }
    }
}
```

and `short_status` in `command_failures.rs` becomes `failure.outcome.short()` (the failures golden test
pins that nothing moved). `retry/delivered.rs`:

```rust
//! What became of each leg of an event, and of the event as a whole.

use super::TransportOutcome;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegOutcome {
    Delivered,
    Pending,
    Failed(TransportOutcome),
}

pub fn leg_word(outcome: LegOutcome, deadlettered: bool) -> String {
    match outcome {
        LegOutcome::Delivered => "delivered".to_string(),
        LegOutcome::Pending if deadlettered => "gave up".to_string(),
        LegOutcome::Pending => "pending".to_string(),
        LegOutcome::Failed(transport) => transport.short(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Delivered,
    Retrying,
    NotDelivered,
}

impl Verdict {
    pub fn word(self) -> &'static str {
        match self {
            Self::Delivered => "Delivered",
            Self::Retrying => "Retrying",
            Self::NotDelivered => "Not delivered",
        }
    }
}

/// AN EVENT IS AS BAD AS ITS WORST LEG: one leg given up on is an event that
/// did not fully arrive, and one still pending or failing is one still trying.
pub fn event_verdict(legs: &[(LegOutcome, bool)]) -> Verdict {
    if legs.iter().any(|(_, deadlettered)| *deadlettered) {
        return Verdict::NotDelivered;
    }
    if legs.iter().all(|(outcome, _)| *outcome == LegOutcome::Delivered) {
        Verdict::Delivered
    } else {
        Verdict::Retrying
    }
}

/// The middle value, the lower of the two middles for an even count.
pub fn median(values: &[u64]) -> Option<u64> {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted.get(sorted.len().checked_sub(1)? / 2).copied()
}

pub fn destination_summary(delivered: usize, total: usize, median: Option<u64>) -> String {
    match median {
        Some(seconds) => format!(
            "{delivered} of {total} delivered, median {}",
            crate::remind::waited(seconds)
        ),
        None => format!("{delivered} of {total} delivered"),
    }
}

#[cfg(test)]
mod tests;
```

- [ ] **Step 4: Run the domain and failures tests**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-domain retry` and
`cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib command_failures`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-domain/src/retry.rs pns/crates/pns-domain/src/retry pns/crates/pns/src/command_failures.rs
SKIP_AI_COMMIT=1 git commit -m "feat(pns): word a leg's outcome and an event's verdict once"
```

### Task 6.2: Every leg of the last day, from the ledger

**Files:**
- Create: `pns/crates/pns-adapters/src/persistence/sqlite/ledger/deliveries.rs`
- Create: `pns/crates/pns-adapters/src/persistence/sqlite/ledger/tests/deliveries.rs`
- Modify: `pns/crates/pns-adapters/src/persistence/sqlite/ledger.rs` (`mod deliveries;`,
  `deliveries_since`), `ledger/tests.rs` (`mod deliveries;`)
- Modify: `pns/crates/pns-application/src/ports/ledger.rs` (`StoredDelivery`) and its re-export

**Interfaces:**
- Produces:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDelivery {
    pub id: u64,
    pub event: u64,
    pub destination: String,
    pub route: String,
    pub agent: String,
    pub state: String,
    pub project: String,
    /// `started` of the leg's first attempt, written when the event was accepted.
    pub accepted_at: u64,
    /// `finished` of the acknowledging attempt, only when the leg was delivered.
    pub settled_at: Option<u64>,
    pub outcome: pns_domain::retry::LegOutcome,
    pub retries: u64,
    pub deadlettered: bool,
}
pub fn SqliteStore::deliveries_since(&self, cutoff: u64, limit: u32) -> Result<Vec<StoredDelivery>, LedgerFailure>;
```

- [ ] **Step 1: Write the failing tests**

`ledger/tests/deliveries.rs`, with the ledger tests' own seeding helpers:

```rust
use super::*;

#[test]
fn a_delivered_leg_carries_its_latency_ends() {
    let store = SqliteStore::new(state());
    let claims = created(&store, &remote_input());
    store
        .record(&claims[0].claim, &pns_domain::Delivery::Delivered("ok".into()), 13, Default::default())
        .unwrap();
    let legs = store.deliveries_since(0, 500).unwrap();
    assert_eq!(legs.len(), 1);
    assert_eq!(legs[0].destination, "hermes");
    assert_eq!(legs[0].outcome, pns_domain::retry::LegOutcome::Delivered);
    assert_eq!(legs[0].settled_at, Some(13));
    assert!(legs[0].accepted_at <= 13);
}

#[test]
fn a_failed_leg_is_listed_with_its_status_and_no_settlement() {
    let store = SqliteStore::new(state());
    let claims = created(&store, &remote_input());
    store
        .record(&claims[0].claim, &pns_domain::Delivery::Failed("no".into()), 13, Default::default())
        .unwrap();
    let legs = store.deliveries_since(0, 500).unwrap();
    assert!(matches!(legs[0].outcome, pns_domain::retry::LegOutcome::Failed(_)));
    assert_eq!(legs[0].settled_at, None);
}

#[test]
fn the_cutoff_and_the_cap_bound_the_read() {
    let store = SqliteStore::new(state());
    let claims = created(&store, &remote_input());
    let accepted = store.deliveries_since(0, 500).unwrap()[0].accepted_at;
    assert_eq!(store.deliveries_since(accepted + 1, 500).unwrap(), vec![]);
    assert_eq!(store.deliveries_since(0, 0).unwrap(), vec![]);
    drop(claims);
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters ledger::tests::deliveries`
Expected: compile error, `no method named deliveries_since`.

- [ ] **Step 3: Implement the read**

`ledger/deliveries.rs`:

```rust
//! Every leg the ledger accepted since a moment, delivered or not, newest
//! first.
//!
//! ACCEPTED IS THE FIRST ATTEMPT'S `started`: `prepare` writes a generation-1
//! attempt for every leg in the transaction that inserts the event, so it is
//! the event's own acceptance time. DELIVERED IS `outcome = 1`, never
//! `acknowledged`, which the dead-letter drain sets too.

use super::*;
use pns_application::StoredDelivery;
use pns_domain::retry::{LegOutcome, TransportOutcome};
use rusqlite::{Connection, Row};

const SELECT: &str = "SELECT l.id, l.event, l.destination, l.route, e.agent, e.state, e.project,
        first.started, a.finished, a.outcome, a.http_status, l.generation,
        l.deadlettered_at IS NOT NULL
   FROM ledger_legs l
   JOIN ledger_events e ON e.seq = l.event
   JOIN ledger_attempts first ON first.leg = l.id AND first.generation = 1
   JOIN ledger_attempts a ON a.leg = l.id AND a.generation = l.generation
  WHERE first.started >= ?1
  ORDER BY first.started DESC, l.id DESC
  LIMIT ?2";

pub(super) fn since(connection: &Connection, cutoff: u64, limit: u32) -> Result<Vec<StoredDelivery>, StoreError> {
    let mut query = connection.prepare(SELECT)?;
    let rows = query
        .query_map(rusqlite::params![&cutoff.to_be_bytes()[..], limit], row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn epoch(bytes: Option<Vec<u8>>) -> Option<u64> {
    bytes.and_then(|bytes| <[u8; 8]>::try_from(bytes).ok()).map(u64::from_be_bytes)
}

fn row(row: &Row<'_>) -> rusqlite::Result<StoredDelivery> {
    let finished = epoch(row.get(8)?);
    let status: Option<u16> = row.get(10)?;
    let outcome = match (row.get::<_, u8>(9)?, status) {
        (1, _) => LegOutcome::Delivered,
        (2 | 3, Some(code)) => LegOutcome::Failed(TransportOutcome::Status(code)),
        (3, None) => LegOutcome::Failed(TransportOutcome::NoStatus),
        (2, None) => LegOutcome::Failed(TransportOutcome::NoResponse),
        _ => LegOutcome::Pending,
    };
    let generation: u64 = row.get(11)?;
    Ok(StoredDelivery {
        id: row.get::<_, i64>(0)? as u64,
        event: row.get::<_, i64>(1)? as u64,
        destination: row.get(2)?,
        route: row.get(3)?,
        agent: row.get(4)?,
        state: row.get(5)?,
        project: row.get(6)?,
        accepted_at: epoch(row.get(7)?).unwrap_or(0),
        settled_at: (outcome == LegOutcome::Delivered).then_some(finished).flatten(),
        outcome,
        retries: generation.saturating_sub(1),
        deadlettered: row.get(12)?,
    })
}
```

In `ledger.rs`, beside `failing_legs`:

```rust
    pub fn deliveries_since(&self, cutoff: u64, limit: u32) -> Result<Vec<StoredDelivery>, LedgerFailure> {
        self.failing(|connection| deliveries::since(connection, cutoff, limit))
    }
```

Add `StoredDelivery` (the struct in the Interfaces block) to `pns-application/src/ports/ledger.rs` beside
`StoredFailure` and re-export it from `pns-application/src/lib.rs`. The ledger has no index on
`started`, and the cap bounds the scan's output, which is what a single-threaded page needs; an index is
a later change if the scan measures slow.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters ledger::tests::deliveries`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-adapters/src pns/crates/pns-application/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): read every leg of the last day from the ledger"
```

### Task 6.3: `pns deliveries`

**Files:**
- Create: `pns/crates/pns/src/command_deliveries.rs`
- Create: `pns/crates/pns/src/command_deliveries/tests.rs`
- Modify: `pns/crates/pns/src/lib.rs`, `invocation.rs`, `subcommand_usage.rs`, `legacy/usage.rs`

**Interfaces:**
- Consumes: `StoredDelivery`, `retry::{leg_word, event_verdict, Verdict, median, destination_summary}`,
  `render::title`, `remind::waited`, `utc_clock`, `utc_day`, `view::{Sources, Summary}`.
- Produces:

```rust
pub(crate) const WINDOW_SECS: u64 = 86_400;
pub(crate) const LIMIT: u32 = 500;
pub(crate) const UNREADABLE: &str = "pns: the delivery ledger could not be read";
pub(crate) struct LegRow {
    pub(crate) id: u64, pub(crate) destination: String, pub(crate) route: String,
    pub(crate) status: String, pub(crate) latency: String, pub(crate) retries: u64,
    pub(crate) failing: bool, pub(crate) brief: String,  // "hermes HTTP 404", "phone 1s"
}
pub(crate) struct EventRow {
    pub(crate) title: String, pub(crate) verdict: Verdict, pub(crate) clock: String,
    pub(crate) day: String, pub(crate) legs: Vec<LegRow>,
}
pub(crate) struct DestinationRow { pub(crate) destination: String, pub(crate) line: String }
pub(crate) struct DeliveriesView {
    pub(crate) destinations: Vec<DestinationRow>, pub(crate) events: Vec<EventRow>, pub(crate) capped: bool,
}
pub(crate) fn view(legs: &[StoredDelivery], now: u64) -> DeliveriesView;
pub(crate) fn read(sources: &Sources, now: u64) -> Result<DeliveriesView, &'static str>;
pub(crate) fn render(paint: Paint, view: &DeliveriesView) -> String;
pub(crate) fn tone(verdict: Verdict) -> Tone;
pub(crate) fn summary(sources: &Sources, now: u64) -> Summary;
pub(crate) fn deliveries_mode() -> i32;
```

- [ ] **Step 1: Write the failing tests**

`pns/crates/pns/src/command_deliveries/tests.rs`:

```rust
use super::*;
use pns_application::StoredDelivery;
use pns_domain::retry::{LegOutcome, TransportOutcome};

const NOW: u64 = 1_790_139_720;

fn leg(id: u64, event: u64, destination: &str, outcome: LegOutcome, settled: Option<u64>, dead: bool) -> StoredDelivery {
    StoredDelivery {
        id,
        event,
        destination: destination.into(),
        route: if destination == "hermes" { "uu-runs".into() } else { String::new() },
        agent: "uu".into(),
        state: "failed".into(),
        project: "uu".into(),
        accepted_at: NOW - 6_120,
        settled_at: settled,
        outcome,
        retries: 0,
        deadlettered: dead,
    }
}

fn fixture() -> Vec<StoredDelivery> {
    vec![
        leg(8005, 7, "hermes", LegOutcome::Failed(TransportOutcome::Status(404)), None, true),
        leg(8004, 7, "banner", LegOutcome::Delivered, Some(NOW - 6_120), false),
    ]
}

#[test]
fn the_terminal_listing_is_pinned() {
    assert_eq!(
        render(Paint::Plain, &view(&fixture(), NOW)),
        "pns: deliveries in the last 24 hours\n\
         \x20 banner  1 of 1 delivered, median 0s\n\
         \x20 hermes  0 of 1 delivered\n\
         \n\
         \x20 ● 03:20Z  Not delivered  uu · failed · uu\n\
         \x20    8005  hermes  HTTP 404   -   uu-runs\n\
         \x20    8004  banner  delivered  0s\n"
    );
}

#[test]
fn legs_group_under_their_event_with_its_worst_verdict() {
    let view = view(&fixture(), NOW);
    assert_eq!(view.events.len(), 1);
    assert_eq!(view.events[0].verdict, Verdict::NotDelivered);
    assert_eq!(view.events[0].legs.len(), 2);
    assert!(view.events[0].legs[0].failing);
    assert_eq!(view.events[0].legs[0].brief, "hermes HTTP 404");
    assert_eq!(view.events[0].legs[1].brief, "banner 0s");
    assert_eq!(view.events[0].day, "Today");
}

#[test]
fn the_cap_is_said_when_it_cut() {
    let many: Vec<StoredDelivery> = (0..LIMIT as u64)
        .map(|index| leg(index, index, "banner", LegOutcome::Delivered, Some(NOW - 6_120), false))
        .collect();
    assert!(view(&many, NOW).capped);
    assert!(render(Paint::Plain, &view(&many, NOW)).contains("showing the newest 500"));
}

#[test]
fn nothing_delivered_is_one_sentence() {
    assert_eq!(render(Paint::Plain, &view(&[], NOW)), "pns: nothing was delivered in the last day\n");
}

#[test]
fn the_summary_counts_the_day_and_the_given_up() {
    let summarized = summarize(&view(&fixture(), NOW));
    assert_eq!(summarized.text, "1 in 24h, 1 not delivered");
    assert_eq!(summarized.tone, Tone::Bad);
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib command_deliveries`
Expected: compile error, `file not found for module command_deliveries`.

- [ ] **Step 3: Implement the builder, the renderer and the mode**

`pns/crates/pns/src/command_deliveries.rs`:

```rust
//! `pns deliveries`: every leg of the last day, grouped by the event that
//! produced it, with each destination's count and median latency.

use crate::style::{self, Paint, Tone};
use crate::view::{Sources, Summary};
use pns_application::StoredDelivery;
use pns_domain::retry::{LegOutcome, Verdict, destination_summary, event_verdict, leg_word, median};

pub(crate) const DELIVERIES_USAGE: &str = "\
pns: usage:
  pns deliveries                   every delivery of the last day, by event and destination
";

pub(crate) const WINDOW_SECS: u64 = 86_400;
pub(crate) const LIMIT: u32 = 500;
pub(crate) const UNREADABLE: &str = "pns: the delivery ledger could not be read";

pub(crate) struct LegRow {
    pub(crate) id: u64,
    pub(crate) destination: String,
    pub(crate) route: String,
    pub(crate) status: String,
    pub(crate) latency: String,
    pub(crate) retries: u64,
    pub(crate) failing: bool,
    pub(crate) brief: String,
}

pub(crate) struct EventRow {
    pub(crate) title: String,
    pub(crate) verdict: Verdict,
    pub(crate) clock: String,
    pub(crate) day: String,
    pub(crate) legs: Vec<LegRow>,
}

pub(crate) struct DestinationRow {
    pub(crate) destination: String,
    pub(crate) line: String,
}

pub(crate) struct DeliveriesView {
    pub(crate) destinations: Vec<DestinationRow>,
    pub(crate) events: Vec<EventRow>,
    pub(crate) capped: bool,
}

fn latency(leg: &StoredDelivery) -> Option<u64> {
    leg.settled_at.map(|settled| settled.saturating_sub(leg.accepted_at))
}

fn leg_row(leg: &StoredDelivery) -> LegRow {
    let status = leg_word(leg.outcome, leg.deadlettered);
    let latency = latency(leg).map(pns_domain::remind::waited);
    LegRow {
        id: leg.id,
        destination: leg.destination.clone(),
        route: leg.route.clone(),
        brief: format!("{} {}", leg.destination, latency.as_deref().unwrap_or(&status)),
        latency: latency.unwrap_or_else(|| "-".to_string()),
        status,
        retries: leg.retries,
        failing: leg.outcome != LegOutcome::Delivered,
    }
}

pub(crate) fn view(legs: &[StoredDelivery], now: u64) -> DeliveriesView {
    let mut events: Vec<(u64, &StoredDelivery, Vec<&StoredDelivery>)> = Vec::new();
    for leg in legs {
        match events.iter_mut().find(|(event, _, _)| *event == leg.event) {
            Some((_, _, members)) => members.push(leg),
            None => events.push((leg.event, leg, vec![leg])),
        }
    }
    let mut names: Vec<&str> = legs.iter().map(|leg| leg.destination.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    DeliveriesView {
        destinations: names
            .into_iter()
            .map(|name| {
                let mine: Vec<&StoredDelivery> = legs.iter().filter(|leg| leg.destination == name).collect();
                let times: Vec<u64> = mine.iter().filter_map(|leg| latency(leg)).collect();
                DestinationRow {
                    destination: name.to_string(),
                    line: destination_summary(times.len(), mine.len(), median(&times)),
                }
            })
            .collect(),
        events: events
            .into_iter()
            .map(|(_, first, members)| EventRow {
                title: pns_domain::render::title(&first.agent, &first.state, &first.project),
                verdict: event_verdict(&members.iter().map(|leg| (leg.outcome, leg.deadlettered)).collect::<Vec<_>>()),
                clock: pns_adapters::utc_clock(first.accepted_at),
                day: pns_adapters::utc_day(first.accepted_at, now),
                legs: members.into_iter().map(leg_row).collect(),
            })
            .collect(),
        capped: legs.len() >= LIMIT as usize,
    }
}

pub(crate) fn read(sources: &Sources, now: u64) -> Result<DeliveriesView, &'static str> {
    sources
        .store
        .deliveries_since(now.saturating_sub(WINDOW_SECS), LIMIT)
        .map(|legs| view(&legs, now))
        .map_err(|_| UNREADABLE)
}

pub(crate) fn tone(verdict: Verdict) -> Tone {
    match verdict {
        Verdict::Delivered => Tone::Quiet,
        Verdict::Retrying => Tone::Warn,
        Verdict::NotDelivered => Tone::Bad,
    }
}

pub(crate) fn render(paint: Paint, view: &DeliveriesView) -> String {
    if view.events.is_empty() {
        return "pns: nothing was delivered in the last day\n".to_string();
    }
    let mut out = "pns: deliveries in the last 24 hours\n".to_string();
    for row in &view.destinations {
        out.push_str(&format!("  {:<6}  {}\n", row.destination, row.line));
    }
    out.push('\n');
    for event in &view.events {
        let text = format!("{}Z  {:<13}  {}", event.clock, event.verdict.word(), event.title);
        out.push_str(&style::row(paint, tone(event.verdict), "●", 2, &text));
        out.push('\n');
        for leg in &event.legs {
            let line = format!("{:>9}  {:<6}  {:<9}  {:<2}  {}", leg.id, leg.destination, leg.status, leg.latency, leg.route);
            out.push_str(line.trim_end());
            out.push('\n');
        }
    }
    if view.capped {
        out.push_str(&format!("  showing the newest {LIMIT}\n"));
    }
    out
}

/// Counted in EVENTS, the notifications the operator was sent, not legs.
pub(crate) fn summarize(view: &DeliveriesView) -> Summary {
    let events = view.events.len();
    let dead = view.events.iter().filter(|event| event.verdict == Verdict::NotDelivered).count();
    match dead {
        0 => Summary { text: format!("{events} in 24h"), tone: Tone::Quiet },
        dead => Summary { text: format!("{events} in 24h, {dead} not delivered"), tone: Tone::Bad },
    }
}

pub(crate) fn summary(sources: &Sources, now: u64) -> Summary {
    match read(sources, now) {
        Ok(view) => summarize(&view),
        Err(_) => Summary { text: "unreadable".into(), tone: Tone::Quiet },
    }
}

pub(crate) fn deliveries_mode() -> i32 {
    if !crate::arguments_after_subcommand().is_empty() {
        eprintln!("{DELIVERIES_USAGE}");
        return 2;
    }
    let Some(now) = pns_adapters::now_secs() else {
        eprintln!("pns deliveries: this machine has no clock to place a day with");
        return 1;
    };
    match read(&Sources::live(), now) {
        Ok(view) => {
            print!("{}", render(Paint::for_stdout(), &view));
            0
        }
        Err(sentence) => {
            eprintln!("{sentence}");
            1
        }
    }
}

#[cfg(test)]
mod tests;
```

Register
`deliveries` in `invocation.rs`, `SUBCOMMAND_USAGE` and `USAGE` (`  pns deliveries                   every
delivery of the last day`).

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib command_deliveries subcommand_usage`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): add pns deliveries, every leg of the last day"
```

### Task 6.4: The Deliveries page, its route and its index row

**Files:**
- Create: `pns/crates/pns/src/site/deliveries.rs`
- Create: `pns/crates/pns/src/site/deliveries/tests.rs`
- Modify: `pns/crates/pns/src/site.rs`, `site/index.rs`, `site/tests.rs`

**Interfaces:**
- Consumes: `command_deliveries::{read, summary, tone, DeliveriesView, EventRow}`, `shell::row_class`.
- Produces: `pub(super) fn site::deliveries::page(view: Result<&DeliveriesView, &'static str>,
  rendered_at: u64) -> String`.

- [ ] **Step 1: Write the failing tests**

`pns/crates/pns/src/site/deliveries/tests.rs`:

```rust
use super::*;
use crate::command_deliveries::view;
use pns_application::StoredDelivery;
use pns_domain::retry::{LegOutcome, TransportOutcome};

const NOW: u64 = 1_790_139_720;

fn fixture(route: &str) -> Vec<StoredDelivery> {
    let leg = |id, destination: &str, outcome, settled, dead| StoredDelivery {
        id,
        event: 7,
        destination: destination.into(),
        route: route.into(),
        agent: "uu".into(),
        state: "failed".into(),
        project: "uu".into(),
        accepted_at: NOW - 6_120,
        settled_at: settled,
        outcome,
        retries: 0,
        deadlettered: dead,
    };
    vec![
        leg(8005, "hermes", LegOutcome::Failed(TransportOutcome::Status(404)), None, true),
        leg(8004, "banner", LegOutcome::Delivered, Some(NOW - 6_120), false),
    ]
}

#[test]
fn every_value_the_builder_spelled_is_on_the_page() {
    let built = view(&fixture("uu-runs"), NOW);
    let served = page(Ok(&built), NOW);
    for row in &built.destinations {
        assert!(served.contains(&escaped(&row.destination)) && served.contains(&escaped(&row.line)));
    }
    for event in &built.events {
        for value in [&event.title, &event.clock, &event.day] {
            assert!(served.contains(&escaped(value)), "{value:?}");
        }
        assert!(served.contains(event.verdict.word()));
        for leg in &event.legs {
            for value in [&leg.status, &leg.latency, &leg.brief, &leg.route] {
                assert!(served.contains(&escaped(value)), "{value:?}");
            }
        }
    }
}

#[test]
fn a_failing_leg_links_its_failure_record() {
    let served = page(Ok(&view(&fixture("uu-runs"), NOW)), NOW);
    assert!(served.contains("<a href=\"/failures/8005\">8005</a>"), "{served}");
    assert!(!served.contains("/failures/8004"), "{served}");
}

#[test]
fn a_route_is_escaped_once() {
    let served = page(Ok(&view(&fixture("<b>&"), NOW)), NOW);
    assert!(served.contains("&lt;b&gt;&amp;"), "{served}");
}

#[test]
fn nothing_delivered_and_unreadable_are_the_terminal_sentences() {
    assert!(page(Ok(&view(&[], NOW)), NOW).contains("Nothing was delivered in the last day."));
    assert!(page(Err(crate::command_deliveries::UNREADABLE), NOW).contains("the delivery ledger could not be read"));
}
```

In `site/tests.rs`, the route test for `/deliveries` in the same shape as the others.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib site::`
Expected: compile errors, `no variant Deliveries` and `cannot find function page in module deliveries`.

- [ ] **Step 3: Implement the page**

`pns/crates/pns/src/site/deliveries.rs`:

```rust
//! The Deliveries page: `pns deliveries`' view, laid out as the listing card
//! with the per-destination summary in a tinted box above the timeline.

use super::shell::{escaped, footer, page as shell_page, row_class};
use crate::command_deliveries::{DeliveriesView, EventRow, LIMIT, tone};

pub(super) fn page(view: Result<&DeliveriesView, &'static str>, rendered_at: u64) -> String {
    let body = match view {
        Err(sentence) => format!("<p>{}</p>", escaped(sentence)),
        Ok(view) if view.events.is_empty() => "<p>Nothing was delivered in the last day.</p>".to_string(),
        Ok(view) => {
            let summary: String = view
                .destinations
                .iter()
                .map(|row| format!("<dt>{}</dt><dd>{}</dd>", escaped(&row.destination), escaped(&row.line)))
                .collect();
            let capped = if view.capped { format!("<p class=\"fh-muted fh-small\">showing the newest {LIMIT}</p>") } else { String::new() };
            format!("<div class=\"fh-detail fh-sum\"><dl>{summary}</dl></div>\n{}{capped}", entries(view))
        }
    };
    shell_page(
        "pns deliveries",
        &format!(
            "<section id=\"failure-history\"><a class=\"fh-back\" href=\"/\">pns</a>\
             <header class=\"fh-header\"><h2>Deliveries</h2></header>\n{body}\n{}</section>",
            footer(rendered_at)
        ),
    )
}

fn entries(view: &DeliveriesView) -> String {
    let mut out = String::new();
    let mut day = None;
    let last = view.events.len() - 1;
    for (index, event) in view.events.iter().enumerate() {
        if day != Some(&event.day) {
            out.push_str(&format!("<div class=\"fh-day\"><strong>{}</strong></div>\n", escaped(&event.day)));
            day = Some(&event.day);
        }
        out.push_str(&entry(event, index == last));
    }
    out
}

fn entry(event: &EventRow, last: bool) -> String {
    let last = if last { " fh-last" } else { "" };
    let brief = event.legs.iter().map(|leg| escaped(&leg.brief)).collect::<Vec<_>>().join(" · ");
    let legs: String = event
        .legs
        .iter()
        .map(|leg| {
            let id = if leg.failing {
                format!("<a href=\"/failures/{0}\">{0}</a>", leg.id)
            } else {
                leg.id.to_string()
            };
            let route = if leg.route.is_empty() { String::new() } else { format!(" ({})", escaped(&leg.route)) };
            format!(
                "<dt>{id}</dt><dd>{}{route} · {} · {} · {} retries</dd>",
                escaped(&leg.destination),
                escaped(&leg.status),
                escaped(&leg.latency),
                leg.retries
            )
        })
        .collect();
    format!(
        "<article class=\"{class}{last}\"><time class=\"fh-clock\">{clock}</time>\
         <div class=\"fh-content\"><span class=\"fh-dot\" aria-hidden=\"true\"></span>\
         <div class=\"fh-title\"><strong>{title}</strong><span class=\"fh-state\">{chip}</span></div>\
         <div class=\"fh-sub\">{brief}</div>\
         <details><summary>Details</summary><div class=\"fh-detail\"><dl>{legs}</dl></div></details>\
         </div></article>\n",
        class = row_class(tone(event.verdict)),
        clock = escaped(&event.clock),
        title = escaped(&event.title),
        chip = event.verdict.word(),
    )
}

#[cfg(test)]
mod tests;
```

Add `#failure-history .fh-sum { margin:0 0 26px; }` to the CSS constant in `shell.rs`. In `site.rs`,
`Target::Deliveries` on `/deliveries`, and the arm:

```rust
        Some(Target::Deliveries) => {
            let view = crate::command_deliveries::read(sources, now);
            ok(&deliveries::page(view.as_ref().map_err(|sentence| *sentence), now))
        }
```

In `index.rs`, the Deliveries row after Sessions, with `summary: crate::command_deliveries::summary(sources,
now)`, name "Deliveries", blurb "every delivery in the last day".

- [ ] **Step 4: Run the site tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib site::`
Expected: all pass.

- [ ] **Step 5: Run the gates and check file sizes**

```bash
just test-rust; echo "test-rust exit $?"
just lint-check; echo "lint-check exit $?"
just test-unit; echo "test-unit exit $?"
```

Expected: three zero exit codes; every touched `.rs` under 500 total lines.

- [ ] **Step 6: Commit**

```bash
git add pns/crates/pns/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): serve the Deliveries page"
```

---

## Pull request 7: Config as loaded

Branch `feat/pns-page-config`. Worktree:
`herdr worktree create --cwd /Users/stephen/workspaces/Ivy/webdavis/dotfiles --branch feat/pns-page-config --no-focus`

### Task 7.1: One list of secret-bearing keys

**Files:**
- Create: `pns/crates/pns-adapters/src/config/secrets.rs`
- Modify: `pns/crates/pns-adapters/src/config/mod.rs` (`mod secrets; pub use secrets::{SECRET_KEYS,
  SECRET_TABLES};`)
- Modify: `pns/crates/pns/src/bin/pns-config-render.rs` (its private lists become these)
- Test: `pns/crates/pns/src/bin/pns-config-render/tests.rs`

**Interfaces:**
- Produces:

```rust
/// Every key whose value is a credential, spelled as the config file and the
/// values file write it.
pub const SECRET_KEYS: &[&str];
/// The open tables every key of which is secret-bearing.
pub const SECRET_TABLES: &[&str];
```

- [ ] **Step 1: Write the failing test**

Append to `pns-config-render/tests.rs`:

```rust
/// The two secrets the shipped file does not arm are secrets all the same.
#[test]
fn a_literal_webhook_secret_or_calendar_token_is_refused_by_name() {
    for (table, key) in [("github", "webhook_secret"), ("calendar", "refresh_token")] {
        let mut inner = toml::Table::new();
        inner.insert(key.to_string(), toml::Value::String("literal".to_string()));
        let mut outer = toml::Table::new();
        let parent = if table == "calendar" { "quiet" } else { "plugins" };
        outer.insert(table.to_string(), toml::Value::Table(inner));
        let mut values = toml::Table::new();
        values.insert(parent.to_string(), toml::Value::Table(outer));
        let error = refuse_literal_secrets(&values).expect_err("a literal secret is refused");
        assert!(error.contains(key), "{error}");
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --features dev-tools --bin pns-config-render`
Expected: FAIL, `a literal secret is refused` panics for `plugins.github.webhook_secret`.

- [ ] **Step 3: Move and complete the list**

`pns-adapters/src/config/secrets.rs`:

```rust
//! The keys whose values are credentials: the one list the values-file check
//! and the Config page's tests read.

pub const SECRET_KEYS: &[&str] = &[
    "plugins.phone.device_token",
    "plugins.log.bot_token",
    "plugins.lights.bridge_host",
    "plugins.lights.certificate",
    "plugins.lights.api_key",
    "plugins.github.personal_access_token",
    "plugins.github.webhook_secret",
    "plugins.home_presence.api_key",
    "quiet.calendar.client_id",
    "quiet.calendar.client_secret",
    "quiet.calendar.refresh_token",
];

/// A signing key and a channel id are both secrets, and neither table has a
/// fixed set of keys, so the TABLE is named rather than its keys.
pub const SECRET_TABLES: &[&str] = &["plugins.log.keys", "plugins.log.channels"];
```

In `pns-config-render.rs`, delete `SECRET_BEARING_KEYS` and `SECRET_BEARING_TABLES` and read
`pns_adapters::SECRET_KEYS` and `pns_adapters::SECRET_TABLES` in `secret_bearing_keys`. The two doc
paragraphs that explained the constants move onto the new file's items.

- [ ] **Step 4: Run the binary's tests and the shipped-values check**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --features dev-tools --bin pns-config-render`
and `just test-unit`
Expected: all pass; `pns-config-template.test.sh` still renders the committed template byte for byte.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-adapters/src/config pns/crates/pns/src/bin
SKIP_AI_COMMIT=1 git commit -m "refactor(pns): keep one list of secret-bearing config keys, completed"
```

### Task 7.2: The config, disclosed by construction

**Files:**
- Create: `pns/crates/pns-adapters/src/config/disclosure.rs`
- Create: `pns/crates/pns-adapters/src/config/disclosure/sections.rs` (the typed sections)
- Create: `pns/crates/pns-adapters/src/config/disclosure/tests.rs`
- Modify: `pns/crates/pns-adapters/src/config/mod.rs` (`mod disclosure; pub use disclosure::{Disclosure,
  Row, Section, Shown, SHOWN_PLUGIN_KEYS, disclosed};`), `pns-adapters/src/lib.rs` (re-export)
- Modify: `pns/crates/pns-domain/src/recap/window.rs` (`period_text`)

**Interfaces:**
- Consumes: `Config`, `TABLE_KEYS`, `SECRET_KEYS`, `SECRET_TABLES`, `duration::spelled`,
  `profiles::surfaces_line`.
- Produces:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shown { Value(String), Hidden, Unset }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row { pub key: String, pub value: Shown }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section { pub name: String, pub rows: Vec<Row> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disclosure { pub sections: Vec<Section> }
impl Disclosure { pub fn lines(&self) -> Vec<String>; }  // "[routes]", "  default = pns-events", "  api_key = hidden"
pub const SHOWN_PLUGIN_KEYS: &[(&str, &[&str])];         // loaded plugin name, keys whose values show
pub fn disclosed(config: &Config) -> Disclosure;
// pns-domain/src/recap/window.rs
pub fn period_text(periods: &Periods, window: Window) -> String; // "08:00-12:00"
```

- [ ] **Step 1: Write the failing tests**

`disclosure/tests.rs`:

```rust
use super::*;

/// A config holding every schema key of every plugin table, each set to a
/// sentinel naming its own path, plus the calendar's three credentials.
fn planted() -> Config {
    let mut config = crate::parse_config("").expect("an empty file is every default");
    for (table, keys) in crate::TABLE_KEYS {
        let Some(plugin) = table.strip_prefix("plugins.") else { continue };
        if plugin.contains('.') {
            continue;
        }
        let loaded_names: &[&str] = if plugin == "log" { &["hermes", "discord"] } else { &[plugin] };
        for name in loaded_names {
            let mut settings = toml::Table::new();
            for key in *keys {
                settings.insert(key.to_string(), toml::Value::String(format!("SENTINEL:{name}.{key}")));
            }
            let mut open = toml::Table::new();
            open.insert("route-a".to_string(), toml::Value::String(format!("SENTINEL:{name}.open")));
            for open_key in ["keys", "channels", "image_cards"] {
                if keys.contains(&open_key) {
                    settings.insert(open_key.to_string(), toml::Value::Table(open.clone()));
                }
            }
            config.plugins.insert(name.to_string(), crate::PluginEntry { enabled: true, settings });
        }
    }
    config.quiet_calendar.source = crate::CalendarSource::Google(crate::GoogleCalendar {
        calendars: vec!["work".into()],
        client_id: "SENTINEL:calendar.client_id".into(),
        client_secret: "SENTINEL:calendar.client_secret".into(),
        refresh_token: "SENTINEL:calendar.refresh_token".into(),
    });
    config
}

fn shown(name: &str, key: &str) -> bool {
    SHOWN_PLUGIN_KEYS
        .iter()
        .any(|(plugin, keys)| *plugin == name && keys.contains(&key))
}

/// THE SENTINEL TEST: a sentinel reaches the text only when its key is on the
/// allowlist, so every secret, and every key nobody allowlisted, is absent.
#[test]
fn only_an_allowlisted_key_ever_shows_its_value() {
    let text = disclosed(&planted()).lines().join("\n");
    for (name, entry) in &planted().plugins {
        for key in entry.settings.keys() {
            let sentinel = format!("SENTINEL:{name}.{key}");
            assert_eq!(text.contains(&sentinel), shown(name, key), "{name}.{key}: {text}");
        }
        assert!(!text.contains(&format!("SENTINEL:{name}.open")), "an open table's value showed");
    }
    assert!(!text.contains("SENTINEL:calendar"), "a calendar credential showed: {text}");
}

/// THE CONTROL: the test above cannot pass by printing nothing.
#[test]
fn an_allowlisted_key_does_show() {
    let text = disclosed(&planted()).lines().join("\n");
    assert!(text.contains("SENTINEL:phone.ack_deadline"), "{text}");
    assert!(text.contains("route-a = hidden"), "{text}");
}

/// THE CLASSIFICATION TEST: no secret-bearing key is on the allowlist, and
/// every allowlisted key is a real schema key.
#[test]
fn the_allowlist_holds_no_secret_and_no_unknown_key() {
    for (plugin, keys) in SHOWN_PLUGIN_KEYS {
        let schema_table = if matches!(*plugin, "hermes" | "discord") { "log" } else { *plugin };
        let row = crate::TABLE_KEYS
            .iter()
            .find(|(table, _)| *table == format!("plugins.{schema_table}"))
            .unwrap_or_else(|| panic!("no schema row for {plugin}"));
        for key in *keys {
            assert!(row.1.contains(key), "{plugin}.{key} is not a schema key");
            let path = format!("plugins.{schema_table}.{key}");
            assert!(!crate::SECRET_KEYS.contains(&path.as_str()), "{path} is a secret");
            assert!(!crate::SECRET_TABLES.contains(&path.as_str()), "{path} is a secret table");
        }
    }
}

#[test]
fn typed_settings_are_spelled_and_the_calendar_credentials_are_hidden() {
    let lines = disclosed(&planted()).lines();
    assert!(lines.contains(&"[routes]".to_string()));
    assert!(lines.contains(&"  default = pns-events".to_string()), "{lines:#?}");
    assert!(lines.contains(&"  client_secret = hidden".to_string()), "{lines:#?}");
    let unset = disclosed(&crate::parse_config("").unwrap()).lines();
    assert!(!unset.iter().any(|line| line.contains("client_secret = hidden")), "{unset:#?}");
}
```

In the tests of `pns-domain/src/recap/window.rs`:

```rust
#[test]
fn a_period_is_spelled_as_the_config_writes_it() {
    let periods = Periods {
        nightshift: Period { start: 0, end: 480 },
        morning: Period { start: 480, end: 720 },
        afternoon: Period { start: 720, end: 1_080 },
        evening: Period { start: 1_080, end: 1_440 },
    };
    assert_eq!(period_text(&periods, Window::Morning), "08:00-12:00");
    assert_eq!(period_text(&periods, Window::Evening), "18:00-24:00");
}
```

The `"  default = pns-events"` expectation holds because the shipped `Routes::default()` names
`pns-events`; if the default route's name differs, pin the real one.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters config::disclosure` and
`cargo test --locked --manifest-path pns/Cargo.toml -p pns-domain recap::window`
Expected: compile errors, `file not found for module disclosure` and `cannot find function period_text`.

- [ ] **Step 3: Implement `period_text`**

In `recap/window.rs`, beside `Periods::of`:

```rust
/// One configured period as the config writes it, `HH:MM-HH:MM`.
pub fn period_text(periods: &Periods, window: Window) -> String {
    let period = periods.of(window).unwrap_or(Period { start: 0, end: 0 });
    let clock = |minutes: u32| format!("{:02}:{:02}", minutes / 60, minutes % 60);
    format!("{}-{}", clock(period.start), clock(period.end))
}
```

- [ ] **Step 4: Implement the disclosure**

`pns-adapters/src/config/disclosure.rs`:

```rust
//! The resolved config as a page may show it: typed settings spelled field by
//! field, plugin settings by allowlist, every other value hidden.
//!
//! NOTHING HERE READS THE FILE'S TEXT OR `Debug`. A value reaches a line only
//! when this module names its field or its key allowlists it, so a key the
//! schema gains tomorrow is hidden until somebody decides it is not a secret.

mod sections;

use super::Config;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shown {
    Value(String),
    Hidden,
    Unset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub key: String,
    pub value: Shown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub rows: Vec<Row>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disclosure {
    pub sections: Vec<Section>,
}

impl Disclosure {
    pub fn lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for section in &self.sections {
            lines.push(format!("[{}]", section.name));
            for row in &section.rows {
                let value = match &row.value {
                    Shown::Value(text) => text.as_str(),
                    Shown::Hidden => "hidden",
                    Shown::Unset => "unset",
                };
                lines.push(format!("  {} = {value}", row.key));
            }
        }
        lines
    }
}

/// The plugin keys whose values show, by LOADED plugin name (`[plugins.log]`
/// loads as `hermes` or `discord`). Every key not here renders `hidden`.
pub const SHOWN_PLUGIN_KEYS: &[(&str, &[&str])] = &[
    ("banner", &["click_command", "click_type", "terminal_bundle_id", "type"]),
    ("discord", &["type"]),
    ("github", &["poll_interval", "webhook_port"]),
    ("hermes", &["type"]),
    ("home_presence", &["alert_route", "type"]),
    ("lights", &["type"]),
    ("phone", &["ack_deadline", "card_while_watching", "marker_file", "type"]),
    (
        "presence",
        &["desk_input_max_age", "desk_room", "excluded_rooms", "poll_interval", "reading_max_age", "rooms", "type"],
    ),
];

pub fn disclosed(config: &Config) -> Disclosure {
    let mut sections = sections::typed(config);
    for (name, entry) in &config.plugins {
        let mut rows = vec![Row { key: "enabled".into(), value: Shown::Value(entry.enabled.to_string()) }];
        let allowed = SHOWN_PLUGIN_KEYS
            .iter()
            .find(|(plugin, _)| *plugin == name.as_str())
            .map_or(&[][..], |(_, keys)| *keys);
        for (key, value) in &entry.settings {
            if key == "enabled" {
                continue;
            }
            match value {
                toml::Value::Table(open) => rows.extend(open.keys().map(|inner| Row {
                    key: format!("{key}.{inner}"),
                    value: Shown::Hidden,
                })),
                value if allowed.contains(&key.as_str()) => {
                    rows.push(Row { key: key.clone(), value: Shown::Value(spelled(value)) })
                }
                _ => rows.push(Row { key: key.clone(), value: Shown::Hidden }),
            }
        }
        sections.push(Section { name: format!("plugins.{name}"), rows });
    }
    Disclosure { sections }
}

/// A TOML value as the config would write it, without the crate's `display`
/// feature: strings bare, arrays bracketed.
fn spelled(value: &toml::Value) -> String {
    match value {
        toml::Value::String(text) => text.clone(),
        toml::Value::Integer(number) => number.to_string(),
        toml::Value::Float(number) => number.to_string(),
        toml::Value::Boolean(flag) => flag.to_string(),
        toml::Value::Datetime(moment) => moment.to_string(),
        toml::Value::Array(items) => format!("[{}]", items.iter().map(spelled).collect::<Vec<_>>().join(", ")),
        toml::Value::Table(_) => "hidden".to_string(),
    }
}

#[cfg(test)]
mod tests;
```

`disclosure/sections.rs` spells the typed settings, each field named:

```rust
use super::{Row, Section, Shown};
use crate::config::{CalendarSource, Config};
use pns_domain::recap::window::{Window, period_text};

fn value(key: &str, text: impl Into<String>) -> Row {
    Row { key: key.into(), value: Shown::Value(text.into()) }
}

fn duration(secs: u64) -> String {
    pns_domain::duration::spelled(std::time::Duration::from_secs(secs))
}

fn secret(key: &str, text: &str) -> Row {
    Row { key: key.into(), value: if text.is_empty() { Shown::Unset } else { Shown::Hidden } }
}

fn optional(key: &str, text: Option<&str>) -> Row {
    Row { key: key.into(), value: text.map_or(Shown::Unset, |text| Shown::Value(text.into())) }
}

pub(super) fn typed(config: &Config) -> Vec<Section> {
    let section = |name: &str, rows: Vec<Row>| Section { name: name.into(), rows };
    let recap = &config.recap;
    let calendar = &config.quiet_calendar;
    let mut calendar_rows = vec![
        value("enabled", calendar.enabled.to_string()),
        value("poll_interval", duration(calendar.poll_secs)),
        value("deadline", duration(calendar.deadline_secs)),
    ];
    match &calendar.source {
        CalendarSource::Command(argv) => calendar_rows.push(value("command", argv.join(" "))),
        CalendarSource::Google(google) => {
            calendar_rows.push(value("type", "google"));
            calendar_rows.push(value("calendars", google.calendars.join(", ")));
            calendar_rows.push(secret("client_id", &google.client_id));
            calendar_rows.push(secret("client_secret", &google.client_secret));
            calendar_rows.push(secret("refresh_token", &google.refresh_token));
        }
    }
    vec![
        section("routes", vec![
            value("default", config.routes.default_route()),
            value("urgent", config.routes.urgent_route()),
        ]),
        section(
            "delivery_class",
            config
                .delivery_classes
                .iter()
                .map(|(name, class)| value(name, format!("route {}, bypass_mute {}", class.route, class.bypass_mute)))
                .collect(),
        ),
        section("recap", vec![
            value("nightshift", period_text(&recap.periods, Window::Nightshift)),
            value("morning", period_text(&recap.periods, Window::Morning)),
            value("afternoon", period_text(&recap.periods, Window::Afternoon)),
            value("evening", period_text(&recap.periods, Window::Evening)),
            value("retain", pns_domain::duration::spelled(recap.retain)),
            value("replay_card", recap.replay_card.to_string()),
            value("summarizer", recap.summarizer.kind.word()),
        ]),
        section("stale", vec![
            value("enabled", config.stale_enabled.to_string()),
            value("escalate_after", duration(config.stale_escalate_after_secs)),
            optional("route", config.stale_route.as_deref()),
        ]),
        section("retry", vec![
            value("max_retries", config.retry_limits.max_retries.to_string()),
            value("event_max_age", duration(config.retry_limits.event_max_age_secs)),
            value("step", duration(config.retry_backoff.step_secs)),
        ]),
        section("failures", vec![
            value("serve", config.failures.page_enabled.to_string()),
            value("port", config.failures.page_port.to_string()),
        ]),
        section("storage", vec![value(
            "busy_deadline",
            pns_domain::duration::spelled(config.storage_busy_deadline),
        )]),
        section("focus", vec![
            value("enabled", config.focus_enabled.to_string()),
            value("modes", config.focus_modes.join(", ")),
        ]),
        section("remind", vec![value("delay", duration(config.remind_delay_secs))]),
        section("gateway", vec![
            value("enabled", config.gateway_enabled.to_string()),
            optional("service", config.gateway_service.as_deref()),
            value("remote_deadline", duration(config.remote_deadline_secs)),
        ]),
        section(
            "profiles",
            config
                .profiles
                .iter()
                .map(|(name, profile)| value(name, pns_domain::profiles::surfaces_line(profile)))
                .chain([
                    value("rules", config.profile_rules.len().to_string()),
                    value("location_poll", duration(config.location_poll_secs)),
                    // Names only: a location's value is the network fingerprint it matches.
                    value("locations", config.profile_locations.keys().cloned().collect::<Vec<_>>().join(", ")),
                ])
                .collect(),
        ),
        section("quiet.calendar", calendar_rows),
        section("lights", vec![
            value("enabled", config.lights.is_some().to_string()),
            optional("dim_window", config.lights.as_deref().and_then(|lights| lights.dim_window.as_deref())),
        ]),
    ]
}
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters config::disclosure` and
`cargo test --locked --manifest-path pns/Cargo.toml -p pns-domain recap::window`
Expected: all pass. If the sentinel test names a key that shows and should not, the fix is the
allowlist, never the test.

- [ ] **Step 6: Commit**

```bash
git add pns/crates/pns-adapters/src pns/crates/pns-domain/src/recap/window.rs
SKIP_AI_COMMIT=1 git commit -m "feat(pns): disclose the loaded config with every secret hidden by construction"
```

### Task 7.3: `pns config show`, and the shipped config checked against the disclosure

**Files:**
- Create: `pns/crates/pns/src/command_config.rs`
- Create: `pns/crates/pns/src/command_config/tests.rs`
- Modify: `pns/crates/pns/src/lib.rs`, `invocation.rs`, `subcommand_usage.rs`, `legacy/usage.rs`
- Modify: `pns/crates/pns/src/bin/pns-config-render.rs` (`check` refuses a disclosed vault placeholder)
- Test: `pns/crates/pns/src/bin/pns-config-render/tests.rs`

**Interfaces:**
- Consumes: `disclosed`, `Disclosure::lines`, `load_config`, `config_path`, `parse_config`.
- Produces:

```rust
pub(crate) const CONFIG_USAGE: &str;
pub(crate) const DID_NOT_LOAD: &str =
    "the config did not load; run `pns doctor` at the terminal for the reason";
pub(crate) enum Loaded { Read(Disclosure), Missing(Disclosure), Failed(String) }
pub(crate) struct ConfigView { pub(crate) path: String, pub(crate) loaded: Loaded }
pub(crate) fn read(sources: &Sources) -> ConfigView;
pub(crate) fn render(view: &ConfigView) -> String;         // the terminal form; prints a load error in full
pub(crate) fn summary(sources: &Sources) -> Summary;       // "loaded", "missing", "did not load"
pub(crate) fn config_mode() -> i32;
// pns-config-render.rs
fn leaked(lines: &[String]) -> Option<&String>;
```

- [ ] **Step 1: Write the failing tests**

`pns/crates/pns/src/command_config/tests.rs`:

```rust
use super::*;

fn view(loaded: Loaded) -> ConfigView {
    ConfigView { path: "~/.config/pns/config.toml".into(), loaded }
}

#[test]
fn the_terminal_form_names_the_path_and_prints_the_disclosure() {
    let disclosure = pns_adapters::disclosed(&pns_adapters::parse_config("").unwrap());
    let printed = render(&view(Loaded::Read(disclosure.clone())));
    assert!(printed.starts_with("pns: config as loaded from ~/.config/pns/config.toml, secrets hidden by key\n"));
    for line in disclosure.lines() {
        assert!(printed.contains(&line), "{line}");
    }
}

#[test]
fn a_missing_file_says_the_defaults_are_running() {
    let disclosure = pns_adapters::disclosed(&pns_adapters::parse_config("").unwrap());
    assert!(render(&view(Loaded::Missing(disclosure))).contains("no config file; pns is running on its defaults"));
}

#[test]
fn the_terminal_prints_a_load_error_in_full() {
    let printed = render(&view(Loaded::Failed("line 3: expected a table".into())));
    assert!(printed.contains("line 3: expected a table"), "{printed}");
}

#[test]
fn the_home_directory_is_written_as_a_tilde() {
    assert_eq!(tilde("/Users/someone/.config/pns/config.toml", "/Users/someone"), "~/.config/pns/config.toml");
    assert_eq!(tilde("/etc/pns.toml", "/Users/someone"), "/etc/pns.toml");
}

#[test]
fn the_summary_is_the_load_state() {
    let defaults = || pns_adapters::disclosed(&pns_adapters::parse_config("").unwrap());
    assert_eq!(summarize(&view(Loaded::Read(defaults()))).text, "loaded");
    assert_eq!(summarize(&view(Loaded::Missing(defaults()))).text, "missing");
    assert_eq!(summarize(&view(Loaded::Failed(String::new()))).text, "did not load");
}
```

Append to `pns-config-render/tests.rs`, and add `leaked` to its `use super::{lookup,
refuse_literal_secrets};` line:

```rust
#[test]
fn a_disclosed_vault_placeholder_is_named() {
    let lines = vec!["[plugins.phone]".to_string(), "  device_token = from-the-vault:Phone:Password".to_string()];
    assert_eq!(leaked(&lines), Some(&lines[1]));
    assert_eq!(leaked(&["  device_token = hidden".to_string()]), None);
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --features dev-tools --lib --bin pns-config-render command_config leaked`
Expected: compile errors, `file not found for module command_config` and `cannot find function leaked`.

- [ ] **Step 3: Implement the command**

`pns/crates/pns/src/command_config.rs`:

```rust
//! `pns config show`: the config pns resolved, with every secret hidden by
//! key. The site's `/config` page lays out the same `ConfigView`, and prints
//! no load error, whose text can quote a secret's line.

use crate::style::Tone;
use crate::view::{Sources, Summary};
use pns_adapters::Disclosure;

pub(crate) const CONFIG_USAGE: &str = "\
pns: usage:
  pns config show                  the loaded config, secrets hidden by key
";

pub(crate) const DID_NOT_LOAD: &str =
    "the config did not load; run `pns doctor` at the terminal for the reason";

pub(crate) enum Loaded {
    Read(Disclosure),
    Missing(Disclosure),
    Failed(String),
}

pub(crate) struct ConfigView {
    pub(crate) path: String,
    pub(crate) loaded: Loaded,
}

pub(crate) fn tilde(path: &str, home: &str) -> String {
    match path.strip_prefix(home).filter(|_| !home.is_empty()) {
        Some(rest) => format!("~{rest}"),
        None => path.to_string(),
    }
}

pub(crate) fn read(sources: &Sources) -> ConfigView {
    let path = pns_adapters::config_path(&sources.home);
    let defaults = || pns_adapters::disclosed(&pns_adapters::parse_config("").expect("an empty file parses"));
    let loaded = match pns_adapters::load_config(&path) {
        Ok(pns_adapters::LoadOutcome::Loaded(config)) => Loaded::Read(pns_adapters::disclosed(&config)),
        Ok(pns_adapters::LoadOutcome::Missing) => Loaded::Missing(defaults()),
        Err(error) => Loaded::Failed(error.detail().to_string()),
    };
    ConfigView { path: tilde(&path.to_string_lossy(), &sources.home), loaded }
}

pub(crate) fn render(view: &ConfigView) -> String {
    let (lead, disclosure) = match &view.loaded {
        Loaded::Read(disclosure) => (format!("pns: config as loaded from {}, secrets hidden by key", view.path), disclosure),
        Loaded::Missing(disclosure) => ("pns: no config file; pns is running on its defaults".to_string(), disclosure),
        Loaded::Failed(detail) => return format!("pns: config error ({detail})\n"),
    };
    let mut out = format!("{lead}\n");
    for line in disclosure.lines() {
        out.push_str(&line);
        out.push('\n');
    }
    out
}

pub(crate) fn summarize(view: &ConfigView) -> Summary {
    match view.loaded {
        Loaded::Read(_) => Summary { text: "loaded".into(), tone: Tone::Quiet },
        Loaded::Missing(_) => Summary { text: "missing".into(), tone: Tone::Warn },
        Loaded::Failed(_) => Summary { text: "did not load".into(), tone: Tone::Bad },
    }
}

pub(crate) fn summary(sources: &Sources) -> Summary {
    summarize(&read(sources))
}

pub(crate) fn config_mode() -> i32 {
    match crate::arguments_after_subcommand().as_slice() {
        [verb] if verb == "show" => {
            let view = read(&Sources::live());
            print!("{}", render(&view));
            i32::from(matches!(view.loaded, Loaded::Failed(_)))
        }
        _ => {
            eprintln!("{CONFIG_USAGE}");
            2
        }
    }
}

#[cfg(test)]
mod tests;
```

`parse_config("")` cannot fail (an empty file is every default, which the disclosure tests already rely
on), so the one `expect` is a compiled-in invariant. Register `config` in `invocation.rs`,
`("config", CONFIG_USAGE)` and `("config show", CONFIG_USAGE)` in `SUBCOMMAND_USAGE`, and `  pns config
show                  the loaded config, secrets hidden` in `USAGE`.

- [ ] **Step 4: Extend the shipped-config check**

In `pns-config-render.rs`:

```rust
/// A disclosed line carrying a vault placeholder is a secret the Config page
/// would show.
fn leaked(lines: &[String]) -> Option<&String> {
    lines.iter().find(|line| line.contains("from-the-vault:"))
}
```

and in `check`, after the snapshot comparison:

```rust
    let lines = pns_adapters::disclosed(&config).lines();
    if let Some(line) = leaked(&lines) {
        return Err(format!("the config page would show a vault secret: {line}"));
    }
```

`test/unit/pns-config-template.test.sh` already runs `--check` over the committed values file, so the
shipped config is now checked against the disclosure on every `just test-unit`.

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --features dev-tools --lib --bin pns-config-render`
and `just test-unit`
Expected: all pass, the committed values' check included.

- [ ] **Step 6: Commit**

```bash
git add pns/crates/pns/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): add pns config show, and check the shipped config hides every secret"
```

### Task 7.4: The Config as loaded page, its route and its index row

**Files:**
- Create: `pns/crates/pns/src/site/config.rs`
- Create: `pns/crates/pns/src/site/config/tests.rs`
- Modify: `pns/crates/pns/src/site.rs`, `site/index.rs`, `site/tests.rs`, `site/shell.rs` (`.fr-hidden`)

**Interfaces:**
- Consumes: `command_config::{read, summary, ConfigView, Loaded, DID_NOT_LOAD}`, `Shown`.
- Produces: `pub(super) fn site::config::page(view: &ConfigView, rendered_at: u64) -> String`.

- [ ] **Step 1: Write the failing tests**

`pns/crates/pns/src/site/config/tests.rs`:

```rust
use super::*;
use crate::command_config::{ConfigView, Loaded};
use pns_adapters::{Disclosure, Row, Section, Shown};

const NOW: u64 = 1_790_139_720;

fn view(loaded: Loaded) -> ConfigView {
    ConfigView { path: "~/.config/pns/config.toml".into(), loaded }
}

fn disclosure(value: &str) -> Disclosure {
    Disclosure {
        sections: vec![
            Section { name: "routes".into(), rows: vec![Row { key: "default".into(), value: Shown::Value(value.into()) }] },
            Section {
                name: "plugins.phone".into(),
                rows: vec![
                    Row { key: "device_token".into(), value: Shown::Hidden },
                    Row { key: "url".into(), value: Shown::Unset },
                ],
            },
        ],
    }
}

#[test]
fn every_disclosed_row_is_on_the_page() {
    let served = page(&view(Loaded::Read(disclosure("pns-events"))), NOW);
    for text in ["routes", "default", "pns-events", "plugins.phone", "device_token", "hidden", "unset", "~/.config/pns/config.toml"] {
        assert!(served.contains(text), "{text:?}");
    }
    assert!(served.contains("Loaded"));
}

#[test]
fn a_value_is_escaped_once() {
    let served = page(&view(Loaded::Read(disclosure("<b>&</b>"))), NOW);
    assert!(served.contains("&lt;b&gt;&amp;&lt;/b&gt;"), "{served}");
}

/// A LOAD ERROR NEVER REACHES THE PAGE: its text can quote the line it
/// failed on, and that line can be a secret.
#[test]
fn a_load_error_shows_only_the_fixed_sentence() {
    let served = page(&view(Loaded::Failed("api_key = \"SENTINEL-SECRET".into())), NOW);
    assert!(!served.contains("SENTINEL-SECRET"), "{served}");
    assert!(served.contains(crate::command_config::DID_NOT_LOAD));
    assert!(served.contains("fr-status fr-red"));
}

#[test]
fn a_missing_file_says_the_defaults_are_running() {
    let served = page(&view(Loaded::Missing(disclosure("pns-events"))), NOW);
    assert!(served.contains("No config file; pns is running on its defaults."), "{served}");
}

/// THE PAGE OVER A PLANTED CONFIG: the disclosure's sentinel test, run
/// through the page renderer, with its control.
#[test]
fn no_planted_secret_reaches_the_page() {
    let mut config = pns_adapters::parse_config("").expect("an empty file is every default");
    let mut settings = toml::Table::new();
    settings.insert("device_token".into(), toml::Value::String("SENTINEL-SECRET".into()));
    settings.insert("ack_deadline".into(), toml::Value::String("SENTINEL-SHOWN".into()));
    config.plugins.insert("phone".into(), pns_adapters::PluginEntry { enabled: true, settings });
    let served = page(&view(Loaded::Read(pns_adapters::disclosed(&config))), NOW);
    assert!(!served.contains("SENTINEL-SECRET"), "{served}");
    assert!(served.contains("SENTINEL-SHOWN"), "{served}");
}
```

In `site/tests.rs`, the route test for `/config` in the same shape as the others.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib site::`
Expected: compile errors, `no variant Config` and `cannot find function page in module config`.

- [ ] **Step 3: Implement the page**

`pns/crates/pns/src/site/config.rs`:

```rust
//! The Config as loaded page: `pns config show`'s view, laid out as the
//! record card, one folded block per section.

use super::shell::{escaped, footer, page as shell_page};
use crate::command_config::{ConfigView, DID_NOT_LOAD, Loaded};
use pns_adapters::{Disclosure, Shown};

pub(super) fn page(view: &ConfigView, rendered_at: u64) -> String {
    let (chip, class, meaning, body) = match &view.loaded {
        Loaded::Read(disclosure) => (
            "Loaded",
            "fr-status fr-quiet",
            format!("{}, secrets hidden by key", escaped(&view.path)),
            sections(disclosure),
        ),
        Loaded::Missing(disclosure) => (
            "Missing",
            "fr-status",
            "No config file; pns is running on its defaults.".to_string(),
            sections(disclosure),
        ),
        Loaded::Failed(_) => ("Did not load", "fr-status fr-red", escaped(DID_NOT_LOAD), String::new()),
    };
    let card = format!(
        "<article id=\"failure-record\"><a class=\"fr-back\" href=\"/\">pns</a>\
         <header><div class=\"fr-heading\"><h2>Config as loaded</h2><span class=\"{class}\">{chip}</span></div>\
         <p class=\"fr-meaning\">{meaning}</p></header>\n{body}\n{}</article>",
        footer(rendered_at)
    );
    shell_page("pns config", &card)
}

fn sections(disclosure: &Disclosure) -> String {
    disclosure
        .sections
        .iter()
        .enumerate()
        .map(|(index, section)| {
            let open = if index == 0 { " open" } else { "" };
            let rows: String = section
                .rows
                .iter()
                .map(|row| {
                    let value = match &row.value {
                        Shown::Value(text) => escaped(text),
                        Shown::Hidden => "<span class=\"fr-hidden\">hidden</span>".to_string(),
                        Shown::Unset => "<span class=\"fr-hidden\">unset</span>".to_string(),
                    };
                    format!("<dt>{}</dt><dd>{value}</dd>", escaped(&row.key))
                })
                .collect();
            format!(
                "<details{open}><summary>{}</summary><dl class=\"fr-fields\">{rows}</dl></details>",
                escaped(&section.name)
            )
        })
        .collect()
}

#[cfg(test)]
mod tests;
```

Add `#failure-record .fr-hidden { color:var(--fr-muted); font-style:italic; }` to the CSS constant. In
`site.rs`, `Target::Config` on `/config`, and the arm `Some(Target::Config) =>
ok(&config::page(&crate::command_config::read(sources), now)),`. In `index.rs`, the Config row last, with
`summary: crate::command_config::summary(sources)`, name "Config as loaded", blurb "the settings pns is
running with, secrets hidden".

- [ ] **Step 4: Run the site tests to see them pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --lib site::`
Expected: all pass.

- [ ] **Step 5: Run the gates and check file sizes**

```bash
just test-rust; echo "test-rust exit $?"
just lint-check; echo "lint-check exit $?"
just test-unit; echo "test-unit exit $?"
```

Expected: three zero exit codes; every touched `.rs` under 500 total lines.

- [ ] **Step 6: Commit**

```bash
git add pns/crates/pns/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): serve the Config as loaded page, secrets hidden by construction"
```

---

## After the seventh pull request

The index lists eight rows in the spec's order (D9). The operator's own apply then checks what no test
can: each route answers at `127.0.0.1:8646` through moshi's browser preview, and opening `/health`
produced no banner, no phone card, no Discord post and no lamp pulse.
