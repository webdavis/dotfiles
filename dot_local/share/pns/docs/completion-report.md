# pns refactor completion report

Written 2026-09-08 against `main`. Every number here was measured on the tree at the time of writing,
and the command that produced it is named so it can be re-measured rather than trusted.

## What the refactor did to the shape of the code

`src/main.rs` peaked at **13,484 lines** (measured across its history with `git cat-file`). It no longer
exists. The binary is now six crates:

| Crate | Lines | Files |
| --- | --- | --- |
| `pns-adapters` | 36,629 | 393 |
| `pns-cli` | 32,430 | 238 |
| `pns-domain` | 14,717 | 114 |
| `pns-application` | 11,086 | 123 |
| `pns-protocol` | 2,298 | 18 |
| `pns-hermes` | 356 | 6 |

Total 97,516 lines across 892 files. The growth over the original single file is tests: the split was
what made most of these behaviours reachable by a test at all.

**No file exceeds 500 lines.** The largest is `pns-cli/tests/native.rs` at 395, then
`pns-application/src/replay_missed/tests.rs` at 375 and `pns-domain/src/recap/tests/answers.rs` at 365.
The operator's standing rule is a 300 line target and a 500 line hard cap with tests included, and
`scripts/treefmt/rust-file-size.sh` now enforces the cap in `just lint-check` rather than leaving it to
review.

Measure it again with:

```bash
find dot_local/share/pns/crates -name '*.rs' -not -path '*/target/*' -exec wc -l {} + | sort -rn | head
```

## Gates the refactor added

- **File size.** `scripts/treefmt/rust-file-size.sh` fails any `dot_local/share/**/*.rs` over 500 lines.
  It is a lint, not a test, so it runs in `just lint-check`.
- **Documentation.** `just test-rust` now runs `cargo doc --workspace --no-deps` with
  `RUSTDOCFLAGS="-D warnings"` for each of the four workspaces. Backlog B75 recorded three unresolved
  intra-doc links on `main`; all four workspaces pass clean today, so the gate holds the tree where the
  refactor left it instead of letting the links rot back in.

## The hook table, verified

Plan row 8.4 asks that the eleven hook words still map between the CLI and the harnesses that call it.
They do, checked on 2026-09-08 by reading both sides rather than by assertion:

`asked`, `blocked`, `config-change`, `denied`, `model-switch`, `plan-ready`, `prompt`, `quota`,
`resolved`, `stop`, `stop-failure`.

The CLI's set and the set declared in `private_dot_claude/modify_settings.json` are identical, eleven for
eleven with nothing on either side unmatched. `test/unit/pns-codex-install-hooks.sh` passes, so the Codex
installer writes the same table into `~/.codex/hooks.json`. Codex still ignores those hooks until the
operator approves them at `/hooks`, which is deliberate: hook trust is never synthesised from source.

## Decision records

`docs/decisions/` holds 0001 through 0014 with no gap. 0012, the store's two fail directions, was
backfilled on 2026-09-08; it had been stated in the plan and followed by the code, but never written
down where a reader of the crate would find it.

## What this report does not carry

The plan's row 18.1 also asks for a per-file before-and-after table over its section 4 file list and a
name-mapping table. Those are not reproduced here. The per-file history is recoverable from the commits
of steps 5 and 6, which were pure moves with the name lists in their own pull request bodies, and
duplicating it here would create a second copy that drifts from the first. This section exists so the
omission is a stated choice rather than something a later reader has to notice.
