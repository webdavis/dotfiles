<!-- Keep this file evergreen. Avoid adding point-in-time content (current sprint
goals, active branches, temporary workarounds) that wouldn't make sense if
multiple workstreams, PRs, or branches were in progress simultaneously.
Document general principles, workflows, and architecture, not transient
project state. Conditional operational detail belongs in docs/runbooks/. -->

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

A [chezmoi](https://www.chezmoi.io/) dotfiles repository. **This checkout is the chezmoi source
directory**: `.chezmoi.toml.tmpl` sets both `sourceDir` and `workingTree` to this repo's path, so there
is no separate `~/.local/share/chezmoi` copy to keep in sync.

Chezmoi applies the source state here to the target state in `$HOME`. Everything not excluded by
`.chezmoiignore` reaches the target; the source-name prefixes control naming and mode rather than
membership: `dot_` maps to `.`, `private_` restricts permissions to the user, `executable_` sets `+x`,
`symlink_` creates a link, `modify_` runs the file as a script over the existing target, `run_` executes
it, and a `.tmpl` suffix marks a Go template. An unprefixed entry such as `Library/` deploys under its
own name.

## Runbooks

Conditional detail lives under `docs/runbooks/` and is read on demand, not carried in this file:

| Runbook                                           | Covers                                                                 |
| ------------------------------------------------- | ---------------------------------------------------------------------- |
| `docs/runbooks/agent-skills-store.md`             | the cross-harness skills store, its lock, and the plugin update record |
| `docs/runbooks/claude-code-settings.md`           | the `modify_settings.json` field model and plugin-state trade          |
| `docs/runbooks/git-hooks.md`                      | all four hooks, the dispatcher design, and the pre-push history        |
| `docs/runbooks/local-daemons.md`                  | atuin, happy and tailscaled: config, gotchas, diagnostic ladders       |
| `docs/runbooks/macos-defaults.md`                 | the two defaults runners, the capture workflow, the gotchas            |
| `docs/runbooks/macos-fresh-machine-quickstart.md` | first-apply setup, TCC grants, LuLu, and SSH hardening                 |
| `docs/runbooks/age-key.md`                        | the age identity behind encrypted source files                         |

## Key Commands

### Linting and formatting

All lint and format tooling is orchestrated by [treefmt](https://treefmt.com/) via
[treefmt-nix](https://github.com/numtide/treefmt-nix): `treefmt.nix` holds the formatter configuration,
and the flake's `checks.treefmt` derivation makes `nix flake check` fail on any format drift.

```bash
just l             # run all eleven formatters (six rewrite, five validate only)
just L             # lint-check: check-only drift gate (runs `nix flake check`)
just s             # shellcheck, plain files and rendered chezmoi templates
just S             # shfmt (format shell files) only
just m             # mdformat only
just n             # nixfmt only
just t             # taplo (TOML) only
just j             # jq (JSON validation, incl. rendered osquery configs) only
just y             # yq (YAML validation) only
just lint-actions  # actionlint + zizmor on .github/workflows
```

The eleven are shellcheck, shfmt, mdformat, nixfmt, taplo, actionlint, and the five validators
(`jq-validate`, `yq-validate`, `shellcheck-rendered-template`, `osquery-config-render`,
`espanso-match-render`), which never write and fail the run on bad input.

`just l` auto-formats in place. `just lint-check` never mutates the working tree or index: treefmt has no
dry-run mode, so the check runs on a sandboxed copy inside the Nix check derivation. It runs
`nix flake check` for the host system only; `just c` is the `--all-systems` variant. Lint drift is gated
at pre-push and in CI.

To enter an interactive dev shell with all tools: `nix develop`.

### Testing

```bash
just test-unit          # Unit suite only (the fast commit gate)
just test-integration   # Integration suite only
just test-e2e           # End-to-end suite only
just test-system        # The suite that tests the checker and runner themselves
just test               # All four suites (CI runs this)
just ship               # the three gates CI runs, in CI order, the explicit pre-PR sweep
```

Tests live in suites by DESIGN: `test/unit/` (single component, stub/fixture driven, no flows, no sleeps,
FAST is the admission rule), `test/integration/` (multi-component with stubbed boundaries), `test/e2e/`
(whole-script flows and deliberately timing-bound tests), and `test/test-system/` (tests of the checker
and runner themselves). All are plain executable `.sh` scripts (source-only, `.chezmoiignore`d) plus
optional bats suites (`test/**/*.bats`).

The **commit** gate runs `just test-unit` only, kept fast on purpose: it runs the one runner
(`test/run-test-suite.sh`) with `--shuffle --warn-slow-ms 200`, so order is seed-shuffled each run
(replay a failure with `TEST_SEED=<seed>`, printed every run, since Bats 1.11 has no native shuffle;
shuffling degrades to sorted order on a host with neither `gshuf` nor `shuf`). A WARN-ONLY performance
summary lists any test over the threshold as a refactor-or-move-suite candidate; warnings never fail the
run.

**CI** runs `just test`, which is exactly the four suite recipes, and `just ship` runs CI's whole gate
list on demand (the `--all-systems` flake check, `just test` inside the flake's `run` shell, and the
zizmor workflow audit, in that order; `test/unit/ship-ci-gate-parity.sh` fails when `ship` and
`.github/workflows/lint.yml` stop describing the same work). The pre-push hook deliberately runs no
suite. Each suite's runner executes its own `.sh` and `.bats` once (bats via
`nix develop .#run --command bats --jobs 4` when the host lacks bats, and the checker's placement rules
keep any bats from hiding outside a suite).

So a commit can briefly carry an integration or e2e regression, and so can a push: **CI is the only gate
that runs the suite**, and it runs on pull requests and on pushes to `main` only (that trigger scope is
asserted by `test/unit/ship-ci-gate-parity.sh`). A push to a topic branch with no open pull request runs
the suite nowhere; `just ship` is how you cover that window deliberately.

`just validate-tests` (`test/validate-tests.sh`, a dependency of all four suite recipes) fails if a
`*.sh` or `*.bats` sits outside a recognized suite. Only `validate-tests.sh` and `run-test-suite.sh` may
sit at `test/` root. Three trees are carved out: a suite's own `helpers/`, the shared cross-suite
`test/helpers/`, and `test/fixtures/**`. The two helper trees admit non-executable `*.sh` only, so an
executable file or a `.bats` there still fails. The checker also rejects any symlink below `test/`, a
non-executable suite `*.sh`, and a nested file in a flat suite. Add a test by dropping a new executable
`test/<suite>/<name>.sh` in place (with `REPO_ROOT` depth `dirname "${BASH_SOURCE[0]}")/../..`); it is
picked up automatically.

### Chezmoi operations

```bash
just d                                      # chezmoi diff --exclude=templates
just a                                      # chezmoi apply --exclude=templates --force
just c                                      # nix flake check --all-systems
chezmoi status                              # show pending changes
chezmoi diff                                # diff all (including templates)
chezmoi edit <file>                         # edit a template (prefer over direct edit of .tmpl)
```

**`--exclude=templates` does not make an apply vault-free.** It skips `.tmpl` entries, but it does NOT
skip a `modify_` template (measured 2026-08-02, recorded in `test/unit/claude-enabled-plugins.sh`), and
two modify-templates call `keepassxc`: `modify_private_dot_claude.json` (target `~/.claude.json`) and
`Library/Application Support/Claude/modify_private_claude_desktop_config.json`. So `just a` and `just d`
still reach the vault and need KeePassXC available; without an interactive TTY they fail rather than
prompt. To stay off the vault entirely, apply specific files by name:

```bash
chezmoi apply ~/.fzf_bindings               # specific non-template, non-modify file
```

Ten targets pull secrets through `keepassxc` and need KeePassXC unlocked: `~/.gitconfig`,
`~/.aws/credentials`, `~/.claude.json`, `~/.composio/user_data.json`, `~/.config/atuin/config.toml`,
`~/.config/himalaya/config.toml`, `~/.config/relay/auth.json`, `~/.config/gogcli/credentials.json`,
`~/Library/Application Support/Claude/claude_desktop_config.json`, and
`~/Library/Application Support/espanso/match/identity.yml`. Non-KeePassXC targets (for example
`~/.bashrc` and `~/.claude/settings.json`) are safe to apply from automation.

### Cutover tooling

Two operator-run scripts in `scripts/`, invoked by absolute path (no justfile recipe):

- `scripts/cutover-gate.sh <1|2|3|4|5>` runs one ordered cutover gate (preflight, activation,
  reconciliation, soak, closure), keeping its ledger under `~/.local/state/cutover`. Covered by
  `test/integration/cutover-gate-*.sh`.
- `scripts/live-reconcile.sh` converges the live skills fan-out to the committed lock, `--dry-run` first.
  Covered by `test/integration/live-reconcile.sh`.

## Architecture

### Source-only files

Dev and CI files excluded from `$HOME` via `.chezmoiignore`: `README.md`, `LICENSE`, `CLAUDE.md`,
`AGENTS.md`, `.gitignore`, `.gitattributes`, `assets/`, `docs/`, `private/`, `justfile`, `.envrc`,
`scripts/`, `test/`, `.shellcheckrc`, `.editorconfig`, `.mdformat.toml`, `.githooks/`, `graphify-out/`,
`flake.nix`, `flake.lock`, `treefmt.nix`, plus the failsafe globs `tmp.*`, `*.rayconfig`,
`*extension-diagnostics*` and `**/.DS_Store`, the vendored-skill `.git`/`node_modules` trees, and the
Rust `target` dirs under `.local/share/herdr/`. A trailing OS-conditional block ignores `Library` and six
macOS-only `.local/bin` scripts on Linux.

`.worktrees/` is NOT in `.chezmoiignore`; it is gitignored and treefmt-excluded instead.

### Minimum chezmoi version

`.chezmoiversion` requires >= 2.62.3.

### Secrets management

Secrets are managed via chezmoi's KeePassXC integration (`keepassxc-cli`). The database path is
configured in `.chezmoi.toml.tmpl`. Templates select a field off the record, for example
`{{ (keepassxc "Entry Name").Password }}`, or read a custom attribute with
`{{ keepassxcAttribute "entry-name" "attr-name" }}`. The `.install-password-manager.sh` hook, wired as
`[hooks.read-source-state.pre]`, installs KeePassXC when missing; it is best effort and only warns when
`brew` cannot.

Whole-file secrets use `age` encryption (identity at `~/.config/chezmoi/key.txt`); see
`docs/runbooks/age-key.md`.

### System package management

Packages are declared in `.chezmoidata/system_packages_autoinstall.yaml` under `packages.macos.homebrew`
with keys `taps`, `formulae`, `casks` and `mas`, plus three siblings of `homebrew` under
`packages.macos`: `uv` (uv tool installs, e.g. `graphifyy`, which provides the `graphify` CLI behind the
post-commit dispatcher), `npm` (npm globals, e.g. `@colbymchenry/codegraph` and `happy`), and `volta`.
One script, `.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl`, consumes all of them: it
generates a Brewfile from the data, runs `brew bundle`, then runs a guarded
`brew bundle cleanup --force`. Prerequisite: `run_once_before_00-install-homebrew.sh.tmpl` runs the
upstream installer when `command -v brew` finds nothing.

Two behaviors of the cleanup stage matter. Cleanup is **withheld entirely** while `tmux` or `sesh` is
still installed, because the herdr migration owns their teardown. And
`.chezmoitemplates/brew-bundle-cleanup-guard.sh.tmpl` **refuses** a cleanup that would remove more than a
safety threshold unless `HOMEBREW_BUNDLE_ALLOW_BULK_CLEANUP=1` is set.

Third-party taps whose formulae or casks must be trusted under Homebrew's `HOMEBREW_REQUIRE_TAP_TRUST`
gate are listed under a `trusted_taps` key in the same data file. A pre-bundle loop runs `brew tap` then
`brew trust --tap` for each before `brew bundle`, so the bundle does not refuse to load them. Add a tap
there when `brew bundle` reports it as untrusted.

**Homebrew install workflow (for AI agents):**

1. Install the package immediately: `brew install <formula>` or `brew install --cask <cask>`.
1. On success, add it to `.chezmoidata/system_packages_autoinstall.yaml` in the appropriate list
   (formulae, casks, taps, mas), maintaining alphabetical order.
1. Remind the user to run `chezmoi apply` when appropriate. Do not run it directly; see the KeePassXC
   constraint above.

### macOS defaults management

Two `.chezmoiscripts/` runners apply declarative macOS settings on darwin and no-op on Linux:
`run_onchange_after_30-macos-defaults.sh.tmpl` (per-user and per-machine `defaults` records from
`.chezmoidata/macos_defaults.yaml`) and `run_onchange_after_41-macos-system-setup.sh.tmpl` (sudo system
commands and MagicDNS `/etc/hosts` pins from `.chezmoidata/macos_system_setup.yaml`). A third data file,
`.chezmoidata/macos_posture_controls.yaml`, is verify-tier only and is read by the osquery poller at
runtime.

The capture workflow (`just defaults-capture`), the drift check (`just D`), and the two implementation
gotchas that must not be "cleaned up" are in `docs/runbooks/macos-defaults.md`.

### Claude Code settings

`private_dot_claude/modify_settings.json` is a chezmoi modify-template that enforces a fixed set of
stable fields in `~/.claude/settings.json` (permissions, hooks, `skillOverrides`, `statusLine`,
`cleanupPeriodDays`, `autoUpdatesChannel`, `remoteControlAtStartup`, `extraKnownMarketplaces`) while
letting `/config` toggles drift freely. `enabledPlugins` is a third case: the roster is declared, the
per-plugin disable state is read back out of the live file. The full field model, the plugin-state trade
and the corrupt-file recovery path are in `docs/runbooks/claude-code-settings.md`.

### Agent skills (cross-harness store)

`~/.agents/skills` is the single canonical skills store (35 roster skills), serving Claude Code (chezmoi
symlink declarations under `private_dot_claude/skills/`), Codex (native store scan, no declarations) and
hermes (declared symlinks into the default profile and four specialist profiles). Provenance, tiering and
fan-out are recorded in `dot_agents/custom-skill-lock.json`, and `test/unit/skills-roster-fanout.sh`
fails the build whenever the store, the lock tables and the per-harness declarations disagree.
`~/.local/bin/update-skills.sh` refreshes the npx-, clawhub- and app-owned lanes weekly, publishing a new
generation with one atomic exchange.

`docs/runbooks/agent-skills-store.md` carries the delivery model, the lane mechanics, the fork
drift-watch states, the generation-exchange guarantee, the schedule, and how to add or remove a skill.

### Global instruction files

The global ruleset for Claude Code and Codex is one shared partial,
`.chezmoitemplates/global-agent-rules.md`, pulled into both `private_dot_claude/CLAUDE.md.tmpl` (target
`~/.claude/CLAUDE.md`) and `private_dot_codex/AGENTS.md.tmpl` (target `~/.codex/AGENTS.md`) with
`includeTemplate`, between a pair of `shared-rules` markers. Harness-specific rules go in the including
file, below the shared block. `test/integration/global-instruction-parity.sh` renders both targets and
byte-compares what lands between the markers, so an edit to one copy fails the build. Edit the partial,
never a harness copy.

### Git hooks

Four hooks live in the user-wide dir (`core.hooksPath = ~/.config/git/hooks`, set in
`dot_gitconfig.tmpl`). `prepare-commit-msg` prepopulates a Conventional Commits message via
`claude -p --model=sonnet` (bypass with `SKIP_AI_COMMIT=1`). The other three are dispatchers that act
only when the repository tracks its own executable `.githooks/<name>`: this repo's `pre-commit` runs
`just test-unit` plus gitleaks, its `pre-push` runs `just lint-check` and no suite, and its `post-commit`
rebuilds the graphify map (skip with `GRAPHIFY_SKIP_HOOK=1`).

A per-repo `core.hooksPath` override would shadow the user-wide hook, which is why dispatchers exist and
why **Git LFS must not be reintroduced here** (`git lfs install` writes exactly such an override).
`docs/runbooks/git-hooks.md` has the full behavior of each hook and the reasoning behind the pre-push
narrowing.

### Template files

Template files use chezmoi Go templates (`.tmpl` suffix) and live alongside their target files (e.g.
`.chezmoi.toml.tmpl`, `dot_bashrc.tmpl`, `dot_gitconfig.tmpl`, and 36 scripts in `.chezmoiscripts/`).
Templates branch on `.chezmoi.os` and, where they pull secrets, call `keepassxc`. Reusable fragments live
in `.chezmoitemplates/` and are pulled in with `includeTemplate`.

### Template shellcheck workaround

Shell templates contain Go template syntax that shellcheck can't parse directly, so the
`shellcheck-rendered-template` formatter in `treefmt.nix` renders first
(`CI=1 chezmoi --source "$PWD" execute-template --no-tty <file`, with a throwaway `HOME` because the Nix
sandbox has none) and shellchecks the result. `--source "$PWD"` is load-bearing: it is what makes
`includeTemplate` resolve against this checkout.

Its include list is discovered programmatically at Nix eval time, not hand-picked: every
`.chezmoiscripts/*.sh.tmpl` plus every shell `dot_*.tmpl` at the repo root (first line a shell shebang or
`# shellcheck shell=` directive, or a Go-template directive whose first non-directive line is such a
shebang), minus anything the classifier in `scripts/render-coverage-classifier.nix` calls unsafe to
render. A template is unsafe on any of three grounds: it or a partial it includes invokes `keepassxc`
(which needs an interactive unlock), it carries a Go action split across lines, or it passes
`includeTemplate` a name that is not a static string literal.

Three `.chezmoitemplates/` fragments are excluded with documented reasons because they only render
through their includers: `herdr-plugin-build.sh.tmpl`, `herdr-health-check.sh.tmpl` and
`brew-bundle-cleanup-guard.sh.tmpl`.

After a successful render, a blank (empty or whitespace-only) result is skipped rather than shellchecked,
so an OS-gated template on the other OS does not fail SC2148; a render failure stays fatal.
`test/integration/rendered-template-coverage.sh` enforces this universe: it re-reads the formatter's
actual include list via `nix eval` and fails when discovery drops a template, when a stale exclusion
lingers, or when a fixture under `test/fixtures/render-coverage` classifies differently in the bash
mirror and the production Nix predicates. The `CI=1` env var is defensive here (vestigial from an earlier
bashrc branch), but it is load-bearing for the sibling `espanso-match-render` formatter, whose vault
reads sit behind `{{ if (env "CI") }}`.

Two more sibling formatters render before validating: `osquery-config-render` renders the JSON-bodied
`.chezmoitemplates/osquery/**/*.conf` templates via `includeTemplate` and checks them with jq, and
`espanso-match-render` renders the espanso `*.yml.tmpl` match files and checks them with yq.

### OS targeting

`.chezmoiignore` conditionally ignores paths by OS: on Linux it drops `Library` and the six macOS-only
scripts under `.local/bin` (the four `macos-defaults-*` helpers, `rotate-logs.sh` and
`brew-shellenv-cache-refresh.sh`). Template files use `{{ if eq .chezmoi.os "darwin" }}` for
macOS-specific content.

### Dev environment (Nix flake)

`flake.nix` provides two dev shells for `x86_64-linux` and `aarch64-darwin`:

- `default`, interactive shell with colored status output.
- `run`, headless shell used by `just` and CI.

Both share six build inputs: the repo-configured `treefmt` wrapper (bundling shellcheck, shfmt, mdformat
with the GFM plugin, nixfmt, taplo, actionlint and the five validators from `treefmt.nix`), bats,
`parallel` (required by `bats --jobs`, and absent from GitHub's macOS runners), chezmoi, `just` (so CI
can call `nix develop .#run --command just test`), and zizmor.

### CI

GitHub Actions (`.github/workflows/lint.yml`) runs on `macos-latest` on pushes to main and on pull
requests, with workflow-level `permissions: contents: read`, `persist-credentials: false` on checkout,
and actions SHA-pinned to full commit SHAs. `.github/dependabot.yml` keeps the pins fresh weekly behind a
7-day release cooldown; its PRs auto-merge via `.github/workflows/dependabot-automerge.yml`, which uses
`gh pr merge --auto` so branch protection, where `lint` is a required status check on `main`, is what
actually holds the merge until green.

Four steps: checkout, install Nix, `nix flake check --all-systems` (the treefmt drift gate), then
`just test` and `zizmor --offline .github/workflows` inside the flake's `run` shell.

### Background jobs and LaunchAgents

Every scheduled or supervised job on dresden is a chezmoi-tracked plist under `Library/LaunchAgents/`,
bootstrapped by a matching `.chezmoiscripts/run_onchange_after_*` loader.

| LaunchAgent                                        | What it does                                                |
| -------------------------------------------------- | ----------------------------------------------------------- |
| `com.webdavis.atuin-daemon`                        | supervises the atuin history daemon                         |
| `com.webdavis.happy-daemon`                        | supervises the happy remote-control bridge                  |
| `com.webdavis.homebrew-weekly-upgrade`             | weekly unattended `brew upgrade`, reported to the log route |
| `com.webdavis.update-skills`                       | weekly skills-store refresh (24 Monday retry slots)         |
| `com.webdavis.report-plugin-updates`               | weekly record of what Claude Code auto-updated              |
| `com.webdavis.rotate-logs`                         | rotates `~/.local/log/`                                     |
| `com.webdavis.yt-dlp-pot-provider`                 | the yt-dlp proof-of-origin token provider                   |
| `com.webdavis.osquery-heartbeat`                   | proves the osquery pipeline is alive                        |
| `com.webdavis.osquery-results-alerter`             | turns osquery results into notifications                    |
| `com.webdavis.osquery-alert-drainer`               | drains the queued alerts                                    |
| `com.webdavis.osquery-digest`                      | periodic roll-up                                            |
| `com.webdavis.osquery-firewall-gatekeeper-monitor` | watches firewall and Gatekeeper posture                     |
| `com.webdavis.osquery-tailscale-monitor`           | watches tailscaled posture                                  |
| `com.webdavis.osquery-uptime-watchdog`             | watches for a machine that stopped reporting                |

The osquery side is configured by `run_onchange_before_50-setup-osquery.sh.tmpl` from the JSON-bodied
templates under `.chezmoitemplates/osquery/`, with its control catalog in
`.chezmoidata/macos_posture_controls.yaml`.

### SSH hardening

`~/.local/bin/ssh-hardening.sh` (source `dot_local/bin/executable_ssh-hardening.sh`) generates, installs,
verifies, reloads and rolls back a public-key-only sshd drop-in at
`/etc/ssh/sshd_config.d/000-ssh-hardening.conf`. It is operator-invoked: no LaunchAgent, no chezmoiscript
and no justfile recipe runs it. Installing is inert for the running service; only `--reload` restarts
sshd, and it refuses to claim success without a real SSH banner exchange.

Every sshd call it makes runs under a watchdog (`SSH_HARDENING_VERIFY_DEADLINE`, default 120s), which
polls in 0.25s ticks because bash has no wait-with-timeout and stock macOS ships no `timeout(1)`, then
sends `TERM` to the whole process group, waits a 2s grace, sends `KILL`, and returns 124 the way
`timeout(1)` does. It exists because a named pipe in the drop-in directory blocks `sshd -G` forever (sshd
resolves its own `Include` globs with no type filter), and before the watchdog a hang parked the install
with the new drop-in already published and the legacy file already moved aside.
`test/e2e/ssh-hardening-verify-watchdog.sh` drives a TERM-ignoring wedge to pin both the deadline and the
group kill. The reload and lockout-recovery procedure is in
`docs/runbooks/macos-fresh-machine-quickstart.md`.

### Herdr workspace management

Workspaces (project-anchored tab groups, roughly tmux sessions) are configured at
`dot_config/herdr/config.toml`. Eight project workspaces are reached by quick-jump chords bound on nine
keys, mostly `prefix+ctrl+<letter>`, but the dotfiles chord is `prefix+ctrl+.` (a period, sent via CSI-u)
with a `prefix+.` fallback for terminals without CSI-u. The design spec at
`docs/superpowers/specs/2026-06-18-tmux-to-herdr-migration-design.md` has the full mapping table.

On every terminal launch `~/.bashrc` auto-attaches to the persistent herdr session, which opens the
last-focused workspace (homelab in practice, once visited, since the session persists); herdr has no
launch-into-workspace flag. Jump to homelab anytime via the `h` alias or the `prefix+ctrl+h` chord.

Ctrl-h/j/k/l "seamless nav across Neovim splits and herdr panes" is a herdr **plugin**
(`dot_local/share/herdr/plugins/herdr-smart-nav/`, a Rust binary), bound via four
`type = "plugin_action"` keybindings (`herdr-smart-nav.nav_<dir>`), so herdr execs it directly as argv
with no `/bin/sh -lc` wrapper. It is built and linked by `run_onchange_after_57`, mirroring the
`last-workspace` plugin, and it shells the `herdr` CLI rather than using a Rust SDK. The gain over the
old shell-keybinding binary is about 5 ms (the wrapper) and is imperceptible; the value is the idiomatic
plugin integration. Plugin actions get `HERDR_PANE_ID`, and the binary falls back to
`HERDR_ACTIVE_PANE_ID` when that is absent.

### Herdr native status

Workspace state (per-pane agent status: blocked, working, done, idle) is rendered by herdr, with no
third-party plugin or custom script. The sidebar rolls each workspace up to its most-urgent agent state.
Claude Code, Codex, Cursor, OpenCode and others are recognized out of the box.

### Git worktrees (worktrunk)

Git worktrees are managed by [worktrunk](https://worktrunk.dev/). Config in
`dot_config/worktrunk/config.toml`: squash plus rebase plus remove merges with `verify = true`, and
`delete-branch = false` keeps the branch ref after merge. `wt up` fetches with `--prune` and rebases
every worktree against upstream, skipping ones with no upstream or a rebase already in progress, and
aborting and warning rather than leaving a worktree half-rebased.

### Bashrc init ordering

The canonical order inside the interactive block is direnv, starship, zoxide, atuin. Direnv's hook runs
early; starship initializes before the two `PROMPT_COMMAND` writers; zoxide and atuin initialize late
within the interactive block, atuin last.

`bash-preexec` is sourced explicitly from Homebrew (atuin 18.x stopped bundling it) BEFORE `atuin init`,
because atuin's `__atuin_preexec` and `__atuin_precmd` and this repo's long-running command timer all
register into `preexec_functions` and `precmd_functions`. A naked `DEBUG` trap would clobber atuin's
recording.

Carapace provides universal completion, including for `gh` and `git`; it loads after bash-completion@2
and direnv and before starship.

### Long-running command notifier

`dot_bashrc.tmpl` registers `__cmd_notify_preexec` and `__cmd_notify_precmd` via bash-preexec (atuin's
framework). Commands at 30s or longer fire an `alerter` macOS notification; at 5 minutes or longer they
additionally pulse Hue lights via `~/.local/bin/hue-pulse.sh`, which is handed the exit code and pulses
green on success, red otherwise. Interactive TUIs are skipped by a prefix match on the command line:
`vim`, `nvim`, `less`, `man`, `top`, `btop`, `ssh`, `herdr`, `claude`, `hermes`, `codex`, `fzf`.

## Code Style

- Shell files: 2-space indent, case-indent enabled, simplified (`shfmt -i 2 -ci -s`, wired in
  `treefmt.nix`). When running shfmt by hand, pass these flags explicitly, `.editorconfig` only covers
  `dot_fzf*` and `dot_bash*` patterns, for editors. Note that shfmt and shellcheck both exclude `*.tmpl`
  and `dot_agents/skills/**`, so templated shell is covered by the render-then-lint formatter instead.
- **Bash follows the [Wooledge BashGuide](https://mywiki.wooledge.org/BashGuide) practices.** The rules
  that come up most in this repo:
  - `set -euo pipefail` at the top of every script; double-quote every expansion.
  - `[[ ]]` for tests, never `[ ]`, in anything with a bash shebang.
  - Lists are **arrays**, never space-separated strings, no unquoted `$VAR` expansion loops and no
    `shellcheck disable=SC2086` suppressions to make them lint.
  - Never `for x in $(command)`, iterate command output with
    `while IFS= read -r x; do ...; done < <(command)`. If the loop body runs anything that may read stdin
    (git, ssh, ffmpeg), read on a dedicated fd: `while IFS= read -r -u3 x; do ...; done 3< <(command)`.
  - Build JSON with `jq -n --arg`/`--argjson`, never by interpolating variables into a JSON string.
  - `printf` for any output containing variable data; `echo` only for fixed literal text.
  - Don't parse `ls`; use globs (guarded with a `[[ -e ]]`/`[[ -d ]]` test or `nullglob`).
  - Validate numeric arguments with a `[[ =~ ]]` pattern before using them.
  - Unknown CLI arguments/commands are an error: usage to stderr, exit non-zero, never a silent
    fallthrough to help with exit 0.
- Markdown: wrapped at 105 columns, non-consecutive numbering (`mdformat` with `.mdformat.toml`, which
  also pins LF line endings and strict round-trip validation). mdformat never touches skill, agent or
  command definitions (`private_dot_claude/{skills,agents,commands}/**`, `dot_agents/**`), which rely on
  YAML frontmatter it would mangle, nor `docs/superpowers/**`.
- Nix: formatted with nixfmt (RFC 166 style, `treefmt.nix` pins `pkgs.nixfmt-rfc-style` because the bare
  `nixfmt` attribute in nixpkgs 25.05 is still nixfmt-classic).
- TOML: formatted with `taplo`. `dot_aerospace.toml` is excluded (preserves user's visual alignment).
- ShellCheck directives: SC1090 and SC1091 are globally disabled (`.shellcheckrc`).

## Git Commits

**Never include `Co-Authored-By` lines in commit messages.** Claude is never listed as a co-author.

Separate logically distinct changes into their own commits. Each commit should be a single cohesive unit
of work.

## Security

- `*bash_secret*` patterns are gitignored to prevent accidental commits of Bash secret files.
- Claude Code settings deny reads of six sensitive paths (`.env`, `.env.*`, `secrets/**`,
  `credentials.json`, `.aws/credentials`, `.ssh/id_*`) alongside
  `permissions.defaultMode = bypassPermissions`, so the deny list is what stands between an agent and
  those files.
- KeePassXC is the single source of truth for secrets pulled into templates, including the age identity
  itself, which `run_before_05-restore-age-key.sh.tmpl` fetches from the vault at apply time.
- `gitleaks git --staged` blocks any staged plaintext secret at pre-commit.
