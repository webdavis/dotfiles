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
- Code comments are short and concise. One line where one line does it.
- A comment says what the code does or why it is the way it is. It NEVER explains why something is not a
  certain way, what was considered and rejected, what a file does not do, or anything about the
  conversation that produced it. If it is not in the file, it is not mentioned in the file. Put that
  reasoning in the commit message or the pull request body instead.
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

## Git worktrees

herdr never scans for worktrees: its sidebar shows only worktrees it opened itself, so a checkout made
with `git worktree add` or a harness helper stays invisible there.

- Inside herdr (`HERDR_ENV=1`), create a worktree with
  `herdr worktree create --cwd <repo-root> --branch <name> --no-focus`, never with `git worktree add`. It
  lands in `~/.herdr/worktrees/<repo>/<branch>` with a sidebar entry from the start.
- A worktree the harness already made (the Agent tool's `isolation: "worktree"`) gets registered with
  `herdr worktree open --cwd <repo-root> --path <worktree-path> --no-focus`.
- Without `HERDR_ENV`, plain `git worktree add` is the fallback.
- A lane's worktree is removed once that lane's pull request has merged, through herdr so the sidebar row
  goes with the checkout: look the checkout path up in `herdr worktree list` and pass the id it reports
  to `herdr worktree remove --workspace <id> --force`. In a sub-agent `$HERDR_WORKSPACE_ID` is NOT that
  id, it names the session's own workspace.
- `just worktrees-prune` in the dotfiles repository is the operator's bulk sweep of the same thing, read
  first with `--dry-run`. An agent never runs it: merged and clean is all it tests, and that cannot tell
  a finished lane from one a sibling agent is still working in.
- Sub-agents inherit this rule. A brief that sends work to a worktree carries the create line verbatim.

## GitButler

The `gitbutler` cask ships the desktop app and the `but` CLI, and the on-demand `gitbutler` skill carries
the command recipes. No repository on this machine is a GitButler project yet, so these rules are
conditional.

- In a repository where `but status` succeeds, `but` is the version-control interface: status, diffs,
  branches, commits, pushes and history edits. Invoke the `gitbutler` skill for syntax rather than
  guessing flags or translating git habits.
- Everywhere else keep using `git`. Never run `but setup` to make `but` work in a repository: it switches
  the checkout onto a `gitbutler/workspace` branch and installs hooks, which is the operator's call, not
  an agent's.
- Assume other agents are working in the same repository. Do not move, amend, squash, discard, commit,
  push or otherwise modify another agent's work unless asked.
- Use one GitButler branch per agent session and commit only what belongs to that session. Do not push or
  open a pull request unless asked.
- Amend an unpublished local commit when a follow-up fix clearly belongs with it instead of adding a
  fixup commit, and split unrelated changes within one file by hunk. Ask before rewriting pushed,
  reviewed or shared history.

`but agent setup` writes this text into `~/.claude/CLAUDE.md` and `~/.codex/AGENTS.md`, which chezmoi
renders from this partial, so a write there is erased by the next apply. Edit this section instead, and
leave out the `gitbutler-agent-setup` marker comments so the wizard never claims the rendered block.

## Work recaps

A work recap is always agent-initiated, never fired by a hook. Produce one when:

1. a pull request is opened and waiting on a human review, or one is auto-approved and merged;
1. a milestone is reached while a `/goal` runs;
1. overnight work finishes;
1. the day ends (the end-of-day summary);
1. the operator asks, through `/pns:work-recap`.

`/recap` is Claude Code's own built-in and must not be used for this.

The layout is exact:

```
Recap
==========

**Git**
- Branch: `<branch>` (open|merged, position in stack)
- Worktree: `<path>` (kept|removed)
- PR: #<n> (open|merged|none)
- Stack: `<name>` (<k> PRs, trunk main)

 Stack Graph
 -----------
  `main` (*trunk*)
  └─ `<branch-1>`  #<n>  *merged*  ✓
     └─ `<branch-2>`  #<n>  *open*  ×  ← *current*

A  <added file>
M  <modified file>
R  <old> -> <new>
D  <deleted file>

**Summary**
- <what got done, one line each, with its state: merged / open, waiting on CI / applied>

**In-Progress**
- `#<task>` <what is mid-flight and where it lives>
- Blocked on: <the one thing stopping it, or "nothing">

**Upcoming Agent Tasks**
- `#<task>` <next thing the agent will do, and any gate it waits on>

**User Tasks**
1. <the exact command or decision the operator owes, one per line>
```

Readability rules:

- The stack graph and the file list share ONE fenced code block, so the tree and the status column stay
  aligned both in a terminal and in Discord.
- File paths and branch names go in backticks.
- Keep the whole recap under 2000 characters, which is Discord's message limit, for the day it is
  forwarded there.
- Collapse a long file list to counts per status (`A 3  M 4  D 1`) rather than truncating mid-list.
- Short replies still hold for everything that is not the recap.

Where the data comes from: `pns recap git`, run in the worktree the work happened in, prints the Git
block, the stack graph and the file list already in this layout. It reads git for the branch, the
worktree, the trunk, the stack and the diff, and `gh` for the PR number and state. Paste its output
rather than composing those parts by hand, and never guess a PR number: `none` is `gh` saying there is
none, `unknown` is `gh` not answering.

Delivery: the recap goes in the chat reply, and `pns recap agent --stdin` forwards it to the
`#pns-events` Discord channel. It sanitizes the body and fits it under Discord's limit by collapsing the
file list and then shedding whole sections, never by cutting a line in half, and it never sheds User
Tasks. It prints one line saying where the post landed; claim the recap was posted only when that line
says it was.

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

When the `nvim` server fails to connect, run `~/.local/libexec/nvim-mcp/nvim-mcp-connect.sh --diagnose`
in the same pane through the shell tool or terminal. It prints the chosen target or the refusal,
including ambiguous candidates, without starting the server. The operator can export a listed
`NVIM_MCP_SOCKET` and start a new agent session there; changing a child shell's environment does not
retarget an existing server.

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
