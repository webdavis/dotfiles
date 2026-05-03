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

**Never run bare `chezmoi apply` from Claude Code** — the following templates call `keepassxc` and will
fail without an interactive TTY: `~/.gitconfig`, `~/.aws/credentials`, `~/.claude.json`,
`~/.composio/user_data.json`, `~/.config/atuin/config.toml`, `~/.config/himalaya/config.toml`,
`~/Library/Application Support/espanso/match/identity.yml`,
`~/Library/Application Support/gogcli/credentials.json`. Apply those from an interactive terminal with
KeePassXC unlocked. Non-KeePassXC templates (e.g. `~/.bashrc`, `~/.claude/settings.json`) are safe to
apply from automation.

### Claude Code Settings

`private_dot_claude/modify_settings.json` is a chezmoi **modify-template** (no `.tmpl` extension by
chezmoi convention) that selectively enforces a fixed set of stable fields in `~/.claude/settings.json`.
On every `chezmoi apply`, the script reads the current target file, overlays the stable fields below via
`setValueAtPath`, and writes the merged result back. Anything not in the stable list passes through
untouched, so `/config` toggles (e.g., `voiceEnabled`, `useAutoModeDuringPlan`, `alwaysThinkingEnabled`)
drift freely without forcing a chezmoi resync.

**Chezmoi-controlled stable fields:**

- `permissions.allow` (read-only tools), `permissions.deny` (`.env`, `secrets/**`, `.ssh/id_*`, etc.),
  `permissions.defaultMode` = `bypassPermissions`.
- `hooks`: `UserPromptSubmit` marks session start, `Stop` pulses Hue lights, `Notification`
  (`permission_prompt` matcher) fires alerter, `PreToolUse` (`Bash` matcher) writes to
  `~/.claude/audit.log`.
- `statusLine`, `enabledPlugins`, `cleanupPeriodDays` (= 36525, effectively disables session cleanup).

**Free-drift (Claude Code owns):** `alwaysThinkingEnabled`, `useAutoModeDuringPlan`, `voiceEnabled`,
`skipDangerousModePermissionPrompt`, and any future setting `/config` adds.

**Promote a `/config` toggle to stable** by adding a `setValueAtPath` call for that key in
`private_dot_claude/modify_settings.json` and committing.

Background: `/config` writes ergonomic toggles directly into `~/.claude/settings.json` (verified
empirically), and Claude Code does not provide a user-level `~/.claude/settings.local.json` for overrides
— only project-scope `.claude/settings.local.json` exists. The modify-template approach is the cleanest
way to keep policy fields under chezmoi control while letting `/config` mutate everything else freely.
See https://www.chezmoi.io/user-guide/manage-different-types-of-file/ for the `modify_` template +
`setValueAtPath` reference.

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
`dot_config/himalaya/config.toml.tmpl`, `dot_config/osquery/osquery.conf.tmpl`,
`Library/LaunchAgents/*.plist.tmpl`, `Library/Application Support/espanso/match/identity.yml.tmpl`,
`Library/Application Support/gogcli/credentials.json.tmpl`, and scripts in `.chezmoiscripts/`. Templates
conditionally branch on `.chezmoi.os` and, where they pull secrets, call `keepassxc`.

### Template Shellcheck Workaround

Shell templates contain Go template syntax that shellcheck can't parse directly. The lint script renders
first: `CI=1 chezmoi execute-template --no-tty <file | shellcheck -`. Only `dot_bashrc.tmpl` is rendered;
it no longer calls `keepassxc`, so the `CI=1` env var is defensive (vestigial from an earlier version
where bashrc had a CI-vs-interactive branch). Other templates with CI branches (e.g. `identity.yml.tmpl`)
are not shell-linted.

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
`PROMPT_COMMAND`; atuin last). `bash-preexec` is sourced explicitly from Homebrew (atuin 18.x stopped
bundling it) BEFORE `atuin init` — atuin's `__atuin_preexec`/`__atuin_precmd` and our long-running
command timer both register into `preexec_functions` / `precmd_functions`. A naked `DEBUG` trap would
clobber atuin's recording. Direnv hook runs early. Carapace universal completion loads after
`gh completion`.

### Shell History (Atuin)

Atuin daemon mode is enabled (`[daemon] enabled = true; autostart = false`). The daemon's lifecycle is
managed by `~/Library/LaunchAgents/com.webdavis.atuin-daemon.plist` (`KeepAlive=true`,
`atuin daemon start --force` so a stale socket from a prior crash auto-cleans on restart). Command
recording is decoupled from `PROMPT_COMMAND` via the daemon. History stored in SQLite at
`~/.local/share/atuin/history.db`. Sync v2 records opt-in (`[sync] records = true`) future-proofs the
local DB schema even though `auto_sync = false`. `filter_mode = "host"` restricts Ctrl-R to the current
machine's history. Bash's built-in history is fully removed — atuin owns all recording.

**Diagnostic ladder** when history stops recording:

```bash
atuin doctor                              # built-in: socket, db, env, shell hooks
launchctl list | grep atuin               # status: '0' = healthy, '-' = not running
ps aux | grep '[a]tuin daemon'            # daemon process
tail ~/.local/log/atuin-daemon.log        # crash messages
atuin daemon status; atuin --version      # 'Version' line should equal 'atuin <ver>'
```

Past failures: stale `~/.local/share/atuin/atuin.sock` causing `EADDRINUSE` restart loops (now
self-healing via `--force`); missing `bash-preexec` after atuin 18.x dropped its bundle (now sourced
explicitly in bashrc before `atuin init`); `brew` upgrading atuin in-place while the daemon kept running
stale code, silently breaking recording via gRPC schema drift (now self-healing via
`.chezmoiscripts/run_after_45-bounce-atuin-daemon-on-upgrade.sh.tmpl` plus a mtime check in
`dot_bashrc.tmpl` after `atuin init`). `atuin status` is for *sync* status only and errors when not
logged in — it is not a "is the daemon working" check; use `atuin daemon status` (reports `Version`,
`Protocol`, `Healthy`) for daemon health.

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
  `client-session-changed` hook) and displays `<previous-session>:<active-window> <emoji>`. The plugin
  script lives at `~/.local/bin/tmux-last-proc.sh` under chezmoi control, and
  `.chezmoiscripts/run_after_70-install-tmux2k-last-proc.sh.tmpl` copies it into
  `~/.tmux/plugins/tmux2k/plugins/last-proc.sh` on every `chezmoi apply` (silent no-op if tmux2k isn't
  installed yet — fresh machine runs `prefix + I` first). Colors come from `@tmux2k-last-proc-colors` set
  in `dot_tmux.conf`; no need to edit tmux2k's `main.sh` because `get_plugin_colors` falls back to
  user-set tmux options. Direct placement of the file under `dot_tmux/...` is avoided because tpm's
  install check (`if [ -d $plugin_dir ]; skip`) would treat a chezmoi-created path as "already installed"
  and skip cloning tmux2k entirely.

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
