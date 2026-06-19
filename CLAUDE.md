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
auto-formats in place and reports diffs. On commit, the per-repo `.githooks/pre-commit` hook runs
`just lint-check` (check-only) — auto-wired via the user-wide dispatcher, no install step. See Git Hooks.

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
`~/.config/moshi/setting.json`, `~/Library/Application Support/Claude/claude_desktop_config.json`,
`~/Library/Application Support/espanso/match/identity.yml`,
`~/Library/Application Support/gogcli/credentials.json`, and the chezmoiscript
`.chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl` (one-time setup; once it runs successfully
on a given machine, automation can resume). Apply those from an interactive terminal with KeePassXC
unlocked. Non-KeePassXC templates (e.g. `~/.bashrc`, and `~/.claude/settings.json` now that its
modify-template no longer pulls from KeePassXC) are safe to apply from automation.

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
- `hooks`: `UserPromptSubmit` marks session start, `Stop` pulses Hue lights and posts a moshi push
  notification (`claude-moshi-notify.sh`, async; the script reads its webhook secret from the 0600
  `~/.config/moshi/setting.json`, so the hook command carries no secret), `Notification`
  (`permission_prompt` matcher) fires alerter, `PreToolUse` (`Bash` matcher) writes to
  `~/.claude/audit.log`.
- `statusLine`, `enabledPlugins`, `cleanupPeriodDays` (= 36525, effectively disables session cleanup),
  `autoUpdatesChannel` (= `stable`, pins the release channel so updates lag `latest`),
  `remoteControlAtStartup` (= `true`, starts the Remote Control bridge every session).

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

Both hooks live in the **user-wide** hooks dir — `core.hooksPath = ~/.config/git/hooks` (set in
`dot_gitconfig.tmpl`), so they apply to every repo:

- **`prepare-commit-msg` — user-wide AI commit messages.** Prepopulates a Conventional Commits message
  via Claude Sonnet (internals under **AI Commit Messages** below). Bails on `-m`/merge/rebase; bypass
  with `SKIP_AI_COMMIT=1`.
- **`pre-commit` — per-repo lint, via a dispatcher.** `dot_config/git/hooks/executable_pre-commit` runs
  in every repo but only acts when the repository tracks an executable `.githooks/pre-commit`, which it
  then `exec`s. This repo's `.githooks/pre-commit` runs `just lint-check` (check-only — reports drift,
  never mutates the tree or index). No install step: the dispatcher is user-wide and the repo hook is
  committed with its executable bit.

**Why a dispatcher, not `git config core.hooksPath .githooks`?** `core.hooksPath` is single-valued, so a
per-repo override shadows the user-wide `prepare-commit-msg`. The dispatcher keeps the global hook
authoritative while letting any repo opt into pre-commit checks. **Do not reintroduce Git LFS here** —
`git lfs install` writes exactly such an override, and this repo tracks no LFS files.

Bypass all hooks for one commit: `git commit --no-verify`.

## Architecture

### Source-Only Files

Some files are dev/CI only and are excluded from `$HOME` via `.chezmoiignore`: `justfile`, `scripts/`,
`.githooks/`, `flake.nix`, `flake.lock`, `.envrc`, `.shellcheckrc`, `.editorconfig`, `.mdformat.toml`,
`assets/`, `docs/`, `private/`, `README.md`, `LICENSE`, `.gitignore`, `.worktrees/`, `**/.DS_Store`. Only
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

Third-party taps whose formulae or casks must be trusted under Homebrew's `HOMEBREW_REQUIRE_TAP_TRUST`
gate are listed under a `trusted_taps` key in the same data file. A pre-bundle loop in
`run_onchange_before_10-system-packages.sh.tmpl` runs `brew trust --tap` for each before `brew bundle`,
so the bundle does not refuse to load them. Add a tap there when `brew bundle` reports it as untrusted.

**Homebrew install workflow (for AI agents):**

1. Install the package immediately: `brew install <formula>` or `brew install --cask <cask>`.
1. On success, add it to `.chezmoidata/system_packages_autoinstall.yaml` in the appropriate list
   (formulae, casks, taps, mas), maintaining alphabetical order.
1. Remind the user to run `chezmoi apply` when appropriate.

Do **not** run `chezmoi apply` directly — see the KeePassXC constraint above.

### macOS Defaults Management

Two `.chezmoidata/` files declaratively track macOS settings; two `.chezmoiscripts/` runners apply them
at `chezmoi apply` time on darwin (no-op on Linux):

- `.chezmoidata/macos_defaults.yaml` + `.chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl` —
  per-user `defaults write` records, plus a `killall` list (Dock/Finder/SystemUIServer/cfprefsd; cfprefsd
  kill is required for plist changes to take effect immediately).
- `.chezmoidata/macos_system_setup.yaml` +
  `.chezmoiscripts/run_onchange_after_41-macos-system-setup.sh.tmpl` — sudo system commands (one
  `sudo -v` upfront, then loop). Early-returns when the array is empty.

**Daily workflow:**

| Operation                           | Command                                          |
| ----------------------------------- | ------------------------------------------------ |
| Discover available domains          | `just defaults-list`                             |
| Browse one domain's keys            | `just defaults-show <domain>`                    |
| Bulk inspection (paged)             | `just defaults-dump`                             |
| Capture a setting into YAML         | `just defaults-capture <domain> <key> [current]` |
| Check for drift                     | `just D`                                         |
| Force reapply (revert disk to YAML) | `just defaults-apply`                            |

The capture helper is the canonical way to add a tracked setting: toggle it in System Settings, run
`just defaults-capture`, then `chezmoi apply` to commit. The helper refuses to silently overwrite a
tracked entry whose live value diverges from YAML (exits 4) — resolve via `just defaults-apply` to
revert, or hand-edit YAML to capture the new intent.

**Aerospace required defaults:** `com.apple.dock mru-spaces=false` is the single most common Aerospace
breakage. Several `com.apple.WindowManager` keys (Stage Manager, Sequoia tiling) are recommended off. See
the design spec in the chezmoi source tree at
`docs/superpowers/specs/2026-05-05-macos-defaults-management-design.md` for the full list.

**Implementation gotchas that future maintainers must not "clean up":**

- **`drift.sh` requires `shopt -s lastpipe`** (line 14). Bash's default behavior runs the right-hand side
  of a pipeline in a subshell, so `drift_count` increments inside `yq | while ...` would be discarded
  after the loop. Without `lastpipe`, `just D` would always exit 0 even when drift exists — silent false
  negative. The setting is a correctness requirement, not cosmetic.
- **The Tier 1 runner template uses `{{ if index . "host" }}`, not `{{ if .host }}`.** Go's
  `text/template` errors with `map has no entry for key "host"` when the YAML record has no `host` field,
  which is the common case. The `index` form returns the empty value for absent keys (treated as falsy by
  `if`); the `.field` form throws. Don't simplify.

### Template Files

Template files use chezmoi Go templates (`.tmpl` suffix) and live alongside their target files (e.g.
`.chezmoi.toml.tmpl`, `dot_bashrc.tmpl`, `dot_gitconfig.tmpl`, and scripts in `.chezmoiscripts/`).
Templates conditionally branch on `.chezmoi.os` and, where they pull secrets, call `keepassxc`.

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

### Herdr Workspace Management

Workspaces (project-anchored tab groups, ≈ tmux sessions) are configured at
`dot_config/herdr/config.toml`. Eight quick-jump chords in the `prefix+ctrl+<letter>` namespace map to
active project paths; see the design spec at
`docs/superpowers/specs/2026-06-18-tmux-to-herdr-migration-design.md` for the full mapping table.
`~/.bashrc` lands a fresh interactive shell inside the `homelab` workspace on every terminal launch; the
other seven workspaces are on-demand via their jump chords.

### Git Worktrees (Worktrunk)

Git worktrees are managed by [worktrunk](https://worktrunk.dev/). Config in
`dot_config/worktrunk/config.toml`: squash+rebase+remove merges with `verify = true`, and
`delete-branch = false` keeps the branch ref after merge. `wt up` rebases every worktree against upstream
safely.

### Bashrc Init Ordering

Starship initializes early; zoxide and atuin initialize after the interactive block (both modify
`PROMPT_COMMAND`; atuin last). `bash-preexec` is sourced explicitly from Homebrew (atuin 18.x stopped
bundling it) BEFORE `atuin init` — atuin's `__atuin_preexec`/`__atuin_precmd` and our long-running
command timer both register into `preexec_functions` / `precmd_functions`. A naked `DEBUG` trap would
clobber atuin's recording. Direnv hook runs early. Carapace universal completion loads after
`gh completion`. On interactive launch, `herdr workspace create --focus homelab` runs unconditionally —
no `tmux ls` probe, no `sesh-bootstrap.sh` call; herdr is idempotent and silently no-ops if the workspace
already exists.

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

### Happy Daemon (Remote Agent Control)

[happy](https://happy.engineering/) bridges Claude Code sessions to the Happy mobile and web apps for
remote control; the local daemon is that bridge. Its lifecycle is managed by
`~/Library/LaunchAgents/com.webdavis.happy-daemon.plist` (`KeepAlive=true`, `RunAtLoad=true`), loaded on
every `chezmoi apply` by `.chezmoiscripts/run_onchange_after_62-load-happy-daemon-launchagent.sh.tmpl`
(`bootout` + `bootstrap` with a 3-try retry loop, mirroring the atuin loader). `happy` itself is an npm
global tracked under `npm:` in `.chezmoidata/system_packages_autoinstall.yaml`, and logs go to
`~/.local/log/happy-daemon.log`.

**The one gotcha — use `start-sync`, not `start`.** The plist runs `happy daemon start-sync`, which keeps
the daemon in the foreground. The documented command, `happy daemon start`, detaches (forks, then
returns), which under `KeepAlive` looks like an instant exit and restart-loops — orphaning a daemon each
cycle. `start-sync` is the foreground entry point that `start` spawns internally; it is NOT listed in
`happy daemon --help`, so the plist comment is the only record of why it is used. launchd then supervises
a two-process tree: the `start-sync` process it keeps alive, which in turn manages the real daemon.

**Diagnostic ladder** when remote control stops connecting:

```bash
happy daemon status                        # 'Daemon is running' + PID, port, version
launchctl list | grep happy                # col 1 = live PID, col 2 = last exit status
ps aux | grep '[h]appy daemon'             # supervised start-sync process + the daemon it spawns
tail ~/.local/log/happy-daemon.log         # crash messages
happy doctor                               # full diagnostics ('happy doctor clean' kills runaways)
```

### Moshi Integration

Moshi is the user's primary mobile agent bridge (Happy coexists as a secondary option). The `rjyo/moshi`
tap and `moshi-hook` formula are declared in `.chezmoidata/system_packages_autoinstall.yaml`, with
`rjyo/moshi` listed in the shared `trusted_taps:` field; a pre-bundle trust loop in
`.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl` runs `brew trust --tap` for each trusted
tap before `brew bundle` executes.

One-time setup runs from `.chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl`: pairs moshi-hook
with the mobile app (token from KeePassXC entry **`Moshi :: Pairing Token`**), runs `moshi-hook install`
to wire agent hooks into Claude Code / Codex / OpenCode / Gemini / Cursor / Kimi / Qwen / Grok / OMP /
Pi, and starts the brew service.

**Asymmetric herdr integration:** moshi-hook reads `HERDR_ENV`, `HERDR_SESSION`, and `HERDR_PANE_ID`
(which herdr exports natively inside its panes), so no herdr-side configuration is needed for moshi-hook
to operate.

**Done-notification Stop hook (separate from moshi-hook):** the Claude Code `Stop` hook posts a "done"
push via `~/.local/bin/claude-moshi-notify.sh`. chezmoi renders its webhook secret into the 0600 file
`~/.config/moshi/setting.json` (`dot_config/moshi/private_setting.json.tmpl`, from KeePassXC entry
**`Moshi :: Webhook Secret`**); the script reads it at run time, so the secret never appears on the hook
command line or in any process's argv.

### AI Commit Messages

The user-wide `prepare-commit-msg` hook (`dot_config/git/hooks/executable_prepare-commit-msg`, activated
by `core.hooksPath = ~/.config/git/hooks`) pipes the full staged diff (no truncation) to
`claude -p --model=sonnet` with a 30-second timeout, and prepopulates the commit editor with the returned
Conventional Commits message (subject, optional body, optional footers). Bails on
`-m`/`-F`/merge/rebase/cherry-pick and on `SKIP_AI_COMMIT=1`. Chains to a repo-local
`.git/hooks/prepare-commit-msg` if present. Never blocks a commit — worst case the editor opens with an
empty message.

A per-repo `core.hooksPath` override (e.g. what `git lfs install` writes) would shadow this hook; that is
why the per-repo pre-commit lint uses the dispatcher described under Git Hooks rather than an override.

### Long-running Command Notifier

`dot_bashrc.tmpl` registers `__cmd_notify_preexec` and `__cmd_notify_precmd` via bash-preexec (atuin's
framework). Commands ≥ 30s fire an `alerter` macOS notification; ≥ 5 min additionally pulse Hue lights
via `~/.local/bin/hue-pulse.sh`. Known interactive TUIs (vim/less/top/ssh/tmux/claude/fzf) are skipped.

### Herdr Native Status

Workspace state (per-pane agent status: blocked / working / done / idle) is rendered by herdr — no
third-party plugin or custom script. The sidebar rolls each workspace up to its most-urgent agent state.
Claude Code, Codex, Cursor, OpenCode, and others are recognized out of the box.

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
