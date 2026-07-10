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

---

## Progress status (amended 2026-07-10 — the audit becomes permanent)

This section, the **Audit coverage matrix** below it, and every `[audit 2026-07-10]` annotation in the
slice/Phase sections encode the external audit
(`docs/superpowers/audits/2026-07-10-dotfiles-modernization-audit-handoff.md`) and the three operator
rulings that landed after it: **(R1)** MagicDNS was root-caused to the resolver-registration layer, the
supported-fix attempt failed, and a declarative `/etc/hosts` fallback shipped in S5 — RESOLVED; **(R2)**
S7 fixes its four delivery-loss defects BEFORE merge (reverses the earlier "ship the bash as-is" text);
**(R3)** OpenClaw is removed — a Wave-3d cleanup PR owns the package, the AeroSpace F1 binding, and the
docs, and the operator owns the Todoist cleanup.

`main` (`1a6e718`) carries Phase A and S1–S5. S6 is next. Merge SHAs and PR numbers below are verified
against `git log origin/main --merges`.

| Item | State | PR | Merge commit | Remaining follow-up |
| --- | --- | --- | --- | --- |
| Phase A — integration reference | complete | DO-NOT-MERGE reference | — | closed at cutover Gate 5, not merged |
| S1 — Docs | complete | #33 | `1ef7c29` | — |
| S2 — Lint/test/CI | complete | #35 | `90c68c4` | rendered-template coverage regression → **render-coverage** PR |
| S3 — Skills | complete | #36 | `5f21a81` | 3 High defects + the `35922d4` scope-split → **skills-stab** PR |
| S4 — herdr | complete | #37 | `addc8d7` | 2 High + 2 Medium defects → **herdr-stab** PR |
| S5 — Tailscale | complete | #38 | `1a6e718` | copied-daemon re-copy responsibility folds into S6; monitor moved to S9 |
| Wave-3 skills-stab | not started | — | — | audit PR #36 High×3 (defer-forever / fresh-install / additive-fan-out) + Low `35922d4` move |
| Wave-3 herdr-stab | not started | — | — | audit PR #37 High×2 (atomic migration / Cargo+registration) + Medium×2 (`Cargo.toml` hash / LaunchAgent unload) |
| Wave-3 render-coverage + docs | not started | — | — | audit PR #35 Medium (4 template failures + coverage test) + PR #35 Low doc-staleness |
| Wave-3d OpenClaw cleanup | not started | — | — | R3: remove the `openclaw` package, the AeroSpace F1 binding, and the docs together (operator owns Todoist) |
| S6 — Homebrew weekly-upgrade | **next** | — | — | audit S6 gaps folded into the S6 section |
| S7 — Relay pipeline | not started | — | — | R2: four delivery-loss fixes before merge (see S7 section) |
| S8 — Hermes age-encryption | not started | — | — | Linux-boundary + re-scope folded into the S8 section |
| S9 — osquery three-tier | not started | — | — | S5 dependency (`2f430b3` monitor) + path/hunk matrix folded into S9 |
| S10 — macOS defaults / SSH | not started | — | — | physical-presence window + `sshd -T` contract folded into S10 |
| S11 — long-tail chores | not started | — | — | split into the audit's 7 PRs (Thaw = SP5) |
| S12 — global instructions | not started | — | — | unambiguously pre-cutover (see S12 section) |
| Phase D — cutover | not started | — | — | five gates (D1 rewrite below) |
| Phase E — cleanup backlog | ongoing | — | — | every item attached to a Wave-3 PR or a D1 gate |

## Audit coverage matrix (amended 2026-07-10)

Every finding and directive in the 2026-07-10 audit maps to exactly one owning home: **SHIPPED** (already
on `main` via the listed S5 commits), a **Wave-3** stabilization PR, a **per-slice fold** (an
`[audit 2026-07-10]` requirement inside that slice's section), a **D1 cutover gate**, or **this
amendment** (a plan/roadmap edit). Nothing is unmapped.

| # | Audit item | Owning home | Evidence / status |
| --- | --- | --- | --- |
| 1 | Keep Tailscale's copied daemon (correction) | SHIPPED S5 + roadmap edit | copied-daemon model on `main` (`9989812`); supersession note `38bffb6`; June spec/plan history corrected in the roadmap (this amendment) |
| 2 | PR #35 Medium — rendered shell-template coverage regressed (4 failures + coverage test) | Wave-3 render-coverage; Phase E `fix/template-render-coverage` | not started |
| 3 | PR #35 Low — active documentation stale (dependabot / haiku→sonnet / "Both hooks") | Wave-3 render-coverage + docs | not started; larger rewrite stays in S12 |
| 4 | PR #36 High — updates can defer forever | Wave-3 skills-stab | not started |
| 5 | PR #36 High — fresh-machine install not auto-started | Wave-3 skills-stab | not started |
| 6 | PR #36 High — skill fan-out additive, not convergent | Wave-3 skills-stab | not started |
| 7 | PR #36 Low — PR #38 carries an unrelated skills-test fix | Wave-3 skills-stab | `35922d4` lives on `wip/skills-test-hermetic`; move it into skills-stab |
| 8 | PR #37 High — live migration not operationally atomic | Wave-3 herdr-stab | not started |
| 9 | PR #37 High — missing Cargo / failed plugin registration treated as success | Wave-3 herdr-stab | not started |
| 10 | PR #37 Medium — rebuild hashing omits `Cargo.toml` | Wave-3 herdr-stab | not started |
| 11 | PR #37 Medium — removing old Claude LaunchAgent source does not unload it | Wave-3 herdr-stab | one-time idempotent retirement script |
| 12 | PR #38 #1 — status classification incomplete | SHIPPED S5 | `81e7559` (classify on `.BackendState`) + `66a5871` (Starting/Stopped tests) + `9989812` |
| 13 | PR #38 #2 — MagicDNS acceptance fails | SHIPPED S5 (R1) | root-caused registration layer `4830f44`; declarative `/etc/hosts` fallback `6560a59`/`c5614ae`/`f096ecb`/`c90a700`/`164548a` |
| 14 | PR #38 #3 — record the superseded service decision | SHIPPED S5 + roadmap edit | `38bffb6`; June Tailscale spec/plan supersession in the roadmap (this amendment) |
| 15 | PR #38 #4 — split unrelated scope (`35922d4`) | Wave-3 skills-stab | same as item 7 |
| 16 | PR #38 #5 — clean evergreen documentation (`CLAUDE.md` tailscale) | SHIPPED S5 | `6e36512` + `a22ae3b` + `daef534` |
| 17 | Roadmap — progress tracking stale | this amendment | Progress-status table above |
| 18 | Roadmap — S3 describes an obsolete architecture | this amendment | S3 section rewritten to the 31-skill provenance model |
| 19 | Roadmap — Nushell still treated as active | this amendment | roadmap decision 3 / SP4 table / P8 / deferred index / SP3 seam |
| 20 | Roadmap — SP3 called fully designed while contract items remain | this amendment | roadmap SP3 status → "behavior contract approved; final implementation spec pending" |
| 21 | Roadmap — SP5 both standalone and part of S11 | this amendment | S11 section keeps Thaw as standalone SP5; roadmap sub-project table |
| 22 | Roadmap — SP-nix missing from the sequence table | this amendment | SP-nix row added to the roadmap sub-project table + deferred index |
| 23 | Roadmap — OpenClaw simultaneously dropped and documented | Wave-3d PR + this amendment (R3) | removal ruled; PR owns package/F1/docs; operator owns Todoist |
| 24 | Roadmap — Tailscale decision history contradictory | this amendment | June Homebrew-service decision marked superseded-during-execution |
| 25 | Roadmap — S12 ordering contradictory | this amendment | S12 pinned unambiguously pre-cutover |
| 26 | Cutover — replace the empty-diff gate | D1 Gate 1 | expected-delta ledger (this amendment) |
| 27 | Cutover — track the reconciliation tooling | D1 Gate 3 | durable `scripts/` reconcile script; dry-run, idempotent, tested |
| 28 | Cutover — put Phase E into the dependency graph | D1 five gates + Phase E | every Phase E item attached to a gate or PR |
| 29 | Cutover — add operational safety | D1 Gate 1 + Gate 2 | dirty-file classify / Hermes backup / inventory / second session / staged apply |
| 30 | Cutover — explicitly retire old services | D1 Gate 5 | before/after LaunchAgent inventory + managed retirement |
| 31 | Remaining slice gap — S6 | S6 fold | audit requirements added to the S6 section |
| 32 | Remaining slice gap — S7 | S7 fold (R2) | four delivery-loss fixes replace the ship-as-is text |
| 33 | Remaining slice gap — S8 | S8 fold | Darwin guard kept; `re-add --re-encrypt`; re-scope |
| 34 | Remaining slice gap — S9 | S9 fold | path/hunk matrix; S5 dependency; plist render+parse |
| 35 | Remaining slice gap — S10 | S10 fold | physical presence; `sshd -T` contract defined first |
| 36 | Remaining slice gap — S11 | S11 fold | split into the audit's 7 PRs; Thaw standalone SP5 |
| 37 | Remaining slice gap — S12 | S12 fold | pre-cutover; shared partial; render-both-targets tests |
| 38 | Deferred SP3 — Rust notifications | roadmap edit | final spec pending; open-items list refreshed (R7) |
| 39 | Deferred SP4 — Bash improvements | roadmap edit + plan deferred index | nushell NO-GO recorded; successor scope = Bash improvement |
| 40 | Deferred SP5 — Thaw | S11 + roadmap | one standalone install/manifest PR during SP2 |
| 41 | Deferred SP6 — Neovim | plan deferred index | re-check branch state; back up; import live config first |
| 42 | Deferred SP7 — cleanup backlog | plan deferred index + roadmap p-tasks | P8 unblocked; P12 already on `main`; OpenClaw closed |
| 43 | Deferred SP-nix | roadmap edit | conditional research with explicit start triggers |
| 44 | Recommended implementation order (20 steps) | plan deferred index | adopted as the authoritative SP2 sequence |

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
| S12 | CLAUDE.md comprehensive refactor | `CLAUDE.md`, `private_dot_claude/CLAUDE.md`, global AGENTS.md parity | the memory-file rewrite per the spec's CLAUDE.md section — **pre-cutover: runs last of all implementation PRs (before Phase D) so it documents the reimplemented reality** [audit 2026-07-10] | S1–S11 |

**Sequencing rationale:** S1 (docs) and S2 (the checkable foundation — CI must actually run tests before
the rest can be trusted) go first. S3–S11 are feature slices, orderable by dependency (skills before
herdr because herdr's plugins live in the store; relay before nothing; osquery last of the big three
because its diff is smallest relative to main). S12 rewrites the memory files last, against final
reality. Ship in table order unless the operator re-prioritizes. **For all post-S5 work the
authoritative sequence is the audit's 20-step "Authoritative implementation order" (amended 2026-07-10)
in the deferred sub-projects section — it supersedes this prose for everything from the PR #38 repair
onward.**

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
1. **Repeat until satisfied — strict-letter rule [audit 2026-07-10].** Steps 4–5 loop until Fable can
   identify no further mistake or gap in that task's work. **Every fix commit, however small, gets an
   independent reviewer pass** — a fresh review of the actual committed diff, not the fixer reviewing
   itself. **Conductor verification of the fixer's own evidence is never a substitute** for an
   independent reviewer looking at the diff. **Beware `A..B` commit ranges that exclude the boundary
   commit `A`** — a review scoped to `A..B` silently skips `A` itself; use `A^..B` (or review the
   explicit commit list) so no fix commit escapes review.
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

### S3 — Skills-store consolidation (COMPLETE — PR #36, merge `5f21a81`) [audit 2026-07-10]
- **Shipped model (supersedes the obsolete 21-skill framing).** The store settled into a single canonical
  `~/.agents/skills` (31 roster skills) on a four-lane provenance model — **npx-tracked** (official GitHub
  upstreams, refreshed by the npx `skills` CLI), **ClawHub-tracked** (`home-assistant`, `sql-toolkit`,
  `summarize-pro`, refreshed by the `clawhub` CLI), **vendored** (`moshi`/`herdr` forks, `elevenlabs`,
  `tiktok-crawling`, refreshed only by `chezmoi apply`), and **app-owned symlink** (`cua-driver`). The
  lock at `dot_agents/custom-skill-lock.json` records every lane plus the `tiers` (core vs on-demand),
  `hermesProfiles`/`hermesRegistry` (the disjoint two-lane hermes delivery across the five profiles),
  `npxTracked`/`clawhubTracked`, `forks`, and `superpowersRouting` tables; `test/skills-roster-fanout.sh`
  fails the build if any table, the per-harness declarations, or the settings modify-template's
  `skillOverrides` disagree. The full narrative is the repo `CLAUDE.md` "Agent Skills (cross-harness
  store)" section — the source of truth; this plan does not duplicate it.
- **Deleted framing:** the old "21 live / 12 committed / 9 Claude symlinks / 0 Hermes" counts and the
  "9 uncommitted skills to capture" list are superseded and removed — that scope was overtaken by the
  shipped model (execution learning #2).
- **Verified residual debt (Phase E / Wave-3 only).** What remains open is the three High convergence
  defects the audit found in the shipped updater — **updates can defer forever**, **fresh-machine install
  is not auto-started**, and **fan-out is additive not convergent** — owned by the **Wave-3 skills-stab
  PR** (audit PR #36); the `35922d4` skills-test scope-split (audit PR #36 Low / PR #38 #4) moves into that
  same PR from `wip/skills-test-hermetic`; and the Phase E items `fix/harness-skill-reconciliation`,
  `fix/live-reconcile-from-scratch`, and `fix/skill-architecture-diagram` remain.

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

### S5 — Tailscale headless daemon (COMPLETE — PR #38, merge `1a6e718`) [audit 2026-07-10]
- **Outcome (all four PR #38 fixes landed).** Status is now classified on `tailscale status --json`'s
  `.BackendState` (`Running`/`Starting`/`NeedsLogin`/`NeedsMachineAuth`/`Stopped`), with connection
  failure separated from state and unknown states treated as unknown, not "daemon missing" — the full
  state machine plus fake-binary tests for every state (`81e7559`, `66a5871`). **MagicDNS is RESOLVED per
  R1:** root-caused to the macOS resolver-**registration** layer (tailscaled's internal resolver stays
  healthy; the `<tailnet>.ts.net` suffix route half-registers — fails at home too, not just on foreign
  networks), the supported-fix attempt failed, and a **declarative `/etc/hosts` fallback** shipped as
  structured `tailnet_pins` data in `macos_system_setup.yaml` from which the Tier-2 runner generates
  idempotent pin commands (`4830f44`, `6560a59`, `c5614ae`, `f096ecb`, `c90a700`, `164548a`). The
  **superseded-service decision** and evergreen `CLAUDE.md` cleanup landed (`38bffb6`, `6e36512`,
  `a22ae3b`, `daef534`); the copied-daemon re-copy responsibility passes to **S6** (documented manual
  re-copy for now — no doc claim ahead of the code). The old June Tailscale spec/plan history is
  corrected in the roadmap (this amendment).
- **Resolved: `2f430b3` is NOT on main — and it does not matter for S5.** The tailscale-monitor is an
  osquery component: all four monitor files in the delta
  (`run_onchange_after_60-load-osquery-tailscale-monitor-launchagent.sh.tmpl`, its plist,
  `executable_osquery-tailscale-monitor.sh`, `test/osquery-alerter/test_tailscale.bats`) move to **S9**
  with the rest of the six-agent osquery set, carrying the `2f430b3` fix with them. A numeric-name
  match is not slice ownership (same trap as S4's `after_55-osquery` near-miss).
- **Exact S5 file set:** `.chezmoiscripts/run_onchange_after_66-tailscaled-status.sh.tmpl` (28-line
  sudo-free daemon-status reminder, no keepassxc, wholly owned); the manifest's atomic
  **cask→formula swap** (`- tailscale-app` cask removed, `+ tailscale` formula added — one pair, both
  hunks in this slice so main never holds both or neither); `treefmt.nix`'s render-lint include line
  (the after_66 script joins the `shellcheck-rendered-template` include list, per
  fix/template-render-coverage — every slice adds its own templates); CLAUDE.md's new "Tailscale
  (headless daemon)" section — with its **Updates paragraph adapted**: the weekly re-copy it describes is
  performed by `homebrew-weekly-upgrade.sh`, an **S6 file** — S5's wording documents the manual
  `sudo tailscaled install-system-daemon` re-copy after a formula upgrade, and S6 restores the
  automated-weekly wording when it ships the helper (no doc claim ahead of the code that makes it
  true).
- **Operator step:** the one-time `sudo tailscale up --accept-dns=true` + Disable Key Expiry stay
  manual (documented in the status script) — not automatable, flag in the PR body. Also flag the
  two-world note: dresden already runs the headless daemon live; a merge does not touch it (D1 does).

### S6 — Homebrew weekly-upgrade
- **Ledger fixes are load-bearing** (these bit tonight): the Homebrew 6.x `brew bundle` split (install,
  then `brew bundle cleanup --force` against a rendered temp Brewfile — `961465f`); `just brew-upgrade`
  → `~/.local/bin` copy; `SKIP_SYSTEM_PACKAGES` truthiness (`=0`/`=false` must NOT skip → `{{ if eq (env
  "SKIP_SYSTEM_PACKAGES") "1" }}`); guard the uv/npm/volta loops.
- **Wiring:** the Monday-noon plist loads; `RunAtLoad=false`.
- **Audit requirements [audit 2026-07-10]:**
  - **Depend explicitly on S5's copied-daemon model** — S6 owns the Tailscale daemon re-copy
    (`sudo /opt/homebrew/opt/tailscale/bin/tailscaled install-system-daemon`) after a formula upgrade,
    and restores the "automated weekly" `CLAUDE.md` wording S5 deferred to it.
  - **Re-copy Tailscale only when the formula binary changes** — compare the upgraded user-owned formula
    binary against the running root-owned `/usr/local/bin/tailscaled` (hash/byte) and skip the re-copy
    when unchanged.
  - **Mutual exclusion** — a lock (e.g. `flock`) so two upgrade runs cannot overlap.
  - **Continue-on-failure with an aggregate exit** — an individual failing step is logged but does not
    abort the run; the run returns a non-zero **aggregate** status if any step failed.
  - **Tests:** missing tools, partial failures, logging, loader rendering, and Tailscale-refresh failure.
  - **Split option:** if the `before_10` per-ecosystem package-runner refactoring makes S6 too large,
    split that refactor into its own PR (sizing authority = operator review speed).

### S7 — Relay notification pipeline (bash) [audit 2026-07-10 — R2]
- **[R2 — reverses the earlier "ship the bash exactly as it runs on dresden" text.]** Classify the relay
  defects into **delivery blockers** and **harmless baseline quirks**, and **fix the four delivery-loss
  defects BEFORE merging S7** — they can silently drop notifications, which is daily-critical:
  - **Fail-closed idle probe** — a missing HIDIdleTime aborts all channels instead of failing open
    (`relay.sh:68`); fail open.
  - **Whole-file `jq -rs` transcript slurp** — one half-written trailing JSONL line discards the whole
    summary (`relay-agent.sh:17`); parse line-by-line, skipping an unterminated final line.
  - **Stale directory lock** — a wedged `mkdir` lock (e.g. after SIGKILL) suppresses later notifications
    (`hue-pulse.sh`); recover from a stale lock.
  - **Missing flag value** — a value-flag as the last argument aborts parsing (`relay.sh`), breaking the
    "always exits 0" contract.
  Add **characterization tests** for any baseline quirk deliberately retained (the harmless ones). SP3
  (the Rust rewrite) still replaces the whole bash design later — but these four are fixed now so `main`
  does not carry known notification-dropping bugs. Note the delivery-blocker/quirk split in the PR body.
- ~~Delete the old `com.claude.code.plist.tmpl`~~ — **moved to S4** (2026-07-09): the plist's only
  payload execs the tmux-coupled `claude-restart.sh`, which S4 deletes, so the pair ships in S4's atomic
  cluster (keeping it here would leave main's LaunchAgent exec'ing a nonexistent file between S4 and S7).
- **Operator apply** needed (`private_auth.json.tmpl` is KeePassXC-gated).

### S8 — Hermes age-encryption
- This is the SP1 work (already committed on the working branch as `c13cc18`/`a0e7d8e`/`3696c92`) shipped
  as one clean PR. The age-tripwire fix and the fresh-machine restore script are part of it.
- **Operator step:** the `age` recipient in `.chezmoi.toml.tmpl` is the operator's public key (already
  set); the private key restore rides KeePassXC. Round-trip verify in the PR (`chezmoi cat` == live).
- **[audit 2026-07-10 — reverses the earlier §R4 "generalize the darwin guard" text.]**
  - **Keep the `{{ if eq .chezmoi.os "darwin" }}` guard** on `run_once_before_05-restore-age-key` — do
    NOT generalize it until a complete **Linux credential-source and identity design** exists. The current
    macOS paths and KeePassXC assumptions do not constitute Linux support; the multi-recipient
    laptop→home-server migration stays deferred (roadmap deferred index).
  - **Rotation uses `chezmoi re-add --re-encrypt`, not the destructive `chezmoi forget` + `add`
    sequence** — the R4 runbook below is corrected to the installed workflow.
  - **Enumerate the managed encrypted targets explicitly** in the PR (not just "`~/.hermes/config.yaml`").
  - **Rehearse rotation in a scratch source and destination** before touching live secrets.
  - **Re-scope S8 up front** — encrypted **per-profile Hermes configs**
    (`dot_hermes/profiles/*/encrypted_config.yaml.age`) and **codegraph Hermes-MCP state** materially
    expand the slice, so Phase E `fix/hermes-encrypted-profile-configs` rides here; **round-trip test each
    captured profile independently**.
  - Ship `docs/runbooks/age-key.md` (rotation + disaster-recovery) and the `test/age-restore.sh` DR drill;
    KeePassXC entry name is `chezmoi :: Private Key :: age` (spec corrected 2026-07-04).

### S9 — osquery three-tier alerting
- **Smallest big slice** — most of osquery is already on main; carry only the PR#25 delta.
- **In scope:** alerting/dispatch design improvements. **Sign-off gate:** any `.chezmoitemplates/osquery/
  *.conf` query/pack content change is listed in the PR body for explicit user approval before merge.
- **Wiring:** all 6 LaunchAgents + loaders; the 87-bat suite green; the pipeline manifest baseline.
- **Audit requirements [audit 2026-07-10]:**
  - **Build an exact path-and-hunk matrix before implementation** — the real PR #25 delta against the
    converged `main`, not the early file list.
  - **S5 dependency:** the Tailscale monitor moved into S9 (the four monitor files carrying `2f430b3`) —
    S5's re-scope already recorded this; S9 depends on S5's settled model.
  - **Render and parse every plist**; **test every loader label and path**.
  - **Split** dispatch / results-alerter / the six pollers+loaders / pack changes into separate PRs if the
    real diff is not quickly reviewable (the sizing fallback below).
  - **Sign-off gate unchanged:** every `.chezmoitemplates/osquery/*.conf` query/pack content change stays
    behind explicit operator sign-off.
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
  `KbdInteractiveAuthentication no` so password auth is actually closed — the **full accepted effective
  config is defined in the audit bullet below** (the undefined "address UsePAM's interaction" wording is
  removed and replaced), test-driven per the sshd `-T` effective-config seam.
- **SSH hardening — audit requirements [audit 2026-07-10] (perform ONLY while physically present):**
  - **Define the exact accepted `sshd -T` effective config BEFORE implementation** — the full set of
    keys/values that constitutes "password auth is closed" (at minimum `passwordauthentication no`,
    `kbdinteractiveauthentication no`, and the chosen `usepam` value with its interaction spelled out).
    This **replaces** the undefined "address UsePAM's interaction" requirement — the accepted config is a
    concrete effective-output contract, not a to-do.
  - **Validate syntax and effective config before reload** (`sshd -t`, then diff `sshd -T` against the
    accepted set).
  - **Keep the existing session open**, **prove a new key-only session works**, and **test rollback**
    before closing the original session.
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
- **Audit requirements [audit 2026-07-10] — split S11 into the audit's 7 small PRs:**
  1. Shell and brew-cache work.
  2. Secret permission changes.
  3. Git hygiene.
  4. Desktop and hotkey cleanup.
  5. Log rotation.
  6. Fork maintenance.
  7. Plugin installation.
  **Thaw stays a standalone SP5 PR** (not folded into S11). **OpenClaw is already ruled (R3):** the
  Wave-3d OpenClaw-cleanup PR owns the `openclaw` package removal, the AeroSpace F1 binding, and the docs
  together, and the operator owns the Todoist cleanup — OpenClaw is NOT an S11 chore.

### S12 — CLAUDE.md comprehensive refactor (pre-cutover) [audit 2026-07-10]
- Runs last. Per the spec's CLAUDE.md section: global file → minimal (preferences + bias-correction +
  toolchain + gates only, no operational detail, no dead skill references); repo file → identity +
  commands + architecture map + conventions, conditional deep-dives extracted to `docs/runbooks/` or
  skills; **every factual claim re-verified against the live repo at write time**; global AGENTS.md
  parity added. Fold in the verified staleness fixes (haiku→sonnet hook, pre-bats Testing section, wrong
  source-dir description, tmux/yabai remnants, single-template shellcheck claim).
- **Audit requirements [audit 2026-07-10]:** S12 is **unambiguously pre-cutover** — it runs after ALL
  implementation PRs (S6–S11 + the Wave-3 stabilization PRs) but BEFORE Phase D cutover, so `main`
  documents the reimplemented reality and the cutover applies converged instruction files. Build the
  **shared Claude + Codex rules partial** (one `.chezmoitemplates` partial included by both
  `~/.claude/CLAUDE.md` and `~/.codex/AGENTS.md`); **render both global targets in tests** and
  **byte-compare the shared block** across them; **re-verify every command and path against the converged
  `main`** at write time; **move conditional operational detail into runbooks** rather than the
  always-loaded instruction files (Phase E `fix/codex-agents-parity` rides here).

---

## Phase D — Cutover (five gates) [audit 2026-07-10]

### Task D1: Switch main live, verify, close the reference PRs — five sequential gates

The audit split the single cutover step into **five gates** so that Phase E items which only complete
*after* apply have a named home, and so the reference PRs are not closed before the soak proves
convergence. Each gate must pass before the next begins. **`$INTEGRATION_PR` (set in A1) is PR #32**, the
DO-NOT-MERGE reference; the source PRs are **#25** (osquery three-tier) and **#31** (herdr/Tailscale/
brew/moshi).

#### Gate 1 — Preflight (before switching the live source)

- [ ] **Classify every dirty/untracked primary-worktree file** — for each, decide keep / discard /
  back-up. Nothing ambiguous crosses into the apply.
- [ ] **Back up uncaptured Hermes profile state** per the backup convention
  (`~/workspaces/backups/YYYY-MM-DDTHH-MM-SS.<name>.backup[.ext]`) — the per-profile `config.yaml`
  enablement/`platform_toolsets` and codegraph MCP state are otherwise untracked encrypted `.age` files
  (Phase E `fix/hermes-encrypted-profile-configs`).
- [ ] **Inventory current LaunchAgents and services** — capture the before-state for the Gate 5
  before/after diff (`launchctl list`, the `com.webdavis.*` set, the osquery jobs).
- [ ] **Expected-delta ledger — REPLACES the old empty-diff gate.** The old gate was contradictory: the
  plan permits improvements over the integration branch yet also demanded an empty final diff. Instead,
  classify **every** reference-branch hunk (`git diff main integration/modernization`) as one of:
  **landed-unchanged**, **intentionally-improved**, **deliberately-omitted-with-reason**, or **missing**.
  **Only a `missing` hunk blocks cutover** — the other three are expected and recorded in the ledger.

#### Gate 2 — Staged activation

- [ ] Open a **second remote session** first, so a broken apply cannot lock you out.
- [ ] After S12 merges, point dresden's chezmoi source at `main`
  (`git -C ~/workspaces/Ivy/webdavis/dotfiles checkout main && git pull`).
- [ ] **Operator** runs a full interactive `chezmoi apply` (KeePassXC unlocked) **in stages**, not one
  shot — keep the integration branch and previously deployed files available for rollback.
- [ ] **Verify remote reachability** (Tailscale / SSH) before ending the original session.

#### Gate 3 — Tracked live reconciliation

- [ ] Run the durable reconciliation script — **tracked under `scripts/` (or a chezmoi-managed
  executable), NOT `.superpowers/` scratch** (`.superpowers/` is gitignored; the old
  `.superpowers/sdd/live-reconcile-skills.sh` was never tracked). It must support **`--dry-run`**, be
  **idempotent**, and have **tests**. Run `--dry-run` first, then live, to prove a from-scratch machine
  converges identically (Phase E `fix/live-reconcile-from-scratch`).
- [ ] `just test` green + live smoke checks: `relay.sh` fires a test notification; `hermes gateway`
  healthy; osquery alerter behaves (`osquery-heartbeat.sh` sends its one ✅);
  `chezmoi diff --exclude=templates` clean.

#### Gate 4 — Soak

- [ ] Let the converged `main` run for a soak window; watch the daily-critical paths (notifications,
  hermes, osquery, shell startup) for regressions. **Do not close any reference PR during the soak.**

#### Gate 5 — Final closure

- [ ] **Retire services whose source files were deleted** — a **before/after LaunchAgent inventory**
  (against Gate 1's capture) and managed `launchctl bootout` / plist removal for anything orphaned (e.g.
  the old Claude `com.claude.code.plist`). Drop the Phase E `graphify-out/` band-aid excludes here.
- [ ] Close **PR #25**, **PR #31**, and the **integration reference PR #32** via `gh-axi pr close` —
  **only after the soak passes** — each with a comment linking the slice PRs that superseded it.

**Phase E → gate attachment.** Every Phase E item completes at a named home:
`fix/live-reconcile-from-scratch` → Gate 3; `fix/graphify-out-excludes-drop` → Gate 5;
`fix/harness-skill-reconciliation` → Gate 3 (Hermes-side pruning, coordinated at cutover);
`fix/hermes-encrypted-profile-configs` → S8 (backed up at Gate 1); `fix/codex-agents-parity` → S12;
`fix/template-render-coverage` → the Wave-3 render-coverage PR; `fix/moshi-herdr-drift-check` and
`fix/pre-commit-path-filter` → S11; `fix/skill-architecture-diagram` → Wave-3 skills-stab / S12 docs.

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
1. Re-encrypt every managed secret to the new recipient with the installed non-destructive workflow:
   `chezmoi re-add --re-encrypt` (re-encrypts the existing `encrypted_*` files under the new recipient —
   **corrected 2026-07-10**, replacing the destructive `chezmoi forget` + `chezmoi add --encrypt` pair the
   audit flagged). Enumerate the encrypted targets explicitly rather than assuming a single file: today
   `~/.hermes/config.yaml`, plus the re-scoped per-profile Hermes configs
   (`dot_hermes/profiles/*/encrypted_config.yaml.age`) and codegraph state as those land in S8.
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

- **SP3 — Notification rewrite (Rust): behavior contract approved; final implementation spec PENDING.**
  Not "fully designed" [audit 2026-07-10]. The roadmap spec (amended 2026-07-10) carries SP3's status and
  open-items list — event input schemas, per-harness event mapping, native-push ownership (R7),
  per-channel retry/failure, presence thresholds, lights quiet hours, migration coexistence/rollback, and
  acceptance boundaries. SP3 is a **stateless per-event executable, not a daemon/service**. Sequenced
  after SP2 cutover per the authoritative order below.
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
  starting earlier would recreate the two-writer hunk problem. **Sequencing decided (conductor's call,
  operator delegated 2026-07-09): after SP3** — SP3 replaces the live relay pipeline whose shipped bugs
  can drop notifications (daily-critical beats quality-of-life), and SP4's bashrc work then lands atop
  SP3's finished notifier shim instead of underneath it. Five workstreams: (1) consolidate every alias out of bashrc into `.bash_aliases`; (2) `dot_fzf_bindings`
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
- **SP-nix — nix-darwin go/no-go** (research-first sibling of SP4, banked in §R6). **Do not start it
  merely because it appears in the roadmap** [audit 2026-07-10] — start only after one of these triggers:
  a larger Mac fleet; a material maintenance failure in the current `defaults` system; or a proven design
  that preserves the current single-apply and secrets model.

### Authoritative implementation order (audit 2026-07-10 — supersedes the older per-slice sequencing prose)

The audit's recommended order is adopted as the authoritative SP2 sequence for everything from the PR #38
repair onward; it supersedes the "ship in table order" / "Sequencing rationale" prose for post-S5 work.
Completed items (S1–S5, steps 1–2) are struck; the rest are the standing plan of record. Cutover steps
map to the D1 gates below.

1. ~~Repair PR #38 without changing its copied-daemon architecture.~~ (done — merged as #38)
1. ~~Resolve or explicitly accept the MagicDNS failure.~~ (done — R1: declarative `/etc/hosts` fallback)
1. **Amend the roadmap and SP2 plan.** ← this amendment
1. Land the skills stabilization PR.
1. Land the herdr stabilization PR.
1. Land rendered-template coverage and documentation fixes.
1. Implement S6 against the settled Tailscale model.
1. Resolve the S7 delivery-defect policy.
1. Resolve S8's Linux and encrypted-profile boundary.
1. Implement S7 and the re-scoped S8.
1. Re-scope and split S9.
1. Implement S10 during a physical-presence window.
1. Split S11 and ship SP5 (Thaw) separately.
1. Complete and mechanically verify S12.
1. Run cutover preflight and expected-delta reconciliation (D1 Gate 1).
1. Activate `main` in stages (D1 Gate 2).
1. Run tracked live reconciliation (D1 Gate 3).
1. Soak, then final closure (D1 Gate 4 → Gate 5).
1. Continue with SP3, SP4, SP6, then SP7.
1. Start SP-nix only if its trigger occurs.

## Phase E — End-of-SP2 cleanup backlog

Debts discovered during execution (chiefly S3). Each is deferred for a stated reason; all must be
resolved before SP2 closes. Labelled `fix/<name>` for tracking; a `(→ Sn)` tag means it rides that
slice. **[audit 2026-07-10] Every item is attached to a named owner — a Wave-3 stabilization PR or a D1
cutover gate — in the "Phase E → gate attachment" map at the end of Task D1; the per-item owner notes
below match that map (nothing floats unattached).**

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

**Concretized by the audit [audit 2026-07-10] — owned by the Wave-3 render-coverage PR.** The audit's
manual sweep found **four hidden failures** across the ~20 shell-script templates the treefmt include
list omitted: an **unquoted `$HOME` in the system-setup render**, and **three osquery loaders whose
shebang renders on line two**. The required correction is now concrete: (1) **discover all safely
renderable shell templates programmatically** (not an enumerated allowlist); (2) **fix the four current
failures**; (3) **add a coverage test that fails when a new shell template is omitted** from render-lint.
This supersedes the "decide in S11" hedge — it lands in the Wave-3 render-coverage PR (audit PR #35
Medium).

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
