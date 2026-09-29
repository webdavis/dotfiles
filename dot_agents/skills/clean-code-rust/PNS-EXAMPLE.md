# Worked example: pns

`webdavis/pns`, read at commit `e4632e8` on 2026-09-28. The notification engine, a Cargo workspace
of six crates with 1,247 tracked `.rs` files. Everything here was measured from that commit, not
recalled; re-read the repository before trusting a number.

Everything here is **specific to `pns`**. Read it for what an answer to the general method looks
like in practice. Do not apply any of it to another tool without deriving the same answer from that
tool's own source. Where its shape and the method differ, the difference is noted rather than
smoothed over.

## The workspace

    crates/pns-domain
    crates/pns-application
    crates/pns-protocol
    crates/pns-hermes
    crates/pns-adapters
    crates/pns

The root `Cargo.toml` lists all six under `members` with `resolver = "3"` and
`default-members = ["crates/pns"]`. The path dependencies, read from each crate's own manifest:

| Crate             | Depends on                                                      |
| ----------------- | --------------------------------------------------------------- |
| `pns-domain`      | nothing                                                         |
| `pns-application` | `pns-domain`                                                    |
| `pns-protocol`    | `pns-domain`                                                    |
| `pns-hermes`      | no pns crate                                                    |
| `pns-adapters`    | `pns-domain`, `pns-application`, `pns-protocol`, `pns-hermes`   |
| `pns`             | all five                                                        |

The external dependencies and the rule that admits them ("a dependency is taken only where a
format demands one") are in `docs/decisions/0015-a-dependency-only-where-a-format-demands-one.md`.
`pns-domain` and `pns-application` have none. `rusqlite` appears only in `pns-adapters`, pinned
exactly (`=0.39.0`, `bundled`), and in the binary crate's `[dev-dependencies]`.

`crates/pns-domain/tests/boundary.rs` holds the one test that pins a manifest:
`the_domain_crate_names_no_dependency_so_its_policy_stays_std_only` reads `../Cargo.toml` and fails
if any non-dev dependency table appears.

## Where the shape differs from the method

- **The binary crate is `crates/pns`, not `crates/pns-cli`.** Its `[[bin]] name = "pns"` keeps the
  name every caller invokes. A second binary, `http-capture`, is built only under the `dev-tools`
  feature.
- **A sixth crate, `pns-hermes`,** holds the signed Hermes POST client (`SignedPost`,
  `UreqSignedPost`, `PostOutcome`, `sign`, `delivered`, `outcome_line`, `skipped_line`). It depends
  on no pns crate. No other repository depends on it: `webdavis/uu` names no pns crate in any
  manifest at its `main` of the same day.
- **`main.rs` is three lines** calling `pns::run()`. The composition root is `crates/pns/src/lib.rs`
  (180 lines), not `main.rs`.
- **One `rust-toolchain.toml`, at the workspace root** (`channel = "stable"`), rather than one per
  crate.

## The gates

The `justfile` defines them, and CI (`.github/workflows/ci.yml`, `macos-latest`) runs
`rustup toolchain install`, `brew install just` and then `just gates`:

    just gates    # fmt-check lint doc test

    cargo fmt --all --check
    cargo clippy --locked --workspace --all-targets --features dev-tools -- -D warnings \
      -W clippy::undocumented_unsafe_blocks -W clippy::unnecessary_safety_comment
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
    cargo test --locked --workspace --features dev-tools --no-fail-fast

The two extra clippy lints make a missing `// SAFETY:` comment above an `unsafe` block, or a stray
one above safe code, a gate failure rather than a review finding.

`treefmt.toml` runs `scripts/treefmt/rust-file-size.sh` over `crates/**/*.rs`, which fails any file
over 500 physical lines, plus `mdformat` (with `docs/**` excluded, because the `Given` / `When` /
`Then` line breaks carry meaning) and `taplo`. At this commit no `.rs` file exceeds 300 lines; the
largest is `crates/pns/tests/recap_engine.rs` at 298.

`crates/pns/tests/support/budget.rs` is the speed guard: over `TEST_BUDGET_MS` (1,000) warns, over
`TEST_CEILING_ON_A_LOADED_MAC_OR_A_CI_RUNNER_MS` (20,000) fails unless the test called
`allow_slow("reason")` (`crates/pns/tests/support/sandbox.rs`) naming a structural cause.

## The consumers outside the repository

The command-line surface is a compatibility contract. The callers live in `webdavis/dotfiles`
(41 files at its `main` of the same day); enumerate them there before changing a subcommand or a
flag. The Claude Code hooks build the binary's path with `joinPath`, so search for the word, not
the path:

    grep -rlw pns --exclude-dir=.git --exclude-dir=docs .

The ones that call it:

1. **The install**: `.chezmoiscripts/run_onchange_after_57-install-cargo-git-tools.sh.tmpl` runs
   `cargo install --git` for the `pns` entry of `.chezmoidata/system_packages_autoinstall.yaml`,
   with `features: dev-tools`, into `~/.cargo/bin`, and
   `run_onchange_after_58-record-pns-and-posture-upgrades.sh.tmpl` restarts `pns.gateway` on a new
   binary.
2. **The Claude Code hooks** in `private_dot_claude/modify_settings.json` (`pns hook prompt`,
   `stop`, `model-switch`, `stop-failure`, `quota` and more), and the Codex hooks that
   `.chezmoiscripts/run_after_72-relay-codex-hooks.sh.tmpl` has pns merge into Codex's hooks file.
3. **The shell notifier** in `dot_bashrc.tmpl`: `pns shell begin` and
   `pns shell end --exit-code <code> --elapsed <secs>`.
4. **`pns loop begin|end`**, from the `pns-loop` skill.
5. **The gateway LaunchAgent**, `pns.gateway`, running `pns gateway run`.
6. **pns.nvim** (`dot_config/nvim/lua/plugins/pns.lua`), with `minimum_version = "0.2.0"` against
   the binary crate's `version = "0.2.0"`.
7. **The reminder extensions** for the pi and omp harnesses, both rendered from
   `.chezmoitemplates/pns-reminder-extension.ts.tmpl`.
8. **Other tools' alert scripts**: `dot_config/uu/scripts/executable_send-alert-to-pns.sh` and
   `dot_config/lights/scripts/executable_send-to-pns.sh`.
9. **The config file** `dot_config/pns/private_config.toml.tmpl`. pns reads plain TOML and ships no
   configuration-manager template syntax (decision 0016); the renderer and the outside-the-package
   pin of decision 0011 went with it, so the template belongs to the dotfiles alone.

## Where the reasoning lives

- **The vocabulary**, verified against the source, including the words in circulation that the code
  does not use: `docs/specs/glossary.md`.
- **The behavior**, one `Given` / `When` / `Then` specification per area, under `docs/specs/`.
  `docs/specs/unpinned-behaviors.md` lists every specified behavior no test pins, which is what a
  later move must close first.
- **The port contracts** each `pns-application` port promises its use cases:
  `docs/specs/port-contracts.md`.
- **The decisions**, `docs/decisions/0001` to `0016`. Several answer questions the method asks of
  any tool: the lamp state is `unread` and the legacy `glow` file is deleted rather than migrated
  (0004); passing both delivery-scope flags is refused at the legacy adapter (0007); a notification
  never fails the work it reports on (0010); the store has two fail directions (0012); delivery is
  at-least-once with retained outcomes (0013).

**`doctor` and a bare `pulse` change the real world** (decision 0005): they post real banners and
drive the lamps, so no test and no verification harness may run them.
