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

## Git Commits

**Never add `Co-Authored-By: Claude` (or any Claude/Anthropic co-author trailer) to commit messages.**
This applies to all commits, amends, and squashes, in every repository. Do not include the "🤖 Generated
with Claude Code" footer either. Commits should look as if the user authored them directly.

A global `prepare-commit-msg` hook at `~/.config/git/hooks/` prepopulates conventional commit messages
via Claude haiku. Set `SKIP_AI_COMMIT=1` to bypass for a single commit.

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
