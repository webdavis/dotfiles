<!-- Keep this file evergreen. Avoid adding point-in-time content (current sprint
goals, active branches, temporary workarounds) that wouldn't make sense if
multiple workstreams, PRs, or branches were in progress simultaneously.
Document general principles, workflows, and architecture — not transient
project state. -->

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

A [chezmoi](https://www.chezmoi.io/) dotfiles repository. Chezmoi manages files in
`~/.local/share/chezmoi/` (source state) and applies them to `$HOME` (target state). Files use chezmoi
naming conventions: `dot_` prefix maps to `.`, `private_` sets permissions, `executable_` sets +x, and
`.tmpl` suffix indicates Go templates.

## Key Commands

### Linting & Formatting

All lint/format tooling runs via the Nix flake dev shell. Use the justfile shortcuts:

```bash
just l          # Run all linters (shellcheck, shfmt, mdformat, nixfmt, taplo, jq, yq)
just s          # Shellcheck only
just S          # shfmt (format shell files) only
just m          # mdformat only
just n          # nixfmt only
just t          # taplo (TOML) only
just j          # jq (JSON) only
just y          # yq (YAML) only
```

These invoke `nix develop .#run --command ./scripts/lint.sh` with the appropriate flag. The lint script
auto-formats in place and reports diffs. The pre-commit hook runs `just l` — install it with `just h`.

To enter an interactive dev shell with all tools: `nix develop`.

### Chezmoi Operations

```bash
just d                                      # chezmoi diff --exclude=templates
just a                                      # chezmoi apply --exclude=templates --force
just c                                      # nix flake check --all-systems
chezmoi status                              # show pending changes
chezmoi diff                                # diff all (including templates)
chezmoi edit <file>                         # edit a template (prefer over direct edit of .tmpl)
```

**Important for AI agents:** always use `--exclude=templates` or apply specific non-template files by
name:

```bash
chezmoi apply --exclude=templates --force   # safe — no KeePassXC prompt
chezmoi apply ~/.tmux.conf                  # specific non-template file
chezmoi diff --exclude=templates            # diff non-template files
```

**Never run bare `chezmoi apply` from Claude Code** — template files (`~/.bashrc`, `~/.gitconfig`,
`~/Library/Application Support/espanso/match/identity.yml`, `~/.claude/settings.json`) call `keepassxc`
and will fail without an interactive TTY. Those applications are the user's step, from an interactive
terminal with KeePassXC unlocked.

### Claude Code Settings

`dot_claude/settings.json.tmpl` is managed by chezmoi and deploys to `~/.claude/settings.json`.
Configures `defaultMode: "bypassPermissions"` with an allow-list for read-only tools and a deny list for
sensitive paths (`.env`, `secrets/**`, `.ssh/id_*`). Hooks wired: `UserPromptSubmit` marks session start,
`Stop` pulses Hue lights if session >5 min, `Notification` (permission_prompt matcher) fires alerter,
`PreToolUse` (Bash matcher) appends to `~/.claude/audit.log`. `alwaysThinkingEnabled: true` forces
extended thinking; `cleanupPeriodDays: 36525` effectively disables session cleanup.

### Git Hooks

Install pre-commit hook (runs full lint): `just h`. Commits also trigger a global `prepare-commit-msg`
hook at `~/.config/git/hooks/` that prepopulates conventional commit messages via Claude haiku.

Bypass the AI commit hook: `SKIP_AI_COMMIT=1 git commit ...`.

## Architecture

### Source-Only Files

Some files are dev/CI only and are excluded from `$HOME` via `.chezmoiignore`: `justfile`, `scripts/`,
`flake.nix`, `flake.lock`, `.envrc`, `.shellcheckrc`, `.editorconfig`, `.mdformat.toml`, `assets/`,
`docs/`, `private/`, `README.md`, `LICENSE`, `.gitignore`, `.worktrees/`, `**/.DS_Store`. Only
chezmoi-managed files (`dot_`, `private_`, `run_`, etc. prefixes) reach the target state.

### Minimum Chezmoi Version

`.chezmoiversion` requires >= 2.62.3.

### Secrets Management

Secrets are managed via chezmoi's KeePassXC integration (`keepassxc-cli`). The database path is
configured in `.chezmoi.toml.tmpl`. Template files (`.tmpl`) use `{{ keepassxc "entry-name" }}` or
`{{ keepassxcAttribute "entry-name" "attr-name" }}` to pull secrets at apply time. The
`.install-password-manager.sh` hook auto-installs KeePassXC if missing.

### System Package Management

Packages declared in `.chezmoidata/system_packages_autoinstall.yaml` under `packages.macos.homebrew` with
keys: `taps`, `formulae`, `casks`, `mas`. The
`.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl` script generates a Brewfile from this
data and runs `brew bundle --cleanup` whenever the data changes. Prerequisites:
`run_once_before_00-install-homebrew.sh.tmpl` ensures `/opt/homebrew/bin/brew` exists on fresh machines.

**Homebrew install workflow (for AI agents):**

1. Install the package immediately: `brew install <formula>` or `brew install --cask <cask>`.
1. On success, add it to `.chezmoidata/system_packages_autoinstall.yaml` in the appropriate list
   (formulae, casks, taps, mas), maintaining alphabetical order.
1. Remind the user to run `chezmoi apply` at 22:00 local time (America/Denver) that day.

Do **not** run `chezmoi apply` directly — see the KeePassXC constraint above.

### Template Files

Template files use chezmoi Go templates (`.tmpl` suffix) and live alongside their target files. Notable
templates: `.chezmoi.toml.tmpl`, `dot_bashrc.tmpl`, `dot_gitconfig.tmpl`, `dot_aws/credentials.tmpl`,
`dot_config/gh/private_hosts.yml.tmpl`, `dot_config/atuin/config.toml.tmpl`,
`dot_config/osquery/osquery.conf.tmpl`, `dot_claude/settings.json.tmpl`,
`Library/LaunchAgents/*.plist.tmpl`, `Library/Application Support/espanso/match/identity.yml.tmpl`, and
scripts in `.chezmoiscripts/`. Templates conditionally branch on `.chezmoi.os` and pull secrets from
KeePassXC. Bashrc uses a `{{- if (env "CI") }}` branch so CI rendering doesn't call keepassxc.

### Template Shellcheck Workaround

Shell templates contain Go template syntax that shellcheck can't parse directly. The lint script renders
first: `CI=1 chezmoi execute-template --no-tty <file | shellcheck -`. The `CI=1` env var drives the
template's keepassxc-avoiding branch.

### OS Targeting

`.chezmoiignore` conditionally ignores paths by OS (e.g., `.config/yabai` and `Library` on Linux).
Template files use `{{ if eq .chezmoi.os "darwin" }}` for macOS-specific content.

### Dev Environment (Nix Flake)

`flake.nix` provides two dev shells (for `x86_64-linux` and `aarch64-darwin`):

- `default` — interactive shell with colored status output.
- `run` — headless shell used by `just` and CI.

Tools provided: chezmoi, shellcheck, shfmt, mdformat (with GFM plugin), nixfmt-tree, taplo, jq, yq-go.

### CI

GitHub Actions (`.github/workflows/lint.yml`) runs on `macos-latest`. Runs
`nix flake check --all-systems` and `./scripts/lint.sh` on pushes to main and PRs.

### Tmux Session Management

Sessions are managed by [sesh](https://github.com/joshmedeski/sesh). Named sessions live in
`dot_config/sesh/sesh.toml` (13 configured: uriel, openclaw, homelab, ivy, casually-concerned, dotfiles,
nvim-config, essential-feed, webdavis-profile, job-hunting, justdavis-ansible, maeve, dresden).
`~/.local/bin/sesh-bootstrap.sh` creates the three default sessions (uriel/openclaw/homelab) and is
invoked from bashrc, `tmux-refresh.sh`, and the Claude Code LaunchAgent. `prefix + o` opens the fuzzy
picker; `prefix + C-o <letter>` jumps to a named session via the SESH key table; `prefix + \\` toggles
last session; `prefix + R` reloads `~/.tmux.conf`.

### Git Worktrees (Worktrunk)

Git worktrees are managed by [worktrunk](https://worktrunk.dev/). Config in
`dot_config/worktrunk/config.toml`: squash+rebase+remove merges; array-of-tables `[[pre-merge]]` hooks
run `just l` and `just test` before merge. `wt up` rebases every worktree against upstream safely.

### Bashrc Init Ordering

Starship initializes early; zoxide and atuin initialize after the interactive block (both modify
`PROMPT_COMMAND`; atuin last). Atuin sources `bash-preexec`, which the long-running command timer uses
via `preexec_functions` / `precmd_functions` — a naked `DEBUG` trap would clobber atuin's recording.
Direnv hook runs early. Carapace universal completion loads after `gh completion`.

### Shell History (Atuin)

Atuin daemon mode is enabled (`[daemon] enabled = true; autostart = true`). Command recording is
decoupled from `PROMPT_COMMAND` via the daemon. History stored in SQLite at
`~/.local/share/atuin/history.db`. Sync v2 records opt-in (`[sync] records = true`) future-proofs the
local DB schema even though `auto_sync = false`. `filter_mode = "host"` restricts Ctrl-R to the current
machine's history; switch to `global` if cross-machine recall becomes important. Bash's built-in history
is fully removed — atuin owns all recording.

### AI Commit Messages

Global `core.hooksPath = ~/.config/git/hooks` activates a `prepare-commit-msg` hook that: truncates the
staged diff to 5 KB, pipes it to `claude -p --model=haiku` with a 4-second timeout, and prepopulates the
commit editor with the returned conventional message. Bails on merge/rebase/cherry-pick. Set
`SKIP_AI_COMMIT=1` to bypass. Chains to repo-local `.git/hooks/prepare-commit-msg` if present.

### Long-running Command Notifier

`dot_bashrc.tmpl` registers `__cmd_notify_preexec` and `__cmd_notify_precmd` via bash-preexec (atuin's
framework). Commands ≥ 30s fire an `alerter` macOS notification; ≥ 5 min additionally pulse Hue lights
via `~/.local/bin/hue-pulse.sh`. Known interactive TUIs (vim/less/top/ssh/tmux/claude/fzf) are skipped.

### Tmux Window/Pane Status Indicators

Passive indicators via tmux2k:

- **Window list:** each window's active pane gets an emoji (🤖 agents, 🧪 test runners, 🔨 build tools, ⏳
  other) via `~/.local/bin/tmux-window-emoji.sh` called from `@tmux2k-window-list-format`.
- **Right-side status:** a custom tmux2k plugin (`last-proc`) reads `@prev-session` (set by the
  `client-session-changed` hook) and displays `<previous-session>:<active-window> <emoji>`.

Replaces the default battery slot in `@tmux2k-right-plugins` with `last-proc network ram`.

## Code Style

- Shell files: 2-space indent, case-indent enabled, simplified (`shfmt -i 2 -ci -s`). Always pass these
  flags explicitly — `.editorconfig` only covers `dot_fzf*` and `dot_bash*` patterns, and the Nix
  `default` shell hook wrapper only applies in interactive `nix develop` sessions, not when lint.sh is
  invoked via `nix develop .#run --command` (subprocess execution).
- Markdown: wrapped at 105 columns, non-consecutive numbering (`mdformat` with `.mdformat.toml`).
- Nix: formatted with `nixfmt-tree`.
- TOML: formatted with `taplo`. `dot_aerospace.toml` is excluded (preserves user's visual alignment).
- ShellCheck directives: SC1090 and SC1091 are globally disabled (`.shellcheckrc`).

## Git Commits

**Never include `Co-Authored-By` lines in commit messages.** Claude is never listed as a co-author.

Separate logically distinct changes into their own commits. Each commit should be a single cohesive unit
of work.

## Security

- `*bash_secret*` patterns are gitignored to prevent accidental commits of Bash secret files.
- Claude Code settings include a deny list for sensitive paths (`.env`, `secrets/**`, `credentials.json`,
  `.aws/credentials`, `.ssh/id_*`) that applies even under `bypassPermissions`.
- KeePassXC database is the single source of truth for secrets pulled into templates.
