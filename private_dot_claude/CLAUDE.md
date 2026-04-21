<!-- Keep this file evergreen. Avoid adding point-in-time content (current sprint
goals, active branches, temporary workarounds) that wouldn't make sense if
multiple workstreams, PRs, or branches were in progress simultaneously.
Document general principles, workflows, and architecture — not transient
project state. -->

# Global CLAUDE.md

## Collaboration style

- Terse, direct responses. No trailing recap unless asked.
- Verify before asserting; show evidence (commands, output).
- Separate logically distinct changes into their own commits.
- **No `Co-Authored-By: Claude` lines in commits.** Claude is never an author.

## Toolchain (locked-in choices — do not suggest migrating)

- **Shell:** bash (10+ years). Not switching to zsh.
- **Multiplexer:** tmux. Not switching to zellij or cmux.
- **Version manager:** not using `mise`. Nix flakes handle per-project toolchain needs.
- **File manager / git TUI:** neither `yazi` nor `lazygit` wanted.
- **Terminal:** Ghostty. Not switching.
- **Editor:** Neovim (overhaul is a separate sub-project).
- **Secrets:** KeePassXC via chezmoi's `{{ keepassxc ... }}` templates. Not migrating to sops/age for
  per-machine secrets.

## Workflow defaults

- **Chezmoi:** from automation, always use `--exclude=templates` —
  `chezmoi apply --exclude=templates --force`. Template files (bashrc, gitconfig, espanso identity,
  Claude settings) require an interactive terminal with KeePassXC unlocked and are therefore the user's
  step, not an agent's.
- **Git:** `pull.rebase=true`, `push.autoSetupRemote=true`, `commit.verbose=true`. Global
  `prepare-commit-msg` at `~/.config/git/hooks/` prepopulates conventional commit messages via Claude
  haiku; set `SKIP_AI_COMMIT=1` to bypass.
- **Agents:** prefer parallel subagents for independent work; stop at environmental blockers (brew
  install, KeePassXC unlock, destructive rm -rf, long-running VM clones) and surface them to the user
  rather than attempt them blindly.
