---
name: clean-code-rust
description: "The Rust bindings of this repository's clean-code standard. Use when building or restructuring a Rust tool this repository owns (pns, uu, the herdr plugins): converting a crate into a Cargo workspace, drawing crate boundaries, choosing between a trait, an enum and a concrete type, picking a persistence crate, or running the Rust quality gates. Read the language-neutral method alongside it."
---

# Clean code: Rust

**Read `~/.agents/skills/clean-code/SKILL.md` first.** That skill carries the method: the
ordered ladder, the five module roles and their dependency direction, SOLID, the test obligations,
the delivery ladder, the sol review, and the completion report. This file states only how those are
spelled and enforced in Rust, and it wins wherever the two disagree on a number or a mechanism.

The Rust tools this repository owns today: `pns` (`dot_local/share/pns`), `uu`
(`dot_local/share/uu`), and the two herdr plugins under `dot_local/share/herdr/plugins/`. The worked
example is [`PNS-EXAMPLE.md`](PNS-EXAMPLE.md).

## The workspace

The five roles are five crates in one Cargo workspace:

    crates/<tool>-domain
    crates/<tool>-application
    crates/<tool>-protocol
    crates/<tool>-adapters
    crates/<tool>-cli

**`Cargo.toml` is the enforcer.** A crate can only `use` what its own `[dependencies]` names, so an
inward dependency from domain or application code to a concrete adapter fails to compile rather than
passing review by luck. A declared cycle is refused by cargo outright.

The binary target keeps the tool's name (`[[bin]] name = "<tool>"`), because every caller invokes it
by that name. Keep `Cargo.lock` committed and build `--locked`.

`main.rs` targets 50 to 150 lines, preferably under 100, and must be below 150 at completion.

### What the domain crate excludes

Filesystem access, SQLite, TOML, JSON, HTTP, environment variables, process spawning, macOS APIs,
vendor APIs, executable discovery, and CLI output. Prefer `std`-only unless a small dependency is a
genuine domain primitive.

## Rust abstraction choices

- a **concrete type** when there is one implementation and no substitution need
- a **closure** for one injected operation such as a clock or a mapper
- a **trait** for a stable external capability or a meaningful contract
- an **enum** for a closed set of alternatives, which is how the neutral rule "make invalid states
  unrepresentable" is spelled here: replace conflicting boolean pairs with one enum
- a **newtype** for validated identifiers and sensitive values
- **generics** when compile-time composition improves clarity
- **`dyn Trait`** at the composition root's heterogeneous collections only

Do not create a trait for every struct, create one-method wrapper types merely for dependency
injection, use `Box<dyn Trait>` throughout domain code, introduce generic parameters that obscure the
use case, build a service locator, or hide branching inside macros to make files look shorter.

The destination interface:

    trait NotificationDestination: Send + Sync {
        fn id(&self) -> &DestinationId;
        fn capabilities(&self) -> DestinationCapabilities;
        fn deliver(&self, request: &DeliveryRequest) -> DeliveryOutcome;
    }

## Errors and outcomes

Do not use `Option` where several materially different failure or unknown states must be
distinguished for diagnostics or policy: use a typed `Result` or a purpose-built enum.

Do not panic on ordinary external failures. A panic is acceptable only for a compiled-in invariant
whose violation is a programmer error and cannot depend on operator input or runtime conditions.
Avoid `unwrap` and `expect` on untrusted input.

Do not add `Arc<Mutex<_>>` by default. First consider ownership, immutable sharing, message passing,
task confinement, or a transaction.

Do not introduce Tokio or another async runtime solely to make the architecture look modern. The
synchronous, deadline-bounded model is acceptable.

Isolate `unsafe` macOS and terminal operations in the smallest possible adapter modules, document the
safety invariants, and add focused tests.

## Visibility

Private by default. `pub(super)` for narrow parent collaboration, `pub(crate)` for internal
cross-module collaboration, `pub` only for intentional crate APIs. Do not make the module tree public
so integration tests can reach it; curate exports in `lib.rs`.

## Persistence

Prefer synchronous `rusqlite` over an async database layer unless a runtime requirement proves
otherwise. WAL mode, versioned migrations, explicit transactions, bounded busy timeouts, restrictive
file permissions, typed codecs. **Every caller handles `SQLITE_BUSY` with a bounded timeout.**

Domain and application code must not depend on `toml::Value` or free-form plugin tables.

A crate that reaches outside its own folder with `include_str!` or an `env!("CARGO_MANIFEST_DIR")`
path join stops compiling the day the crate moves repositories. Keep the crate's tests against
fixtures it owns, and pin any generated-file equality from the outer repository instead.

## Tests

Unit tests live beside their implementation under `#[cfg(test)] mod tests;`. A large unit-test module
may live in a private child file (`src/lights/schedule.rs` beside `src/lights/schedule/tests.rs`);
that is still `cfg(test)` and does not enter production builds.

**Tooling that exists, and tooling that does not.** `cargo-mutants` and `cargo-fuzz` are **not
installed**: mutation testing is done by hand, per behavior, against an unmutated control, and the
table goes in the report. `cargo-miri` **is** installed and the local toolchain is nightly, but CI is
stable macOS, so **Miri results are local evidence, never a CI gate**. Add `proptest` only after
naming the input space it covers better than examples do.

`cargo test --workspace` runs crates in parallel competing for the same CPU, so measure the speed gate
under that configuration.

## Quality gates

    just test-rust
    just lint-check
    just ship
    cargo fmt --all -- --check
    cargo check --workspace --all-targets
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace --no-fail-fast
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

Plus the builder's own cargo line and every dependent sibling's test command. Do not add broad lint
suppressions; a `#[allow]` must be narrow and explain why the lint is wrong at that location.

## The file-size command

Count physical lines after `rustfmt`, with this command and no other. `tokei` mis-parses this tree:
its totals fall thousands of lines short of `wc -l`.

    git ls-files '<crate-path>/*.rs' | while IFS= read -r f; do
      awk -v F="$f" '
        /^[[:space:]]*#\[cfg\(test\)\]/ && !seen { seen = 1 }
        !seen { impl++ }
        { total++ }
        END {
          if (F ~ /(^|\/)tests(\.rs|\/)/) impl = 0
          printf "%5d impl %5d total  %s\n", impl, total, F
        }' "$f"
    done | sort -k3,3rn

Implementation lines are those before the first `#[cfg(test)]`; a file named `tests.rs` or under a
`tests/` directory has zero. **A `#[cfg(test)]` item above production code is itself a finding**, not
a way to shrink the number.

Limits, per the operator's standing rule: 200 implementation lines and 300 total are the targets, 250
implementation or 400 total normally requires decomposition, and **no handwritten `.rs` file exceeds
500 total lines, unit tests included, with no waiver**.
