# Rust neotest workflow, and the Java and Elixir adapter disposition, 2026-09-14

Ledger task: "Preserve B95's Rust neotest discovery and duplicate-client follow-up"
(`docs/remaining-work.md:1675`, section "Neovim review follow-up"). Backlog source: B95 in
`~/.claude/pipeline/backlog-consolidated-2026-09-02.md:785`.

Nothing was installed, nothing in the repository was edited, and no operator state was changed. Every
probe ran under `nvim --headless --clean -u NONE` against a throwaway cargo crate and a throwaway clone
of rustaceanvim in the session scratchpad. Measurement wall clock was 2026-09-13 22:38 to 22:52 local
(Mountain Daylight Time, UTC-6), which is 2026-09-14 in Coordinated Universal Time.

## The question

Three questions, in the order the ledger asks them.

1. Does Rust have a working neotest workflow, and can Neovim language coverage be called complete without
   one? Spec 5.3 of `docs/superpowers/specs/2026-09-01-nvim-overhaul-design-v4.md` assigns the Rust row
   to `mrcjkb/rustaceanvim`'s own neotest adapter. B95 recorded two blockers against that row and neither
   was root-caused.
1. What is the disposition of Java and Elixir, measured against plan task 46b step 2
   (`docs/superpowers/plans/2026-09-01-nvim-overhaul-plan.md:1348`), which says "An adapter that cannot
   be shown working is removed from this PR rather than shipped unproven, and the body says which and
   why"? Their absence from the configured adapter list is not by itself an instruction to add them.
1. Is B95's duplicate-client blocker still live against the configuration on `main` today?

## Verdict

**Rust: the intended workflow is verified as WORKING, and the language row is NOT complete. Adopt
`mrcjkb/rustaceanvim` for it, conditional on one operator decision.** The adapter discovered the
namespace and both tests on a scratch two-test crate on this machine. B95's blocker (b) is root-caused
and is a readiness race with rust-analyzer, not a broken adapter. Blocker (a) is still live and its fix
is the option mason-lspconfig documents for exactly this case. What is left is a choice between three
readiness policies, which is the operator's, because two of the three ship a known papercut and the third
adds code to `lua/plugins/neotest.lua`.

**Java: REJECT for now, withheld under plan 46b step 2.** Its step-2 verification, "a JUnit 5 scratch
project", cannot be attempted at all. This machine has no Java runtime, no Maven, no Gradle, no
JDTLS-based language server, and no `java` tree-sitter parser, and `rcasia/neotest-java` requires four of
those five. Nothing in the operator's workspaces is a Java project.

**Java's row in spec 5.3 understates the cost and should be amended, not merely unticked.** The plan line
reads as one filetype-lazy pin. The adapter's own prerequisites are a Java Development Kit (JDK), Maven
or Gradle, `nvim-jdtls` or `nvim-java`, and the `java` parser.

**Elixir: REJECT for now, withheld under plan 46b step 2.** Its step-2 verification, "`mix test` in a
scratch app", cannot be attempted either: there is no `elixir`, no `mix` and no `erl` on this machine and
none is declared in `.chezmoidata/system_packages_autoinstall.yaml`. `jfpedroza/neotest-elixir`'s newest
commit is `a242aeb`, dated 2025-01-19, which is twenty months old as of this reading.

**Separate live defect found, not part of B95: the deployed rust-analyzer is nine months stale and its
`cargo metadata` call fails on every Rust buffer today.** This degrades ordinary Rust editing, not only
neotest, and it deserves its own task. Details in finding 6.

## Assumptions made in the operator's place

Each is a choice this document made rather than leaving the work undone. The alternative is stated so the
operator can reverse it.

1. **"Rust's intended workflow" was read as spec 5.3's table row**, that is, rustaceanvim's own neotest
   adapter reached from `<leader>tt`. *Alternative:* read it as "any way to run a Rust test from Neovim",
   in which case finding 9 already satisfies it through overseer and the Rust row closes with no plugin,
   no pin and no decision.
1. **Measurements used Mason's rust-analyzer**, because that is the copy Neovim actually resolves inside
   this configuration (finding 6). *Alternative:* add the rustup component first, giving a
   toolchain-matched server, which very likely removes the `cargo metadata` failure and shifts every
   timing in finding 3.
1. **The scratch project was a single crate with two tests in one `mod tests`**, which is the shape plan
   46 step 2 asks for. *Alternative:* measure against one of this repository's four real cargo
   workspaces, whose workspace load is far slower, which would widen the unready window rather than
   narrow it. That makes the finding worse, not better, so the smaller project is the conservative
   choice.
1. **A pre-ready answer from rust-analyzer was treated as a readiness problem, not as an adapter bug in
   itself.** *Alternative:* treat the silent empty tree and the raised assertion as two upstream defects
   and open issues on rustaceanvim. Finding 3 gives the exact reproduction either way; the raised
   assertion in particular looks like a genuine upstream bug.
1. **Plan 46b step 2's "cannot be shown working" was read as including "cannot be attempted, because the
   toolchain is absent and undeclared".** *Alternative:* read it as requiring the toolchain be installed
   and declared first and only then verified, which would mean adding a JDK, Maven or Gradle, `jdtls`,
   the `java` parser, Elixir and Erlang to the machine before any disposition could be written.
1. **Nothing was installed or updated.** *Alternative:* the operator updates Mason's rust-analyzer and
   asks for the finding-3 probe to be re-run, which is one command and roughly four minutes.

## What was checked, and how

Versions on dresden at the time of the measurement:

| Thing                      | Value                                                                  |
| -------------------------- | ---------------------------------------------------------------------- |
| Neovim                     | `v0.12.5`, LuaJIT 2.1.1788856981                                       |
| rustc                      | `1.98.1 (48a229cea 2026-09-01)`, nightly is the active rustup default  |
| cargo                      | `1.98.1 (797e8a9bc 2026-08-05)`, a rustup shim at `~/.cargo/bin/cargo` |
| rust-analyzer (used)       | `0.3.2727-standalone (9d58a93602 2025-12-21)`, Mason's copy            |
| rust-analyzer (rustup)     | the `~/.cargo/bin/rust-analyzer` shim FAILS, component absent          |
| rustaceanvim               | clone at `a8c4f9af91fddb9156f1a3f7d0806de74e62488e`, 2026-09-14        |
| neotest                    | pinned `27bf921498043f7ecd821d6db68d05de244bbd02`, already deployed    |
| `rcasia/neotest-java`      | `71354dd2c3f59bcc2301528dfccbbfa2b85bb870`, 2026-09-05                 |
| `jfpedroza/neotest-elixir` | `a242aebeaa6997c1c149138ff77f6cacbe33b6fc`, 2025-01-19                 |
| `neotest-vim-test`         | `75c4228882ae4883b11bfce9b8383e637eb44192`, the Zig row, not this task |

Sources read, in order of trust:

- Local source: `dot_config/nvim/lua/plugins/neotest.lua`, `dot_config/nvim/lua/plugins/lsp.lua`,
  `dot_config/nvim/lua/plugins/treesitter.lua`, `dot_config/nvim/lazy-lock.json`, and the installed
  copies under `~/.local/share/nvim/lazy/` of neotest, nvim-nio, mason-lspconfig, mason.nvim,
  overseer.nvim and the five configured adapters.
- Local source of the candidate: the rustaceanvim clone, specifically
  `lua/rustaceanvim/neotest/init.lua`, `lua/rustaceanvim/neotest/trans.lua`,
  `lua/rustaceanvim/rust_analyzer.lua`, `lua/rustaceanvim/health.lua` and `README.md`.
- Installed help text: `~/.local/share/nvim/lazy/mason-lspconfig.nvim/doc/mason-lspconfig.txt`, and
  `cargo metadata --help`, `rustup component list`.
- Repository documents: spec 5.3 of `2026-09-01-nvim-overhaul-design-v4.md`, plan tasks 46 and 46b of
  `2026-09-01-nvim-overhaul-plan.md`, and the B95 entry in the consolidated backlog.
- Upstream, fetched: `crates/project-model/src/cargo_workspace.rs` on rust-analyzer `master`, for finding
  6 only.

Four probes were written and run. They are kept in this session's scratchpad, under `scratchpad/b95/`,
and are reproducible as long as that scratchpad exists.

1. `raw_runnables.lua`: a bare `vim.lsp.start` with no rustaceanvim and no neotest, sending
   `experimental/runnables` by hand. It answers "does rust-analyzer produce test runnables for this file
   at all".
1. `timeline.lua`: the same request every 400 ms for ten seconds after `client.initialized` goes true,
   printing each answer's runnable labels. It answers "when does it produce them".
1. `adapter_internals.lua`: rustaceanvim's own server start, then its own
   `rust_analyzer.get_client_for_file` and its own nio request, at five fixed offsets. It answers "what
   does the adapter itself see, and through which client".
1. `adapter_long.lua` and `adapter_windows.lua`: `require('rustaceanvim.neotest').discover_positions`
   called inside `nio.run` at fixed offsets, printing the resulting position tree or the error. It
   answers "what tree does neotest get".

The scratch crate is one `lib.rs` with `add`, and a `mod tests` holding `adds_two` and `adds_three`.
`cargo test` in it reports `2 passed`.

## Findings

### 1. There is no Rust adapter, and no configured adapter claims a `.rs` file

`dot_config/nvim/lua/plugins/neotest.lua` passes eight adapters to `neotest.setup`: `neotest-python`,
`neotest-golang`, the three JavaScript ones, `neotest-bashunit`, `neotest-busted` and
`neotest-swift-testing`. `lazy-lock.json` lists the same nine neotest rows and no rustaceanvim. Rust
appears in the configuration only as `"rust_analyzer"` in mason-lspconfig's `ensure_installed`
(`lsp.lua:204`), `"codelldb"` in mason-tool-installer (`lsp.lua:243`), and the `"rust"` tree-sitter
parser (`treesitter.lua:204`).

Every configured adapter's file predicate was read. None can accept a `.rs` path: `neotest-golang`
requires `_test.go`, `neotest-busted` requires `_spec.lua`, `neotest-swift-testing` requires `Test.swift`
or `Tests.swift`, `neotest-bashunit` asks its own `parse.is_test_file`, the three JavaScript adapters are
routed through this repository's own `owner_of`, which returns nil for anything outside its extension
table, and `neotest-python` uses its base Python predicate.

So `<leader>tt` in a Rust buffer today reaches no runner. Rust language coverage is not complete.

### 2. The intended adapter works on this machine, once rust-analyzer has loaded the workspace

`adapter_long.lua`, polling `discover_positions` every three seconds after `client.initialized`:

```
t=  3.1s 4 nodes -> lib.rs[file] tests[namespace] adds_two[test] adds_three[test]
t= 24.0s 4 nodes -> lib.rs[file] tests[namespace] adds_two[test] adds_three[test]
... identical through t= 36.0s
```

That is the correct tree: the file, the `tests` namespace, and both tests, each at its own position. The
row is technically shippable. Upstream's floor is `neovim >= 0.12` (its README prerequisites) and this
machine is `0.12.5`, so the version gate is met. Its README also states, in the same section that tells
you not to add `neotest-rust`, that discovery and command construction both go through rust-analyzer over
the Language Server Protocol (LSP) rather than through tree-sitter, which is what makes finding 3
possible in the first place.

### 3. Root cause of B95(b): a readiness race, with two distinct silent failure modes

`timeline.lua`, one request every 400 ms after `client.initialized == true`, bare client:

```
t= 0.03s n=1 | cargo check --workspace
t= 0.40s n=1 | cargo check --workspace
t= 0.80s n=1 | cargo check --workspace
t= 1.20s n=0 |
t= 1.60s n=0 |
t= 2.00s n=0 |
t= 2.40s n=0 |
t= 2.80s n=5 | test-mod tests / test tests::adds_two / test tests::adds_three /
               cargo check -p b95probe --all-targets / cargo test -p b95probe --all-targets
... stable at n=5 through t= 9.60s
```

Three phases, and `client.initialized` is true in all three, so nothing in the client's own state
distinguishes them.

- **Phase A, 0.0 to 0.8 s: exactly one runnable, `cargo check --workspace`.** Read
  `lua/rustaceanvim/neotest/trans.lua`: `runnable_to_position` returns nil unless `cargoArgs[1]` starts
  with `test`. `check` does not, so the position list ends up empty, and
  `lua/rustaceanvim/neotest/init.lua` still takes its success path because `#runnables ~= 0`. It builds a
  tree holding the file node alone, with `file_pos.runnable = nil` because `#namespaces == 0`. Confirmed
  at the adapter level: `t= 0.2s 1 nodes -> lib.rs[file]`.
- **Phase B, 1.2 to 2.4 s: zero runnables.** `init.lua` then early-returns
  `lib.positions.parse_tree(positions)` with `positions` empty and no options table. In the deployed
  neotest, `lua/neotest/lib/positions/init.lua:337` is `local structure = assert(build_structure(...))`,
  and `build_structure` returns nil for an empty list. The call **raises**. Confirmed at the adapter
  level: `t= 6.1s RAISED neotest/lib/positions/init.lua:337: assertion failed!`. The same early return is
  taken when `get_client_for_file` finds no client and when the request returns an error, and under
  rustaceanvim's own server settings a `-32801 content modified` cancellation was observed at 6.1 s,
  which lands on that path too.
- **Phase C, from 2.8 s: correct.** Five runnables, three of them tests.

Phase A is the operator-visible symptom that B95 recorded. `build_spec` returns nil when `pos.runnable`
is nil (read at `init.lua:275`), so pressing `<leader>tt` on that tree produces no run, no error and no
message. It is indistinguishable from "this file has no tests".

The adapter has no readiness gate of any kind: it asks once, at whatever moment neotest schedules
discovery, and both pre-ready answers are treated as final.

### 4. Why B95 saw zero for 240 seconds: neotest re-discovers on a write, not on a timer

`~/.local/share/nvim/lazy/neotest/lua/neotest/client/init.lua` registers five autocommands. Only two
re-run `discover_positions` for a file:

- line 400, `{ "BufAdd", "BufWritePost" }`, which ends in `self:_update_positions(file_path, ...)`;
- line 463, `{ "BufAdd", "BufDelete" }`, which re-walks the file's parent directory.

The `BufEnter` handler at line 480 only calls `_set_focused_file`. There is no interval, no retry and no
LSP-progress subscription. So a tree captured during phase A or B persists for the rest of the session
unless the operator saves the file or the buffer is removed and re-added. Watching the summary for four
minutes, which is what B95 did, could never have recovered it. That closes the "root cause not isolated"
line in the backlog entry.

### 5. B95(a), the duplicate client, is still live, and upstream documents the conflict itself

`lsp.lua:185-209` still has `automatic_enable = true` with `"rust_analyzer"` in `ensure_installed`. The
name collision B95 observed is structural, not incidental:

- nvim-lspconfig's configuration name, and therefore the client name mason-lspconfig enables, is
  `rust_analyzer` with an underscore.
- rustaceanvim names its client `rust-analyzer` with a hyphen, in three places:
  `lua/rustaceanvim/rust_analyzer.lua:31`, `lua/rustaceanvim/health.lua:200`, and
  `lua/rustaceanvim/lsp/init.lua:11` as `local ra_client_name = 'rust-analyzer'`.

So `vim.lsp.get_clients({ name = 'rust-analyzer' })`, which is how `get_active_rustaceanvim_clients`
filters, cannot see mason-lspconfig's client and will not stand down for it. Two servers attach to one
buffer, which is what B95 measured as ids 2 and 3.

rustaceanvim's README carries its own warning for this, verbatim: "Do not call the
`nvim-lspconfig.rust_analyzer` setup or set up the LSP client for `rust-analyzer` manually, as doing so
may cause conflicts."

B95's proposed fix shape is confirmed correct and supported. mason-lspconfig's installed help text at
`doc/mason-lspconfig.txt:124-129` gives the option, and its own example uses this exact server:

```lua
automatic_enable = {
  exclude = { "rust_analyzer", "ts_ls" }
}
```

The type is declared `boolean | string[] | { exclude: string[] }` at line 138, and
`lua/mason-lspconfig/features/automatic_enable.lua:24-27` implements it. Note the trade: this
configuration currently states `automatic_enable = true` as one uniform rule, and the exclude form makes
Rust the one hand-managed exception. That is a real cost, small, and it is the operator's to accept.

### 6. Separate live defect: a stale rust-analyzer whose `cargo metadata` call fails

Found while probing, not part of B95, and it affects every Rust buffer rather than only neotest.

Mason's installed rust-analyzer is the `2025-12-21` build, from a receipt dated `2025-12-30`. Mason's
registry cache on disk is version `2026-09-13-prime-temper` and offers
`pkg:github/rust-lang/rust-analyzer@2026-09-07`. mason-lspconfig's `ensure_installed` installs a missing
package but does not upgrade an installed one, so the stale copy persists silently and indefinitely.

That stale build invokes `cargo metadata --lockfile-path ...`, and this machine's `cargo metadata` has no
such option. `cargo metadata --help` on cargo 1.98.1 lists only `--filter-platform`, `--no-deps`,
`--format-version`, `-v`, `-q`, `--color`, `--config`, `-Z`, `-h`, the feature flags, and the manifest
options. The failure is in `~/.local/state/nvim/lsp.log`, twice per start, once for the workspace and
once for the sysroot:

```
WARN `cargo metadata` failed and returning succeeded result with `--no-deps`
error=`cargo metadata` exited with an error: error: unexpected argument '--lockfile-path' found
  ... project_model::cargo_workspace::FetchMetadata::exec
```

rust-analyzer falls back to a `--no-deps` read, which is deliberate upstream behavior, so the workspace
loads without dependency resolution. Test runnables still appear, which is why finding 2 passes, but
dependency-aware completion and type information are degraded for every Rust file on this machine today.

Upstream `master` handles the flag correctly: in `crates/project-model/src/cargo_workspace.rs` the
`--lockfile-path` push is gated on a `LockfileUsage::WithFlag` variant and is accompanied by
`-Zunstable-options` and a channel override, with the `--no-deps` result kept as the fallback. So the
stale Mason build is the side that is wrong. Two remediations, both the operator's to run: update Mason's
copy, or `rustup component add rust-analyzer`, which `rustup component list` shows as available and not
installed on both the nightly and stable toolchains. **Neither was verified on this machine**, because
verifying either requires installing something.

One related detail, verified because it decides whether rustaceanvim's default command works here at all.
rustaceanvim defaults its `cmd` to the bare name `rust-analyzer` (`lua/rustaceanvim/health.lua:186`), and
`~/.cargo/bin/rust-analyzer` is a rustup shim that errors with "Unknown binary 'rust-analyzer' in
official toolchain". It resolves correctly anyway, because `lsp.lua:146-161` prepends Mason's `bin` to
`vim.env.PATH` at startup and mason.nvim's own default is `PATH = "prepend"`. So no explicit `cmd` is
needed, but the resolution is load-bearing on that prepend, and rustaceanvim's health check would print
"rust-analyzer wrapper detected" if the prepend ever went away.

### 7. Java: the verification plan 46b step 2 demands cannot be attempted

`rcasia/neotest-java` is alive: HEAD `71354dd`, dated 2026-09-05. Its README's own prerequisites are
Neovim 0.10.4 or newer, nvim-treesitter with the `java` parser installed, and a JDTLS-based Java language
server, either `mfussenegger/nvim-jdtls` or `nvim-java/nvim-java`, with nvim-dap optional. Its feature
list is built on Maven and Gradle and on classpath management read from that language server.

Measured against this machine and this configuration:

| Prerequisite                      | State                                                     |
| --------------------------------- | --------------------------------------------------------- |
| Java runtime                      | ABSENT. `/usr/bin/java` is the macOS stub, no runtime     |
| Maven                             | ABSENT, `mvn` not on PATH                                 |
| Gradle                            | ABSENT, `gradle` not on PATH                              |
| JDTLS-based language server       | ABSENT. No `jdtls` entry, no `nvim-jdtls`, no `nvim-java` |
| `java` tree-sitter parser         | ABSENT from `treesitter.lua`'s `ensure_installed`         |
| any Java package declaration      | ABSENT from the packages YAML                             |
| any Java source in the repository | ABSENT, `git ls-files` matches zero `.java` files         |
| any Maven or Gradle project       | ABSENT under `~/workspaces` to depth 4                    |
| nvim-dap                          | present                                                   |

Step 2 asks for "a JUnit 5 scratch project". There is no compiler, no build tool and no language server
to run one with, so the adapter cannot be shown working. Step 2's own rule applies: it is withheld, and
this document is the "why".

### 8. Elixir: the same answer, plus a stale adapter

`jfpedroza/neotest-elixir` exists but its newest commit is `a242aeb`, "feat: Allow configuration of test
directories and test file pattern (#40)", dated 2025-01-19. Spec 5.3 already recorded it as "2025-01, the
only option", so it has had no commit in the twenty months since that reading.

Machine state: `elixir`, `mix`, `iex` and `erl` are all absent from PATH, none is declared in
`.chezmoidata/system_packages_autoinstall.yaml`, there is no `elixirls` or `lexical` in mason-lspconfig's
`ensure_installed`, `git ls-files` matches zero `.ex` or `.exs` files, and no `mix.exs` exists under
`~/workspaces` to depth 4. The `elixir`, `eex` and `heex` tree-sitter parsers ARE in `treesitter.lua`'s
`ensure_installed`, which is the only Elixir footprint anywhere in the configuration and predates this
program.

Step 2 asks for "`mix test` in a scratch app". There is no Elixir. Withheld, same rule.

### 9. What Rust test running actually goes through today

Worth recording, because it is the yardstick any neotest row has to beat and it makes assumption 1 a real
fork rather than a formality.

`overseer.nvim` ships a `cargo` template provider, and `cargo.lua:39` is
`{ args = { "test" }, tags = { TAG.TEST } }`, so `cargo test` is already an overseer task. This
repository's `lua/plugins/overseer.lua` also hooks the `just` provider (`overseer.lua:484`) and supplies
a quickfix errorformat to templates that ship none, with a comment at `:455-476` recording that a Cargo
error resolves to `src/lib.rs:2:3` under the template's own format. The actual Rust gate in this
repository is `just test-rust`, which overseer's `just` provider offers directly.

So Rust is not untested from Neovim today. What is missing is neotest's per-test tree, per-test output
and jump-to-failure, and the nvim-dap strategy rustaceanvim wires to `codelldb`. Overseer also ships a
`mix` template, so the same argument covers Elixir if it ever arrives.

## The three readiness options, and a recommendation

Every option assumes the mason-lspconfig `exclude` from finding 5, which is not optional: without it two
servers attach.

**Option 1, a readiness gate in this repository's own `neotest.lua`.** Copy rustaceanvim's adapter table
and override `discover_positions` to call the original under `pcall`, retrying on a bounded deadline
until the tree holds at least one non-file node, and returning an empty tree rather than raising when the
deadline passes. Roughly fifteen lines. The copy-and-override pattern is already in this file for vitest,
whose `__call` gate had to be bypassed the same way, with the same "re-audit when the pin moves" note.
This does not patch, fork or modify rustaceanvim: it configures the table rustaceanvim returns, which is
the supported surface its README documents. It is the only option that meets the repository's own bar of
not shipping a known manual workaround.

**Option 2, accept the raw adapter.** Zero lines. Cost: the first `<leader>tt` in a freshly opened Rust
buffer silently does nothing, and the recovery is to save the file, because finding 4 shows `BufEnter`
does not re-discover. This is the same class of papercut as B96's Go parser race, which the ledger is
already carrying as a defect, so shipping a second instance of it deliberately is hard to justify.

**Option 3, withhold the Rust row like Java and Elixir**, and record overseer's `cargo test` plus
`just test-rust` as the Rust test path. Zero lines, zero pins, and spec 5.3's Rust row gets struck with
its reason. Leaves the `<leader>t` group without Rust, permanently.

**Recommendation: Option 1**, then Option 3 if Option 1's fifteen lines are judged not worth it. Option 2
is the one to reject: it is not cheaper than Option 3 in anything but pins, and it buys a known silent
failure.

## What would change the verdict

- **Installing a toolchain-matched rust-analyzer.** If `rustup component add rust-analyzer` or an updated
  Mason copy removes the `cargo metadata` failure, phases A and B in finding 3 may shorten enough that
  Option 2 becomes defensible. They will not vanish: rust-analyzer cannot produce test runnables before
  it has loaded the workspace, whatever version it is.
- **Measuring against a real workspace.** If `pns/` or `posture/` takes tens of seconds to load, the
  unready window is tens of seconds wide and Option 2 is dead outright, which strengthens rather than
  changes the verdict.
- **An upstream fix in rustaceanvim.** If its `discover_positions` gains a readiness gate, or its two
  early returns stop calling `parse_tree` with an empty list, Option 1's wrapper becomes dead code and
  should be deleted rather than kept.
- **A neotest pin bump.** Finding 4's autocommand list and finding 3's `positions/init.lua:337` assertion
  are both read against the pinned `27bf921`. A pin move, which plan task 47b (PR 29d) will do,
  invalidates both line references and the assertion behavior. Re-measure there.
- **A Java or Elixir project arriving.** Both dispositions are "the verification cannot be attempted",
  not "the adapter is bad". A real project, plus the toolchain declarations it needs, reopens each of
  them on its own merits. `rcasia/neotest-java` in particular is actively maintained.
- **Reading "intended workflow" as finding 9.** If the operator takes assumption 1's alternative, the
  Rust row closes today with no plugin and this document's Rust verdict becomes "already complete, amend
  spec 5.3 to say so".

## Open questions for the operator

1. **Which readiness option for the Rust row: 1, 2 or 3?** The recommendation is Option 1. Nothing else
   in this task can proceed until this is answered, because it decides whether the row is a pin, a pin
   plus fifteen lines, or a struck row.
1. **Is the mason-lspconfig `exclude` trade acceptable?** It makes Rust the one server not covered by
   `automatic_enable = true`. If not, the Rust row cannot use rustaceanvim at all and Option 3 is forced.
1. **Does "language coverage complete" mean neotest specifically, or is overseer's `cargo test` plus
   `just test-rust` enough?** This is assumption 1, and it is the cheapest possible close for the whole
   task.
1. **Should the stale Mason rust-analyzer become its own task?** It is a live defect on every Rust
   buffer, independent of neotest, and nothing in the configuration will ever upgrade it. It also needs a
   decision on the general shape: does this repository want Mason packages pinned and refreshed
   deliberately, or is "installed once, never updated" the accepted posture?
1. **Do spec 5.3's Java and Elixir rows get struck, or marked withheld with this document as the
   reason?** Related: should spec 5.3's Java row be corrected regardless, since it describes one
   filetype-lazy pin where the adapter needs a JDK, a build tool, a language server plugin and a parser?
1. **Should the raised assertion in finding 3 be reported upstream to rustaceanvim?** Its two early
   returns call `lib.positions.parse_tree` with an empty list, which raises in the pinned neotest. That
   looks like a straightforward upstream bug with a one-line reproduction.
1. **Is a Java or Elixir project anywhere on the horizon?** If yes, both dispositions should be filed as
   deferred with a trigger rather than withheld, and they need the toolchain decisions in question 4's
   shape.
