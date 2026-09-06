<!-- Shared global ruleset, included verbatim by ~/.claude/CLAUDE.md and ~/.codex/AGENTS.md. Edit here,
     never in a harness copy: test/integration/global-instruction-parity.sh byte-compares the rendered
     block across both targets and fails when they diverge. Harness-specific rules go in the including
     file, below the shared block. -->

## Collaboration style

- Terse, direct. No trailing recap.
- Verify before asserting; show evidence.
- Separate logically distinct changes into their own commits.
- No trailing whitespace, blank lines included.
- Don't open with apologies or affirmations ("You're right!", "Sorry!"). Never apologize before
  disagreeing.
- One precise question at a time when ambiguous. State a recommendation; ask only if contested.
- Never create unsolicited docs (`README.md`, `CHANGELOG.md`, etc.) or docstrings.
- Acronyms in commits and docs: on first use, a **well-known** acronym must give the full name in
  parentheses, `HMAC (hash-based message authentication code)`, then the bare acronym is fine after. A
  **less-common / not-widely-known** acronym is avoided altogether, spell it out every time, never
  introduce the short form (e.g. write "file integrity monitoring", never "FIM").
- Use the `humanizer` skill on prose longer than a paragraph.
- **No em-dashes, ever**: not in prose, PR descriptions, commits, docs, or replies. Use commas, periods,
  or parentheses.

## Verification and sources of truth

For claims with a canonical source, library APIs, CLI flags, config schemas, syntax, protocol details,
version-specific behavior, error messages, changes since early 2025, check the source before asserting:

1. **Local:** installed package, `--help`, `man`, the file, lockfiles.
1. **Official docs** via WebFetch.
1. **WebSearch** when 1-2 are unavailable.
1. **Training data:** last resort. Prefix the claim with `from training, not verified:`.

Reasoning, design, and general knowledge don't require sourcing, only specific verifiable claims.

## Verification before completion

"Should work" ≠ "works." Evidence before assertions.

- Design: obra/superpowers skill.
- Code: run tests (or a minimal reproducer); show output.
- Config: render, parse, or lint it.
- Script: `shellcheck` and run with representative inputs.
- Multi-step refactor: run the project's full check command before claiming done.

## Destructive action gates

Require per-invocation confirmation. Blanket "yes" doesn't carry over.

- `trash` > `rm`, including scratch directories the agent created itself.
- `git push --force` (use `--force-with-lease`; never to main/master), `git reset --hard`,
  `git clean -fd`, `git branch -D`, `git checkout .`
- Dropping DB tables/schemas; `killall`; `shutdown`; `dd`
- Bypassing checks: `--no-verify`, `--no-gpg-sign`, `--no-hooks`, `--skip-checks`
- `chezmoi apply` in any form: the operator runs applies, agents propose changes

## Code discipline

- **YAGNI.** No features, refactors, abstractions, or fallbacks beyond task scope. Three similar lines
  beat a premature abstraction.
- **No backwards-compat hacks for unshipped code.** Dead code gets deleted.
- **Glob before creating.** Prefer editing an existing file.
- **Never patch, fork, or modify the code of third-party tools I don't own** (e.g. osquery). Configure
  them through their own config files and supported options only. If a goal seems to require changing a
  tool's source, stop and say so, don't propose it.

## Shell scripts

- `set -euo pipefail`. Double-quote expansions. `cd X || exit`. No unquoted globs.
- Prefer stable, system-shipped tools (bash, coreutils, `sed`, `awk`, `grep`, `jq`) over newer
  alternatives (`fd`, `rg`, `sd`). Modern tools are for interactive use; scripts need boring reliability.
- ISO 8601 timestamps. On macOS use `gdate -Is` or `date -u +"%Y-%m-%dT%H:%M:%SZ"`, BSD `date` lacks
  `-Is`.

## Git commits

**Never add an AI co-author trailer** (`Co-Authored-By: Claude`, `Co-Authored-By: Codex`, any Anthropic
or OpenAI attribution line) **or a "🤖 Generated with ..." footer.** Commits look as if the user authored
them directly.

Use the `conventional-commits` skill. A user-wide `prepare-commit-msg` hook prepopulates the message;
`SKIP_AI_COMMIT=1` bypasses it.

## Pull request descriptions

`~/.claude/commands/pr.md` is the single source for the body's section contract and for the
anti-AI-pattern self-review that gates posting. Read it before drafting a body and follow it there; never
restate or fork those rules elsewhere. Keep a posted body current when later review rounds change the
facts.

## Task tracking

Use the `todoist-cli` skill (`td` CLI) on non-trivial work:

- **Before:** list relevant tasks; create one if missing.
- **During:** create follow-ups for deferred or surfaced work. Re-prioritize.
- **After:** mark complete; re-prioritize the backlog.

## Tool preferences

Prefer local CLI tools over MCP servers when both work. MCP only for SaaS APIs without a first-class CLI.

- Prefer the `gh-axi` skill over the raw `gh` CLI for every GitHub operation: issues, pull requests,
  workflow runs, releases, everything. `gh` stays installed and authenticated purely as `gh-axi`'s
  runtime dependency; never invoke it directly.
- Prefer the `chrome-devtools-axi` skill over other browser automation (Claude-in-Chrome, Playwright, raw
  `chrome-devtools-mcp`) whenever DevTools-based automation is needed.
- Home Assistant work uses both skills together: `home-assistant` (runtime control: entity states,
  service calls) and `home-assistant-best-practices` (authoring: automations, helpers, dashboards). Load
  both whenever working with Home Assistant.

## Open Neovim buffers

A file that is open in a Neovim buffer is edited through the Neovim MCP tools, never with `Write` or
`Edit`. Check with the MCP `list_buffers` tool before writing a file under the current project; a disk
write collides with the unsaved buffer and the operator loses one side.

The `nvim` server has NO edit tool, which is the trap. Reading is `read`; EDITING is `exec_lua` running
`nvim_buf_set_lines`, which is undoable and never touches the file on disk. Falling back to `Write`
because nothing is called `edit` is the exact mistake this rule exists to prevent.

## Backups

Location: `~/workspaces/backups/`. Naming: `YYYY-MM-DDTHH-MM-SS.Name.backup[.ext]`, timestamp first for
chronological sort, hyphens within date/time/name, period between timestamp and name, `.backup` before
any extension. Applies to files and folders. Examples: `2026-04-20T14-30-00.settings-json.backup.json`,
`2026-04-20T14-30-00.my-project.backup/`.

## Toolchain (locked-in, do not suggest migrating)

- **Shell:** bash.
- **Multiplexer:** herdr.
- **Version manager:** Nix flakes per-project (not `mise`).
- **File manager / git TUI:** `git`, `gh-axi`, and `fzf`.
- **Browser:** `chrome-devtools-axi`.
- **Terminal:** Ghostty.
- **Editor:** Neovim.
- **Secrets:** KeePassXC.

## Agents

- Parallel subagents for independent work.
- Stop at environmental blockers (brew install, KeePassXC unlock, destructive `rm`, VM clones) and
  surface them.
