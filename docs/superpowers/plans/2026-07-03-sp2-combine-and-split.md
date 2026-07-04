# SP2 — Combine & Split: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended)
> or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax
> for tracking.

**Goal:** Ship the union of PR #31 (herdr/tailscale/weekly-brew/moshi) and PR #25 (osquery three-tier)
onto `main` as a sequence of small, self-contained, individually reviewable pull requests — reimplemented
from `main`, folding in review feedback and the SP2-tagged fixes from the work ledger — while a frozen
integration branch stands as the never-merged live-state reference.

**Architecture:** One `integration/modernization` branch preserves the full combined diff as a
DO-NOT-MERGE reference PR (dresden's live source until cutover). Feature work reaches `main` only through
small PRs, each branched from the *current* `main`, each carrying one feature's files fully wired, each
green on `just lint-check` + `just test`, each merged only after user review. Later slices branch from
the main that earlier slices already advanced, so shared-file edits layer cleanly instead of conflicting.

**Tech Stack:** chezmoi (dotfiles source→target), Nix flake dev shell (shellcheck/shfmt/mdformat/nixfmt/
taplo/jq/yq/bats), `just` recipes, bats + hand-rolled `test/*.sh`, gitleaks pre-commit, `gh-axi` (via
`npx -y gh-axi`) for all GitHub operations.

## Global Constraints

Every task's requirements implicitly include these. Values copied verbatim from the spec and the repo's
`CLAUDE.md`.

- **Minimum chezmoi version:** `.chezmoiversion` requires >= 2.62.3.
- **Commits:** Conventional Commits (`conventional-commits` skill). **Never** add `Co-Authored-By: Claude`,
  any Claude/Anthropic co-author trailer, or a "🤖 Generated with Claude Code" footer. Separate logically
  distinct changes into separate commits.
- **Green before every commit:** the per-repo `.githooks/pre-commit` runs `just lint-check` (check-only)
  then `just test` (all `test/*.sh` + the bats suites inside `nix develop .#run`) then `gitleaks git
  --staged`. All three must pass. Do not bypass with `--no-verify` without per-invocation user
  confirmation.
- **GitHub operations:** use `npx -y gh-axi <command>` — never raw `gh`. `gh` stays installed only as
  gh-axi's dependency. Multi-line PR bodies: write to a UTF-8 file, pass `--body-file <path>`.
- **Chezmoi applies:** never run bare `chezmoi apply` from automation (KeePassXC-gated templates need a
  TTY). Use `chezmoi apply --exclude=templates --force`, or apply specific non-template files by name.
  Pause and ask the operator to apply KeePassXC-gated files interactively.
- **Design + testing standard (binding, per the spec's essential-feed section and decisions log #6):**
  **TDD drives the design** — for every piece of new logic, write the failing test first, show the red
  run, implement minimally, show green; no implementation-first work passes review. **SOLID** at the
  language's altitude: single-responsibility units behind clear seams, wired at one composition point.
  **Classist (Detroit-school) testing:** real collaborators in domain tests; test doubles only at true
  I/O boundaries (network, subprocess, filesystem, clock) — the `test/osquery-alerter/lib.bash` harness
  is the in-repo exemplar and the template for all bats work. Transplanted (already-tested) code carries
  its tests in the same PR and runs green; any behavior change to it starts with a failing test.
- **Every PR is self-contained and fully wired** — no dead code, no half-feature waiting on a later PR,
  no file that nothing references by the time the PR merges. A migration that removes an old tool must
  add its replacement in the same PR (main is never left half-migrated).
- **osquery guardrail:** the alerting/dispatch *design* may be improved (slice 9). Query/pack *content*
  changes (`.chezmoitemplates/osquery/*.conf`) are proposed-and-flagged for user sign-off, never made
  unilaterally.
- **Code style:** shell `shfmt -i 2 -ci -s`; markdown wrapped at 105 columns (`mdformat`); TOML `taplo`
  (`dot_aerospace.toml` excluded); Nix `nixfmt`. `trash` > `rm` for destructive removals.

---

## Phase A — Integration reference branch (do once, first)

### Task A1: Create and publish the DO-NOT-MERGE integration branch

**Files:** none in-repo (git + GitHub state only).

**Interfaces:**
- Consumes: current working-branch head `e8094b5` on `feat/cli-agent-tracking-workflow` (the union of
  PR #31 + PR #25 + SP1, verified: `git diff main...HEAD` = 196 files).
- Produces: branch `integration/modernization` pushed to `origin`; a draft PR whose number the cutover
  task (D1) closes with pointers to the landed slices.

- [ ] **Step 1: Branch at the current union head**

```bash
cd ~/workspaces/Ivy/webdavis/dotfiles
git checkout -b integration/modernization   # at the CURRENT head — do not pin a hash; it must include every hotfix to date
```

Expected: `Switched to a new branch 'integration/modernization'`. (No merge step — the union already
exists here; `f7220d9` merged PR #25's branch in earlier.)

**dresden lives here from this moment:** the repo checkout stays on `integration/modernization` (it IS
the live-state branch — hotfixes under the freeze policy land here). `feat/cli-agent-tracking-workflow`
and PR #31 freeze at their final state and are closed at cutover alongside #25 and the integration PR.

- [ ] **Step 2: Push the branch**

```bash
git push -u origin integration/modernization
```

Expected: branch created on `origin` (remote is `git@github.com:webdavis/dotfiles.git`).

- [ ] **Step 3: Write the PR body to a file**

Create `/tmp/sp2-integration-pr-body.md`:

```markdown
**DO NOT MERGE.** This PR is the frozen live-state reference for the repo-modernization effort — the
exact combined diff (PR #31 + PR #25 + the SP1 age-encryption work) that dresden runs today.

It is **never** merged. Its role is a reviewable snapshot while the same work is reimplemented from
`main` as small, self-contained PRs (see `docs/superpowers/plans/2026-07-03-sp2-combine-and-split.md`).
Each small PR fully wires one feature and is reviewed on its own; this branch is closed at cutover with
pointers to the landed slices.

Freeze policy: no new feature work lands here; only hotfixes needed to keep dresden healthy, and every
such hotfix is also folded into its corresponding reimplementation slice.
```

- [ ] **Step 4: Open the draft PR via gh-axi**

```bash
npx -y gh-axi pr create --draft \
  --base main --head integration/modernization \
  --title "DO NOT MERGE — integration reference (modernization)" \
  --body-file /tmp/sp2-integration-pr-body.md
```

Expected: a draft PR URL. Record its number as `$INTEGRATION_PR`.

- [ ] **Step 5: Label it so it can never be merged by reflex**

```bash
npx -y gh-axi label create do-not-merge --color B60205 --description "Reference only; never merge" 2>/dev/null || true
npx -y gh-axi pr edit <INTEGRATION_PR_NUMBER> --add-label do-not-merge
```

Expected: label applied. No commit — this task is pure git/GitHub state.

---

## Phase B — The slice map (the file-level assignment, resolved against the real diff)

Every path in `git diff --name-status main...HEAD` (196 files) is assigned to exactly one slice below.
Eight **shared infra files** are touched by several slices; each slice carries **only its own hunks** of
those files (procedure in the Slice Protocol). No file is orphaned.

**Shared infra files** (never a slice of their own — hunks distributed to the owning slice):
`.chezmoi.toml.tmpl`, `.chezmoiignore`, `.gitignore`, `CLAUDE.md`, `private_dot_claude/CLAUDE.md`,
`dot_bashrc.tmpl`, `dot_profile`, `justfile`.

| Slice | Feature | File groups (from the bucketed delta) | Ledger fixes folded in | Dep |
| --- | --- | --- | --- | --- |
| S1 | Docs | `docs/**` (20: the herdr/tailscale/brew/relay/notifications specs+plans, the modernization brief, this plan, the never-sleep policy), `AGENTS.md` (new symlink→CLAUDE.md) | — | none |
| S2 | Lint/test/CI hardening | `scripts/lint.sh`, `.githooks/pre-commit`, `.github/workflows/lint.yml`, `.editorconfig`/`.shellcheckrc`/`.mdformat.toml` hunks | CI runs tests + `LINT_CHECK=1`; wire **actionlint** + **zizmor** (P9); **SHA-pin** actions (installer has no tags — see research §Actions); `lint.sh` runner-selection subshell bug; `-r` optstring crash; template shellcheck allowlist → programmatic; `find` prune set dedup; bats `grep -c` zero-count false-pass | S1 |
| S3 | Skills-store consolidation | `dot_local/bin/executable_update-skills.sh`, `dot_agents/skills/**`, `private_dot_claude/skills/symlink_*`, `skills-lock.json`, delete `private_dot_claude/skills/web-research-task/**` | update-skills **loader script + `~/.local/log/skills` dir**; declare all store symlinks; remove stale `.agents/skills/moshi-best-practices/`; single symlink-owner | S2 |
| S4 | herdr migration | `dot_config/herdr/**`, `dot_local/share/herdr/plugins/**` (2 Rust plugins), the `run_onchange_after_55/57` build scripts, herdr hunks of `dot_bashrc.tmpl`; **atomically deletes** `dot_tmux.conf`, `dot_config/sesh/**`, `dot_local/bin/executable_{sesh-*,tmux-*,claude-restart}.sh`, `run_after_70-install-tmux2k-last-proc` | herdr plugin build scripts → `.chezmoitemplates` partial; `grep -q "$plugin_id"` anchoring; `dot_fzf_bindings` tmux-dead widgets; `nvm`/`$blue` binding fixes | S2 |
| S5 | Tailscale headless daemon | `run_onchange_after_66-tailscaled-status.sh.tmpl`, tailscale hunks of `system_packages_autoinstall.yaml` + `CLAUDE.md` | tailscale-monitor already fixed (`2f430b3`, on main? verify) | S2 |
| S6 | Homebrew weekly-upgrade | `dot_local/bin/executable_homebrew-weekly-upgrade.sh`, `Library/LaunchAgents/com.webdavis.homebrew-weekly-upgrade.plist.tmpl`, `run_onchange_after_65` loader, `test/homebrew-weekly-upgrade.sh` | `just brew-upgrade` → deployed copy; **Homebrew 6.x bundle `cleanup --force`** (`961465f`); `SKIP_SYSTEM_PACKAGES=0`-still-skips; before_10 per-ecosystem split; uv/npm/volta unguarded loops | S2 |
| S7 | Relay notification pipeline (bash, as-deployed) | `dot_local/bin/executable_{relay,relay-agent,relay-codex-hooks,hue-pulse,claude-stop-pulse,claude-user-prompt-start,claude-audit}.sh`, `private_dot_claude/modify_settings.json`, `dot_config/relay/private_auth.json.tmpl`, `run_after_72-relay-codex-hooks`, notifier hunk of `dot_bashrc.tmpl`; delete `Library/LaunchAgents/com.claude.code.plist.tmpl` | ships **as deployed** — SP3 replaces it later. Do NOT pre-apply SP3 bug fixes here (they belong to the Rust rewrite); ship the bash as-is so main matches dresden | S2 |
| S8 | Hermes age-encryption (SP1) | `dot_hermes/encrypted_private_config.yaml.age`, `dot_hermes/private_dot_env.tmpl`, `.chezmoi.toml.tmpl` age hunk, `run_onchange_before_25`, `run_after_67`, `run_after_68`, `run_once_before_05-restore-age-key`, `test/hermes-config-{encrypted,routes}.sh`, `.gitignore` failsafe hunk (gitleaks gate hunk of `.githooks/pre-commit` ships in S2, not here) | this is the committed SP1 work (`c13cc18`/`a0e7d8e`/`3696c92`) reimplemented as one clean PR; the age-tripwire fix is already in it | S2 |
| S9 | osquery three-tier alerting | `.chezmoitemplates/osquery/**` (config+4 packs), `dot_local/bin/executable_osquery-*`, the 6 osquery LaunchAgents + `after_60` loaders, `after_55` manifest, `before_50` setup, `test/osquery-alerter/**` | **alerting/dispatch redesign in scope**; heartbeat `RunAtLoad` double-ping; **query/pack content changes → flag for sign-off**. NOTE: much of osquery is already on main — this slice is the PR#25 *delta* only | S2 |
| S10 | macOS defaults / system-setup | `.chezmoidata/macos_defaults.yaml`, `.chezmoidata/macos_system_setup.yaml`, `run_onchange_after_30/41`, `dot_local/bin/executable_macos-defaults-*.sh` | defaults trio hardcoded-path + shared-lib consolidation; `after_41` fragile `{{ if .sudo }}`; `ssh-hardening.sh` → a `macos_system_setup.yaml` record | S2 |
| S11 | Shell foundation + secrets hygiene + chores | remaining hunks of `dot_bashrc.tmpl`/`dot_profile`/`justfile`/`.chezmoiignore`, `run_after_44-cache-brew-shellenv` + `test/brew-shellenv-cache-drift.sh`, `dot_aws/private_credentials.tmpl` + `dot_config/himalaya/private_config.toml.tmpl` renames, `dot_config/worktrunk/config.toml`, gitconfig fixes; **installs:** Thaw (SP5), ponytail | credential `private_` renames (`ae02524`); merge.tool name; `core.excludesfile`; git:// url removal; `~/.bash_just_completions`; atuin `~/.atuin/bin/env` guard; yabai ignore; espanso `_pqi.yml` + shadow triggers; Arc→Zen hotkey; log rotation (newsyslog) | S2 |
| S12 | CLAUDE.md comprehensive refactor | `CLAUDE.md`, `private_dot_claude/CLAUDE.md`, global AGENTS.md parity | the memory-file rewrite per the spec's CLAUDE.md section — **post-cutover-adjacent; runs last so it documents the reimplemented reality** | S1–S11 |

**Sequencing rationale:** S1 (docs) and S2 (the checkable foundation — CI must actually run tests before
the rest can be trusted) go first. S3–S11 are feature slices, orderable by dependency (skills before
herdr because herdr's plugins live in the store; relay before nothing; osquery last of the big three
because its diff is smallest relative to main). S12 rewrites the memory files last, against final
reality. Ship in table order unless the operator re-prioritizes.

---

## Phase C — Slice execution protocol

Every slice task S1–S12 executes the **identical protocol** below. Each slice's task section states only
its own specifics (files, wiring to verify, gotchas, review focus); the mechanical cycle is here, once.

**The protocol (run for each slice):**

- [ ] **P-1: Branch from the current main.**
  ```bash
  git checkout main && git pull origin main
  git checkout -b slice/<name>
  ```
  Later slices branch from the main that earlier slices advanced — this is why shared-file hunks layer
  instead of conflicting.

- [ ] **P-2: Assemble the slice's files from the integration branch.** For files owned wholly by this
  slice, take them verbatim: `git checkout integration/modernization -- <path> …`. For a **shared infra
  file**, do NOT take the whole file — apply only this slice's hunks: `git diff main integration/modernization -- <shared-file>`
  to see the full delta, then hand-apply (or `git checkout -p integration/modernization -- <shared-file>`)
  only the hunks this slice owns per the slice map. For **deletions**, `git rm <path>` in the same slice
  that makes the deletion safe (e.g. tmux files die in S4 as herdr lands).

- [ ] **P-3: Fold in the ledger fixes** named in this slice's row — these are *improvements over* the
  integration branch's version (that is the point of reimplementing). Each fix is test-driven where it
  has runtime surface (see the slice's specifics).

- [ ] **P-4: Verify full wiring.** No dead code. If the slice adds a `run_*` script, confirm what triggers
  it and that its target exists. If it adds a LaunchAgent plist, confirm the matching loader script is in
  the same slice. If it adds a deployed binary, confirm something references it (a hook, a keybinding, a
  recipe). Grep the slice's own new symbols to prove each has a consumer.

- [ ] **P-5: Lint + test green.**
  ```bash
  just lint-check && just test
  ```
  Expected: both exit 0. Fix drift before committing (the pre-commit hook re-runs both).

- [ ] **P-6: Commit** in logically-separate Conventional Commits (feature vs its ledger-fix vs docs).
  No AI trailers.

- [ ] **P-7: Open the PR via gh-axi**, body written to a file naming (a) what the slice ships, (b) which
  ledger fixes it folds in, (c) whether it needs an operator `chezmoi apply` and of which files, (d) for
  S9 only, any query/pack content change flagged for sign-off.
  ```bash
  npx -y gh-axi pr create --base main --head slice/<name> \
    --title "<conventional title>" --body-file /tmp/slice-<name>-body.md
  ```

- [ ] **P-8: Operator review gate.** Stop. The operator reviews and merges — **house convention: a merge
  commit via `--merge`, never squash**, subject exactly `Merge pull request #N from webdavis/<branch>
  (#N)` (if asked to merge on their behalf: `npx -y gh-axi pr merge <n> --merge`). The
  `private_dot_claude/commands/pr-merge.md` command still says squash — it is stale against this
  convention; fixing it is an S11 chore. If the slice needs a KeePassXC-gated apply, the operator does
  it here. Do not start the next slice until this PR is merged — the next slice branches from the main
  it created.

**Per-slice specifics** (the parts that differ — read alongside the protocol):

### S1 — Docs
- **Files:** all of `docs/**` in the delta + `AGENTS.md`. `AGENTS.md` is a *symlink* to `CLAUDE.md`
  (`git checkout integration/modernization -- AGENTS.md` preserves the symlink; verify with `ls -l`).
- **Wiring (P-4):** docs are `.chezmoiignore`d — confirm none reach `$HOME` (`chezmoi managed | grep
  docs/` returns nothing).
- **Review focus:** pure additions; fastest slice; establishes the plan/spec paths later PRs reference.

### S2 — Lint/test/CI hardening
- **Measured baseline (do not assume):** `main` has **no `test/` directory and no `test:` recipe in its
  justfile**. S2 therefore *introduces* the test harness to main: the `test:` recipe (the `test/*.sh`
  loop guarded to pass on an empty/missing dir — e.g. `find test -name '*.sh'`-driven — plus the
  bats-inside-`nix develop .#run` line) and the gitleaks staged-scan line in `.githooks/pre-commit`.
  The recipe must be green on a still-empty tree; real test files arrive with their feature slices.
- **Ledger fixes are the substance here.** Test-drive each with the bats/`test-*.sh` harness:
  - CI: edit `.github/workflows/lint.yml` to run `just test` and set `LINT_CHECK=1` on the lint step;
    **SHA-pin** `NixOS/nix-installer-action` and `actions/checkout` to full commit SHAs — the
    installer action publishes **no tags and no releases** (verified via GitHub API 2026-07-04), so
    `@vX`/`@<tag>` is impossible and `@main` is the mutable-supply-chain risk; resolve the current SHA
    at implementation time and add a `# vX.Y.Z` trailing comment. Also add workflow-level
    `permissions: contents: read` (least privilege) and `with: persist-credentials: false` on checkout.
    See the research section (GitHub Actions supply-chain) for the full set + `dependabot.yml` + zizmor.
  - `lint.sh` runner-selection subshell bug: the `parse_cli_options` nameref result is discarded in a
    `$(...)` subshell → every `just <tool>` runs the full suite in write mode. Fix: populate `runners` in
    the caller's shell. Add a `test/lint-runner-selection.sh` asserting `just j` runs only jq.
  - bats `grep -c` zero-count false-pass (`lib.bash` `assert_post_count`/`assert_digest_*`/
    `assert_allowlist_label_count`): capture count into a var, guard non-numeric. Add a bats test that a
    zero-when-expecting-N assertion FAILS.
  - actionlint: add to the flake `run` shell + a `lint.sh` runner + a justfile alias.
- **Review focus:** does CI now actually fail on a broken test / format drift? (Push a deliberately
  broken commit to a throwaway branch to confirm red, then drop it.)

### S3 — Skills-store consolidation
- **Files + deletions** per the map. The restored skills (conventional-commits, humanizer,
  video-transcript-downloader, hyperframes set, last30days, tiktok-crawling, kubernetes-specialist) are
  already in the store on the integration branch — carry them.
- **Ledger fixes:** add `run_onchange_after_*-load-update-skills-launchagent.sh.tmpl` (mirror the atuin
  loader; `mkdir -p ~/.local/log/skills` before bootstrap); declare every store symlink in
  `private_dot_claude/skills/symlink_*`; `trash` the stale `.agents/skills/moshi-best-practices/`.
- **Wiring (P-4):** every `~/.claude/skills/*` symlink target exists in `~/.agents/skills`; the loader
  actually bootstraps the plist.

### S4 — herdr migration (may split into S4a config / S4b plugins / S4c bashrc+tmux-removal)
- **Atomicity is the invariant:** the tmux/sesh deletions and the herdr additions ship together — main
  must never have both, nor neither. If split, S4a/b add herdr and S4c flips bashrc + deletes tmux in
  one PR.
- **Ledger fixes:** consolidate the two plugin build scripts to a `.chezmoitemplates` partial; anchor the
  `grep -q "$plugin_id"` link check; remove `dot_fzf_bindings` tmux-`$TMUX` widgets; fix the `dot_bash_bindings`
  duplicate `\C-gss`, `nvm`, `$blue`.
- **Wiring (P-4):** the two Rust plugins build (`cargo build --release --locked`) and link; the
  auto-attach block guards on `HERDR_ENV`. **Operator apply** needed (builds Rust at apply time).

### S5 — Tailscale headless daemon
- First confirm whether the tailscale-monitor fix (`2f430b3`) is already on main; if so this slice is
  only the daemon-install status reminder + manifest/CLAUDE.md hunks.
- **Operator step:** the one-time `sudo tailscale up` + Disable Key Expiry stay manual (documented in
  the status script) — not automatable, flag in the PR body.

### S6 — Homebrew weekly-upgrade
- **Ledger fixes are load-bearing** (these bit tonight): the Homebrew 6.x `brew bundle` split (install,
  then `brew bundle cleanup --force` against a rendered temp Brewfile — `961465f`); `just brew-upgrade`
  → `~/.local/bin` copy; `SKIP_SYSTEM_PACKAGES` truthiness (`=0`/`=false` must NOT skip → `{{ if eq (env
  "SKIP_SYSTEM_PACKAGES") "1" }}`); guard the uv/npm/volta loops.
- **Wiring:** the Monday-noon plist loads; `RunAtLoad=false`.

### S7 — Relay notification pipeline (bash, as-deployed)
- **Ship the bash exactly as it runs on dresden.** Do NOT fix the SP3-tagged relay bugs here (fail-closed
  idle probe, jq slurp, mkdir lock, regex anchor) — those are the Rust rewrite's job; main must match
  dresden's current live behavior so SP3 replaces a known baseline. Note this explicitly in the PR body.
- Delete the old `com.claude.code.plist.tmpl` (superseded by the modify_settings hooks).
- **Operator apply** needed (`private_auth.json.tmpl` is KeePassXC-gated).

### S8 — Hermes age-encryption
- This is the SP1 work (already committed on the working branch as `c13cc18`/`a0e7d8e`/`3696c92`) shipped
  as one clean PR. The age-tripwire fix and the fresh-machine restore script are part of it.
- **Operator step:** the `age` recipient in `.chezmoi.toml.tmpl` is the operator's public key (already
  set); the private key restore rides KeePassXC. Round-trip verify in the PR (`chezmoi cat` == live).

### S9 — osquery three-tier alerting
- **Smallest big slice** — most of osquery is already on main; carry only the PR#25 delta.
- **In scope:** alerting/dispatch design improvements. **Sign-off gate:** any `.chezmoitemplates/osquery/
  *.conf` query/pack content change is listed in the PR body for explicit user approval before merge.
- **Wiring:** all 6 LaunchAgents + loaders; the 87-bat suite green; the pipeline manifest baseline.

### S10 — macOS defaults / system-setup
- **Ledger fixes:** defaults trio shared-lib + `chezmoi source-path` (kills the worktree-writes-primary
  bug); `after_41` `{{ if index . "sudo" }}`; add `ssh-hardening.sh` as a `macos_system_setup.yaml`
  record so a fresh machine actually locks sshd.
- **Operator apply** needed (Tier-2 sudo runner prompts once).

### S11 — Shell foundation + secrets hygiene + chores
- **Files:** the remaining shared-infra hunks + the brew-shellenv cache + the credential `private_`
  renames (`ae02524`) + worktrunk + gitconfig.
- **Ledger fixes:** merge.tool name-not-command; `core.excludesfile` (ship a `dot_gitignore_global` or
  drop the line); remove git:// url rewrites, `~/.bash_just_completions` source, atuin `~/.atuin/bin/env`
  guard, the linux yabai ignore; espanso `_pqi.yml` import + shadow-trigger renames; Arc→Zen hotkey;
  newsyslog log rotation.
- **Installs (SP5 + directive):** Thaw (`brew install --cask thaw` → add to manifest); ponytail (`/plugin
  marketplace add DietrichGebert/ponytail` + `/plugin install`, `hermes plugins install
  DietrichGebert/ponytail --enable`, promote to `enabledPlugins`).
- **Operator apply** needed (credential renames re-deploy at 0600; already done live tonight, but main
  must carry the renamed sources).

### S12 — CLAUDE.md comprehensive refactor
- Runs last. Per the spec's CLAUDE.md section: global file → minimal (preferences + bias-correction +
  toolchain + gates only, no operational detail, no dead skill references); repo file → identity +
  commands + architecture map + conventions, conditional deep-dives extracted to `docs/runbooks/` or
  skills; **every factual claim re-verified against the live repo at write time**; global AGENTS.md
  parity added. Fold in the verified staleness fixes (haiku→sonnet hook, pre-bats Testing section, wrong
  source-dir description, tmux/yabai remnants, single-template shellcheck claim).

---

## Phase D — Cutover

### Task D1: Switch main live, verify, close the reference PRs

- [ ] **Step 1:** After S12 merges, point dresden's chezmoi source at `main`
  (`git -C ~/workspaces/Ivy/webdavis/dotfiles checkout main && git pull`).
- [ ] **Step 2: Operator** runs a full interactive `chezmoi apply` (KeePassXC unlocked).
- [ ] **Step 3:** `just test` green + live smoke checks: `relay.sh` fires a test notification; `hermes
  gateway` healthy; osquery alerter behaves (`osquery-heartbeat.sh` sends its one ✅);
  `chezmoi diff --exclude=templates` clean.
- [ ] **Step 4:** Close PR #31, PR #25, and the `integration/modernization` reference PR via `gh-axi pr
  close`, each with a comment linking the slice PRs that superseded it.

---

## Self-Review

**Spec coverage:** every SP2-tagged item in the work ledger maps to a slice's "ledger fixes" column (S2
CI/lint items; S3 skills; S4 herdr consolidation; S6 brew; S9 osquery; S10 defaults/ssh; S11 the long
tail; S12 CLAUDE.md). The spec's provisional slice map (its items 1–11) maps to S1–S11; SP1 = S8; SP5
Thaw = S11; the CLAUDE.md refactor = S12. Combine mechanics (integration branch, DO-NOT-MERGE PR, freeze
policy) = Phase A. Cutover checklist = Phase D. No spec section is unassigned.

**Placeholder scan:** the one deferred detail — exact hunk boundaries inside the 8 shared files — is
resolved by a *procedure* (P-2: `git diff main integration -- <file>` then apply only the slice's hunks),
not a "TODO"; the slice map names which slice owns which shared file's concern. Rust-fix and
content-change specifics are named per slice, not hand-waved.

**Type/name consistency:** branch names (`integration/modernization`, `slice/<name>`), the gh-axi
invocation (`npx -y gh-axi`), and the green gate (`just lint-check && just test`) are identical across
every task. `$INTEGRATION_PR` is set in A1 and consumed in D1.

**Re-evaluated 2026-07-04 (max effort), five defects fixed:** stale commit pin in A1 (now "current
head"); dresden's live branch made explicit (switches to `integration/modernization`; PR #31 freezes);
merge instruction corrected from squash to the recorded `--merge` + exact-subject convention (and the
stale `pr-merge.md` flagged as an S11 chore); measured that `main` has **no** `test/` dir or `test:`
recipe, so S2 now explicitly introduces the harness green-on-empty; the gitleaks pre-commit hunk was
double-assigned to S2 and S8 — resolved to S2.

---

## Research-backed refinements (deep-research, 2026-07-04)

A deep-research pass investigated the problem domains behind the plan's features — web-search fan-out →
fetch authoritative sources → adversarial verification of each recommendation against both its sources
and dresden's real constraints (solo operator, chezmoi, macOS→home-server, low-ops). Eight domains were
queried; **seven returned** (five verdict SOUND, two core-sound-with-trims); **one was blocked** (see
Gaps). **Nothing below is implemented — these are plan/spec edits and banked decisions for the execution
step.** Each domain names the slice(s) it amends and its key sources.

### R1 — Split-PR mechanic: hand-rolled HOLDS; add three process guards (amends Phase B / C / D)

The stacked-diff tools (Graphite, ghstack, git-spr/spr, Sapling, jujutsu) do **not** improve this plan
for a solo sequential-merger, and several actively conflict with house rules. Confirmed reasons: (1) the
plan *reimplements from main with new work* (spec decision 5), while split tools mechanically partition
*existing* commits — they can't fold in the ledger fixes; (2) the tools' value is hiding **reviewer
latency** to unblock a *team*, which a solo self-reviewer does not have; (3) Graphite's `--by-hunk` is
just `git add -p` under a SaaS layer, ghstack/spr/Sapling replace GitHub-native merge (breaking the
house `--merge`-with-exact-subject convention) and route metadata off-box (breaking the public-repo +
`gh-axi`-only rules). **jujutsu** is the one tool that genuinely improves hunk-splitting (scriptable
`jj-hunk`, auto-rebase) but adopting a parallel VCS for a one-time decomposition is YAGNI — banked as a
future standalone go/no-go, like nushell, never bolted onto SP2.

**Changes to apply:**

- **Phase B:** replace the "shared infra files" prose with a **per-file hunk-ownership table** — columns
  `shared file × owning slice(s) × one-line what` — so every hunk of the 8 shared files is pre-assigned
  to exactly one slice. The real risk is an **orphaned** hunk (lands in no slice) or a **double-counted**
  hunk (two slices grab it → the second conflicts); a table makes the "no file orphaned" claim
  verifiable, not asserted. Build the table by walking `git diff main integration/modernization -- <file>`
  for each of the 8 during Phase B.
- **Phase C P-2:** keep `git checkout -p` as option (a) interactive, and add option (b) as the **agent
  default** — `git diff main integration/modernization -- <file>` → trim the patch to this slice's hunks
  → `git apply --index`. Deterministic, non-interactive (no y/n/s/e responder for a subagent), and the
  trimmed patch is a reviewable artifact. Stays git-native (honors "boring, system-shipped tools").
- **Phase C P-4:** add a per-shared-file assertion — after applying a slice's hunks,
  `git diff <slice-branch> integration/modernization -- <file>` should show **only the other slices'**
  hunks (proves this slice took its own and no more).
- **Phase D D1:** add an **empty-diff reconciliation gate** — before closing the reference PR, assert
  `git diff main integration/modernization -- <file>` is **empty** for each of the 8 shared files,
  proving every hunk landed exactly once across the slice sequence.
- **Phase C preamble:** add a short "Tooling considered and rejected" note (Graphite/ghstack/spr/
  Sapling/jj + why), so it is not re-litigated mid-execution.

**New features:** hunk-ownership table + empty-diff gate `[plan]`; non-interactive `git apply` P-2
variant `[plan]`; optional local-stack authoring via `git rebase --update-refs` (git ≥ 2.38, already in
the toolchain — build several review-independent slices as a local stack, re-sync branch pointers with
one rebase after each merge) `[plan]`.
Sources: graphite.com/guides/how-to-split-an-existing-pull-request, graphite.com/blog/stacked-prs,
graphite.com/docs/privacy-and-security. *(verdict: core SOUND; the "no tool helps" claim trimmed to "no
tool helps a solo sequential-merger" — jj does help sustained stacking.)*

### R2 — Formatter orchestration: PROMOTE treefmt-nix to S2's primary approach (amends S2)

treefmt-nix is the right replacement for the ~510-line hand-rolled `lint.sh`, and S2 should promote it
from a creative-liberty *alternative* to its **primary** design. Verified against the treefmt-nix README:
built-in `shellcheck` and `actionlint` modules exist, `config.build.check` gives fail-on-drift for CI,
and one global `excludes` replaces the prune-set duplicated 6× in `lint.sh` (resolving that ledger
consolidation outright). Boundary: treefmt replaces **`just lint-check`** (the lint/format half of the
gate) only — **`just test`** (the hand-rolled `.sh` + bats loop) stays as-is.

**Changes to apply (all S2):** promote treefmt-nix to S2's primary; **keep** the S2 fixes that live
*outside* `lint.sh` (bats `grep -c` zero-count false-pass, CI-runs-`just test`, SHA-pin); change CI
acceptance from "does `lint.sh -c` fail on drift" to "does **`nix flake check --all-systems`** fail on
format drift" (treefmt's check derivation); port `lint.sh`'s 4 chezmoi-specific checks
(shellcheck-on-rendered-templates via `CI=1 chezmoi execute-template`, the osquery-config render+jq,
etc.) as **custom treefmt formatters**; enable **actionlint via its treefmt-nix module** (satisfies old
P9 without hand-wiring); rewire the seams (`.githooks/pre-commit` + justfile aliases → treefmt; CI drops
the separate `lint.sh -c` step); record a decision point on whether jq/yq parse-validation stays inside
treefmt (unified, off-label) or splits out.
**New features:** `treefmt.nix` module + flake wiring (`treefmt-nix.lib.evalModule`) `[repo]`; the 4
chezmoi checks as custom formatters `[repo]`; actionlint-via-module `[plan]`.
Sources: github.com/numtide/treefmt-nix (README), treefmt.com/latest/getting-started/configure.
*(verdict: SOUND — README spot-fetched; ~450-line deletion confirmed.)*

### R3 — GitHub Actions supply-chain: the plan's pin instruction was unachievable; SHA-pin instead (amends S2)

Direction right, specifics wrong. **`NixOS/nix-installer-action` has zero tags and zero releases**
(verified via the GitHub API), so the plan's "pin to `<current-release-tag>`" is impossible. Correct
fix, per GitHub's own 2025 SHA-pinning guidance and the `tj-actions` supply-chain compromise
(CVE-2025-30066): **pin both actions to full commit SHAs** with a trailing `# vX.Y.Z` comment (already
applied inline to S2 above). Additional S2 deliverables:

- Workflow-level `permissions: contents: read` (least privilege — the job only reads code).
- `with: persist-credentials: false` on the checkout step.
- Wire **zizmor** (GitHub Actions static analysis, in nixpkgs) beside actionlint — flake run-shell + a
  `scripts/lint.sh`/treefmt runner + a justfile alias.
- Add **`.github/dependabot.yml`** (`package-ecosystem: github-actions`, weekly, review-gated, no
  auto-merge) so SHA pins get updated through the normal review loop.
- Record as rejected (so it is not re-litigated): `step-security/harden-runner` — macOS-hosted runners
  are unsupported.

Sources: docs.github.com/actions/reference/security/secure-use, github.blog/changelog/2025-08-15 (SHA
pinning), cisa.gov (tj-actions CVE-2025-30066), github.com/zizmorcore/zizmor.
*(verdict: SOUND — the empty-tags correction independently re-verified.)*

### R4 — Secrets-at-rest: age HOLDS; three real gaps to close (amends S8 + spec)

chezmoi-native age (one X25519 key, distributed via KeePassXC) is the current ecosystem best practice
for a solo macOS chezmoi setup — sops-nix/agenix/git-crypt were each evaluated and rejected for concrete
reasons (added to the spec Decisions log, below). But three **verified** gaps:

- **Real bug (fixed):** the spec named the KeePassXC entry `chezmoi :: age identity` while the live
  restore script reads `chezmoi :: Private Key :: age` — a fresh-machine restore would query the wrong
  entry and fail. Spec corrected 2026-07-04.
- **Darwin-gated restore:** `run_once_before_05-restore-age-key.sh.tmpl` opens with
  `{{ if eq .chezmoi.os "darwin" }}`, so the **future Linux home-server cannot restore the key**. S8
  drops/generalizes the guard.
- **No rotation / DR story:** no documented procedure exists for rotating the age key or recovering it.
  S8 adds `docs/runbooks/age-key.md` with the two workflows spelled out below (written now, per the
  "behaviors first" rule — this is the plan text S8 will implement, not the implementation).

**Runbook 1 — rotate the age key** (do this if the private key may have leaked, or on a schedule):

1. Generate a new identity beside the old: `chezmoi age-keygen --output=$HOME/.config/chezmoi/key.new`.
1. Update `.chezmoi.toml.tmpl`: set `[age] recipient` to the **new** public key (keep the old identity in
   an `identities` list temporarily so existing ciphertext still decrypts during the transition), then
   `chezmoi init`.
1. Re-encrypt every managed secret to the new recipient: for each `encrypted_*` source file,
   `chezmoi forget <target>` then `chezmoi add --encrypt <target>` (re-encrypts under the new recipient).
   Today that is just `~/.hermes/config.yaml`.
1. Verify round-trip: `diff <(chezmoi cat ~/.hermes/config.yaml) ~/.hermes/config.yaml` is empty;
   `head -1` of each `encrypted_*` file is an age header, not plaintext.
1. Drop the old identity from `identities`, `mv key.new key.txt`, and **update the KeePassXC entry**
   `chezmoi :: Private Key :: age` Password field to the new `AGE-SECRET-KEY` line.
1. `just l && just test` (the hermes guards enforce), commit.
1. **git-history caveat:** rotation does **not** scrub old ciphertext from git history — anyone who had
   the old key can still decrypt old commits. So if the key actually leaked, the real recovery action is
   to **rotate the upstream secrets themselves** (the Hermes webhook secret, Discord token, etc.), not
   just the age key. List those in the runbook.

**Runbook 2 — disaster recovery** (dead/lost machine, rebuilding from scratch):

1. On the new machine, retrieve the KeePassXC database (per the fresh-machine quickstart) and unlock it.
1. `chezmoi init` — `run_once_before_05-restore-age-key` writes `~/.config/chezmoi/key.txt` from the
   `chezmoi :: Private Key :: age` entry automatically (this is why the key lives in KeePassXC, not only
   on the dead disk).
1. `chezmoi apply` — every `encrypted_*` file decrypts with the restored key.
1. **DR drill (the test that proves the above):** a `test/age-restore.sh` that, in a scratch HOME,
   simulates a bare machine — no `key.txt` present — stubs the KeePassXC lookup, runs the restore script,
   and asserts a known ciphertext fixture decrypts. Catches a broken restore path *before* a real
   disaster, not during one.

**New features:** the `age-key.md` runbook (both workflows above) + the `test/age-restore.sh` DR drill
`[repo]`; generalize the darwin guard so the restore runs on Linux too `[repo]`; **multi-recipient
migration design** (each machine keeps its own identity; `.chezmoi.toml.tmpl` lists both public keys as
`recipients` so files encrypt to both — the laptop→home-server path) `[dresden]`.
Sources: chezmoi.io/user-guide/encryption/age, chezmoi.io/user-guide/frequently-asked-questions/
encryption, discourse.nixos.org (git-crypt/agenix/sops-nix comparison).
*(verdict: SOUND — spot-verified against the live repo and the chezmoi age doc.)*

### R5 — Agent skills/memory: architecture correct; reproducibility + supply-chain gaps (amends S3 + S12)

The `~/.agents` store + symlink fan-out + `AGENTS.md`→`CLAUDE.md` model is correct and, in places, ahead
of the ecosystem (AGENTS.md convention, Anthropic's Agent Skills). But **verified on disk**: **21 live
store skills vs 12 committed vs 9 Claude `symlink_` declarations vs 0 Hermes declarations** — a fresh
`chezmoi apply` reproduces only ~9 of 21 skills into Claude and none into Hermes.

**Keep/deprecate decision (operator, 2026-07-04): keep ALL 21 — deprecate none** (the operator uses each
at different times; the earlier "overlap" flags were retracted as unfounded — `last30days` [trend
research], `tiktok-crawling` [bulk scrape], and `video-transcript-downloader` [transcripts] do genuinely
different jobs, and the four `hyperframes*` skills are an interdependent suite whose descriptions
cross-delegate, so they are all-or-nothing). The task is therefore purely **reproducibility**, not
culling.

**The 9 uncommitted skills S3 must capture** (committed or install-manifested — verified 2026-07-04):
`chrome-devtools-axi`, `cua-driver`, `elevenlabs`, `gh-axi`, `home-assistant`, `kubernetes-specialist`,
`last30days`, `sql-toolkit`, `tiktok-crawling`. **Note `gh-axi` and `chrome-devtools-axi` are among
them** — the repo's own *preferred* GitHub and browser tools would silently not reproduce on a fresh
machine. Each has a known source (npx-skills / clawhub) captured during this session; S3 records those.

**Changes to apply:**

- **S3:** commit the full skill roster (or a committed `name→source` install-manifest) so a fresh
  machine reproduces every skill; make `update-skills.sh` **install-capable** (today its loops
  `[ -d "$STORE/$n" ] || continue` skip anything absent → refresh-only); complete the fan-out
  declarations (declare all store→Claude symlinks; add the missing `dot_hermes/skills/` declarations);
  resolve the **three-way** fan-out ownership (the ledger names two writers — the third is
  `npx skills … --global`); reconcile the lockfiles (`skills-lock.json` has a stale `moshi-best-practices`
  entry and 12 vs 20 live); add a **supply-chain gate** — pin each vendored git-clone to a commit SHA
  and/or verify `computedHash` before the atomic swap.
- **S12:** specify the global `AGENTS.md` parity **mechanism** (a `.chezmoitemplates` partial included by
  both `~/.claude/CLAUDE.md` and `~/.codex/AGENTS.md`), not just "add parity."

**New features:** committed full roster / install-manifest `[repo]`; single `.chezmoitemplates` partial
for the global ruleset `[dresden]`; SHA-pin + hash-verify vendored skills before swap `[repo]`.
Sources: agents.md, anthropic.com/engineering/…agent-skills, platform.claude.com/docs/…agent-skills/
best-practices, developers.openai.com/codex/skills.
*(verdict: SOUND — the 20/12/9/0 counts independently re-verified on disk 2026-07-04.)*

### R6 — nix-darwin vs chezmoi defaults: chezmoi HOLDS; bank the decision (amends S10 + spec)

S10's chezmoi `defaults write` approach is the correct call — do **not** rewrite macOS system config in
nix-darwin. Decisive, sourced reasons: the repo's entire secret model is chezmoi/KeePassXC (nix-darwin
would fork it); chezmoi is already the single apply path; and a one-tool-per-concern split lowers ops
for a solo maintainer. **Changes:** add a 2–3 sentence "Why chezmoi, not nix-darwin" note to S10; append
a spec Decisions-log entry recording the decision; add **SP-nix** (a deferred, research-first nix-darwin
go/no-go, sibling of SP4 nushell) to the roadmap sub-project table + deferred index; annotate the
existing "5 non-osquery plists → `.chezmoitemplates` partial" ledger item as the chosen chezmoi-native
DRY win.
**New features:** deferred **SP-nix** go/no-go sub-project `[dresden]`; explicit Decisions-log entry
`[plan]`.
Sources: github.com/nix-darwin/nix-darwin, carlosvaz.com/…declarative-macos-management-with-nix-darwin,
github.com/nix-darwin/nix-darwin/issues/1207.
*(verdict: SOUND — nix-darwin maintenance + capabilities spot-verified.)*

### R7 — Presence/notification: design HOLDS and leads prior art; reconcile native push (amends SP3 in spec)

The empirically-verified presence model and the stateless/no-daemon architecture stay unchanged — they
are ahead of the prevailing prior art. The one material gap is **timing/coexistence with Claude Code's
own native mobile push** (v2.1.110), which is itself presence-aware ("skips while focused on the
connected terminal"). Two systems pushing to the phone will double-fire. **Changes (SP3 contract in the
spec):** bank a decision to disable "Push when Claude decides" and keep the route through relay (single
owner); drive the Claude waiting-on-you vs ready-for-you classes from the **native Notification matchers**
(`agent_needs_input` / `agent_completed`); name **ntfy** as the reference self-hostable phone channel
(its http/view **action buttons** give free approve/deny/focus — the actionable-notification feature SP3
deferred); mirror **Apprise**'s tag-based routing + priority-tier escalation in the Rust composition
root; record the banner as a shell-out channel (`terminal-notifier -execute` / `alerter`, or `notify-rust`
for a Rust-native macOS click). Add "confirm native-push coexistence + which Notification matchers to
wire" to the SP3 final-spec open-items.
**New features:** reconcile native push (single owner) `[plan]`; ntfy pluggable channel with action
buttons `[dresden]`; Apprise-style tag routing + priority escalation in the SP3 composition root
`[plan]`.
Sources: code.claude.com/docs/remote-control, code.claude.com/docs/hooks-guide, docs.ntfy.sh/publish.
*(verdict: core SOUND; trimmed — the native-push coexistence is a real add; the rest confirms the
existing design.)*

### R8 — macOS endpoint hardening: keep osquery standalone; add two cheap layers, skip the enterprise ones (amends S9 / S10; operator sign-off)

The initial subagent tripped a cyber-topic classifier; the research was recovered two ways — decomposed
in-loop product lookups, then a re-run subagent that also ran **read-only checks on dresden itself** (the
richer source, used below). Findings for a **solo personal Mac**, decision-oriented.

**⚠ Top action item — SIP is currently DISABLED on dresden** (`csrutil status` → disabled), almost
certainly a yabai-era leftover (Aerospace does not need SIP off). System Integrity Protection is a core
macOS defense; re-enabling requires a one-time Recovery-mode `csrutil enable`. **Verify nothing still
depends on it, then re-enable.** Not automatable (Recovery-only, by design) and osquery-adjacent →
operator action, tracked as a check-only assertion. This is the single highest-value finding.

Also measured live: FileVault **on**, Gatekeeper **on**, app firewall **on** but **stealth mode off**;
osquery 5.23.1 already deployed with the six `com.webdavis.osquery-*` jobs; none of the tools below
installed.

| Tool | Purpose | brew cask | Maint. | Verdict for dresden |
| --- | --- | --- | --- | --- |
| **osquery standalone** | scheduled host queries | (installed) | low | **KEEP — already correct.** Fine on one machine in exactly the shape already built (local config + differential queries + local alerter); Fleet is multi-device overkill. Caveat for S9: `interval` counts *daemon-uptime*, and sleep pauses it — a "daily" query can take days on a laptop; schedule tighter than the wall-clock target. |
| **LuLu** (Objective-See) | outbound firewall | `lulu` | low after ~1wk training | **ADD.** The open-source Little Snitch; the one layer Apple's inbound-only firewall lacks (catches malware phone-home). v4 supports profiles. |
| **BlockBlock** (Objective-See) | real-time persistence *alerts* | `blockblock` | set-and-forget | **ADD (corrected from my first pass).** NOT redundant with osquery — osquery differential-logs launchd on a schedule; BlockBlock is a real-time GUI *alert* the instant anything installs persistence. Different capability. |
| **KnockKnock** (Objective-See) | on-demand persistence scan + VirusTotal | `knockknock` | zero (run monthly) | **ADD.** On-demand VT lookups osquery doesn't do; complements, not duplicates. |
| **OverSight** (Objective-See) | mic/camera-on alerts | `oversight` | zero | **ADD.** Not covered by osquery at all. |
| **Secretive** (maxgoedjen) | SSH keys in the Secure Enclave | `secretive` | one-time migration | **ADD — high value for a developer.** Makes SSH keys non-exfiltratable (they never leave the enclave). New. |
| **Santa** (North Pole Security) | binary allowlisting | `santa` | **high** | **SKIP for solo.** Its value is LOCKDOWN mode = perpetual allowlist curation on a machine that compiles new binaries daily; MONITOR mode mostly duplicates BlockBlock+KnockKnock+osquery. Google archived `google/santa` 2025-02; maintained fork is `northpolesec/santa`. |
| **Pareto Security** | menu-bar settings auditor | `pareto-security` | zero | **OPTIONAL — redundant here.** The repo's `macos_defaults.yaml` drift system + the posture assertions below already cover it declaratively. |
| **Native: FileVault / firewall+stealth / Gatekeeper / SIP** | baseline posture | n/a | none | **Track in chezmoi.** macOS-version gotchas (verified): script the firewall via `socketfilterfw` **only** — Sequoia+ removed the `com.apple.alf` plist as source of truth; Gatekeeper can no longer be flipped by `spctl` (System-Settings-gated now); FileVault/SIP are **check-only** assertions (don't automate enable — Recovery/DEP-gated). Set **stealth mode on** (currently off) via `socketfilterfw --setstealthmode on` in `macos_system_setup.yaml`. |

**Operator sign-off required** (osquery/security is your domain, spec decision 4): a recommendation for
you to approve — nothing lands in S9/S10 without your yes. If accepted: the four Objective-See casks +
Secretive are `.chezmoidata` cask entries; stealth-mode + the SIP/FileVault/Gatekeeper check-only
assertions fold into S10; and S9 gains the two osquery gap-queries the research flagged (a differential
`listening_ports`⋈`processes`, and a `system_extensions`/`kernel_extensions` differential) **iff** the
packs don't already cover them — a query-content change, so explicitly your call.
Sources: objective-see.org/tools, github.com/northpolesec/santa (+ archived google/santa), santa.dev,
osquery.readthedocs.io/deployment/configuration, support.apple.com/121011 (Sequoia firewall plist),
developer.apple.com SIP docs, github.com/maxgoedjen/secretive, drduh.github.io/macOS-Security-and-Privacy-Guide.

### Gaps (honest)

- **R1 / R7 were verdict OVERCLAIMED, then trimmed:** the surviving items above are only the parts that
  passed the fit-to-dresden verification; the discarded parts (a blanket "no tool ever helps"; some
  speculative notification tooling) are intentionally not carried.
- **Point-in-time values to resolve at implementation, not now:** the exact `nix-installer-action` /
  `checkout` commit SHAs (look up fresh — pins drift), and the current treefmt-nix module set.
