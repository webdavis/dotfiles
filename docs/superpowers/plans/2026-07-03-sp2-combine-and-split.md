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
- **Design + testing standard (HARD RULE, binding — per the spec's essential-feed section, decisions log
  #6, and the [essential-feed-case-study](https://github.com/essentialdevelopercom/essential-feed-case-study)
  repo the strategy was drawn from):** **TDD drives the design** — for every piece of new logic, write the
  failing test first, show the red run, implement minimally, show green; no implementation-first work
  passes review. **SOLID** at the language's altitude: single-responsibility units behind clear seams,
  wired at one composition point. **Classist (Detroit-school) testing:** real collaborators in domain
  tests; test doubles only at true I/O boundaries (network, subprocess, filesystem, clock) — the
  `test/osquery-alerter/lib.bash` harness is the in-repo exemplar and the template for all bats work.
  Transplanted (already-tested) code carries its tests in the same PR and runs green; any behavior change
  to it starts with a failing test. **Fable enforces this rule** at step 5 of the Fable-conductor
  gap-closure loop (Phase C) — it is not waivable.
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

**Build the hunk-ownership table first (Phase B, step 1 — per §R1).** Before any slicing, walk
`git diff main integration/modernization -- <file>` for each of the 8 shared files and record which
slice(s) own which hunks in a table (shared-file × owning-slice × one-line-what). This is deferred to
execution on purpose — it must be computed against the *live* diff — but it is the first Phase B action,
not an afterthought; the Phase D empty-diff gate verifies every hunk landed exactly once.

**Sizing authority.** This table's grouping is a starting point, not a size guarantee. **The operator's
review speed is the authority** — any slice whose real diff is too large to review quickly sub-splits on
the spot (S4 and S9 carry pre-noted splits; S8 is small — the clean SP1 work). No PR should exceed a
quick review.

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

### The Fable-conductor gap-closure loop (STANDING — governs every section)

**Fable 5 is the standing conductor for every section** (each slice S1–S12, and the Phase A/B/D tasks).
This loop is not opt-in and is not re-instructed per section — it is the default execution model for the
whole plan. Do not skip it, shortcut it, or ask whether to run it; run it. The mechanical P-1…P-8
protocol below is **step 3's inner cycle**, not a replacement for this loop.

1. **Plan review.** Fable reads this section's plan text (its slice row, per-slice specifics, folded
   ledger fixes, and any amending `R*`/Phase-E items) and identifies every gap and every improvement it
   can — missing wiring, untested surface, stale assumptions, unstated decisions, sizing risk.
1. **Plan adjustment.** Fable edits the plan to close those gaps and writes the improvements up to the
   best of its ability *before* implementing — the plan is the source of truth the implementers read, so
   it is corrected first, not retroactively.
1. **Orchestrated implementation.** Fable executes the section as orchestrator, running the P-1…P-8
   protocol below. **Model policy (operator-set, 2026-07-09, supersedes the
   subagent-driven-development cheapest-tier rule): every implementer and fixer dispatch is Opus at
   max effort; Fable runs its own conductor jobs (gap identification, planning, instructing, reviewing)
   at high effort.** Always name the model explicitly in the dispatch — never inherit by omission.
1. **Implementer (Opus, max effort) implementation.** The implementer executes the task at Fable's
   behest, under the full Global Constraints (TDD-first, green-before-commit, Conventional Commits, no
   AI trailers).
1. **Conductor review + correction.** When the implementer finishes, Fable reviews its work against the
   section plan, names every mistake and residual gap, and instructs the model (or a fresh one) to
   implement the fixes. Findings move as files, per the skill's handoff rules.
1. **Repeat until satisfied.** Steps 4–5 loop until Fable can identify no further mistake or gap in that
   task's work.
1. **End-of-section sweep.** After the section's tasks are all individually clean, Fable does one final
   sweep across *everything* the section produced (the whole slice diff), identifies any remaining gaps
   and improvements, and runs the 4–6 loop on them until satisfied. Only then does Fable decide whether
   the section is PR-ready and hand it to the operator review gate (P-8).

> **HARD RULE — TDD + SOLID is non-negotiable, and Fable enforces it.** Every implementation in every
> section MUST follow the TDD-SOLID strategy identified from the
> [essential-feed-case-study](https://github.com/essentialdevelopercom/essential-feed-case-study) repo
> and specified in the **Global Constraints** "Design + testing standard" bullet: failing test first →
> red → minimal implementation → green; SOLID single-responsibility units behind clear seams wired at one
> composition point; Classist (Detroit-school) testing with real collaborators and doubles only at true
> I/O boundaries. Fable is responsible for ensuring this rule is met — a task whose work is
> implementation-first, or whose new logic lacks a shown red-then-green, fails Fable's step-5 review and
> is sent back. This gate is not waivable by the implementer, by sizing pressure, or by "transplanted
> code" (transplanted code carries its tests and runs green; any behavior change to it starts red).

**The protocol (run for each slice — this is step 3's inner cycle):**

- [ ] **P-1: Branch from the current main.**
  ```bash
  git checkout main && git pull origin main
  git checkout -b slice/<name>
  ```
  Later slices branch from the main that earlier slices advanced — this is why shared-file hunks layer
  instead of conflicting.

- [ ] **P-2: Assemble the slice's files from the integration branch.** For files owned wholly by this
  slice, take them verbatim: `git checkout integration/modernization -- <path> …`. For a **shared infra
  file**, do NOT take the whole file — apply only this slice's hunks (per the Phase B hunk-ownership
  table), two options: **(a) interactive** `git checkout -p integration/modernization -- <shared-file>`
  (answer y/n per hunk); **(b) agent default (non-interactive, deterministic — use this under
  subagent-driven execution):** `git diff main integration/modernization -- <shared-file>` → trim the
  patch to this slice's hunks → `git apply --index` (the trimmed patch is a reviewable artifact). For
  **deletions**, `git rm <path>` in the same slice that makes the deletion safe (e.g. tmux files die in
  S4 as herdr lands).

- [ ] **P-3: Fold in the ledger fixes** named in this slice's row — these are *improvements over* the
  integration branch's version (that is the point of reimplementing). Each fix is test-driven where it
  has runtime surface (see the slice's specifics).

- [ ] **P-4: Verify full wiring.** No dead code. If the slice adds a `run_*` script, confirm what triggers
  it and that its target exists. If it adds a LaunchAgent plist, confirm the matching loader script is in
  the same slice. If it adds a deployed binary, confirm something references it (a hook, a keybinding, a
  recipe). Grep the slice's own new symbols to prove each has a consumer. **Shared-file check:** after
  applying this slice's hunks, `git diff <slice-branch> integration/modernization -- <shared-file>` should
  show **only the *other* slices' hunks** — proving this slice took exactly its own, no more, no less.

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
- **Research amendments (§R2, §R3):** promote **treefmt-nix** to the *primary* lint/format orchestrator —
  it deletes ~450 lines of `lint.sh`, one global `excludes` replaces the 6×-duplicated prune set,
  actionlint runs via its treefmt module, CI's fail-on-drift becomes `nix flake check --all-systems`, and
  the 4 chezmoi-specific checks port to custom formatters. The SHA-pin + `permissions: contents: read` +
  `persist-credentials: false` + **zizmor** + `.github/dependabot.yml` set is §R3 (already reflected in
  the CI bullet above).
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
- **Research amendments (§R5):** keep-all-21 decision, so the fix is reproducibility, concrete — **9
  skills are uncommitted** (`chrome-devtools-axi`, `cua-driver`, `elevenlabs`, `gh-axi`, `home-assistant`,
  `kubernetes-specialist`, `last30days`, `sql-toolkit`, `tiktok-crawling` — including the *preferred*
  gh-axi/chrome-devtools-axi), Hermes has **0** symlink declarations, and `update-skills.sh` is
  refresh-only (skips absent skills). Commit or install-manifest all 21, declare the Hermes fan-out, make
  update-skills install-capable, and add the vendored-skill SHA/`computedHash` supply-chain gate before
  the atomic swap.

### S4 — herdr migration (re-scoped 2026-07-09 against live state, per learning #2)
- **Decision: ONE PR — the S4a/b/c split is retracted.** The atomicity invariant (below) means any
  split's final PR still carries the risky flip (bashrc + deletions), so splitting buys nothing; and the
  content is dresden's already-live state, familiar to the operator. Commits stay logically separated
  instead.
- **Atomicity is the invariant:** the tmux/sesh deletions and the herdr additions ship together — main
  must never have both, nor neither.
- **Exact file set (verified against the live `origin/main`→`integration/modernization` diff):**
  - **ADD:** `dot_config/herdr/config.toml`; the two Rust plugins under `dot_local/share/herdr/plugins/`
    (`herdr-last-workspace`, `herdr-smart-nav` — transplants that CARRY their own `#[cfg(test)]` suites
    and must run green via `cargo test`); `.chezmoiscripts/run_onchange_before_15-install-herdr.sh.tmpl`;
    the `after_55-build-herdr-last-workspace` + `after_57-build-herdr-smart-nav` build scripts. NOT
    `after_55-osquery-pipeline-manifest` (S9 — a numeric-glob near-miss).
  - **DELETE (atomic cluster):** `dot_tmux.conf`, `dot_config/sesh/**`, the six
    `dot_local/bin/executable_{sesh-*,tmux-*}` scripts, `run_after_70-install-tmux2k-last-proc`, AND —
    moved here from S7 — `dot_local/bin/executable_claude-restart.sh` +
    `Library/LaunchAgents/com.claude.code.plist.tmpl`: the plist's only payload is exec'ing
    `claude-restart.sh`, which drives tmux, so the pair is tmux-coupled and dies with tmux (deleting the
    script in S4 while S7 kept the plist would leave main's LaunchAgent exec'ing a nonexistent file).
  - **Shared-file hunks S4 owns:** `dot_bashrc.tmpl` — ONLY the tmux/sesh→herdr semantics (TERM, the
    `t`/`h` aliases, the tmux-purge alias and `__tmux_last_proc_precmd` block removals, the notifier
    skip-list word swap tmux→herdr, and the end-of-file herdr auto-attach block with its `HERDR_ENV` /
    `SSH_ORIGINAL_COMMAND` / vscode guards); NOT the relay notifier rewrite (S7), NOT the
    interactive-guard/PATH restructure or brew-cache sourcing (S11). `CLAUDE.md` — the tmux→herdr
    section rewrites (Tmux Session Management → Herdr Workspace Management; drop the tmux2k indicators
    section; notifier line). `.chezmoiignore` + `.gitignore` — the 2-line herdr `target/` ignore hunks.
    `.chezmoidata/system_packages_autoinstall.yaml` — remove the `sesh` and `tmux` formula lines
    (targeted line edits; the file's other diffs belong to S5/S6). `justfile`, `dot_profile`, and
    `private_dot_claude/CLAUDE.md` carry NO S4 hunks (verified — their diffs are other slices').
- **Ledger fixes (all are NEW work — the integration branch never fixed them; verified byte-identical
  to main):** consolidate the two plugin build scripts' common core into a `.chezmoitemplates` partial
  (red-first: a rendered-template test asserting both scripts share the partial's anchored logic); anchor
  the `grep -q "$plugin_id"` link check (unanchored substring match); remove the 14 tmux-`$TMUX` dead
  widgets from `dot_fzf_bindings`; fix `dot_bash_bindings` — the duplicate `\C-gss` vi-command binding
  (line ~181 should be `\C-gsh`, per its sibling insert-mode line), drop the stale `nvm` bindings
  (toolchain has no nvm), audit `$blue` for use-before-definition — with a red-first
  duplicate-keybinding invariant test (`test/bash-bindings-unique.sh`: no `(keymap, key-seq)` bound
  twice).
- **Wiring (P-4):** the two Rust plugins build (`cargo build --release --locked`) and their tests pass
  (`cargo test`); the auto-attach block guards on `HERDR_ENV`; nothing on main references any deleted
  path afterwards (`git grep` each deleted basename). **Two-world validation (learning #1):** never a
  live apply from this branch — validate via cargo build/test + `chezmoi execute-template` renders +
  `just lint-check && just test`; the live apply is the operator's, at P-8.
- **Operator apply** needed (builds Rust at apply time; installs herdr via the curl installer; tears
  down tmux LaunchAgent state).

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
- ~~Delete the old `com.claude.code.plist.tmpl`~~ — **moved to S4** (2026-07-09): the plist's only
  payload execs the tmux-coupled `claude-restart.sh`, which S4 deletes, so the pair ships in S4's atomic
  cluster (keeping it here would leave main's LaunchAgent exec'ing a nonexistent file between S4 and S7).
- **Operator apply** needed (`private_auth.json.tmpl` is KeePassXC-gated).

### S8 — Hermes age-encryption
- This is the SP1 work (already committed on the working branch as `c13cc18`/`a0e7d8e`/`3696c92`) shipped
  as one clean PR. The age-tripwire fix and the fresh-machine restore script are part of it.
- **Operator step:** the `age` recipient in `.chezmoi.toml.tmpl` is the operator's public key (already
  set); the private key restore rides KeePassXC. Round-trip verify in the PR (`chezmoi cat` == live).
- **Research amendments (§R4):** generalize the `{{ if eq .chezmoi.os "darwin" }}` guard on
  `run_once_before_05-restore-age-key` so the Linux home-server restores the key too; ship
  `docs/runbooks/age-key.md` (rotation + disaster-recovery workflows are spelled out in §R4) and a
  `test/age-restore.sh` DR drill. KeePassXC entry name is `chezmoi :: Private Key :: age` (spec corrected
  2026-07-04). Multi-recipient migration is deferred (see the spec's laptop→home-server item).

### S9 — osquery three-tier alerting
- **Smallest big slice** — most of osquery is already on main; carry only the PR#25 delta.
- **In scope:** alerting/dispatch design improvements. **Sign-off gate:** any `.chezmoitemplates/osquery/
  *.conf` query/pack content change is listed in the PR body for explicit user approval before merge.
- **Wiring:** all 6 LaunchAgents + loaders; the 87-bat suite green; the pipeline manifest baseline.
- **Research amendments (§R8):** the two osquery "gap-queries" once proposed are **retracted** —
  `listening_ports_non_loopback` and `kernel_extensions`/`system_extensions` monitoring already exist in
  `intrusion-detection.conf`. osquery's genuinely-unfinished work (Mouse analysis agent, approval-UX PR2,
  FleetDM migration, deferred beaconing/Wazuh) lives on the `docs/osquery-design` branch and is **out of
  SP2 scope** — S9 ships only the PR#25 three-tier delta.
- **Sizing fallback:** if that delta is too large for a quick review, sub-split by component (dispatch lib
  / results-alerter / the six pollers+loaders / packs), mirroring S4.

### S10 — macOS defaults / system-setup
- **Ledger fixes:** defaults trio shared-lib + `chezmoi source-path` (kills the worktree-writes-primary
  bug); `after_41` `{{ if index . "sudo" }}`; add `ssh-hardening.sh` as a `macos_system_setup.yaml`
  record so a fresh machine actually locks sshd — **and fix the script's PAM hole (roadmap high-sev,
  found missing in the 2026-07-09 audit):** the drop-in sets only `PasswordAuthentication no`, but
  `UsePAM yes` + the `KbdInteractiveAuthentication` default leave PAM password login open (verified
  live per the roadmap ledger) — the hardening record must also set
  `KbdInteractiveAuthentication no` (and address UsePAM's interaction) so password auth is actually
  closed, test-driven per the sshd `-T` effective-config seam.
- **Operator apply** needed (Tier-2 sudo runner prompts once).
- **Research amendments (§R8, §R6):** the R8 endpoint additions already landed on the working branch
  (`36d2d27`) — the `lulu` + `oversight` casks and the firewall **stealth-mode** `macos_system_setup.yaml`
  record — so S10 carries them to main, plus check-only posture assertions (FileVault / Gatekeeper / SIP
  via `fdesetup status` / `spctl` / `csrutil status`; SIP + FileVault are assert-only, never auto-enable).
  §R6: macOS system config stays chezmoi-native — nix-darwin is deferred to SP-nix, so no nix-darwin work
  in this slice.

### S11 — Shell foundation + secrets hygiene + chores
- **Files:** the remaining shared-infra hunks + the brew-shellenv cache + the credential `private_`
  renames (`ae02524`) + worktrunk + gitconfig.
- **Ledger fixes:** merge.tool name-not-command; `core.excludesfile` (ship a `dot_gitignore_global` or
  drop the line); remove git:// url rewrites, `~/.bash_just_completions` source, atuin `~/.atuin/bin/env`
  guard, the linux yabai ignore; espanso `_pqi.yml` import + shadow-trigger renames + the mid-word
  autocorrect `word:true` fix; Arc→Zen hotkey; newsyslog log rotation; `run_after_35-setup-yt-dlp`
  network-on-every-apply + deno/node mismatch (roadmap known-bug set — unassigned until the 2026-07-09
  audit); the inert `gh` `hosts.yml.tmpl`. (P12 gitconfig autocorrect: **already on main** —
  `autocorrect = prompt`, verified 2026-07-09; no work.)
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

- [ ] **Step 0 — empty-diff reconciliation gate (§R1):** before switching main, assert every hunk landed
  exactly once: for each of the 8 shared infra files, `git diff main integration/modernization -- <file>`
  must be **empty** (allowing for intentional post-split improvements — investigate any non-empty diff:
  it means an orphaned or double-counted hunk). Also `git diff main integration/modernization --stat`
  should show no unexpected file left only on the reference branch. This is the proof that the split
  reproduced the whole integration branch.
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

**Deferred feature idea `[dresden]` — OverSight → relay bridge.** OverSight's Action tab can `exec` a
script on every camera/microphone event, passing `-device <camera|microphone> -event <on|off> -process
<pid> -activeCount <n>`. A tiny wrapper pointing at `relay.sh` (or the SP3 Rust service) would turn an
*unexpected* camera/mic activation into a phone push — a genuine "someone/something is watching"
signal that reuses the existing notification fan-out. Not now (needs the wrapper written + a
whitelist of expected callers like Zoom/Photo Booth so it only fires on the unexpected); banked as a
future security↔notification integration once SP3 lands.

### Gaps (honest)

- **R1 / R7 were verdict OVERCLAIMED, then trimmed:** the surviving items above are only the parts that
  passed the fit-to-dresden verification; the discarded parts (a blanket "no tool ever helps"; some
  speculative notification tooling) are intentionally not carried.
- **Point-in-time values to resolve at implementation, not now:** the exact `nix-installer-action` /
  `checkout` commit SHAs (look up fresh — pins drift), and the current treefmt-nix module set.

---

## Deferred sub-projects — NOT SP2 scope, tracked in the roadmap spec (do not lose these)

These operator-decided items live in `docs/superpowers/specs/2026-07-02-repo-modernization-roadmap-design.md`
(the sub-project table), not in this plan — listed here so the plan is self-contained about what it
deliberately does NOT cover:

- **SP4 — bash→nushell evaluation: RESOLVED — NO-GO, operator-ratified 2026-07-09.** The evaluation ran
  during S4 (report: `docs/research/2026-07-09-sp4-nushell-evaluation.md`); verdict NO-GO on three legs:
  reedline binds one key event per binding — no multi-keystroke chord grammar (verified against the
  line-editor docs AND reedline #69, where chords remain an open feature request), which the ~365-chord
  binding surface cannot survive; atuin's nushell integration is its weakest (and atuin is the
  thrice-broken subsystem here); cost/value fails against a working ~15ms bash hot path. Adjacent shells
  (zsh/fish/others) were surveyed at ratification: zsh is the only chord-capable candidate, and the
  operator chose to **stay on bash**. Consequences: GH #5 closes as evaluated/declined; SP3 designs its
  notifier seam against bash-preexec with NO shell-portability abstraction; P8 quick wins unblock
  (shell-config placement = bash).
- **SP4 (successor scope, operator-directed 2026-07-09) — Bash setup improvement.** The vacated SP4
  slot becomes a bash-improvement sub-project (own spec cycle: brainstorming → plan → the standing
  Fable loop), running AFTER SP2's cutover (D1) — its targets (`dot_bashrc.tmpl`, `dot_bash_aliases`,
  `dot_bash_bindings`, `dot_fzf_bindings`) are shared files that S7/S11 still carry hunks in, so
  starting earlier would recreate the two-writer hunk problem. Roadmap position unchanged: after SP3
  (only bashrc-shim contact between them; may start at cutover if the operator re-prioritizes). Five
  workstreams: (1) consolidate every alias out of bashrc into `.bash_aliases`; (2) `dot_fzf_bindings`
  code quality + new bindings (candidates: zoxide-backed dir jump, git-stash picker, process killer,
  worktree switcher, herdr workspace picker); (3) invert the bindings architecture — ONE data table
  (`key | keymap | description | command`) from which the `bind` statements, an fzf-driven menu (view →
  pick → execute), and the tests are all generated (kills overlap by construction; replaces the fragile
  reverse-parsing `__bash_bindings_list_bash_bindings`); (4) Charm-tools verdict RECORDED — gum
  (script UI prompts): no, fzf already covers the menu; bubbletea + lipgloss (Go TUI/styling
  libraries): no, YAGNI; crush (AI coding agent): no, redundant with Claude Code/Codex; vhs (scripted
  terminal recorder): optional later for chord demos/deterministic pty tests — ZERO new dependencies
  now; (5) generated per-binding tests (registration + firing) so a broken binding is caught at
  commit time, not at the keyboard.
- **SP6 — nvim-overhaul.** Re-evaluate the v1/v2/v3 design generations (Fable conducts the
  re-evaluation — operator directive) plus the 10 unpushed commits on the `nvim-overhaul` branch, then
  implement under its own spec. Not started; runs after SP2.
- **SP7 backlog — small chores**, including **P6: install `bandwhich`, `doggo`, `ouch`** ("still valid,
  trivial" — manifest entries + `brew install`), P3 package-manager audit, P5 Determinate Nix review,
  P8 quick wins (placement depends on SP4's verdict).
- **SP-nix — nix-darwin go/no-go** (research-first sibling of SP4, banked in §R6).

## Phase E — End-of-SP2 cleanup backlog

Debts discovered during execution (chiefly S3). Each is deferred for a stated reason; all must be
resolved before SP2 closes. Labelled `fix/<name>` for tracking; a `(→ Sn)` tag means it rides that
slice.

### fix/harness-skill-reconciliation

Audit **every harness's live skill directory** against the approved roster and per-profile allowlists —
any skill present or enabled that the operator did not approve is flagged and removed/opted-out. The
roster/fan-out test enforces the *declared* set in the repo, but live directories can drift (direct
`npx`/`clawhub` installs, `--clone-from` artifacts, hand-installs). Cover all four surfaces:

- **Store** (`~/.agents/skills`): must be exactly the roster (31 skills) — no extra dirs, no stray
  installs. This is the source every harness reads, so a stray here contaminates all of them.
- **Claude Code** (`~/.claude/skills`): must be exactly the roster's store symlinks — no non-symlink
  entries, no skills absent from the roster.
- **Codex** (scans `~/.agents/skills` natively; check `~/.codex/skills` and any `--agent codex` install
  targets for strays that would double-list or add unapproved skills).
- **Hermes profiles** (`~/.hermes/profiles/{butters,concerned,elaine,nicodemus}/skills/` + default):
  each physically carries the full bundled catalog (~30 category dirs — apple, devops, creative,
  security, …) plus `--clone-from default` artifacts; the five-profile plan intends a curated subset
  scoped by `skills.disabled`. PR #36's gap-A reconcile removed the frozen *store-skill* clones and
  planted the correct store symlinks, but the bundled catalog dirs — and any skill *enabled* beyond a
  profile's source-of-truth allowlist — remain.

Fix: for each surface, diff live contents against the approved set and remove/opt-out the rest
(`hermes -p <p> skills opt-out`/`uninstall` on the Hermes side; `trash` strays elsewhere), then verify
(`hermes -p <p> skills list`, and the roster test against the live dirs). Hermes-side pruning is Bob's
domain — coordinate at cutover. Consider promoting the live-vs-approved diff into an automated check so
drift is caught, not hunted.

### fix/hermes-encrypted-profile-configs (→ S8)

The five profiles' `config.yaml` (enablement + `platform_toolsets`) are persisted only as **untracked
encrypted `.age` files** in the primary checkout; codegraph's Hermes-MCP enablement likewise. A fresh
machine reproduces skill *presence* but not per-profile *curation* or the MCP wiring. Fix rides S8 (the
age machinery): track `dot_hermes/profiles/*/encrypted_config.yaml.age` + the codegraph MCP config,
round-trip verify, extend the DR drill.

### fix/codex-agents-parity (→ S12)

Global-rules notes (the Home Assistant pairing line, and any future rule) currently reach Codex only via
a hand-edit to the **untracked `~/.codex/AGENTS.md`**. R5/S12's shared `.chezmoitemplates` partial —
included by both `~/.claude/CLAUDE.md` and `~/.codex/AGENTS.md` — is not built. Fix: implement the parity
partial in S12 and migrate the HA note (and the rest of the global ruleset) into it so both harnesses
share one source.

### fix/graphify-out-excludes-drop

`.gitignore`'s `graphify-out/` entry and `treefmt.nix`'s `graphify-out/**` exclude are band-aids, kept
because the old global graphify post-commit hook still fires in this repo until the opt-out dispatcher
(S3) is applied live. Fix: after the cutover `chezmoi apply` deploys the dispatcher and this repo's
`.githooks/no-graphify` marker suppresses graphify here, drop both excludes.

### fix/live-reconcile-from-scratch

PR #36's live skills convergence (frozen-clone cleanup, per-profile symlink planting, stale hub-install
retirement, Codex-overlay + routing re-assert, live `skillOverrides` merge) was performed **ad-hoc on
the live machine** during review. The reproducible path is `.superpowers/sdd/live-reconcile-skills.sh`,
run once post-cutover-apply on the converged `main` source. Fix: at cutover, after `chezmoi apply`, run
the reconcile script (`--dry-run` then live) to prove a from-scratch machine converges identically; then
the ad-hoc live state and the script are reconciled.

### fix/moshi-herdr-drift-check (→ S11)

The fork upstream drift-check + relay notification for the `moshi`/`herdr` local forks is banked for S11
(it needs `relay.sh` from S7). The lock's `forks` table and the weekly drift-check pass exist; the
notify path is the missing piece. Fix: wire the relay push in S11.

Also (found during S4 re-scope, 2026-07-09): the vendored `moshi` fork's `SKILL.md` still documents a
tmux-based remote transport ("Mosh plus tmux", `command -v tmux`, "at least one tmux session") — stale
against the herdr migration on this machine (though possibly still valid guidance for remote hosts that
do run tmux). It is fork *content* (skills lane, not S4's), so S4 does not touch it; resolve it in the
same S11 fork-maintenance pass — decide whether the guidance is host-specific or needs a herdr rewrite,
and bump the fork's `lastComparedTreeHash` notes accordingly.

### fix/pre-commit-path-filter (found 2026-07-09 roadmap audit)

The roadmap's S2 design-alternative "pre-commit: skip the bats suite on docs/YAML-only commits (path
filter)" never made it into the S2 plan text or implementation — every commit (including docs-only)
runs the full `just lint-check && just test` (observed live: plan-edit commits run the whole suite).
Friction, not correctness. Fix: a path filter in `.githooks/pre-commit` that skips `just test` (never
the lint gate or gitleaks) when the staged diff touches only docs/markdown. Slot: S11 or post-SP2.

### fix/template-render-coverage (found 2026-07-09 roadmap audit)

The old `lint.sh` shellcheck-rendered ~12 templates via an allowlist; S2's treefmt port carried only 2
(bashrc, osquery before_50) and S4 added its 3 herdr scripts — so ~7 previously-linted shell templates
lost render-lint coverage, and the S2 ledger fix "template shellcheck allowlist → programmatic" was
never implemented (treefmt.nix uses an explicit include list). Fix: either make the include list
programmatic (all `.chezmoiscripts/*.sh.tmpl` that render without keepassxc, discovered not
enumerated), or restore the missing entries slice-by-slice as their files land (S6/S7/S9/S10 each add
their scripts). Decide in S11 at the latest; each slice SHOULD add its own templates meanwhile (S4 has,
belatedly, in its final-review round).

### fix/skill-architecture-diagram

A living node-graph of the cross-harness skill architecture, so the whole system is legible at a glance
and every gap is visible — and so `fix/harness-skill-reconciliation` above has a map to reconcile
against. Keep it accurate to the live repo at authoring time: **regenerate from
`~/.agents/custom-skill-lock.json` + `dot_local/bin/executable_update-skills.sh` + `test/`, do not
hand-transcribe** (consider seeding it with `graphify`/`codegraph`). Commit the rendered artifact and its
source (Mermaid or Graphviz — whichever renders cleanest) under `docs/`. Execution prompt:

> Create a node-graph of the current skill architecture across all harnesses. A reader must be able to
> trace, for every skill, its full lifecycle. The graph must show:
>
> - **Origin & fan-out** — the canonical store (`~/.agents/skills/`) and how each skill reaches each
>   harness: Claude (`~/.claude/skills/` symlinks), Codex (native store scan + committed
>   `agents/openai.yaml` overlays), Hermes default (`~/.hermes/skills/`) and specialist profiles
>   (`~/.hermes/profiles/<agent>/skills/`), and the app-owned symlink case (`~/.cua-driver`).
> - **Provenance lane per skill** (npx-GitHub / clawhub / vendored-fork / app-owned) and **which lock
>   file** records it — `~/.agents/custom-skill-lock.json` (tables: `npxTracked`, `clawhubTracked`,
>   `forks`, `hermesProfiles`, `hermesRegistry`, `tiers`, `superpowersRouting`) and npx's own
>   `~/.agents/.skill-lock.json` — naming the **tool/script that owns each lock**.
> - **Update mechanism per skill** — the exact command + infrastructure that refreshes it (`npx skills
>   update`, `clawhub … update`, `hermes -p <p> skills update`, the CUA app's `cua-driver skills
>   update`, or "vendored — chezmoi-only / drift-alert-only") and **which pass of
>   `~/.local/bin/update-skills.sh`** drives it.
> - **Schedule** — when each updates (the weekly `com.webdavis.update-skills` LaunchAgent, Monday 04:00;
>   `chezmoi apply` for vendored; the CUA app's own cadence).
> - **Upstream** — the source repo/registry for each skill, or "no upstream (local fork / bespoke)".
> - **Tiering** — core vs on-demand, and the mechanism per harness (Claude `skillOverrides`, Codex
>   `allow_implicit_invocation: false`, Hermes `skills.disabled`).
> - **Tests** — which TDD suite(s) under `test/` cover each skill, script, and behavior (roster/fan-out
>   invariants, install supply-chain, hermes-phase, superpowers routing, fork-drift, launchagent-path,
>   cua refresh, post-commit dispatcher); draw test→subject edges so untested surfaces stand out.
> - **Gaps in red** (or a clearly distinct style) — any skill with no update path, no upstream, or no
>   test coverage; any live-vs-declared mismatch; every open `fix/*` from this Phase E backlog; and the
>   deferred S8 (encrypted profile configs, codegraph MCP) and S12 (Codex AGENTS.md parity) dependencies.
> - Anything else that aids comprehension — a legend, per-lane colour, a summary count table.

---

## Execution learnings (2026-07-09, from S3 — carry forward to S4–S12 and D1)

Hard-won during the skills slice; each applies to the remaining slices.

1. **The two-world apply trap.** dresden's chezmoi source is the *integration* branch, which does NOT
   contain merged slice work. A live `chezmoi apply` — or even `chezmoi diff` — from the integration
   checkout will *revert* a merged slice's files back to the old system (S3: the live updater was still
   the old 206-era script). Corollary: after a slice merges, the live machine does not automatically
   match `main` — the committed PR is *desired* state; converging the live machine is a *separate* step.
   To validate a slice applies cleanly, dry-run-apply the *slice source* into a scratch `$HOME`
   (`chezmoi --source <worktree> --destination <tmp> apply --dry-run`), or wait for D1. Every slice with
   a live-state component (S4 herdr, S7 relay, S9 osquery, S10 defaults) has this exposure.

2. **Re-scope each slice against live state at execution — do not trust the plan's file lists/counts.**
   They were computed early and drift. S3's original scope ("commit the 9 uncommitted of 21 skills") was
   completely overtaken (the store grew, then settled into a 31-skill npx / clawhub / vendored-fork /
   app-owned model with weekly auto-update). Before executing, diff the slice's declared files against
   both the live integration branch and the live machine, and re-scope. The map is a starting point.

3. **Single-writer per worktree.** Never dispatch a second agent into a worktree another agent is
   writing — concurrent writers corrupt the work (hit twice this session; caught by the second agent
   freezing with zero writes). To add scope to in-flight work, *message the running agent*; to add work
   after, wait for it to report done, then verify the worktree is clean before dispatching.

4. **Live reconciliation is a first-class, scripted step — not ad-hoc.** The gap between committed
   desired-state and the live machine (symlinks, `settings.json` merges, LaunchAgent reloads,
   stale-copy cleanup, secret-bearing configs) is real and error-prone by hand. Keep it in an
   idempotent, `--dry-run`-able reconcile script, run once at cutover to prove a from-scratch machine
   converges. Doing it ad-hoc live (as S3 did under review pressure) leaves the reproducible path
   unproven — hence `fix/live-reconcile-from-scratch`.

5. **Discovered debt → a `fix/*` item in Phase E immediately.** Don't stretch a slice to fix everything
   it touches; track it and move on. Phase E now holds S3's.

6. **Skills are their own provenance model, not one lane.** npx `skills add` is GitHub-only; ClawHub
   skills install/update via the `clawhub` CLI; deliberate local forks stay vendored (drift-alerted);
   app-owned skills (cua-driver) update via the app. `hermes skills install` has a security scan gate
   (caution needs `--force`, dangerous is hard-blocked) that direct npx/clawhub writes bypass. Any
   future skill work must place each skill in the right lane, not assume one mechanism.
