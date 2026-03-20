# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

A [chezmoi](https://www.chezmoi.io/) dotfiles repository. Chezmoi manages files in
`~/.local/share/chezmoi/` (source state) and applies them to `$HOME` (target state). Files use chezmoi
naming conventions: `dot_` prefix maps to `.`, `private_` sets permissions, `executable_` sets +x, and
`.tmpl` suffix indicates Go templates.

## Key Commands

### Linting & Formatting

All lint/format tooling requires the Nix flake dev shell. Use the justfile shortcuts:

```bash
just l          # Run all linters (shellcheck, shfmt, mdformat, nixfmt)
just s          # Shellcheck only
just S          # shfmt (format shell files) only
just m          # mdformat only
just n          # nixfmt only
```

These all invoke `nix develop .#run --command ./scripts/lint.sh` with appropriate flags. The lint script
(`scripts/lint.sh`) auto-formats files in place and reports diffs.

To enter an interactive dev shell with all tools available: `nix develop`

### Chezmoi Operations

```bash
chezmoi status          # Show what would change
chezmoi diff            # Show diffs between source and target
chezmoi apply           # Apply source state to home directory
chezmoi add <FILE>      # Add a file to source state
chezmoi edit <FILE>     # Edit a template file (use this instead of editing .tmpl files directly)
```

All config changes (tms config, tmux config, bashrc, etc.) must go through the chezmoi source directory
(`~/.local/share/chezmoi/`), then be applied via `chezmoi apply` or `chezmoi apply <target>`. Never edit
target-state files directly — they will be overwritten on next apply.

**Important:** When using Claude Code, always specify target files or use `--exclude=templates`:

```
chezmoi apply --exclude=templates --force   # Apply all non-template files
chezmoi apply ~/.tmux.conf                  # Apply a specific non-template file
chezmoi diff --exclude=templates            # Diff non-template files
```

Never run bare `chezmoi apply` from Claude Code — it will fail on template files that call `keepassxc`.
Template files (`~/.bashrc`, `~/.gitconfig`, `~/.aws/credentials`) must be applied from an interactive
terminal with KeePassXC unlocked.

### Claude Code Settings

`dot_claude/settings.json` is managed by chezmoi and deployed to `~/.claude/settings.json`. It configures
auto-approved permissions for read-only tools (Read, Grep, Glob, WebFetch, WebSearch) and safe bash
commands (find, cat, ls, head, tail, wc, grep, tree) so they run without prompting.

### Git Hooks

Install pre-commit hook (runs full lint suite): `just h`

## Architecture

### Source-Only Files

Some files exist only for development and CI — they are excluded from `$HOME` via `.chezmoiignore`:
`justfile`, `scripts/`, `flake.nix`, `flake.lock`, `.envrc`, `.shellcheckrc`, `.editorconfig`, `assets/`,
`docs/`, `private/`, `README.md`, `LICENSE`, `.gitignore`. Only chezmoi-managed files (those with `dot_`,
`private_`, `run_`, etc. prefixes) are applied to the target state.

### Minimum Chezmoi Version

`.chezmoiversion` requires >= 2.62.3.

### Secrets Management

Secrets are managed via chezmoi's KeePassXC integration (`keepassxc-cli`). The database path is
configured in `.chezmoi.toml.tmpl`. Template files (`.tmpl`) use `{{ keepassxc "entry-name" }}` to pull
secrets at apply time. The `.install-password-manager.sh` hook auto-installs KeePassXC if missing.

### System Package Management

Packages are declared in `.chezmoidata/system_packages_autoinstall.yaml` under `packages.macos.homebrew`
with keys: `taps`, `formulae`, `casks`, `mas`. The `run_onchange_system_packages_autoinstall.sh.tmpl`
script generates a Brewfile from this data and runs `brew bundle --cleanup` whenever the data changes.

**Homebrew install workflow (for AI agents):**

1. Install the package immediately by running `brew install <formula>` (or `brew install --cask <cask>`).
1. If the install succeeds, add the package to `.chezmoidata/system_packages_autoinstall.yaml` in the
   appropriate list (`formulae`, `casks`, `taps`, or `mas`), maintaining alphabetical order.
1. Create a reminder for the user to run `chezmoi apply` at 22:00 local time (America/Denver) that day.

Do **not** run `chezmoi apply` directly — see the KeePassXC constraint above.

### Template Files

Seven files use chezmoi Go templates (`.tmpl` suffix): `.chezmoi.toml.tmpl`, `dot_bashrc.tmpl`,
`dot_gitconfig.tmpl`, `dot_aws/credentials.tmpl`, `dot_config/gh/private_hosts.yml.tmpl`,
`dot_config/atuin/config.toml.tmpl`, and `run_onchange_system_packages_autoinstall.sh.tmpl`. Templates
conditionally branch on `.chezmoi.os` (darwin/linux) and pull secrets from KeePassXC.
`dot_config/atuin/config.toml.tmpl` does not currently use secrets but retains the `.tmpl` suffix for
future `history_filter` integration (requires Rust regex, not bash glob patterns).

### Template Shellcheck Workaround

Shell templates (e.g., `dot_bashrc.tmpl`) contain Go template syntax that shellcheck cannot parse
directly. The lint script works around this by rendering templates first:
`CI=1 chezmoi execute-template --no-tty <file | shellcheck -`. The `CI=1` env var matters because the
template uses `{{- if (env "CI") }}` to branch — in CI it substitutes a fake HISTIGNORE value instead of
calling `keepassxc` (which would prompt interactively). When editing `.tmpl` shell files, ensure the CI=1
rendering path also produces valid shell.

### OS Targeting

The `.chezmoiignore` conditionally ignores paths by OS (e.g., `.config/yabai` is ignored on Linux).
Template files use `{{ if eq .chezmoi.os "darwin" }}` for macOS-specific content.

### Dev Environment (Nix Flake)

`flake.nix` defines two dev shells targeting `x86_64-linux` and `aarch64-darwin`:

- `default` — interactive shell with colored status output
- `run` — headless shell used by `just` and CI

Tools provided: chezmoi, shellcheck, shfmt, mdformat (with GFM plugin), nixfmt-tree.

### CI

GitHub Actions (`.github/workflows/lint.yml`) runs on `macos-latest`. It runs
`nix flake check --all-systems` and the full lint suite on pushes to main and PRs.

### Tmux Session Management

Sessions are managed by [tms (tmux-sessionizer)](https://github.com/jrmoulton/tmux-sessionizer), not
tmuxinator. `tms start` bootstraps all marked sessions with correct `session_path`. `#{session_path}` is
used in split/window bindings (`dot_tmux.conf`) so new panes inherit the session root directory.
`tmux-refresh.sh` (`dot_local/bin/executable_tmux-refresh.sh`) handles kill/purge/restart using tms.
`dot_config/tms/config.toml` is managed by chezmoi — bookmarks and marks configured there are
automatically deployed on apply.

### Bashrc Init Ordering

Zoxide must be initialized after all other `PROMPT_COMMAND`-modifying tools (especially starship) to
avoid zoxide's doctor warning. In `dot_bashrc.tmpl`, `eval "$(zoxide init bash)"` is placed after the
interactive-only block and before `export PATH`. Atuin init follows zoxide (both modify `PROMPT_COMMAND`;
atuin last).

### Shell History (Atuin)

Atuin replaces the bash history flush/reload `PROMPT_COMMAND` cycle that was racy across tmux panes.
History is stored in SQLite (`~/.local/share/atuin/history.db`), eliminating race conditions.
`dot_config/atuin/config.toml.tmpl` retains the `.tmpl` suffix for future `history_filter` integration
(requires Rust regex syntax, not bash glob patterns used by HISTIGNORE). Atuin's built-in
`secrets_filter` handles sensitive command filtering for now. Bash's built-in history (`HISTSIZE`,
`HISTFILE`, `shopt -s histappend`) is kept as a safety net — both systems write independently.

## Code Style

- Shell files: 2-space indent, case-indent enabled, simplified (`shfmt -i 2 -ci -s`). Always pass these
  flags explicitly — `.editorconfig` only covers `dot_fzf*` and `dot_bash*` patterns, and the Nix
  `default` shell hook wrapper only applies in interactive `nix develop` sessions, not when lint.sh is
  invoked via `nix develop .#run --command` (subprocess execution).
- Markdown: wrapped at 105 columns, non-consecutive numbering (`mdformat` with `.mdformat.toml`)
- Nix: formatted with `nixfmt-tree`
- ShellCheck directives: SC1090 and SC1091 are globally disabled (`.shellcheckrc`)

## Git Commits

Do not include `Co-Authored-By` lines in commit messages. Claude should never be listed as a co-author.

Separate logically distinct changes into their own commits. Each commit should be a single cohesive unit
of work.

## Security

`*bash_secret*` patterns are gitignored (`.gitignore`) to prevent accidental commits of Bash secret
files.
