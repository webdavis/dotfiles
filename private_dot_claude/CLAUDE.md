<!-- Evergreen file. General principles, not transient state. -->

# Global Rules

## Collaboration style

- Terse, direct. No trailing recap.
- Verify before asserting; show evidence.
- Separate logically distinct changes into their own commits.
- No trailing whitespace, blank lines included.
- Don't open with apologies or affirmations ("You're right!", "Sorry!"). Never apologize before
  disagreeing.
- One precise question at a time when ambiguous. State a recommendation; ask only if contested.
- Never create unsolicited docs (`README.md`, `CHANGELOG.md`, etc.) or docstrings.
- Use the `humanizer` skill on prose longer than a paragraph.

## Verification and sources of truth

For claims with a canonical source — library APIs, CLI flags, config schemas, syntax, protocol details,
version-specific behavior, error messages, changes since early 2025 — check the source before asserting:

1. **Local:** installed package, `--help`, `man`, the file, lockfiles.
1. **Official docs** via WebFetch.
1. **WebSearch** when 1–2 are unavailable.
1. **Training data:** last resort. Prefix the claim with `from training, not verified:`.

Reasoning, design, and general knowledge don't require sourcing — only specific verifiable claims.

## Verification before completion

"Should work" ≠ "works." Evidence before assertions.

- Code: run tests (or a minimal reproducer); show output.
- Config: render, parse, or lint it.
- Script: `shellcheck` and run with representative inputs.
- Multi-step refactor: run the project's full check command before claiming done.

## Destructive action gates

Require per-invocation confirmation. Blanket "yes" doesn't carry over.

- `rm -rf`, `rm` outside a scratch dir
- `git push --force` (use `--force-with-lease`; never to main/master), `git reset --hard`,
  `git clean -fd`, `git branch -D`, `git checkout .`
- Dropping DB tables/schemas; `killall`; `shutdown`; `dd`
- Bypassing checks: `--no-verify`, `--no-gpg-sign`, `--no-hooks`, `--skip-checks`
- `chezmoi apply` from automation on template files (agents use `--exclude=templates`)

## Code discipline

- **YAGNI.** No features, refactors, abstractions, or fallbacks beyond task scope. Three similar lines
  beat a premature abstraction.
- **No backwards-compat hacks for unshipped code.** Dead code gets deleted.
- **Glob before creating.** Prefer editing an existing file.

## Shell scripts

- `set -euo pipefail`. Double-quote expansions. `cd X || exit`. No unquoted globs.
- Prefer stable, system-shipped tools (bash, coreutils, `sed`, `awk`, `grep`, `jq`) over newer
  alternatives (`fd`, `rg`, `sd`). Modern tools are for interactive use; scripts need boring reliability.
- ISO 8601 timestamps. On macOS use `gdate -Is` or `date -u +"%Y-%m-%dT%H:%M:%SZ"` — BSD `date` lacks
  `-Is`.

## Git commits

**Never add `Co-Authored-By: Claude`, any Claude/Anthropic co-author trailer, or a "🤖 Generated with
Claude Code" footer.** Commits look as if the user authored them directly.

Use the `conventional-commits` skill. A global `prepare-commit-msg` hook at `~/.config/git/hooks/`
prepopulates messages via Claude haiku; `SKIP_AI_COMMIT=1` bypasses.

## Task tracking

Use the `todoist-cli` skill (`td` CLI) on non-trivial work:

- **Before:** list relevant tasks; create one if missing.
- **During:** create follow-ups for deferred or surfaced work. Re-prioritize.
- **After:** mark complete; re-prioritize the backlog.

## Tool preferences

Prefer local CLI tools over MCP servers when both work. MCP only for SaaS APIs without a first-class CLI.

## Backups

Location: `~/workspaces/backups/`. Naming: `YYYY-MM-DDTHH-MM-SS.Name.backup[.ext]` — timestamp first for
chronological sort, hyphens within date/time/name, period between timestamp and name, `.backup` before
any extension. Applies to files and folders. Examples: `2026-04-20T14-30-00.settings-json.backup.json`,
`2026-04-20T14-30-00.my-project.backup/`.

## Toolchain (locked-in — do not suggest migrating)

- **Shell:** bash.
- **Multiplexer:** tmux.
- **Version manager:** Nix flakes per-project (not `mise`).
- **File manager / git TUI:** neither `yazi` nor `lazygit`.
- **Terminal:** Ghostty.
- **Editor:** Neovim.
- **Secrets:** KeePassXC.

## Agents

- Parallel subagents for independent work.
- Stop at environmental blockers (brew install, KeePassXC unlock, destructive `rm`, VM clones) and
  surface them.
