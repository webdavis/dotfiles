<!-- Keep this file evergreen. Avoid adding point-in-time content (current sprint
goals, active branches, temporary workarounds) that wouldn't make sense if
multiple workstreams, PRs, or branches were in progress simultaneously.
Document general principles, workflows, and architecture — not transient
project state. -->

# Global Rules

## Collaboration style

- Terse, direct responses. No trailing recap unless asked.
- Verify before asserting; show evidence (commands, output).
- Separate logically distinct changes into their own commits.
- **No** trailing whitespace; blank lines included.
- **Don't open responses with apologies or affirmations.** Not "You're right!", not "Great question!",
  not "Sorry!", not "Absolutely!". State the thing directly. Never apologize before disagreeing.
- **One precise question at a time** when a spec is ambiguous. Don't offer three options when one is
  clearly better; state a recommendation with brief reasoning and ask only if it seems contested.
- **Never create unsolicited documentation.** No `README.md`, `CHANGELOG.md`, `NOTES.md`, etc. unless
  explicitly asked. Don't add docstring-style comments unless asked.
- **Use the `humanizer` skill on prose.** Commit-message bodies, PR descriptions, docs, and any chat
  response longer than a paragraph. Strips common AI-writing tells (em-dash overuse, rule of three,
  promotional language, etc.).

## Verification and sources of truth

For claims about anything with a canonical source of truth — library APIs, function signatures, CLI
flags, config schemas, protocol details, version-specific behavior, syntax, error messages, or changes
since early 2025 — check the source before asserting. Verification priority:

1. **Local source of truth:** read the installed package (`node_modules/`, `site-packages/`, `vendor/`),
   run `--help` or `man`, read the file being modified, check lockfiles for installed versions.
1. **Official documentation via WebFetch:** the vendor's docs site, the language's stdlib docs, the
   project's README or spec.
1. **WebSearch:** only when the above are unavailable, or when the question is about recent changes.
1. **Training data:** last resort. When falling back to this, prefix the claim with
   `from training, not verified:` so the user can decide whether to trust it.

Reasoning, design discussion, architectural tradeoffs, and established general knowledge do not require
verification. The rule fires on specific, verifiable claims, not on analysis.

## Verification before completion

Always check work by testing before claiming it's done. "Should work" ≠ "works"; "looks right" ≠
"correct"; evidence before assertions, always.

- After editing code: run the relevant tests (project suite or a targeted invocation) and show the
  output. If there's no test, write a minimal reproducer and run it.
- After editing a config: render, parse, or lint it (`shellcheck`, `jq empty`, `yq eval '.'`,
  `taplo format --check`, template-render the file) and show the result.
- After editing a shell script: `shellcheck` it and, where possible, run the script with representative
  inputs.
- After a multi-step refactor: run the project's full check command (`just l`, `make check`, `npm test`,
  etc.) before claiming done.

## Destructive action gates

Never run any of the following without explicit per-invocation user confirmation. A blanket "yes" from
earlier in the session does not carry over.

- `rm -rf`, or `rm` on anything outside a scratch directory
- `git push --force` (use `--force-with-lease` instead; force-push to `main`/`master` is never OK),
  `git reset --hard`, `git clean -fd`, `git branch -D`, `git checkout .`
- Dropping DB tables or schemas; `killall`; `shutdown`; `dd`; any disk-level operation
- Bypassing safety checks: `--no-verify`, `--no-gpg-sign`, `--no-hooks`, `--skip-checks`
- `chezmoi apply` from an automated context on template files — those require an interactive terminal
  with KeePassXC unlocked (agent-driven applies use `--exclude=templates`)

## Code discipline

- **YAGNI.** Don't add features, refactor surrounding code, introduce abstractions, or write fallbacks
  for scenarios that can't happen. Bug fixes don't need surrounding cleanup. A one-shot operation doesn't
  need a helper. Three similar lines beat a premature abstraction.
- **No backwards-compat hacks for unshipped code.** No `// removed` comments, no deprecated alias
  re-exports, no feature flags for code that hasn't been released. If something is dead, delete it.
- **Glob before creating.** Before creating a new script, config, or doc, glob/grep for existing files
  that cover the same concern. Prefer editing an existing file over creating a new one.

## Shell scripts

- Strict mode: every `#!/usr/bin/env bash` script begins with `set -euo pipefail`.
- Every variable expansion is double-quoted unless there's a concrete reason not to.
- Every `cd` is followed by `|| exit` (or `|| return` in functions).
- No unquoted globs.
- **Prefer stable, system-shipped tools over cutting-edge alternatives.** Scripts should favor
  widely-available binaries (bash, coreutils, `sed`, `awk`, `grep`, `jq`) over newer tools (`fd`, `rg`,
  `sd`). The modern tools are great interactively; scripts need boring reliability and broad portability.
- Use ISO 8601 timestamps (`YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS`) in any output you produce. macOS BSD
  `date` lacks `-Is`; use `gdate -Is` (GNU coreutils) or a portable `date -u +"%Y-%m-%dT%H:%M:%SZ"`.

## Git Commits

**Never add `Co-Authored-By: Claude` (or any Claude/Anthropic co-author trailer) to commit messages.**
This applies to all commits, amends, and squashes, in every repository. Do not include the "🤖 Generated
with Claude Code" footer either. Commits should look as if the user authored them directly.

Use the `conventional-commits` skill to format commit messages. A global `prepare-commit-msg` hook at
`~/.config/git/hooks/` prepopulates conventional commit messages via Claude haiku; set `SKIP_AI_COMMIT=1`
to bypass for a single commit.

## Task tracking (Todoist)

Use the `todoist-cli` skill (invokes the `td` CLI) to manage tasks. On any non-trivial work:

- **Before starting:** list relevant tasks (`td task list -f "/<project>"` or by label) to understand
  priorities, dependencies, and deadlines. If the work isn't already a task, create one.
- **While working:** create follow-up tasks for issues discovered but deferred, and any dependencies that
  surface. Re-prioritize if what you learn changes what should come next.
- **After completing:** mark the corresponding task(s) complete. Re-prioritize the remaining backlog if
  the completion changes the ordering.

## Tool preferences

- **Prefer local CLI tools over MCP servers** when both can do the job. CLIs are faster, have more
  predictable output, and don't add a network or auth layer. Reach for MCP only when the CLI genuinely
  can't cover the need (e.g., authenticated SaaS APIs without a first-class CLI).

## Backups

### Location

All backups live in `~/workspaces/backups/`.

### Naming convention

`YYYY-MM-DDTHH-MM-SS.Name.backup[.ext]`

- Date and time come first (sorts chronologically)
- Hyphens between date and time components
- A period between the timestamp and the name
- Hyphens within the name (replace spaces)
- `.backup` goes after the file or folder name
- File extension, if present, comes last
- Same convention applies to both files and folders

Examples:

- `2026-04-20T14-30-00.settings-json.backup.json`
- `2026-04-20T14-30-00.my-project.backup/`

## Toolchain (locked-in choices — do not suggest migrating)

- **Shell:** bash. Not switching to zsh.
- **Multiplexer:** tmux. Not switching to zellij or cmux.
- **Version manager:** not using `mise`. Nix flakes handle per-project toolchain needs.
- **File manager / git TUI:** neither `yazi` nor `lazygit` wanted.
- **Terminal:** Ghostty.
- **Editor:** Neovim.
- **Secrets:** KeePassXC. Not migrating to sops/age for per-machine secrets.

## Agents

- Prefer parallel subagents for independent work.
- Stop at environmental blockers (brew install, KeePassXC unlock, destructive rm -rf, long-running VM
  clones) and surface them rather than attempting them blindly.
