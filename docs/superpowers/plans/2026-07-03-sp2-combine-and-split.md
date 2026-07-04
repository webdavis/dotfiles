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
| S2 | Lint/test/CI hardening | `scripts/lint.sh`, `.githooks/pre-commit`, `.github/workflows/lint.yml`, `.editorconfig`/`.shellcheckrc`/`.mdformat.toml` hunks | CI runs tests + `LINT_CHECK=1`; wire **actionlint** (P9); pin `nix-installer-action@<tag>`; `lint.sh` runner-selection subshell bug; `-r` optstring crash; template shellcheck allowlist → programmatic; `find` prune set dedup; bats `grep -c` zero-count false-pass | S1 |
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
    pin `NixOS/nix-installer-action@<current-release-tag>` (look up the tag, don't use `@main`).
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
