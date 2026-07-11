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

All lint/format tooling is orchestrated by [treefmt](https://treefmt.com/) via
[treefmt-nix](https://github.com/numtide/treefmt-nix): `treefmt.nix` holds the formatter configuration,
and the flake's `checks.treefmt` derivation makes `nix flake check` fail on any format drift. Use the
justfile shortcuts:

```bash
just l             # Format everything in place (shfmt, mdformat, nixfmt, taplo + jq/yq validators)
just L             # lint-check: check-only drift gate (runs `nix flake check`)
just s             # Shellcheck only (incl. rendered chezmoi templates)
just S             # shfmt (format shell files) only
just m             # mdformat only
just n             # nixfmt only
just t             # taplo (TOML) only
just j             # jq (JSON validation, incl. rendered osquery configs) only
just y             # yq (YAML validation) only
just lint-actions  # actionlint + zizmor on .github/workflows
```

`just l` auto-formats in place. `just lint-check` never mutates the working tree or index: treefmt has no
dry-run mode, so the check runs on a sandboxed copy inside the Nix check derivation. On commit, the
per-repo `.githooks/pre-commit` hook runs `just lint-check` (check-only) — auto-wired via the user-wide
dispatcher, no install step. See Git Hooks.

To enter an interactive dev shell with all tools: `nix develop`.

### Testing

```bash
just test               # Run every test in test/ (build-tool style; pre-commit runs this too)
```

Tests are plain executable `test/*.sh` scripts (source-only — `.chezmoiignore`d). `just test` runs them
all and fails if any exits non-zero — and it is green when `test/` is missing or empty. Shell tests use
host tools (e.g. `brew`) and run outside the Nix shell; bats suites (`test/**/*.bats`) run inside
`nix develop .#run` (the flake provides `bats`). Add a test by dropping a new executable `test/<name>.sh`
in place — it is picked up automatically.

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
chezmoi apply ~/.fzf_bindings               # specific non-template file
chezmoi diff --exclude=templates            # diff non-template files
```

**Never run bare `chezmoi apply` from Claude Code** — the following templates call `keepassxc` and will
fail without an interactive TTY: `~/.gitconfig`, `~/.aws/credentials`, `~/.claude.json`,
`~/.composio/user_data.json`, `~/.config/atuin/config.toml`, `~/.config/himalaya/config.toml`,
`~/Library/Application Support/Claude/claude_desktop_config.json`,
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

All three hooks live in the **user-wide** hooks dir (`core.hooksPath = ~/.config/git/hooks`, set in
`dot_gitconfig.tmpl`), so they apply to every repo:

- **`prepare-commit-msg` — user-wide AI commit messages.** Prepopulates a Conventional Commits message
  via Claude Sonnet (internals under **AI Commit Messages** below). Bails on `-m`/merge/rebase; bypass
  with `SKIP_AI_COMMIT=1`.
- **`pre-commit` — per-repo lint + tests + secret scan, via a dispatcher.**
  `dot_config/git/hooks/executable_pre-commit` runs in every repo but only acts when the repository
  tracks an executable `.githooks/pre-commit`, which it then `exec`s. This repo's `.githooks/pre-commit`
  runs `just lint-check` (check-only — reports drift, never mutates the tree or index), then `just test`
  (the full `test/` suite — see Testing), then `gitleaks git --staged --redact` (blocks any staged
  plaintext secret; gitleaks is provisioned as a Homebrew formula, and the stage is skipped when the
  binary is absent). All three must pass; a failure blocks the commit. No install step: the dispatcher is
  user-wide and the repo hook is committed with its executable bit.
- **`post-commit` — graphify knowledge-graph rebuild by default, per-repo opt-out, via a dispatcher.**
  `dot_config/git/hooks/executable_post-commit` launches graphify's detached rebuild after every commit
  in every repo, then chains a repo's own executable `.githooks/post-commit` (same composition as the
  pre-commit dispatcher). It supersedes the unmanaged hook `graphify hook install` wrote to the same
  path, inlining that hook's body verbatim — the graphify CLI exposes no hook-run action (`graphify hook`
  is only `install|uninstall|status`) — and keeping the `# graphify-hook-start`/`-end` markers so a rerun
  of the installer detects it as already installed instead of appending a second copy. A repo opts out of
  the rebuild (never the chain) by carrying a `.githooks/no-graphify` marker; this repo does, which is
  what ends the graphify-out/ litter here (the `.gitignore` and treefmt `graphify-out/` excludes stay as
  band-aids until the hook is applied live). The hook never fails a commit — internal errors exit 0.
  Per-commit escape hatch: `GRAPHIFY_SKIP_HOOK=1`. `test/post-commit-graphify-dispatcher.sh` covers the
  decision logic with a stub interpreter.

**Why a dispatcher, not `git config core.hooksPath .githooks`?** `core.hooksPath` is single-valued, so a
per-repo override shadows the user-wide `prepare-commit-msg`. The dispatcher keeps the global hook
authoritative while letting any repo opt into pre-commit checks. **Do not reintroduce Git LFS here** —
`git lfs install` writes exactly such an override, and this repo tracks no LFS files.

Bypass all hooks for one commit: `git commit --no-verify`.

## Architecture

### Source-Only Files

Some files are dev/CI only and are excluded from `$HOME` via `.chezmoiignore`: `justfile`, `scripts/`,
`test/`, `treefmt.nix`, `.githooks/`, `flake.nix`, `flake.lock`, `.envrc`, `.shellcheckrc`,
`.editorconfig`, `.mdformat.toml`, `assets/`, `docs/`, `private/`, `README.md`, `LICENSE`, `.gitignore`,
`.worktrees/`, `**/.DS_Store`. Only chezmoi-managed files (`dot_`, `private_`, `run_`, etc. prefixes)
reach the target state.

### Minimum Chezmoi Version

`.chezmoiversion` requires >= 2.62.3.

### Secrets Management

Secrets are managed via chezmoi's KeePassXC integration (`keepassxc-cli`). The database path is
configured in `.chezmoi.toml.tmpl`. Template files (`.tmpl`) use `{{ keepassxc "entry-name" }}` or
`{{ keepassxcAttribute "entry-name" "attr-name" }}` to pull secrets at apply time. The
`.install-password-manager.sh` hook auto-installs KeePassXC if missing.

### System Package Management

Packages declared in `.chezmoidata/system_packages_autoinstall.yaml` under `packages.macos.homebrew` with
keys: `taps`, `formulae`, `casks`, `mas` — plus sibling `uv` (uv tool installs, e.g. `graphifyy`, which
provides the `graphify` CLI behind the post-commit dispatcher), `npm` (npm globals, e.g.
`@colbymchenry/codegraph`; its hermes MCP (Model Context Protocol) enablement lives in the encrypted
hermes config and is tracked as separate follow-up work), and `volta` lists consumed by the same script.
The `.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl` script generates a Brewfile from
this data and runs `brew bundle --cleanup` whenever the data changes. Prerequisites:
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
  `sudo -v` upfront, then loop), plus structured `tailnet_pins` data from which the template generates
  the MagicDNS `/etc/hosts` pin commands. Early-returns when both lists are empty.

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

Shell templates contain Go template syntax that shellcheck can't parse directly, so the
`shellcheck-rendered-template` formatter in `treefmt.nix` renders first
(`CI=1 chezmoi execute-template --no-tty <file`) and shellchecks the result. Its include list is
discovered programmatically at Nix eval time, not hand-picked: every `.chezmoiscripts/*.sh.tmpl` plus
every shell `dot_*.tmpl` at the repo root (first line a shell shebang or `# shellcheck shell=` directive,
or a Go-template directive whose first non-directive line is such a shebang), minus any template that (or
whose `includeTemplate` partial) invokes `keepassxc` through a `{{ ... }}` directive, since those need an
interactive KeePassXC unlock. Two includeTemplate fragments
(`.chezmoitemplates/herdr-plugin-build.sh.tmpl` and `.chezmoitemplates/herdr-health-check.sh.tmpl`) are
excluded with documented reasons because they only render through their includers. After a successful
render, a blank (empty or whitespace-only) result is skipped rather than shellchecked, so an OS-gated
template on the other OS (which renders to nothing) does not fail SC2148; a render failure stays fatal.
`test/rendered-template-coverage.sh` enforces this universe: it re-reads the formatter's actual include
list via `nix eval` and fails when discovery drops a template, with a fixture layer under
`test/fixtures/render-coverage` guarding the classifier against blind spots. The `CI=1` env var is
defensive (vestigial from an earlier bashrc CI-vs-interactive branch). A sibling formatter,
`osquery-config-render`, renders the JSON-bodied `.chezmoitemplates/osquery/*.conf` templates via
`includeTemplate` and validates the result with jq.

### OS Targeting

`.chezmoiignore` conditionally ignores paths by OS (e.g., `.config/yabai` and `Library` on Linux).
Template files use `{{ if eq .chezmoi.os "darwin" }}` for macOS-specific content.

### Dev Environment (Nix Flake)

`flake.nix` provides two dev shells (for `x86_64-linux` and `aarch64-darwin`):

- `default` — interactive shell with colored status output.
- `run` — headless shell used by `just` and CI.

Tools provided: the repo-configured `treefmt` wrapper (bundling shellcheck, shfmt, mdformat with the GFM
plugin, nixfmt, taplo, actionlint, and the jq/yq/chezmoi-render validators from `treefmt.nix`), plus
bats, chezmoi, and zizmor.

### CI

GitHub Actions (`.github/workflows/lint.yml`) runs on `macos-latest` on pushes to main and PRs, with
workflow-level `permissions: contents: read`, `persist-credentials: false` on checkout, and actions
SHA-pinned to full commit SHAs (`.github/dependabot.yml` keeps the pins fresh weekly; its PRs auto-merge
once the lint check passes, via `.github/workflows/dependabot-automerge.yml`; `lint` is a required status
check on `main` under branch protection, so the auto-merge cannot land until it is green). Steps:
`nix flake check --all-systems` (the treefmt drift gate), `just test`, and
`zizmor --offline .github/workflows` — the latter two inside the flake's `run` shell.

### Agent Skills (cross-harness store)

`~/.agents/skills` is the single canonical skills store (31 roster skills). It serves Claude Code always
(symlinks declared in chezmoi: `private_dot_claude/skills/symlink_*` for the full roster), Codex always
(it scans the store natively — no declarations), and hermes for exactly the store-symlink subset of the
delivery model below (`dot_hermes/skills/` and `dot_hermes/profiles/<name>/skills/` symlinks). The
committed roster is the complete wanted set — `test/skills-roster-fanout.sh` fails the build if the
store, the lock's `tiers` / `hermesProfiles` / `hermesRegistry` / `npxTracked` / `clawhubTracked` tables,
the per-harness declarations, or the settings modify-template's `skillOverrides` ever disagree.

**Store provenance — who installs and refreshes each store copy** (the lock at
`dot_agents/custom-skill-lock.json` records it):

- **npx-tracked** (the `npxTracked` table, 23 skills): the store copy is installed and refreshed by the
  official npx `skills` CLI from an official GitHub upstream, latest from `main` (no pin).
  `~/.local/bin/update-skills.sh` installs and refreshes them via an explicit
  `npx skills add <repo> --skill <name> --agent claude-code --agent codex -g -y` per repo group, run
  against the weekly candidate generation (never the bulk `npx skills update`, whose lock-walk logs some
  failures at exit 0; the explicit add also reconciles lock-absent roster skills). No `~/.codex` dir;
  Codex reads the store natively. These skills are NOT vendored in chezmoi. Includes the 13 curated
  HeyGen HyperFrames skills (router `hyperframes`; domains
  `hyperframes-core/-animation/-keyframes/-creative`, `media-use`, `hyperframes-cli`,
  `hyperframes-registry`; workflows `general-video`, `website-to-video`, `faceless-explainer`,
  `embedded-captions`, `motion-graphics`; `figma`, `music-to-video`, and four others deliberately
  excluded). Also includes `home-assistant-best-practices` (the official `homeassistant-ai/skills` repo's
  one skill): Home Assistant config/YAML AUTHORING guidance, not runtime control; it complements the
  clawhub-tracked `home-assistant` runtime skill everywhere, and it is the one Home Assistant skill that
  DOES fan out to hermes (default profile), as authoring guidance atop Bob's native Home Assistant
  runtime tools.
- **ClawHub-tracked** (the `clawhubTracked` table, 3 skills: `home-assistant`, `sql-toolkit`,
  `summarize-pro`): the store copy is installed and refreshed by the `clawhub` CLI from ClawHub — the npx
  lane cannot source ClawHub (`npx skills add` is GitHub-only), so ClawHub-only skills get their own
  auto-update lane instead of staying vendored. Each entry records the owner-qualified slug and registry.
  `update-skills.sh` installs an absent one in a throwaway `--workdir` and moves the CLI's nested
  `@owner/<name>` output flat into the candidate store (v0.23.1 always nests; the skill's
  `.clawhub/origin.json` travels along and pins the owner); the weekly lane then refreshes each in place
  with `clawhub --workdir <candidate>/.agents --dir skills update <name>` (bare store names resolve
  through `origin.json` even when several ClawHub users publish the name). Two mechanical realities
  (verified live): Finder `.DS_Store` litter breaks the CLI's fingerprint match, so it is scrubbed
  pre-update, and the repo-asserted Codex overlay makes the CLI refuse with "local changes"; the pass
  sets exactly that file aside (byte-equal check) and retries once, and any OTHER local change is a loud
  relayed WARN. Automation never passes `--force`, and never `--force-install` (ClawHub's scan bypass).
- **Vendored** (committed under `dot_agents/skills/`, refreshed only by `chezmoi apply`): the `forks`
  table records each one's upstream for weekly drift-watch. `moshi` and `herdr` are deliberate content
  forks (`fork: true`); `elevenlabs` is vendored because npx cannot install it full-tree (its `SKILL.md`
  sits at the repo root beside a `scripts/` dir npx drops, even with `--full-depth`). `tiktok-crawling`
  is the one plain committed dir with no `forks` entry: a ClawHub-published skill left vendored because
  hermes owns its hub copy via `hermesRegistry` and its hub name differs from the roster name
  (`tiktok-scraping-yt-dlp`).
- **App-owned symlink** (`cua-driver`): the store entry is a symlink into `~/.cua-driver`; the app owns
  the content. The official mechanism covers all three harnesses (`cua-driver skills status` links Claude
  Code, Codex via the store, and hermes itself), and the weekly run refreshes the pack via
  `cua-driver skills update` — the app's own GitHub-Releases updater, never a write through the symlink.

**Tier model (the lock's `tiers` table):** every roster skill is `core` (7) or `on-demand` (24). Core
skills auto-load in every harness; on-demand skills stay installed everywhere but load only when
explicitly invoked: in Claude Code via `skillOverrides.<name> = "user-invocable-only"` — one
`setValueAtPath` per skill in the settings modify-template (per-key, so overrides the user sets for other
skills drift freely); in Codex via an additive `agents/openai.yaml`
(`policy: allow_implicit_invocation: false` — Codex then never auto-invokes the skill, while explicit
`$name` invocation keeps working). The overlay is committed next to each vendored skill; for npx- and
clawhub-tracked skills (whose folders the add/update passes replace wholesale) `update-skills.sh`
re-asserts it on every run from the tiers table — and when an upstream skill ships its own
`agents/openai.yaml` (the official `hyperframes-keyframes` carries an `interface:` block there), the
policy is APPENDED so upstream metadata survives, never overwritten. Store entries that are SYMLINKS to
app-owned content (`cua-driver`) never get an overlay — writing through the link would modify content
this repo does not own, so `cua-driver` stays implicitly invocable in Codex (a deliberate, documented
asymmetry).

**Hermes delivery is two-lane, under the five-profile architecture** (default/Bob, elaine, butters,
concerned, nicodemus):

- **Store-symlink lane (the lock's `hermesProfiles` table)** — the store copy is symlinked into the named
  profiles' `skills/` dirs (`default` = `~/.hermes/skills`, a specialist =
  `~/.hermes/profiles/<name>/skills`), declared in chezmoi and re-asserted by `update-skills.sh` at run
  time (creating profile `skills/` dirs when absent). `[]` means the store copy reaches no hermes
  profile. Fan-out is driven ENTIRELY by this table: non-empty means symlink, `[]` means do not. The
  final live-truth map: default = `herdr`, `moshi`, `lobster`, `todoist-cli`, `summarize-pro`,
  `home-assistant-best-practices`; butters = `chrome-devtools-axi`; concerned = `elevenlabs`,
  `last30days`; elaine = `lobster`; nicodemus = `gh-axi`, `kubernetes-specialist`, `sql-toolkit`.
  `home-assistant` maps to `[]`: hermes carries native Home Assistant runtime tools, so the runtime skill
  would be redundant there — its store copy serves Claude/Codex only (the authoring companion,
  `home-assistant-best-practices`, is what default carries).
- **Hermes-owned lane (the lock's `hermesRegistry` table)** — hermes installed the skill from a registry
  (skills.sh / ClawHub / the official registry) and owns a real hub dir in the profile. The weekly
  `update-skills.sh` hermes phase keeps these fresh: `hermes -p <profile> skills update <lockKey>` per
  entry, keyed by the entry's `lockKey`, never a list name (a ClawHub slug can differ from the skill's
  frontmatter name: `tiktok-crawling` installs `tiktok-scraping-yt-dlp`). These skills have NO store
  symlink declaration — a store symlink would shadow the hub-owned dir, which is why `hermesRegistry` and
  the non-empty `hermesProfiles` set are DISJOINT. Blocked/refused updates are loud logged warnings
  (relayed via `relay.sh`), never errors; automation never passes `--force` (bypassing a security scan
  needs per-invocation operator confirmation) and never uninstalls. `held: true` skips a skill visibly
  (none currently held). The default profile (Bob) is walked like any other — its un-entanglement is done
  (2026-07-09), and with `sql-toolkit` and `summarize-pro` since moved to the clawhub-tracked store lane,
  the registry table holds no default-profile entry: `conventional-commits` in nicodemus, the rest in
  concerned. The retired hub installs (nicodemus `sql-toolkit`, default `summarize-pro`) are unowned live
  state to hand-remove — never automated.

Name collisions resolve catalog-first (operator ruling): the `humanizer` and `hyperframes` store copies
serve Claude/Codex only and are never symlinked hermes-side — hermes gets those names from its own
catalog/hub. `summarize-pro` and `todoist-cli` left the collision set: their only hermes copies were hub
installs (since retired), so no catalog copy wins those names and the store symlink is the wanted
delivery. `test/skills-roster-fanout.sh` enforces this independently of the tables so a future lock edit
cannot quietly re-route a collision name through the store.

**Superpowers→hermes routing (the lock's `superpowersRouting` table):** the live
`~/.hermes/skills/hermes-superpowers/` mirror is hand-patched so the five skills with hermes-native
adaptations (`writing-plans`, `requesting-code-review`, `subagent-driven-development`,
`systematic-debugging`, `test-driven-development`) are referenced by their adaptation names instead of
`superpowers:<name>`, keeping the workflow out of the disabled legacy duplicates. The mapping lives in
the lock's `superpowersRouting` table, and `~/.local/bin/assert-hermes-superpowers-routing.sh` re-asserts
it idempotently on every `update-skills.sh` run and after any superpowers re-mirror — a re-assert that
fixes anything is logged loudly (and relayed), because it means something stomped the mirror.
`assert-hermes-superpowers-routing.sh --check` is the health probe: non-zero lists the stale files and
changes nothing. Scope is the hermes mirror ONLY — Claude Code's superpowers plugin keeps its
`superpowers:*` references untouched.

**Local forks (`moshi`, `herdr`) — updating:** they deliberately diverge from upstream, so
`update-skills.sh` never touches them. When updating them — or when their upstreams ship new features —
first compare against upstream (https://herdr.dev/docs/preview/agent-skill/ and
https://getmoshi.app/skill), then port wanted changes into the vendored copy by hand; the divergences are
documented in the lock's `forks` notes. The weekly run drift-checks the `forks` upstreams and, when one
changed, alerts in the run log (`~/.local/log/skills/`) and via `relay.sh` when that exists — after the
hand comparison, bump that fork's `lastComparedTreeHash` to the new upstream hash.

**Generation-exchange updates:** every npx- and clawhub-tracked skill lives inside ONE live generation
directory, `~/.agents/.skills-current` (real dirs under `skills/`, the npx CLI lock, and
`generation.json` as the ready marker); the store names `~/.agents/skills/<name>` are stable symlinks
into it and `~/.agents/.skill-lock.json` is a symlink to its lock, so sibling references like
`../hyperframes-core` stay coherent within one generation. The weekly run builds a candidate generation
as a fake HOME under `~/.agents/.skills-generations/<id>/home`, runs the package-CLI lanes against it
under `env -i` (HOME/XDG/TMPDIR/npm cache pinned inside), validates the whole candidate, and publishes
with one atomic exchange (`gmv --exchange --no-copy -T`); a lane or validation failure discards the whole
candidate and the live generation is untouched. The honest guarantee: any path resolution during or after
the exchange yields a complete tree from exactly one generation; a session that cached a resolved path
keeps a complete previous generation for at least a week (one is retained), then gets a clean ENOENT,
never partial content. Out-of-band writers (the HyperFrames workflows self-update via
`npx hyperframes skills update`, upstream-controlled, no supported disable) bypass this exactly as they
always did; the weekly recovery pass detects a store real dir where a link is expected and re-absorbs
that content into the next candidate. The weekly success stamp is the ISO week PLUS the roster-lock and
updater hashes, so a roster or updater change after a Monday success un-stamps the week and a later slot
rebuilds; per-skill failure streaks escalate the alert wording at 2 consecutive failed weeks. Accepted
narrowing: the explicit add targets `--agent claude-code --agent codex` only, so out-of-roster agent
copies (devin, goose) are no longer refreshed by these runs.

`update-skills.sh` runs weekly via the `com.webdavis.update-skills` LaunchAgent (24 hourly Monday retry
slots, 00:00-23:00, `RunAtLoad=false`, logs to `~/.local/log/skills/`); a slot defers while a harness
shows recent activity and the last slot alerts loudly (`UPDATE_SKILLS_FORCE=1` bypasses, used by tests);
the same gate covers the hermes registry-update phase, which runs after the store refresh (hermes skill
updates are unattended-safe: no GUI restarts, no gateway restart; sessions pick up content at next start,
and a deferred run just means the updates land on a later slot). The script installs only what the lock
declares, so the registered-skill count cannot grow from a run.

**Adding a skill:** if it has an official full-tree GitHub upstream, add an `npxTracked` entry
(`{"repo": "owner/repo"}`); if it is ClawHub-published, add a `clawhubTracked` entry
(`{"slug": "@owner/name", "registry": "https://clawhub.ai"}`); otherwise vendor it under
`dot_agents/skills/` (with a `forks` drift-watch entry when it has a watchable upstream). Then add its
row to `tiers` (plus the `skillOverrides` template entry and the `agents/openai.yaml` overlay when
on-demand) and `hermesProfiles` (`[]` when hermes should not carry it from the store, the named profiles
when it should), add a `hermesRegistry` entry when hermes owns it from a registry (never both a non-empty
`hermesProfiles` mapping and a `hermesRegistry` entry — they are disjoint), declare its Claude symlink
and — only for store-symlinked skills — the mapped hermes symlinks, and run `just test` — the roster test
tells you what is missing. **Removing one:** delete the store entry (or `npxTracked` row), every lock
table row, and every declaration in the same commit.

**On-demand use of an unregistered skill:** point the agent at the file — "read
`~/.agents/skills/<name>/SKILL.md` and follow it." Router/search-and-load indirection layers were
evaluated and rejected (measured lossy and slow at this library size); Hermes's larger native catalog
(`~/.hermes/skills/<category>/`) remains Hermes-only.

### Herdr Workspace Management

Workspaces (project-anchored tab groups, ≈ tmux sessions) are configured at
`dot_config/herdr/config.toml`. Eight project workspaces are reached by quick-jump chords — bound on nine
keys, mostly `prefix+ctrl+<letter>`, but the dotfiles chord is `prefix+ctrl+.` (a period, sent via CSI-u)
with a `prefix+.` fallback for terminals without CSI-u. See the design spec at
`docs/superpowers/specs/2026-06-18-tmux-to-herdr-migration-design.md` for the full mapping table. On
every terminal launch `~/.bashrc` auto-attaches to the persistent herdr session, which opens the
last-focused workspace (homelab in practice, once visited, since the session persists) — herdr has no
launch-into-workspace flag. Jump to homelab anytime via the `h` alias or the `prefix+ctrl+h` chord; the
other workspaces are on-demand via their own chords.

Ctrl-h/j/k/l "seamless nav across Neovim splits and herdr panes" is a herdr **plugin**
(`dot_local/share/herdr/plugins/herdr-smart-nav/`, a Rust binary), bound via four
`type = "plugin_action"` keybindings (`herdr-smart-nav.nav_<dir>`) — so herdr execs it directly as argv,
with no `/bin/sh -lc` wrapper. Built + linked by `run_onchange_after_57` (mirrors the `last-workspace`
plugin). It shells the `herdr` CLI (no Rust SDK); the gain over the old shell-keybinding binary is ~5 ms
(the wrapper) and is imperceptible — the value is the idiomatic plugin integration. Plugin actions get
`HERDR_PANE_ID` (not `HERDR_ACTIVE_PANE_ID`).

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

### Tailscale (headless daemon)

Tailscale runs as the open-source `tailscale` **formula** (not the `tailscale-app` GUI cask) as a launchd
**system daemon** via `sudo tailscaled install-system-daemon` (a root-owned copy in `/usr/local/bin`; the
brew formula stays user-owned so `brew upgrade` runs unattended) — it boots before login and uses the
`utun` interface, so there is no Network/System Extension to re-approve after updates (the GUI variants'
weakness on a headless host). State persists at `/Library/Tailscale` across reboots. Auth is a one-time
manual `sudo tailscale up --accept-dns=true` plus flipping **Disable Key Expiry** on the node in the
admin console — node-key expiry will not force reauthentication (no auth keys, no rotation, no
KeePassXC). `run_onchange_after_66-tailscaled-status.sh.tmpl` is a sudo-free reminder that prints those
one-time steps when the daemon is down or unauthenticated; it never runs sudo or authenticates.

**DNS:** always `--accept-dns=true` — never a static `100.100.100.100` global resolver (breaks
off-tailnet). The OSS-macOS weak spot is the resolver registration layer (`tailscale/tailscale#13461`,
`#19139`): tailscaled's internal MagicDNS resolver stays healthy, but its registration of the
`<tailnet>.ts.net` suffix route with macOS can silently half-fail (search-domain fragment written, no
nameserver route) — including at home — so tailnet names stop resolving through the system resolver while
all other DNS works. Remedy:
`sudo tailscale set --accept-dns=false && sudo tailscale set --accept-dns=true`, then
`sudo dscacheutil -flushcache; sudo killall -HUP mDNSResponder`; verify with
`dscacheutil -q host -a name <peer>.<tailnet>.ts.net` (not `dig` — dig bypasses `/etc/resolver`). Durable
fallback: needed peers are pinned in `/etc/hosts` declaratively — structured `tailnet_pins` data in
`macos_system_setup.yaml` from which the Tier-2 sudo runner template generates the idempotent pin
commands at `chezmoi apply` (tailscaled never manages that file, so entries coexist; tailnet IPs are
stable per node).

**Updates:** `brew upgrade` updates the user-owned formula (no extension re-approval needed), but the
running daemon is a separate root-owned copy a formula upgrade does not touch — after upgrading the
`tailscale` formula, re-run `sudo /opt/homebrew/opt/tailscale/bin/tailscaled install-system-daemon` to
refresh the daemon copy. On this machine (dresden) `sudo` is passwordless (the operator's `!authenticate`
sudoers config — not managed by this repo), so the re-copy is a single command; on a fresh machine expect
a password prompt.

**Daemon-host role:** when an always-home Mac exists and takes over the daemon-host role, this machine
(dresden, which is carried) cuts back to the GUI `tailscale-app` cask (better roaming DNS) and the
always-home Mac runs this daemon — make the chezmoi config machine-conditional then.

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
via `~/.local/bin/hue-pulse.sh`. Known interactive TUIs (vim/less/top/ssh/herdr/claude/fzf) are skipped.

### Herdr Native Status

Workspace state (per-pane agent status: blocked / working / done / idle) is rendered by herdr — no
third-party plugin or custom script. The sidebar rolls each workspace up to its most-urgent agent state.
Claude Code, Codex, Cursor, OpenCode, and others are recognized out of the box.

## Code Style

- Shell files: 2-space indent, case-indent enabled, simplified (`shfmt -i 2 -ci -s`, wired in
  `treefmt.nix`). When running shfmt by hand, pass these flags explicitly — `.editorconfig` only covers
  `dot_fzf*` and `dot_bash*` patterns, for editors.
- **Bash follows the [Wooledge BashGuide](https://mywiki.wooledge.org/BashGuide) practices.** The rules
  that come up most in this repo:
  - `set -euo pipefail` at the top of every script; double-quote every expansion.
  - `[[ ]]` for tests, never `[ ]`, in anything with a bash shebang.
  - Lists are **arrays**, never space-separated strings — no unquoted `$VAR` expansion loops and no
    `shellcheck disable=SC2086` suppressions to make them lint.
  - Never `for x in $(command)` — iterate command output with
    `while IFS= read -r x; do ...; done < <(command)`. If the loop body runs anything that may read stdin
    (git, ssh, ffmpeg), read on a dedicated fd: `while IFS= read -r -u3 x; do ...; done 3< <(command)`.
  - Build JSON with `jq -n --arg`/`--argjson`, never by interpolating variables into a JSON string.
  - `printf` for any output containing variable data; `echo` only for fixed literal text.
  - Don't parse `ls`; use globs (guarded with a `[[ -e ]]`/`[[ -d ]]` test or `nullglob`).
  - Validate numeric arguments with a `[[ =~ ]]` pattern before using them.
  - Unknown CLI arguments/commands are an error: usage to stderr, exit non-zero — never a silent
    fallthrough to help with exit 0.
- Markdown: wrapped at 105 columns, non-consecutive numbering (`mdformat` with `.mdformat.toml`).
- Nix: formatted with nixfmt (RFC 166 style — `treefmt.nix` pins `pkgs.nixfmt-rfc-style` because the bare
  `nixfmt` attribute in nixpkgs 25.05 is still nixfmt-classic).
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
