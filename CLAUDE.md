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

All lint and format tooling is orchestrated by the STANDALONE [treefmt](https://treefmt.com/) binary,
configured in `treefmt.toml`.

```bash
just l             # run all twelve formatters (four rewrite in place, eight check only)
just L             # lint-check: drift gate (treefmt --no-cache --fail-on-change); `just c` is an alias
just s             # shellcheck, plain files and rendered chezmoi templates
just S             # shfmt (format shell files) only
just m             # mdformat only
just t             # taplo (TOML) only
just j             # jq (JSON validation, incl. rendered osquery configs) only
just y             # yq (YAML validation) only
just lint-actions  # actionlint + zizmor on .github/workflows
```

Four formatters rewrite files (shfmt, mdformat, taplo, stylua); the other eight only read and fail the
run on bad input, and the three render-then-validate ones are plain scripts under `scripts/treefmt/`.

`just l` auto-formats in place. `just lint-check` is the drift gate at pre-push and in CI; standalone
treefmt has no dry-run mode and no sandbox, so a red gate has ALSO already written the fixes into the
working tree: stage them and retry. A full uncached run measures ~16s; cached runs only process changed
files.

### Testing

```bash
just test-unit          # Unit suite only (the fast commit gate)
just test-bashunit      # One suite's `<name>.test.sh` files alone (default test/unit)
just test-integration   # Integration suite only
just test-e2e           # End-to-end suite only
just test-rust          # cargo test for the two herdr plugins and the pns crate (+ fmt/clippy for pns)
just test               # The three shell suites plus the Rust tests (CI runs this)
just ship               # the three gates CI runs, in CI order, the explicit pre-PR sweep
```

**Tests must be fast or they go** (operator ruling): every test passes within a second, measured, and a
slow one is deleted rather than tolerated. Bash tests are migrating from bats to **bashunit** (operator
ruling 2026-09-03): a bashunit file is `test/<suite>/<name>.test.sh`, non-executable, one behavior per
`function test_*`, run as one of the three lanes of `test/run-test-suite.sh`. The files still on bats are
one behavior per `@test` through HOST bats-core, run by each suite's own runner; both shapes are legal
until the migration finishes, and no new bats file is added. The Neovim config's Lua specs
(`dot_config/nvim/tests/*_spec.lua`) run under `nvim --headless --clean -l` through `just test-nvim`,
also a dependency of `test-unit`; Rust is tested with `cargo test`. A large purge in 2026-08 left 160+
deleted files in git history as a cherry-pick pool: restore individual logic asserts from it, never
wholesale.

**We test the behavior of tools we wrote, and nothing else** (operator ruling 2026-08-05). Not chezmoi,
not Homebrew, not launchd, not any third-party behavior, and not deployment. In scope: pns, the osquery
pipeline, rotate-logs, update-skills, the macos-defaults library, ssh-hardening, cutover-gate,
live-reconcile, the cli-print-style library, the two herdr Rust plugins. Out of scope, and deleted on
sight: LaunchAgent plist field assertions, "is this hook wired in", `.chezmoiignore` OS branching,
roster-versus-lock-table agreement, justfile-versus-CI-workflow parity, markdown heading guards, and
meta-tests about how other tests are written. The question to ask is whether gutting our source logic
while leaving the declarations intact would turn the test red. If it would not, it is not testing our
behavior. **This deliberately leaves declarations unguarded**, which is the accepted price: a config that
disagrees with itself is now caught by review, not by a gate.

The **commit** gate runs `just test-unit` only, kept fast on purpose: it runs `just test-nvim`, then
`just test-neotest-bashunit`, then the one runner (`test/run-test-suite.sh`) with
`--shuffle --warn-slow-ms 200`, so order is seed-shuffled each run (replay a failure with
`TEST_SEED=<seed>`, printed every run, since Bats 1.11 has no native shuffle; shuffling degrades to
sorted order on a host with neither `gshuf` nor `shuf`). A WARN-ONLY performance summary lists any test
over the threshold as a refactor-or-move-suite candidate; warnings never fail the run.

**CI** runs `just test`, and `just ship` runs CI's three gates as literal command lines
(`just lint-check`, `just test`, `just lint-actions-security`). Nothing enforces that those two stay in
agreement any more: the parity test was declaration-consistency checking, not tool behavior, so it went
with the 2026-08-05 scope ruling. **Edit one and you must edit the other by hand.** The pre-push hook
deliberately runs no suite. Each suite's runner executes its own `.test.sh`, `.sh` and `.bats` once,
through host bashunit and host bats-core.

So a commit can briefly carry an integration or e2e regression, and so can a push: **CI is the only gate
that runs the suite**, and it runs on pull requests and on pushes to `main` only. A push to a topic
branch with no open pull request runs the suite nowhere; `just ship` is how you cover that window
deliberately.

`just validate-tests` (`test/validate-tests.sh`, a dependency of every suite recipe) fails if a `*.sh` or
`*.bats` sits outside a recognized suite. Only `validate-tests.sh` and `run-test-suite.sh` may sit at
`test/` root. Three trees are carved out: a suite's own `helpers/`, the shared cross-suite
`test/helpers/`, and `test/fixtures/**`. The two helper trees admit non-executable `*.sh` only, so an
executable file or a `.bats` there still fails, and neither they nor `test/fixtures/**` may hold a
`<name>.test.sh`, because a test file outside a suite is one no suite recipe runs. The checker also
rejects any symlink below `test/`, a non-executable suite `*.sh`, and a nested file in a flat suite. Add
a test by dropping a new executable `test/<suite>/<name>.sh` in place (with `REPO_ROOT` depth
`dirname "${BASH_SOURCE[0]}")/../..`); it is picked up automatically.

A bashunit file inverts that mode rule and must NOT be executable: bashunit sources its test files, while
`run-test-suite.sh` discovers `*.sh` by `-perm -u+x`, so an executable `<name>.test.sh` would run twice,
the second time as a bare script where `assert_same` does not exist.

**bashunit is never handed a directory**, in the runner or in a recipe. Its own path argument scans
RECURSIVELY for `*[tT]est.sh` plus a `.bash` twin, so a directory reaches a fixture named `latest.sh`, an
executable `contest.sh` the `*.sh` lane also runs, and every other suite's files, which would put an
integration test in the unit gate and leave it out of its own recipe. `run-test-suite.sh` passes an
exact, suite-local, `-maxdepth 1` list of `*.test.sh` paths instead. Color is disabled with `NO_COLOR=1`,
because `--no-color` is ignored in either position on 0.50.1.

### Chezmoi operations

```bash
just d                                      # chezmoi diff
just a                                      # chezmoi apply
chezmoi status                              # show pending changes
chezmoi edit <file>                         # edit a template (prefer over direct edit of .tmpl)
```

**THE OPERATOR RUNS APPLIES. Agents do not.** Both recipes above reach templates, so both need KeePassXC
unlocked and an interactive terminal. An agent proposes changes and lets the operator apply them. This
holds until the vault is replaced with a password manager an agent can unlock.

**Why `--exclude=templates` was retired** (it was the mandated agent apply until 2026-08-10). It left the
deployed copy of a templated target behind its source, while the osquery known-good manifest derives its
hashes from the SOURCE. The two then disagree, and the pipeline audit reads that as tampering: a FALSE
CRIT page on every tick until a full apply catches up, across ten manifested templated targets (seven
osquery LaunchAgent plists, `posture-controls.json`, and the two osquery staging files). It also never
delivered what it was for: it does NOT skip a `modify_` template (measured 2026-08-02), and two of those
call `keepassxc`, so the excluded apply reached the vault anyway.

**A by-name apply is NOT a supported shortcut.** `chezmoi apply <path>` deploys that path without running
the `run_` scripts, so `run_after_05-osquery-known-good-manifests.sh` never refreshes the known-good
manifests. Deploy a MANIFESTED file that way and its hash no longer matches the manifest, which the
pipeline audit reads as tampering and pages CRIT on every tick until a full apply. The manifested set is
the osquery pipeline under `~/.local/libexec/osquery/`, the managed scripts under `~/.local/bin` and
`~/.local/libexec`, and the osquery LaunchAgents. Use a full `chezmoi apply`; it is what keeps the
deployed state and the manifests derived from the same source state. The by-name form existed to dodge
the vault, which is no longer a goal now that the operator applies with it unlocked.

Fourteen targets pull secrets through `keepassxc` and need KeePassXC unlocked: `~/.gitconfig`,
`~/.aws/credentials`, `~/.claude.json`, `~/.codex/config.toml`, `~/.composio/user_data.json`,
`~/.config/atuin/config.toml`, `~/.config/himalaya/config.toml`, `~/.config/openhue/config.yaml`,
`~/.config/pns/config.toml`, `~/.config/uu/config.toml`, `~/.config/gogcli/credentials.json`,
`~/.hermes/.env`, `~/Library/Application Support/Claude/claude_desktop_config.json`, and
`~/Library/Application Support/espanso/match/identity.yml`. Non-KeePassXC targets (for example
`~/.bashrc` and `~/.claude/settings.json`) are safe to apply from automation.

### Cutover tooling

Two operator-run scripts in `scripts/`, invoked by absolute path (no justfile recipe):

- `scripts/cutover-gate.sh <1|2|3|4|5>` runs one ordered cutover gate (preflight, activation,
  reconciliation, soak, closure), keeping its ledger under `~/.local/state/cutover`. Covered by
  `test/unit/cutover-gate-{usage,deletion-agreement,managed-comparison}.sh`.
- `dot_local/libexec/unattended-upgrades/agent-skills/executable_live-reconcile.sh` converges the live
  skills fan-out to the committed lock, `--dry-run` first. Covered by
  `test/unit/live-reconcile-app-owned-exemption.sh`.

## Architecture

### Source-only files and OS targeting

`.chezmoiignore` declares both: which dev and CI files never reach `$HOME`, and a trailing OS-conditional
block that drops `Library` and the macOS-only helpers on Linux. Read the file rather than a copy of it
here; this paragraph used to transcribe it and drifted twice. Templates branch on
`{{ if eq .chezmoi.os "darwin" }}` for macOS-specific content.

One thing the file does not say: `.worktrees/` is deliberately NOT in it; it is gitignored and
treefmt-excluded instead.

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
with keys `taps`, `formulae`, `casks` and `mas`, plus two siblings of `homebrew` under `packages.macos`:
`uv` (uv tool installs, e.g. `graphifyy`, which provides the `graphify` CLI behind the post-commit
dispatcher) and `fnm` (the node runtime plus the npm CLI tools that run on it, e.g. `happy` and
`@tobilu/qmd`, grouped under one `node` version; bump that one value to move every tool to a new LTS).
fnm's `~/.local/share/fnm/aliases/default/bin` is a version-free path that LaunchAgents and the bashrc
rely on. Gotcha: npm is an env-node script, so every scripted npm call must put that fnm dir first in
PATH, or npm runs on whatever `node` PATH finds and installs into that node's prefix. One script,
`.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl`, consumes all of them: it generates a
Brewfile from the data, runs `brew bundle`, then runs a guarded `brew bundle cleanup --force`.
Prerequisite: `run_once_before_00-install-homebrew.sh.tmpl` runs the upstream installer when
`command -v brew` finds nothing.

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
`cleanupPeriodDays`, `autoUpdatesChannel`, `remoteControlAtStartup`, `spinnerTipsEnabled`, `effortLevel`,
`extraKnownMarketplaces`) while letting `/config` toggles drift freely. `enabledPlugins` is a third case:
the roster is declared, the per-plugin disable state is read back out of the live file. The full field
model, the plugin-state trade and the corrupt-file recovery path are in
`docs/runbooks/claude-code-settings.md`.

### Codex settings

`private_dot_codex/modify_private_config.toml` is the same mechanism for `~/.codex/config.toml`, which
Codex rewrites from its own model while it runs. Stable fields are overwritten on every apply (model and
reasoning, sandbox and approval policy, `notify`, `[features]`, `[memories]`, `tui.vim_mode_default`, the
two git marketplaces and the four MCP servers this repo declares), everything else drifts freely, and
`[projects.*]` is the third case: the roster of trusted roots is declared and every undeclared live entry
is preserved.

**`[hooks.state]` is hook trust and is never written from source.** It is only ever read back out of the
live file, because a snapshot in version control would let an apply silently re-grant trust the operator
revoked at the Codex prompt; a fresh machine renders none of it and Codex re-prompts, which is correct. A
no-op apply emits the live file back byte for byte instead of re-serializing, so `just d` stays quiet.
`fromToml` hard errors on input that is not TOML and a modify-template that errors aborts the WHOLE
apply, so `.chezmoiscripts/run_before_13-quarantine-unparseable-codex-config.sh` moves an unparseable
file into `~/workspaces/backups` first, at the cost of every hook approval on the machine.

### Agent skills (cross-harness store)

`~/.agents/skills` is the single canonical skills store (37 roster skills), serving Claude Code (chezmoi
symlink declarations under `private_dot_claude/skills/`), Codex (native store scan, no declarations) and
hermes (declared symlinks into the default profile and four specialist profiles). Provenance, tiering and
fan-out are recorded in `dot_agents/custom-skill-lock.json`. **Nothing enforces that those three agree
any more:** the roster guard was declaration-consistency checking, not tool behavior, so it went with the
2026-08-05 scope ruling. Adding or removing a skill means editing the store, every lock table and every
per-harness declaration by hand, and a missed one now surfaces as a skill quietly not reaching a harness
rather than as a red build. `~/.local/libexec/unattended-upgrades/agent-skills/update-skills.sh`
refreshes the npx-, clawhub- and app-owned lanes weekly, publishing a new generation with one atomic
exchange.

`docs/runbooks/agent-skills-store.md` carries the delivery model, the lane mechanics, the fork
drift-watch states, the generation-exchange guarantee, the schedule, and how to add or remove a skill.

### Global instruction files

The global ruleset for Claude Code and Codex is one shared partial,
`.chezmoitemplates/global-agent-rules.md`, pulled into both `private_dot_claude/CLAUDE.md.tmpl` (target
`~/.claude/CLAUDE.md`) and `private_dot_codex/AGENTS.md.tmpl` (target `~/.codex/AGENTS.md`) with
`includeTemplate`, between a pair of `shared-rules` markers. Harness-specific rules go in the including
file, below the shared block. Edit the partial, never a harness copy: the test that byte-compared the two
rendered copies went with the slow-suite purge on 2026-08-05, so nothing catches a divergence now.

`backpass` (npm, on the fnm lane, with `acpx` as its runtime dependency) reads agent session transcripts
and proposes evidence-backed edits to that ruleset. Its own user-scope default would optimize
`~/.claude/CLAUDE.md`, a rendered chezmoi target, so every accepted edit would be erased by the next
apply. `dot_config/backpass/config.json` (target `~/.config/backpass/config.json`) therefore overrides
`user.memoryFiles` to the SOURCE partial in this checkout: one file, both harnesses, and the edit
survives. Analysis never writes; `backpass apply --scope user` is the only writer and gates every edit
interactively. Its skill extractions still default to `~/.agents/skills`, which is the managed store, so
an accepted extraction lands an undeclared real directory there that only Codex's native scan picks up.

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
`.chezmoi.toml.tmpl`, `dot_bashrc.tmpl`, `dot_gitconfig.tmpl`, and most of `.chezmoiscripts/`). Templates
branch on `.chezmoi.os` and, where they pull secrets, call `keepassxc`. Reusable fragments live in
`.chezmoitemplates/` and are pulled in with `includeTemplate`.

### Template shellcheck workaround

Shell templates contain Go template syntax that shellcheck can't parse directly, so the
`shellcheck-rendered-template` formatter (`scripts/treefmt/shellcheck-rendered-template.sh`) renders
first (`CI=1 chezmoi --source "$PWD" execute-template --no-tty <file`, with a throwaway `HOME` because
chezmoi's read-source-state pre hook chdirs there) and shellchecks the result. `--source "$PWD"` is
load-bearing: it is what makes `includeTemplate` resolve against this checkout.

`treefmt.toml` hands the script EVERY `.chezmoiscripts/*.sh.tmpl` and root `dot_*.tmpl`, and the script
classifies per file (the old nix-eval classifier is gone with the flake): not a shell template (no shell
shebang or `# shellcheck shell=` directive in the leading lines) means skip, and a file that mentions
`keepassxc`, or transitively includes a `.chezmoitemplates/` partial that does, means skip (an
interactive vault unlock cannot render headless; the grep is deliberately broad, so an over-match costs
one skipped lint, never a false failure).

After a successful render, a blank (empty or whitespace-only) result is skipped rather than shellchecked,
so an OS-gated template on the other OS does not fail SC2148; a render failure stays fatal (the per-file
body is `scripts/treefmt/lib-shellcheck-rendered-template.sh`, driven by
`test/unit/rendered-template-shellcheck-wrapper.sh` with stubs). The `CI=1` env var is defensive here,
but it is load-bearing for the sibling `espanso-match-render` formatter, whose vault reads sit behind
`{{ if (env "CI") }}`.

Two more sibling formatters render before validating: `osquery-config-render` renders the JSON-bodied
`.conf` files under `dot_local/libexec/osquery/osquery-converge/desired/` (two of the six are templates;
`execute-template` on a file holding no template action renders it to itself, so one code path covers
both kinds) and checks them with jq, and `espanso-match-render` renders the espanso `*.yml.tmpl` match
files and checks them with yq.

### Dev environment (no nix)

The contributor toolchain is Homebrew plus uv, no dev shell (the flake was removed 2026-08-05).
`just setup` installs it into a fresh checkout: `brew bundle --file=Brewfile.dev` for the binary tools
(actionlint, age, bash, bashunit, bats-core, chezmoi, coreutils, gitleaks, jq, just, luacheck,
shellcheck, shfmt, stylua, taplo, treefmt, uv, yq, zizmor), then a uv install of mdformat and its six
plugins. On dresden those formulae are also declared in `.chezmoidata/system_packages_autoinstall.yaml`,
so the weekly bundle keeps them; `Brewfile.dev` is what a machine without that bundle needs. Nix remains
installed on the machine for unrelated uses; this repo never invokes it.

**mdformat is version pinned and the pins live in two places.** It rewrites markdown, so a version bump
silently rewraps every file and fails the drift gate on work nobody did. The exact `==` versions are in
the `setup` recipe and again in the toolchain step of `.github/workflows/lint.yml`; nothing enforces that
the two agree, so they must be moved together by hand. The same hand-sync applies to `Brewfile.dev`
against that workflow step, which installs the same formulae by name (`gitleaks` is the one addition, for
the pre-commit hook; CI never commits).

**bashunit is pinned in CI and gated by the unit and full test suites.** A release that changed an output
shape would leave `neotest-bashunit`'s frozen fixtures green while it misreported real runs, so the
adapter records the release it was measured against in `parse.verified_version` and
`just test-neotest-bashunit` refuses to certify fixtures captured from a different one, naming both
versions. CI cannot rely on that gate alone: the runner's cached Homebrew index poured 0.43.0 against
fixtures measured on 0.50.1 and the gate did its job by turning the whole Lint job red. So the toolchain
step in `.github/workflows/lint.yml` downloads bashunit's release asset itself and verifies it against a
sha256 pinned beside the version, rather than taking the formula or running an unpinned installer script.
Pinning the checksum in this repository is what the version alone cannot do: a compromised upstream
release can rewrite a tag and the checksum it publishes, but not the one in our tree.

The release is now named in three places, and nothing enforces that they agree: `BASHUNIT_VERSION` and
`BASHUNIT_SHA256` in that workflow step, and `parse.verified_version`. Move all three together by hand,
and moving any of them means re-measuring every fixture rather than editing a number.

Local stays Homebrew (`Brewfile.dev`, `.chezmoidata/system_packages_autoinstall.yaml`), because Homebrew
has no declarative version pin the way `uv`'s `==` does. The cost: a `brew upgrade bashunit` past the pin
makes `just test-unit` refuse on a machine where nobody touched a test, until the fixtures are
re-measured and all three pins are updated. That is the stylua trade below, accepted for the same reason:
a visible failure on an untouched file beats a silent behavior change.

**stylua is deliberately NOT pinned.** It is also a byte rewriter, same class of risk as mdformat, but
Homebrew has no declarative version pin the way `uv`'s `==` does. The cost: a newer stylua on a fresh
machine or CI runner can rewrite or reject Lua that was clean under the older one. Accepted because the
failure is visible, not silent: the drift gate goes red on a file nobody touched, which is a signal to
investigate, not corruption that slips through. A version skew shows up as `just lint-check` failing on
unmodified `dot_config/nvim/**/*.lua` files right after a `brew upgrade stylua`.

### CI

GitHub Actions (`.github/workflows/lint.yml`) runs on `macos-latest` on pushes to main and on pull
requests, with workflow-level `permissions: contents: read`, `persist-credentials: false` on checkout,
and actions SHA-pinned to full commit SHAs. `.github/dependabot.yml` keeps the pins fresh weekly behind a
7-day release cooldown; its PRs auto-merge via `.github/workflows/dependabot-automerge.yml`, which uses
`gh pr merge --auto` so branch protection, where `lint` is a required status check on `main`, is what
actually holds the merge until green.

Five steps: checkout, install the toolchain (brew + uv), then the three gates as literal commands:
`just lint-check`, `just test`, `just lint-actions-security`. Nothing classifies or compares those steps
against the justfile: `test/unit/ship-ci-gate-parity.sh` was deleted in `7e8e24a4` under the 2026-08-05
scope ruling.

### Where deployed scripts live

`~/.local/bin` holds only what the OPERATOR TYPES. Everything invoked by launchd, a hook, a keybinding or
a `just` recipe lives under `~/.local/libexec`, because `just` and launchd are the interface and the
script beneath them is an implementation detail. Today that leaves exactly one file in `bin`
(`ssh-hardening.sh`).

Four rules decide the shape below `libexec`, in this order:

1. **A directory names a DOMAIN, a SYSTEM, or a FUNCTION**, never a dependency and never a vendor.
   `osquery/`, `macos-defaults/` and `tailscale/` name what the scripts act on (all three hold scripts
   this repo authored, not scripts those projects ship); `pns/` names the system the scripts belong to;
   `unattended-upgrades/` names what they do. A directory named for a CLI a script happens to shell out
   to would need `jq/` and `curl/` siblings to be consistent, so that axis is not used.
1. **A directory exists only when it has more than one member.** A leaf with no private helpers stays a
   flat file (`compress-and-truncate-local-logs.sh`, `control-hue-lights.sh`). Make the group the day a
   second member arrives, not in anticipation of one. The exception is a single file whose own name
   cannot carry its domain: `tailscale/reconcile-hosts-pin.sh` keeps its directory because the filename
   says nothing about Tailscale and it is the only root-executed script in the tree.
1. **A tool with PRIVATE helpers gets a directory named after itself**, and its entrypoint keeps the
   tool's name inside it (`osquery/results-alerter.sh` beside `osquery/results-alerter/`, and
   `osquery/osquery-converge.sh` beside `osquery/osquery-converge/`). Never `main.sh`: the basename is
   what shows up in `ps`, in launchd output and in every log line, so five directories of `main.sh` would
   be five indistinguishable processes. That directory holds a tool's private DATA as well as its private
   code (`osquery-converge/desired/` is the state the tool installs; `osquery/posture-controls.json` is
   the flat-file version of the same idea), because the alternative is data under `share/` that none of
   the integrity coverage anchored on this tree reaches.
1. **`helpers/` holds code shared ACROSS a group**; a helper every caller of which sits in one
   subdirectory lives in that subdirectory instead. `unattended-upgrades/helpers/log-entries.sh` is
   shared by the three bash weekly jobs (`uu`, the fourth weekly job, is a standalone Rust binary and
   shares none of it), while `agent-skills/assert-hermes-superpowers-routing.sh` stays in `agent-skills/`
   because both of its callers, `update-skills.sh` and `live-reconcile.sh`, are there too. This mirrors
   the `test/<suite>/helpers/` split.

Names are verb-first where a bare noun would not say what happens (`compress-and-truncate-local-logs.sh`,
`control-hue-lights.sh`). A stutter is accepted when removing it would leave a meaningless basename:
`macos-defaults/macos-defaults-apply.sh` stays, because `apply.sh` in a log line says nothing.

**`pns/` IS THE RUST ENGINE NOW, and the directory says so.** `pns` is the compiled binary, built at
apply time from the crate at `~/.local/share/pns` and installed here because launchd and the hooks are
what run it. Its four destinations (phone, Discord, banner, lights) are compiled-in plugins the
`~/.config/pns/config.toml` file selects by name, so adding one is a registration rather than a file
dropped in a directory. The HOOKS are the engine too:
`pns hook prompt|stop|stop-failure|blocked|asked|plan-ready|denied|resolved|model-switch|quota|config-change`
reads the harness payload on stdin and runs the one event path, and `pns gate <harness>-hook` is the
presence-gated pass-through to moshi-hook. That gate answers the BARE word too (`pns pi-hook`), which is
the spelling moshi's own generated pi and omp extensions are stuck with: their `helperBinary` field holds
one pathname and has no room for a subcommand. `hooks/codex/install-hooks.sh` is the last bash left under
`pns/`, because writing another tool's config file is what it does.

**The shipped config template is a GENERATED FILE.** `dot_config/pns/private_config.toml.tmpl` is
`render`'s own output over the committed `dot_config/pns/config-values.toml`, produced by
`just pns-config-render`; a hand edit to the template fails a byte-equality test in `just test-rust`. The
values file carries only what differs from the shipped posture, plus the keepassxc entry name behind each
secret, and never a secret value itself.

**Moving a script is never just a move.** Its path is referenced by LaunchAgent plists, `.chezmoiscripts`
runners, Claude Code hook declarations in `modify_settings.json`, aerospace and herdr keybindings, the
`.chezmoiignore` OS-conditional block, the justfile, and osquery's file-integrity watch paths plus the
known-good manifest generator and the alerter's verdict routing. Chezmoi also does not delete the old
target: the file stays in `$HOME` at its former path until it is removed by hand.

### Background jobs and LaunchAgents

Every scheduled or supervised job on dresden is a chezmoi-tracked plist under `Library/LaunchAgents/`,
bootstrapped by a matching `.chezmoiscripts/run_onchange_after_*` loader.

| LaunchAgent                                        | What it does                                         |
| -------------------------------------------------- | ---------------------------------------------------- |
| `com.webdavis.atuin-daemon`                        | supervises the atuin history daemon                  |
| `com.webdavis.happy-daemon`                        | supervises the happy remote-control bridge           |
| `com.webdavis.update-skills`                       | weekly skills-store refresh (24 Monday retry slots)  |
| `com.webdavis.report-plugin-updates`               | weekly record of what Claude Code auto-updated       |
| `com.webdavis.pns-daemon`                          | the pns clock: runs leased jobs between events       |
| `com.webdavis.uu`                                  | weekly unattended-upgrades run, one lane per subject |
| `com.webdavis.rotate-logs`                         | rotates `~/.local/log/`                              |
| `com.webdavis.yt-dlp-pot-provider`                 | the yt-dlp proof-of-origin token provider            |
| `com.webdavis.osquery-heartbeat`                   | proves the osquery pipeline is alive                 |
| `com.webdavis.osquery-results-alerter`             | turns osquery results into notifications             |
| `com.webdavis.osquery-alert-drainer`               | drains the queued alerts                             |
| `com.webdavis.osquery-digest`                      | periodic roll-up                                     |
| `com.webdavis.osquery-firewall-gatekeeper-monitor` | watches firewall and Gatekeeper posture              |
| `com.webdavis.osquery-tailscale-monitor`           | watches tailscaled posture                           |
| `com.webdavis.osquery-uptime-watchdog`             | watches for a machine that stopped reporting         |

The root daemon's own side is CONVERGED, not written once. `~/.local/libexec/osquery/osquery-converge.sh`
compares each of the six files we own in `/var/osquery` (plus the two directory modes) against the
desired state deployed beside it under `osquery-converge/desired/`, installs whatever drifted with
`sudo /usr/bin/install -o root -g wheel -m 0644` out of a private 0700 copy of that staging tree, and
restarts osqueryd only when something did, requiring the ppid-1 parent to be a DIFFERENT process from the
one running before the stop and still up after a settle window. No drift means no privileged call, no
restart and no output. Anything irregular (a symlink standing in for a target directory, the staging tree
or the vendor plist) is refused rather than repaired, because `install -d` follows a link. Two callers:
`.chezmoiscripts/run_after_50-setup-osquery.sh` on every apply (a PLAIN script, so `--exclude=templates`
still runs it) and uu's `brew` lane right after its upgrade pass, because the osquery cask upgrade is
what wipes those files and it runs unattended; a converge tool that is not deployed FAILS that lane's
step rather than passing quietly. The control catalog stays in
`.chezmoidata/macos_posture_controls.yaml`. KNOWN LIMIT: `--exclude=templates` does not refresh the two
templated desired-state files, so config CHANGES ship on a full apply; wipe repair needs only the staging
already on disk. Editing either of them also pages a CRIT until that full apply, because the known-good
manifest records the new render while the deployed copy still holds the old one. That is a property of
manifesting intent, not of the converge: ten templated targets sit in the pipeline manifest arm and all
of them behave this way.

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
with the new drop-in already published and the legacy file already moved aside. The watchdog's deadline
and group kill are no longer pinned by a test, which went with the slow-suite purge on 2026-08-05. The
reload and lockout-recovery procedure is in `docs/runbooks/macos-fresh-machine-quickstart.md`.

### Herdr workspace management

Workspaces (project-anchored tab groups, roughly tmux sessions) are configured at
`dot_config/herdr/config.toml`. Eight project workspaces are reached by quick-jump chords bound on nine
keys, mostly `prefix+ctrl+<letter>`, but the dotfiles chord is `prefix+ctrl+.` (a period, sent via CSI-u)
with a `prefix+.` fallback for terminals without CSI-u. The design spec at
`docs/superpowers/specs/2026-06-18-tmux-to-herdr-migration-design.md` has the full mapping table.

The jump itself is a herdr **plugin** (`dot_local/share/herdr/plugins/herdr-workspace-jump/`, a Rust
binary), bound via nine `type = "plugin_action"` keybindings. herdr has no built-in create-or-focus:
`workspace create` is not idempotent and `workspace focus` takes an id rather than a label, so the label
is resolved against the live list first, exact match and first hit (labels are NOT unique on this
machine). It sends two newline-delimited JSON requests per jump through `HERDR_SOCKET_PATH`, opening a
fresh connection for each request. Every socket failure falls back to the `herdr` CLI through
`HERDR_BIN_PATH`. A keybinding passes NO arguments, so the label and the working directory are baked into
each action's argv in the manifest, one action per workspace; herdr does not run an action through a
shell, so the manifest's `~` is expanded by the plugin. Built and linked by `run_onchange_after_56`.

The same plugin owns the workspace-level most-recently-used toggle on `prefix+ctrl+\\` (herdr ships
`last_pane` but no workspace equivalent). Its `[[events]]` hook on `workspace.focused` fires for EVERY
focus change, mouse and picker included, which is why it stays correct where a key-bound script did not:
a script only sees the switches routed through itself. The two-deep state lives at
`~/.local/state/herdr/plugins/herdr-workspace-jump/mru` and needs no extra socket call, because checking
that the target still exists reuses the same workspace list a jump already reads.

On every terminal launch `~/.bashrc` auto-attaches to the persistent herdr session, which opens the
last-focused workspace (homelab in practice, once visited, since the session persists); herdr has no
launch-into-workspace flag. Jump to homelab anytime via the `h` alias (which invokes the same plugin
action) or the `prefix+ctrl+h` chord.

Ctrl-h/j/k/l "seamless nav across Neovim splits and herdr panes" is a herdr **plugin**
(`dot_local/share/herdr/plugins/herdr-smart-nav/`, a Rust binary), bound via four
`type = "plugin_action"` keybindings (`herdr-smart-nav.nav_<dir>`), so herdr execs it directly as argv
with no `/bin/sh -lc` wrapper. It is built and linked by `run_onchange_after_57`, and it shells the
`herdr` CLI rather than using a Rust SDK. Plugin actions get `HERDR_PANE_ID`, and the binary falls back
to `HERDR_ACTIVE_PANE_ID` when that is absent.

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

`worktree-path` puts worktrees at `~/.herdr/worktrees/<repo>/<branch>`, which is herdr's own layout:
herdr hardcodes `<directory>/<repo>/<branch-slug>` and worktrunk's path is templatable, so worktrunk
bends to match and both tools create worktrees in one place rather than two.

### Bashrc init ordering

The canonical order inside the interactive block is direnv, starship, zoxide, atuin. Direnv's hook runs
early and is the first of the three `PROMPT_COMMAND` writers; starship initializes next; zoxide and atuin
initialize late within the interactive block, atuin last.

`bash-preexec` is sourced explicitly from Homebrew (atuin 18.x stopped bundling it) BEFORE `atuin init`,
because atuin's `__atuin_preexec` and `__atuin_precmd` and this repo's long-running command timer all
register into `preexec_functions` and `precmd_functions`. A naked `DEBUG` trap would clobber atuin's
recording.

Carapace provides universal completion, including for `gh` and `git`; it loads after bash-completion@2
and direnv and before starship.

### Long-running command notifier

`dot_bashrc.tmpl` registers `__cmd_notify_preexec` and `__cmd_notify_precmd` via bash-preexec (atuin's
framework), inside a darwin gate, because the engine is macOS-only. The shell is an engine producer like
the Claude and Codex hooks and the weekly jobs, so both tiers call `~/.local/libexec/pns/pns` rather than
raising their own banner: the state is `done` or `failed` off the exit code, the detail is the command
name and how long it ran, and the pane is `HERDR_PANE_ID`, which is what makes the banner focus that pane
on click. Commands at 30s or longer go through the engine's normal presence gate (banner and Discord
always, phone when away; operator ruling 2026-08-06: away means mobile, and mobile means glancing, so 30s
is enough to earn the phone); at 5 minutes or longer they pass `--long-running`, and the lights are part
of the engine's own delivery plan from there, pulsing green on success and red otherwise off the same
exit code the state came from. The shell used to make a second `pns pulse` call of its own, which meant
the tier was decided twice and could disagree with itself. `pns pulse <exit-code>` still exists, but
nothing in this repo calls it: it is the operator's manual command for signalling the lights by hand and
for checking that a `[plugins.hue]` table's bridge and key actually work. Interactive TUIs are skipped by
a prefix match on the command line: `vim`, `nvim`, `less`, `man`, `top`, `btop`, `ssh`, `herdr`,
`claude`, `hermes`, `codex`, `fzf`. The agent CLIs are on that list because they fire their own relay
hooks.

## Code Style

- Shell files: 2-space indent, case-indent enabled, simplified (`shfmt -i 2 -ci -s`, wired in
  `treefmt.toml`). When running shfmt by hand, pass these flags explicitly, `.editorconfig` only covers
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
