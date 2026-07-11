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

`main` (`1a6e718`) carries Phase A and S1–S5. The **Wave-3 stabilizations** (skills-stab, herdr-stab,
render-coverage) are next — they precede S6 per the authoritative implementation order; S6 does not start
until they merge. Merge SHAs and PR numbers below are verified against `git log origin/main --merges`.

| Item | State | PR | Merge commit | Remaining follow-up |
| --- | --- | --- | --- | --- |
| Phase A — integration reference | complete | DO-NOT-MERGE reference | — | closed at cutover Gate 5, not merged |
| S1 — Docs | complete | #33 | `1ef7c29` | — |
| S2 — Lint/test/CI | complete | #35 | `90c68c4` | rendered-template coverage regression → **render-coverage** PR |
| S3 — Skills | complete | #36 | `5f21a81` | 3 High defects + the `35922d4` scope-split → **skills-stab** PR |
| S4 — herdr | complete | #37 | `addc8d7` | 2 High + 2 Medium defects → **herdr-stab** PR |
| S5 — Tailscale | complete | #38 | `1a6e718` | copied-daemon re-copy responsibility folds into S6; monitor moved to S9 |
| Wave-3a skills-stab | **next** | — | — | audit PR #36 High×3 (defer-forever / fresh-install / additive-fan-out) + Low `35922d4` move |
| Wave-3b herdr-stab | **next** | — | — | audit PR #37 High×2 (atomic migration / Cargo+registration) + Medium×2 (`Cargo.toml` hash / LaunchAgent unload); operative acceptance in the Wave-3b herdr-stab section |
| Wave-3c render-coverage + docs | **next** | — | — | audit PR #35 Medium (4 template failures + coverage test) + PR #35 Low doc-staleness |
| Wave-3d OpenClaw cleanup | in PR (before S12) | - | - | R3 delivered on the branch: repo removal (`openclaw` package + AeroSpace F1 binding + active config/doc refs) + a `run_after` retirement script (boots out the three `ai.openclaw.*` agents, deletes their plists, uninstalls the npm package, gated on a quiescence marker) + docs-truthfulness sweep. PENDING merge and a live interactive `chezmoi apply`; the run_after has not executed on any host yet, so the live gateway retry-loop persists until that apply. Operator owns Todoist |
| S6 — Homebrew weekly-upgrade | queued (after Wave-3) | — | — | audit S6 gaps folded into the S6 section |
| S7 — Relay pipeline | not started | — | — | R2: four delivery-loss fixes before merge (see S7 section) |
| S8 — Hermes age-encryption | not started | — | — | Linux-boundary + re-scope folded into the S8 section |
| S9 — osquery three-tier | not started | — | — | S5 dependency (`2f430b3` monitor) + path/hunk matrix folded into S9 |
| S10 — macOS defaults / SSH | not started | — | — | physical-presence window + `sshd -T` contract folded into S10 |
| S11 — long-tail chores | not started | — | — | split into the audit's 7 PRs (Thaw = SP5) |
| S12 — global instructions | not started | — | — | unambiguously pre-cutover (see S12 section) |
| Phase D — cutover | not started | — | — | five gates (D1 rewrite below) |
| Phase E — cleanup backlog | ongoing | — | — | every item attached to a pre-cutover PR or a D1 gate (`fix/graphify-out-excludes-drop` moved out of Phase E to the SP7 backlog — post-cutover by design) |

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
| 27 | Cutover — track the reconciliation tooling | cutover-tooling PR (builds them) + D1 gates (run them) | `scripts/cutover-gate.sh` (gate runner owning EVERY cutover command, 19-item binding acceptance checklist) + `scripts/live-reconcile.sh` (dry-run, idempotent, tested) — authored + merged before S12; Phase D states invariants only |
| 28 | Cutover — put Phase E into the dependency graph | D1 five gates + Phase E | every Phase E item attached to a gate or a pre-cutover PR; the post-cutover graphify excludes-drop moved out of Phase E to the SP7 backlog |
| 29 | Cutover — add operational safety | D1 Gate 1 + Gate 2 | dirty-file classify to an empty tree / Hermes backup / retirement manifest / second session / pin re-verify + attached staged apply |
| 30 | Cutover — explicitly retire old services | D1 Gate 1 (approve) + Gate 2 (retire) + Gate 3 (verify) | operator-approved retirement manifest (desired-state vs live jobs — launchctl diffs can't find orphans); executed during staged activation; manifest-asserted verification; Gate 4 soaks the retired final topology; Gate 5 closes the reference PRs |
| 31 | Remaining slice gap — S6 | S6 fold | audit requirements added to the S6 section |
| 32 | Remaining slice gap — S7 | S7 fold (R2) | four delivery-loss fixes replace the ship-as-is text |
| 33 | Remaining slice gap — S8 | S8 fold | Darwin guard kept; `re-add --re-encrypt`; re-scope |
| 34 | Remaining slice gap — S9 | S9 fold | path/hunk matrix; S5 dependency; plist render+parse |
| 35 | Remaining slice gap — S10 | S10 fold | physical presence; `sshd -T` contract defined first |
| 36 | Remaining slice gap — S11 | S11 fold | split into the audit's 7 PRs; Thaw standalone SP5 |
| 37 | Remaining slice gap — S12 | S12 fold | pre-cutover; shared partial; render-both-targets tests |
| 38 | Deferred SP3 — Rust notifications | roadmap edit | final spec pending; open-items list refreshed (R7) |
| 39 | Deferred SP4 — Bash improvements | roadmap edit + plan deferred index | nushell NO-GO recorded; successor scope = Bash improvement |
| 40 | Deferred SP5 — Thaw | standalone SP5 PR + roadmap | one standalone install/manifest PR during SP2 — NOT folded into S11 |
| 41 | Deferred SP6 — Neovim | plan deferred index + roadmap edit | the audit's five directives added to both SP6 bullets: re-check branch state (69 behind / 3 ahead at audit), back up both repos, inventory live config, import unchanged first, modernize via later PRs |
| 42 | Deferred SP7 — cleanup backlog | plan deferred index + roadmap p-tasks | deduplicate the ledger into tracked tasks (status/severity/dependencies); P8 unblocked; P12 already on `main`; OpenClaw closed per R3 |
| 43 | Deferred SP-nix | roadmap edit | conditional research with explicit start triggers |
| 44 | Recommended implementation order | plan deferred index | adopted (and extended: Wave-3d + the cutover-tooling PR) as the single authoritative SP2 sequence |

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
- **Records — the immutable SP2-start base SHA:
  `2bd973369158b49535e8e16e80c968444ab23f1d`** — the `main` commit the union diverged from (verified
  2026-07-10: `git merge-base origin/main origin/integration/modernization`; stable no matter how far
  `main` later advances, because the integration branch stays frozen). The Phase D Gate 1 expected-delta
  ledger uses this recorded value as its manifest base — it is never re-derived from a moving `main` at
  cutover time. (The A1-time union was 196 files; the frozen branch takes hotfixes, so the count drifts —
  220 at this writing — which is why Gate 1 regenerates the manifest from the pinned SHAs rather than
  trusting any recorded file count.)

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

Every path in `git diff --name-status main...HEAD` (196 files at A1 time; freeze-policy hotfixes drift
the count — 220 at this writing, regenerate rather than trust it) is assigned to exactly one slice below.
Eight **shared infra files** are touched by several slices; each slice carries **only its own hunks** of
those files (procedure in the Slice Protocol). No file is orphaned.

**Shared infra files** (never a slice of their own — hunks distributed to the owning slice):
`.chezmoi.toml.tmpl`, `.chezmoiignore`, `.gitignore`, `CLAUDE.md`, `private_dot_claude/CLAUDE.md`,
`dot_bashrc.tmpl`, `dot_profile`, `justfile`.

**Build the hunk-ownership table first (Phase B, step 1 — per §R1).** Before any slicing, walk
`git diff main integration/modernization -- <file>` for each of the 8 shared files and record which
slice(s) own which hunks in a table (shared-file × owning-slice × one-line-what). This is deferred to
execution on purpose — it must be computed against the *live* diff — but it is the first Phase B action,
not an afterthought; the Phase D expected-delta ledger (Gate 1) verifies every hunk landed exactly once
or is a documented intentional-improvement/omission.

**Sizing authority.** This table's grouping is a starting point, not a size guarantee. **The operator's
review speed is the authority** — any slice whose real diff is too large to review quickly sub-splits on
the spot (S4 and S9 carry pre-noted splits; S8 is small — the clean SP1 work). No PR should exceed a
quick review.

**Table status [2026-07-10]:** this is the *planning-time* assignment; rows of completed slices are
historical, and wherever a row disagrees with a shipped model or a later ruling, the operative per-slice
sections and their `[audit 2026-07-10]` annotations govern — not this table.

| Slice | Feature | File groups (from the bucketed delta) | Ledger fixes folded in | Dep |
| --- | --- | --- | --- | --- |
| S1 | Docs | `docs/**` (20: the herdr/tailscale/brew/relay/notifications specs+plans, the modernization brief, this plan, the never-sleep policy), `AGENTS.md` (new symlink→CLAUDE.md) | — | none |
| S2 | Lint/test/CI hardening | `scripts/lint.sh`, `.githooks/pre-commit`, `.github/workflows/lint.yml`, `.editorconfig`/`.shellcheckrc`/`.mdformat.toml` hunks | CI runs tests + `LINT_CHECK=1`; wire **actionlint** + **zizmor** (P9); **SHA-pin** actions (installer has no tags — see research §Actions); `lint.sh` runner-selection subshell bug; `-r` optstring crash; template shellcheck allowlist → programmatic; `find` prune set dedup; bats `grep -c` zero-count false-pass | S1 |
| S3 | Skills-store consolidation | `dot_local/bin/executable_update-skills.sh`, `dot_agents/skills/**`, `private_dot_claude/skills/symlink_*`, `skills-lock.json` *(historical — shipped as `dot_agents/custom-skill-lock.json`)*, delete `private_dot_claude/skills/web-research-task/**` | update-skills **loader script + `~/.local/log/skills` dir**; declare all store symlinks; remove stale `.agents/skills/moshi-best-practices/`; single symlink-owner | S2 |
| S4 | herdr migration | `dot_config/herdr/**`, `dot_local/share/herdr/plugins/**` (2 Rust plugins), the `run_onchange_after_55/57` build scripts, herdr hunks of `dot_bashrc.tmpl`; **atomically deletes** `dot_tmux.conf`, `dot_config/sesh/**`, `dot_local/bin/executable_{sesh-*,tmux-*,claude-restart}.sh`, `run_after_70-install-tmux2k-last-proc` | herdr plugin build scripts → `.chezmoitemplates` partial; `grep -q "$plugin_id"` anchoring; `dot_fzf_bindings` tmux-dead widgets; `nvm`/`$blue` binding fixes | S2 |
| S5 | Tailscale headless daemon | `run_onchange_after_66-tailscaled-status.sh.tmpl`, tailscale hunks of `system_packages_autoinstall.yaml` + `CLAUDE.md` | tailscale-monitor fix `2f430b3` — resolved: NOT on main; rides with the monitor files in S9 (see the S5 section) | S2 |
| S6 | Homebrew weekly-upgrade | `dot_local/bin/executable_homebrew-weekly-upgrade.sh`, `Library/LaunchAgents/com.webdavis.homebrew-weekly-upgrade.plist.tmpl`, `run_onchange_after_65` loader, `test/homebrew-weekly-upgrade.sh` | `just brew-upgrade` → deployed copy; **Homebrew 6.x bundle `cleanup --force`** (`961465f`); `SKIP_SYSTEM_PACKAGES=0`-still-skips; before_10 per-ecosystem split; uv/npm/volta unguarded loops | S2 |
| S7 | Relay notification pipeline (bash) *("as-deployed" superseded by R2 — delivery-loss defects are fixed before merge)* | `dot_local/bin/executable_{relay,relay-agent,relay-codex-hooks,hue-pulse,claude-stop-pulse,claude-user-prompt-start,claude-audit}.sh`, `private_dot_claude/modify_settings.json`, `dot_config/relay/private_auth.json.tmpl`, `run_after_72-relay-codex-hooks`, notifier hunk of `dot_bashrc.tmpl` *(the `com.claude.code.plist.tmpl` deletion moved to S4's atomic cluster — not S7's)* | **R2 (2026-07-10):** fix the four delivery-loss defects BEFORE merge (fail-closed idle probe, jq slurp, mkdir lock, missing flag value); characterization tests for retained harmless quirks; SP3 still replaces the whole design later | S2 |
| S8 | Hermes age-encryption (SP1) | `dot_hermes/encrypted_private_config.yaml.age`, `dot_hermes/private_dot_env.tmpl`, `.chezmoi.toml.tmpl` age hunk, `run_onchange_before_25`, `run_after_67`, `run_after_68`, `run_once_before_05-restore-age-key`, `test/hermes-config-{encrypted,routes}.sh`, `.gitignore` failsafe hunk (gitleaks gate hunk of `.githooks/pre-commit` ships in S2, not here) **+ the expanded capture set** — the four existing untracked per-profile captures and codegraph state (see the S8 section) | this is the committed SP1 work (`c13cc18`/`a0e7d8e`/`3696c92`) reimplemented as one clean PR; the age-tripwire fix is already in it | S2 |
| S9 | osquery three-tier alerting | `.chezmoitemplates/osquery/**` (config+4 packs), `dot_local/bin/executable_osquery-*`, the 6 osquery LaunchAgents + `after_60` loaders, `after_55` manifest, `before_50` setup, `test/osquery-alerter/**` | **alerting/dispatch redesign in scope**; heartbeat `RunAtLoad` double-ping; **query/pack content changes → flag for sign-off**. NOTE: much of osquery is already on main — this slice is the PR#25 *delta* only | S2 |
| S10 | macOS defaults / system-setup | `.chezmoidata/macos_defaults.yaml`, `.chezmoidata/macos_system_setup.yaml`, `run_onchange_after_30/41`, `dot_local/bin/executable_macos-defaults-*.sh` | defaults trio hardcoded-path + shared-lib consolidation; `after_41` fragile `{{ if .sudo }}`; `ssh-hardening.sh` → a `macos_system_setup.yaml` record | S2 |
| S11 | Shell foundation + secrets hygiene + chores | remaining hunks of `dot_bashrc.tmpl`/`dot_profile`/`justfile`/`.chezmoiignore`, `run_after_44-cache-brew-shellenv` + `test/brew-shellenv-cache-drift.sh`, `dot_aws/private_credentials.tmpl` + `dot_config/himalaya/private_config.toml.tmpl` renames, `dot_config/worktrunk/config.toml`, gitconfig fixes; **installs:** ponytail (Thaw is a **standalone SP5** PR, not an S11 install) | credential `private_` renames (`ae02524`); merge.tool name; `core.excludesfile`; git:// url removal; `~/.bash_just_completions`; atuin `~/.atuin/bin/env` guard; yabai ignore; espanso `_pqi.yml` + shadow triggers; Arc→Zen hotkey; log rotation (newsyslog) | S2 |
| S12 | CLAUDE.md comprehensive refactor | `CLAUDE.md`, `private_dot_claude/CLAUDE.md`, global AGENTS.md parity | the memory-file rewrite per the spec's CLAUDE.md section — **pre-cutover: runs last of all implementation PRs (before Phase D) so it documents the reimplemented reality** [audit 2026-07-10] | S1–S11 |

**Sequencing rationale:** S1 (docs) and S2 (the checkable foundation — CI must actually run tests before
the rest can be trusted) go first. S3–S11 are feature slices, orderable by dependency (skills before
herdr because herdr's plugins live in the store; relay before nothing; osquery last of the big three
because its diff is smallest relative to main). S12 rewrites the memory files last, against final
reality. Ship in table order unless the operator re-prioritizes. **For all post-S5 work the
authoritative sequence is the single "Authoritative implementation order" (amended 2026-07-10) in the
deferred sub-projects section — it supersedes this prose for everything from the PR #38 repair onward.**

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
1. **End-of-section sweep — DUAL-PROVIDER (operator-directed 2026-07-10).** After the section's tasks
   are all individually clean, the whole slice diff gets TWO independent reviews in parallel: (a) a
   fresh-context Fable reviewer, and (b) a **Codex review (gpt-5.6 sol, ultra effort)** via the codex
   plugin — cross-provider diversity de-correlates reviewer blind spots (the 2026-07-10 external audit
   proved this in-repo: it caught defects four same-model review cycles had missed). Fable, as
   conductor, merges and dedups the two findings sets, adjudicates disagreements with evidence, and
   routes plan-conflicting findings to the operator. ONE fix wave addresses the union; BOTH reviewers
   re-review until both return CLEAN. Per-fix-commit strict-letter re-reviews (step 6) stay Fable-only
   for speed — sol re-joins when a fix wave is large or judgment-heavy. Only after both reviewers are
   CLEAN does Fable decide the section is PR-ready and hand it to the operator review gate (P-8).

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
  `npxTracked`/`clawhubTracked`, `forks`, and `superpowersRouting` tables. `test/skills-roster-fanout.sh`
  validates exactly **five** of these — `tiers`, `hermesProfiles`, `hermesRegistry`, `npxTracked`,
  `clawhubTracked` — against the store, the per-harness declarations, and the settings modify-template's
  `skillOverrides`, failing the build if any disagree. It does **NOT** read `forks` or
  `superpowersRouting` (verified 2026-07-10), so those two tables' drift is NOT test-covered — `forks` is
  the weekly drift-watch's job and `superpowersRouting` is re-asserted by
  `assert-hermes-superpowers-routing.sh`. The full narrative is the repo `CLAUDE.md` "Agent Skills
  (cross-harness store)" section — the source of truth; this plan does not duplicate it.
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
- **Atomicity is the invariant — but scoped to REPO state.** The tmux/sesh deletions and the herdr
  additions ship in one PR — `main` must never have both, nor neither. This is *source-tree* atomicity (a
  fresh `chezmoi apply` from `main` is always internally consistent). It is NOT the same as
  *live-migration ordering safety* on dresden (install and prove herdr BEFORE tmux/sesh are torn down at
  apply time). That ordering safety is a separate, now-operative requirement owned by the **Wave-3b
  herdr-stab** section below — same-PR scope alone does not provide it.
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

### Wave-3b herdr-stab (operative acceptance) [audit 2026-07-10]

S4 shipped the repo-state migration (PR #37); the audit found two High + two Medium defects a follow-up
**herdr-stab PR** must close. These requirements are **binding acceptance** — the PR is not done until
every one is met and test-driven. This is where S4's *live-migration ordering safety* (distinct from its
repo-state atomicity, above) becomes operative.

- **Install-and-verify-before-teardown (the live-migration ordering invariant).** At live-migration time
  herdr must be **installed AND verified** — binary present and runnable, `config.toml` valid, both Rust
  plugins built and registered, and a **second herdr session proven** to spawn and attach — BEFORE
  tmux/sesh are removed. This is the ordering safety S4's repo-state atomicity does NOT provide.
- **Rollback.** Document the exact rollback path: how to restore tmux/sesh from the pre-migration state if
  herdr verification fails, so a failed cutover never strands the machine without a working multiplexer.
- **Plugin-registration error envelope + retry-state retention — and missing Cargo is NON-success.**
  The build/register scripts (`run_onchange_after_55/57`) must treat a failed or skipped plugin
  registration as an **error envelope**, not silent success — and **the `run_onchange` trigger must NOT
  be consumed by a skipped/failed registration OR by a missing-Cargo skip** (a consumed trigger makes
  the next apply believe the work is done and never retry). Retain the retry state until BOTH plugins
  build AND register. **NOTE — this REVERSES a currently-enforced contract:**
  `test/herdr-build-scripts-resilience.sh` today asserts the build partial **exits 0 with a hint when
  Cargo is absent everywhere** ("never aborts"); herdr-stab rewrites that expectation — missing Cargo
  becomes a retryable non-success, not a satisfied trigger. **Separate build from registration** (two
  steps, each with its own failure envelope), and after linking, **verify the exact plugin** — query the
  specific plugin id just registered, not a substring/any-plugin check. (Audit PR #37 High: failed
  registration treated as success.)
- **Hash EVERY build input.** The rebuild-decision hash must cover **every** input to the Rust build —
  including `Cargo.toml` (and `Cargo.lock`), not only the `.rs` sources — so a dependency/manifest change
  forces a rebuild. (Audit PR #37 Medium: the hash omitted `Cargo.toml`.)
- **Old Claude LaunchAgent retirement.** Removing the `com.claude.code.plist` source does NOT unload the
  already-running LaunchAgent. Ship a **one-time idempotent retirement script** (`launchctl bootout`,
  guarded to no-op when already gone) with **stubbed tests** (mirroring the atuin/happy loader test
  pattern). This is the live-side complement to S4's deletion of the plist source. (Audit PR #37 Medium.)
- **Tests (TDD, per Global Constraints):** registration success/failure envelope; trigger-not-consumed on
  failure; **missing-Cargo leaves the trigger retryable** (red-first — this test REVERSES the current
  exit-0-on-missing-cargo assertion in `test/herdr-build-scripts-resilience.sh`, which is updated in the
  same PR); hash-includes-`Cargo.toml`; retirement-script idempotence — each red-first.

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
  osquery component that moves to **S9** with the rest of the six-agent set. The `2f430b3` commit itself
  touches exactly **three** files (verified `git show 2f430b3`):
  `dot_local/bin/executable_osquery-tailscale-monitor.sh`, `test/osquery-alerter/lib.bash`, and
  `test/osquery-alerter/test_tailscale.bats` — **no loader, no plist**. Its loader
  (`run_onchange_after_60-load-osquery-tailscale-monitor-launchagent.sh.tmpl`) and plist enter S9 as
  **transplant dependencies through S9's path/hunk matrix**, NOT as part of this commit. Carrying the fix
  therefore means carrying all three commit files — crucially `lib.bash`, whose helper changes the
  regression coverage needs (a loader+plist-only transplant would drop them). A numeric-name match is not
  slice ownership (same trap as S4's `after_55-osquery` near-miss).
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
  - **Re-scope S8 up front — the four per-profile captures already EXIST, untracked.** Verified on the
    primary checkout's filesystem 2026-07-10: the integration branch *tracks* only the **root** config
    (`dot_hermes/encrypted_private_config.yaml.age`), but four per-profile encrypted captures already
    sit **untracked** in the primary checkout at
    `dot_hermes/profiles/private_<profile>/encrypted_private_config.yaml.age`
    (`private_butters`, `private_concerned`, `private_elaine`, `private_nicodemus` — default/Bob is the
    root). S8 therefore: **(1) inventory, hash, and back up** those four existing untracked sources
    (backup convention) before touching anything; **(2) record their verified paths** in the PR;
    **(3) decide explicitly, per profile, between COMMITTING the existing capture as-is and intentionally
    RECAPTURING newer live state** (`chezmoi add --encrypt` against the live profile config) — never
    silently overwrite or duplicate an existing capture. **Codegraph Hermes-MCP state** is still
    uncaptured (its source name follows chezmoi naming at capture time). This materially expands the
    slice, so Phase E `fix/hermes-encrypted-profile-configs` rides here; **round-trip test each captured
    profile independently**.
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
  - **S5 dependency:** the Tailscale monitor moved into S9 — S5's re-scope recorded this. Commit
    `2f430b3` carries exactly three files
    (`dot_local/bin/executable_osquery-tailscale-monitor.sh`, `test/osquery-alerter/lib.bash`,
    `test/osquery-alerter/test_tailscale.bats` — **not** the loader or plist); the loader + plist join S9
    through the path/hunk matrix as transplant dependencies. S9 depends on S5's settled model.
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
  live per the roadmap ledger) — the hardening record produces the **complete accepted `sshd -T`
  effective config defined in the audit bullet below** (no "decide later" hedge), test-driven per the
  sshd `-T` effective-config seam.
- **SSH hardening — audit requirements [audit 2026-07-10] (perform ONLY while physically present):**
  - **The accepted `sshd -T` effective config — defined NOW (no "at minimum", no "decide/define later"):**
    - `passwordauthentication no`
    - `kbdinteractiveauthentication no`
    - `usepam yes` — macOS **requires** `UsePAM yes` for account and session management (login records,
      sandbox/session setup); turning it off breaks session setup on macOS. It is safe here **because
      both password paths above are `no`**, so PAM has no channel through which to open a password login.
      This is why the value is `yes`, not `no` — the interaction is settled, not a to-do.
    - `pubkeyauthentication yes`
    - `permitrootlogin no`

    This concrete effective-output contract **replaces** the old undefined "address UsePAM's interaction"
    requirement. The drop-in must render `sshd -T` to exactly these values for the listed keys.
  - **Validate syntax and effective config before reload** (`sshd -t`, then diff `sshd -T` against the
    accepted set above — every listed key must match).
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
- **Installs (directive only — NOT Thaw):** ponytail (`/plugin marketplace add DietrichGebert/ponytail`
  + `/plugin install`, `hermes plugins install DietrichGebert/ponytail --enable`, promote to
  `enabledPlugins`). **Thaw is NOT installed in S11** — it ships as the standalone **SP5** PR (see the
  standalone-note below and the SP5 sub-project).
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
  implementation PRs (S6–S11, the Wave-3 stabilization PRs, Wave-3d, and the cutover-tooling PR) but
  BEFORE Phase D cutover, so `main` documents the reimplemented reality and the cutover applies
  converged instruction files. Build the
  **shared Claude + Codex rules partial** (one `.chezmoitemplates` partial included by both
  `~/.claude/CLAUDE.md` and `~/.codex/AGENTS.md`); **render both global targets in tests** and
  **byte-compare the shared block** across them; **re-verify every command and path against the converged
  `main`** at write time; **move conditional operational detail into runbooks** rather than the
  always-loaded instruction files (Phase E `fix/codex-agents-parity` and `fix/skill-architecture-diagram`
  ride here — the diagram documents the FINAL architecture, so S12 is its sole owner, ruled 2026-07-10).

---

## Cutover tooling PR (pre-S12) — the gate runner owns every cutover command [audit 2026-07-10]

**The plan defines WHAT must be true and WHY; the script owns HOW.** Review rounds 3–6 (2026-07-10)
proved that executable shell embedded in plan prose cannot be held to code standards — each wave of
inline commands leaked a fresh scoping or fail-open defect. So Phase D below carries **no command
sequences**: it states invariants and pass criteria, and every command lives in this PR's deliverables —

- **`scripts/cutover-gate.sh <1|2|3|4|5>`** — the gate runner. One entry point per gate; performs every
  check and mechanical action for that gate; stops at each operator checkpoint.
- **`scripts/live-reconcile.sh`** — the live reconciliation tool (as already planned).

Both are tracked, shellcheck-linted, TDD-built (red-first, per Global Constraints), and land in this
pre-S12 PR, reviewed through the same dual review pipeline as every other slice — so the cutover
commands get code review, tests, and lint instead of prose review.

**Gate-runner acceptance checklist (BINDING — every invariant hardened in review rounds 3–6; none may
be dropped, weakened, or left untested):**

1. **Pin-last ordering.** Gate 1 pins `MAIN_SHA` and `INT_SHA` only after the tree is clean and every
   must-ship change has landed on its branch — pinning is the last mutable-state read (Dependabot
   auto-merge moves `main`; the freeze policy admits integration hotfixes).
2. **Immutable manifest.** The expected-delta manifest is
   `git -C "$repo" diff 2bd973369158b49535e8e16e80c968444ab23f1d "$INT_SHA"` — the recorded Phase A
   base SHA to the pinned integration tip — regenerated from the pins at Gate 1; no stored file count
   is normative (196 at A1 time, 220 at this writing).
3. **Ledger classification.** Every manifest hunk is classified against `$MAIN_SHA` as exactly one of
   landed-unchanged / intentionally-improved / deliberately-omitted-with-reason / missing; **only
   `missing` blocks cutover**; Gate 2 activates exactly `$MAIN_SHA`.
4. **Clean tree, fully visible.** `git -C "$repo" status --porcelain --untracked-files=all` empty AND
   `"$repo/graphify-out"` absent (gitignored paths escape porcelain; the live unmanaged post-commit
   hook regenerates it) — both re-checked immediately before the apply.
5. **Repo handle, per shell.** `repo="$HOME/workspaces/Ivy/webdavis/dotfiles"` (absolute), validated
   `[[ -d "$repo/.git" ]] || exit 1` at the top of EVERY runner invocation — each gate runs in its own
   shell, nothing carries over.
6. **Repo-scoped operations.** Every git operation runs `git -C "$repo"`; non-git commands use
   absolute `"$repo"` paths or a guarded `cd "$repo" || exit 1` (`git -C` does not change the cwd for
   non-git commands). The runner never depends on its caller's cwd.
7. **Pins persisted and reloaded fail-closed.** Gate 1 records both pins in the ledger artifact; every
   later gate reloads them from the ledger and validates each as a full 40-hex SHA before use —
   missing/short/empty pins abort.
8. **Guarded fetch.** `git -C "$repo" fetch origin || exit 1` before any pin comparison — a failed
   fetch must never let stale remote-tracking refs satisfy the checks.
9. **Fail-closed comparisons.** Every equality requirement is an explicit `[[ "$x" == "$y" ]] || exit 1`
   in the runner — never prose, never advisory.
10. **Two-pin re-verification.** Gates 2 and 5 require `origin/main == $MAIN_SHA` AND
   `origin/integration/modernization == $INT_SHA`; on either mismatch the runner exits nonzero and the
   procedure restarts Gates 1–4 (re-clean + re-pin + re-classify, re-activate, re-reconcile, re-soak).
11. **Attached-HEAD landing.** Activation checks out `main` and fast-forwards to the pin, then asserts
   branch == `main` AND HEAD == `$MAIN_SHA` — never a detached `checkout $MAIN_SHA`.
12. **Per-domain launchd enumeration.** `launchctl print "gui/$(id -u)"` AND `launchctl print system` —
   NEVER bare `launchctl list`, which reads only the caller's bootstrap context (verified in round 6: a
   non-GUI review shell's `launchctl list` returned **0** jobs while `launchctl print gui/501` exposed
   **499** services).
13. **Retirement universe + preserve list.** The retirement candidate universe is an **EXACT, versioned
   (label, domain) inventory of every label this repo has EVER rendered** — a prefix shorthand like
   "`com.webdavis.*`" is wrong, because repo history holds out-of-prefix labels. **Derivation method
   (the runner regenerates the inventory this way; the tooling PR documents it):**
   `git log --all --diff-filter=AD --name-status -- 'Library/LaunchAgents/*' 'Library/LaunchDaemons/*'`
   plus `--diff-filter=R -M` for renames, plus `git log --all -S '<key>Label</key>' -- ':!Library'` for
   script-rendered labels (exactly one exists: the nix-hook heredoc). **Currently-known inventory
   (derived 2026-07-10; every label `gui/$UID` unless noted):**
   - *In current source:* `com.webdavis.atuin-daemon`, `com.webdavis.happy-daemon`,
     `com.webdavis.osquery-firewall-gatekeeper-monitor`, `com.webdavis.osquery-results-alerter` (label
     history: `osquery-fim-notify` → `osquery-results-notify` → this; renames `4771b6d`/`3de3336`),
     `com.webdavis.osquery-uptime-watchdog`, `com.webdavis.update-skills`,
     `com.webdavis.yt-dlp-pot-provider` (main + integration); `com.webdavis.homebrew-weekly-upgrade`,
     `com.webdavis.osquery-digest`, `com.webdavis.osquery-heartbeat`,
     `com.webdavis.osquery-tailscale-monitor` (integration only, pre-S6/S9);
     **`systems.nixos.nix-installer.nix-hook` — SYSTEM domain, out-of-prefix** (rendered by
     `dot_local/bin/executable_install-nix-repair-hook.sh`); `com.webdavis.paseo-daemon` (side-branch
     source `private_com.webdavis.paseo-daemon.plist.tmpl`, `e29c441` — live on dresden).
   - *Historical — deleted/renamed away (the retirement-candidate class):* `com.claude.code` (deleted
     `68a741b` on main / `f590081` on integration — Wave-3b's retirement script unloads it);
     **`com.github.openclaw-setup.watchdog` — out-of-prefix** (renamed in from
     `com.webdavis.openclaw-watchdog` at `b6d82e6`, deleted at `d15de21` with **no unload anywhere in
     that commit** — the archetypal loaded orphan this gate exists to catch);
     `com.webdavis.openclaw-watchdog` (the pre-rename label — orphanable on a machine that loaded the
     pre-`b6d82e6` plist); `com.webdavis.gha-watcher` (deleted `f297e1f`); `com.webdavis.osquery-report`
     (deleted `6199dcb`); `com.webdavis.osquery-posture-watch` (superseded at `3de3336`);
     `com.webdavis.osquery-fim-notify` and `com.webdavis.osquery-results-notify` (renamed away —
     orphanable pre-rename labels).

   The **PRESERVE list is unchanged and orthogonal** — it guards non-repo services (`io.osquery.agent`
   — package-owned, managed via `osqueryctl`, not rendered by this repo; the Tailscale system daemon;
   `sshd`; Apple system jobs); the universe guards repo history. Retirements are computed ONLY within
   the universe; nothing outside it is ever a retirement candidate. **The tooling PR's tests must prove
   a deleted historical label (e.g. the openclaw watchdog) becomes a retirement candidate when found
   loaded.**
14. **(label, domain, steady-state predicate) manifest entries.** Unconditional `KeepAlive=true` →
   loaded AND running; conditional `KeepAlive` dictionary → predicate per its semantics
   (`systems.nixos.nix-installer.nix-hook`, system domain, `KeepAlive={SuccessfulExit=false}`: healthy
   = loaded, idle, last exit 0); scheduled/demand (`StartInterval`/`StartCalendarInterval`/`WatchPaths`
   triggers, regardless of `RunAtLoad` — which launches once and is NOT persistence) → loaded with the
   trigger registered, not necessarily running (`com.webdavis.osquery-uptime-watchdog` exits after
   every run).
15. **Domain-qualified per-label verification.** `launchctl print "$domain/$label"` per manifest entry:
   approved-retired labels ABSENT (print errors), every desired label satisfies its recorded predicate.
16. **Operator checkpoints.** The runner STOPS for operator approval of the retirement manifest
   (Gate 1) before any service-affecting apply stage, and never performs the interactive
   `chezmoi apply` itself (that is the operator's, staged, at Gate 2). Approval reviews a CORRECT
   manifest — it is a review checkpoint, not a repair mechanism for a wrong one.
17. **Explicit GitHub targeting.** Gate 5 closes PRs with `--repo=webdavis/dotfiles`
   `--hostname=github.com` passed explicitly, in addition to the guarded cd — gh's resolver precedence
   is `--repo` > `GH_REPO` > cwd remote, so cwd alone cannot be trusted (verified in round 6: with an
   inherited `GH_REPO`, a resolver-only test inside this repo selected wrong-owner/wrong-repo).
18. **Reconcile-script contract.** `scripts/live-reconcile.sh` has a `--dry-run` flag, is idempotent,
   and is tested; Gate 3 runs it by absolute path, dry-run before live.
19. **Code standards.** Both scripts pass `shellcheck` and `shfmt`, are TDD-built with stubbed
   launchd/git/gh boundaries (Classist doubles at true I/O boundaries only), and every checklist item
   above has a test.

---

## Phase D — Cutover (five gates) [audit 2026-07-10]

### Task D1: Switch main live, verify, close the reference PRs — five sequential gates

The audit split the single cutover step into **five gates** so that Phase E items which only complete
*after* apply have a named home, and so the reference PRs are not closed before the soak proves
convergence. Each gate must pass before the next begins. Gate roles: **Gate 1** preflight — clean tree,
then pinned SHAs, the immutable-manifest expected-delta ledger, and **operator approval of the retirement
manifest**; **Gate 2** staged activation **and execution of the approved retirement**; **Gate 3** tracked
reconciliation + **post-retirement verification**; **Gate 4** soaks the **final, retired** topology;
**Gate 5** closure-only (re-verify both pins, close the reference PRs — no repo mutation). Retirement lives
in Gates 1–3 (approve / execute / verify), not Gate 5 — so the topology soaked is the topology closed
out. **`$INTEGRATION_PR` (set in A1) is
PR #32**, the DO-NOT-MERGE reference; the source PRs are **#25** (osquery three-tier) and **#31**
(herdr/Tailscale/brew/moshi).

**No commands live in these gates.** Each gate states its invariants and pass criteria; every check and
mechanical action is performed by the tracked, tested gate runner (`scripts/cutover-gate.sh <gate>`)
from the **Cutover tooling PR** section above, whose binding acceptance checklist carries the full
mechanics (repo scoping, fail-closed pin handling, launchd domains, GitHub targeting). Run the gate's
runner entry, then complete the operator checkpoints it stops for.

#### Gate 1 — Preflight (before switching the live source)

Invariants hold **in this order** — the tree is settled and every must-ship change has LANDED before
the SHAs are pinned, because both branches can move until then (Dependabot on `main`; the freeze policy
admits integration hotfixes): a change landed after pinning would sit silently outside the manifest.

- [ ] **1. Clean tree.** Every dirty/untracked file in the primary checkout is classified keep /
  discard / back-up; **kept files leave the source tree** (backup convention) — git preserves
  non-conflicting dirty/untracked files across a checkout and chezmoi deploys the working tree, so
  anything left in place would deploy content the ledger never classified (the primary checkout holds
  exactly such untracked chezmoi sources today). Anything that must ship is **committed and pushed
  before pinning** (to `main`, or to the integration branch as a freeze-policy hotfix) and
  classification re-runs. **Pass:** the runner sees a fully-visible-clean tree — no dirty or untracked
  entries AND no `graphify-out/` residue (gitignored paths escape a porcelain listing; the live
  unmanaged post-commit hook regenerates it) — re-checked immediately before the apply. Gate 2's
  exact-SHA activation claim holds only on a clean tree (checklist items 4–6).
- [ ] **2. Hermes backup.** Uncaptured Hermes profile state is backed up per the backup convention
  (`~/workspaces/backups/YYYY-MM-DDTHH-MM-SS.<name>.backup[.ext]`) — the per-profile `config.yaml`
  enablement/`platform_toolsets` and codegraph MCP state are otherwise untracked encrypted `.age` files
  (Phase E `fix/hermes-encrypted-profile-configs`).
- [ ] **3. Pins, LAST.** The runner records `MAIN_SHA` and `INT_SHA` in the ledger from the
  freshly-fetched **remote-tracking refs — never local branch refs**, which lag the remote (when this
  was written the local `main` was `2bd9733` while `origin/main` was `1a6e718`; a local-ref ledger
  would describe a different revision from the one Gate 2 activates). Pinning is the LAST
  mutable-state read: any commit to either branch afterward invalidates the pins, and Gates 2 and 5
  re-verify both (checklist items 1, 7–10).
- [ ] **4. Retirement manifest, operator-approved.** A `launchctl` before/after inventory diff can
  never find an orphan (a loaded job keeps appearing after its plist is deleted), so the runner builds
  an explicit manifest: the **desired-state set** (every LaunchAgent/LaunchDaemon the pinned
  `$MAIN_SHA` source renders, each entry a **(label, launchd domain, expected steady-state predicate)**
  triple — persistent vs conditional-KeepAlive vs scheduled/demand semantics per checklist item 14),
  the **live loaded set** (enumerated per launchd domain — user AND system — per checklist item 12),
  and the **retirement list**: live jobs absent from the desired set, computed ONLY within the
  **managed-label universe** — the exact, versioned (label, domain) inventory of every label this repo
  has EVER rendered, derived from repository history per checklist item 13 (NOT a `com.webdavis.*`
  prefix match: history holds out-of-prefix labels — `com.github.openclaw-setup.watchdog`, deleted with
  no unload at `d15de21`, is precisely the loaded-orphan class this gate must catch, and the
  system-domain `systems.nixos.nix-installer.nix-hook` is rendered by a script, not a tracked plist) —
  and never touching the **preserve list** of package/OS-owned services (`io.osquery.agent` —
  package-owned, managed via `osqueryctl`; the Tailscale system daemon; `sshd`; Apple system jobs).
  **Checkpoint:** the operator approves every named retirement
  BEFORE any service-affecting apply stage. Approval reviews a CORRECT manifest — it is a review
  checkpoint, not a repair mechanism for a wrong one. Gate 2 executes only the approved list (covering
  retirements performed by apply-time scripts too, e.g. Wave-3b's one-time retirement script).
- [ ] **5. Expected-delta ledger — REPLACES the old empty-diff gate, built from an IMMUTABLE
  manifest.** The old gate was contradictory (the plan permits improvements over the integration branch
  yet demanded an empty final diff) **and** mechanically unsound: a `main`-vs-integration diff shows
  only the **residual** difference — every reference hunk that already landed unchanged on `main` has
  vanished from it — and reads mutable refs. Instead the runner regenerates the manifest from the
  **recorded Phase A base SHA (`2bd9733…`) to the pinned `$INT_SHA`** — the full original combined
  delta, every hunk present, because the base is fixed, not `main` (no recorded file count is
  normative: 196 at A1 time, 220 at this writing; the frozen branch takes hotfixes) — and classifies
  **every** manifest hunk against the pinned `$MAIN_SHA` as exactly one of **landed-unchanged**,
  **intentionally-improved**, **deliberately-omitted-with-reason**, or **missing** (checklist items
  2–3). **Pass: only a `missing` hunk blocks cutover** — the other three are expected and recorded.
  Gate 2 activates exactly `$MAIN_SHA`, the same pinned commit this ledger classified against, so the
  state proved converged is the state cut over to.

#### Gate 2 — Staged activation and service retirement

- [ ] Open a **second remote session** first, so a broken apply cannot lock you out.
- [ ] **Pins re-verified, then activation lands ATTACHED at the pin.** The runner re-verifies — in a
  fresh shell, fail-closed — that both remote branches still equal the recorded pins (checklist items
  5–10). **On either mismatch it aborts and the procedure restarts Gates 1–4** (re-clean + re-pin +
  re-classify, re-activate, re-reconcile, re-soak). After S12 merges, it points dresden's chezmoi
  source at the pinned commit **attached to `main`, never detached** (checklist item 11) — a detached
  checkout would leave the live source floating off-branch.
- [ ] **Operator** runs a full interactive `chezmoi apply` (KeePassXC unlocked) **in stages**, not one
  shot — keep the integration branch and previously deployed files available for rollback. (The runner
  never performs this apply itself — checklist item 16.)
- [ ] **Retire exactly the Gate 1 approved retirement manifest.** Approval already happened at Gate 1 —
  BEFORE this service-affecting apply; the runner executes only the approved list, domain-qualified per
  manifest entry (checklist items 13–15) — e.g. the old Claude `com.claude.code` in the user domain;
  Wave-3b's one-time retirement script is one of the approved executors. Nothing is discovered or
  retired ad hoc mid-apply. Retirement happens HERE, during activation — so Gate 4 soaks the FINAL
  topology, not the pre-retirement one.
- [ ] **Verify remote reachability** (Tailscale / SSH) before ending the original session.

#### Gate 3 — Tracked live reconciliation and post-retirement verification

- [ ] **Live reconciliation, dry-run first.** The runner executes the **already-merged, pinned**
  `scripts/live-reconcile.sh` from the cutover-tooling PR (built, reviewed, and tested pre-S12 — NOT
  authored ad hoc during cutover), `--dry-run` before live (checklist item 18), to prove a
  from-scratch machine converges identically (Phase E `fix/live-reconcile-from-scratch`).
- [ ] **Post-retirement verification against the manifest, not a before/after diff** (a loaded job
  outlives its deleted plist, so diffs can't prove retirement). The runner probes every manifest entry
  domain-qualified (checklist items 12, 15): each approved-retired label is **ABSENT** (actually
  unloaded, not merely plist-deleted), and each desired label satisfies **its recorded steady-state
  predicate** (checklist item 14 — a blanket "running" check would wrongly fail the one-shots and the
  conditional-KeepAlive nix-hook).
- [ ] **Full test suite green + live smoke checks.** The runner executes the repo's test suite from the
  repo itself (checklist item 6), then the smoke set: relay fires a test notification; the hermes
  gateway is healthy; the osquery heartbeat sends its one ✅; chezmoi reports no source↔target drift
  (excluding KeePassXC-gated templates).

#### Gate 4 — Soak the final topology

- [ ] Let the converged `main` — **with retirement already applied** — run for a soak window; watch the
  daily-critical paths (notifications, hermes, osquery, shell startup) for regressions. **Do not close any
  reference PR during the soak.**

#### Gate 5 — Final closure (closure-only)

- [ ] **Pins re-verified before closing anything.** This gate runs days after Gate 1, in a fresh shell:
  the runner reloads both pins from the ledger, validates them, freshly fetches (guarded), and requires
  both remote branches still at their pins AND the live checkout still attached at the pin (checklist
  items 5–11). If either branch moved during the soak (Dependabot auto-merge on `main`; a freeze-policy
  hotfix on integration), the soaked state is not the closing state — **restart Gates 1–4** before
  closure.
- [ ] **Close PR #25, PR #31, and the integration reference PR #32** — the runner targets the GitHub
  repository **explicitly**, never trusting the cwd remote (checklist item 17) — **only after the soak
  passes** — each with a comment linking the slice PRs that superseded it. No repo mutation happens in
  this gate (the `graphify-out/` excludes stay in place through cutover as zero-cost belt-and-braces;
  dropping them is `fix/graphify-out-excludes-drop` in the **SP7 backlog** — post-cutover by design,
  not Phase E).

**Phase E → gate attachment.** Every Phase E item completes at a named home — a pre-cutover PR or a D1
gate:
`fix/live-reconcile-from-scratch` → the **cutover-tooling PR** (builds `scripts/live-reconcile.sh`
pre-cutover) + Gate 3 (runs it); *(`fix/graphify-out-excludes-drop` is no longer a Phase E item — moved
to the **SP7 backlog** 2026-07-10, post-cutover by design; the excludes stay through cutover as
zero-cost belt-and-braces, Gate 5 mutates nothing, and no re-pin is ever needed)*;
`fix/harness-skill-reconciliation` → Gate 3 (Hermes-side pruning, coordinated at cutover);
`fix/hermes-encrypted-profile-configs` → S8 (backed up at Gate 1); `fix/codex-agents-parity` → S12;
`fix/template-render-coverage` → the Wave-3 render-coverage PR; `fix/moshi-herdr-drift-check` and
`fix/pre-commit-path-filter` → S11; `fix/skill-architecture-diagram` → **S12, sole owner** (ruled
2026-07-10 — it documents the final architecture, so it belongs with the docs refactor; the earlier
"Wave-3 skills-stab /" alternative is removed so no PR can assume the other owns it).

---

## Self-Review

**Spec coverage:** every SP2-tagged item in the work ledger maps to a slice's "ledger fixes" column (S2
CI/lint items; S3 skills; S4 herdr consolidation; S6 brew; S9 osquery; S10 defaults/ssh; S11 the long
tail; S12 CLAUDE.md). The spec's provisional slice map (its items 1–11) maps to S1–S11; SP1 = S8; SP5
Thaw = a standalone SP5 PR during SP2, not S11; the CLAUDE.md refactor = S12. Combine mechanics (integration branch, DO-NOT-MERGE PR, freeze
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
- **Phase D D1:** ~~add an **empty-diff reconciliation gate**~~ **superseded 2026-07-10 by the
  expected-delta ledger (D1 Gate 1)** — the empty-diff gate was contradictory (the plan permits
  intentional improvements over the integration branch, which a strict empty diff forbids). The ledger
  classifies each reference-branch hunk as landed-unchanged / intentionally-improved /
  deliberately-omitted-with-reason / missing; only `missing` blocks cutover.
- **Phase C preamble:** add a short "Tooling considered and rejected" note (Graphite/ghstack/spr/
  Sapling/jj + why), so it is not re-litigated mid-execution.

**New features:** hunk-ownership table + ~~empty-diff gate~~ (superseded 2026-07-10 by the
expected-delta ledger, D1 Gate 1) `[plan]`; non-interactive `git apply` P-2
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
  `{{ if eq .chezmoi.os "darwin" }}`, so the **future Linux home-server cannot restore the key**. ~~S8
  drops/generalizes the guard~~ **superseded 2026-07-10 [audit]: the guard is KEPT until a complete
  Linux credential-source and identity design exists** (see the amended S8 section — the macOS paths and
  KeePassXC assumptions do not constitute Linux support).
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
   `~/.hermes/config.yaml` (source `dot_hermes/encrypted_private_config.yaml.age`), plus — as S8 promotes
   them — the four specialist profile captures that already exist untracked at
   `dot_hermes/profiles/private_<profile>/encrypted_private_config.yaml.age` (verified 2026-07-10) and
   codegraph state (uncaptured; its name follows chezmoi's `encrypted_` naming at capture time).
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
`[repo]`; ~~generalize the darwin guard so the restore runs on Linux too~~ **superseded 2026-07-10
[audit]: the guard is KEPT until a real Linux credential design exists (amended S8 section)**;
**multi-recipient migration design** (each machine keeps its own identity; `.chezmoi.toml.tmpl` lists
both public keys as `recipients` so files encrypt to both — the laptop→home-server path) `[dresden]`.
Sources: chezmoi.io/user-guide/encryption/age, chezmoi.io/user-guide/frequently-asked-questions/
encryption, discourse.nixos.org (git-crypt/agenix/sops-nix comparison).
*(verdict: SOUND — spot-verified against the live repo and the chezmoi age doc.)*

### R5 — Agent skills/memory: architecture correct; reproducibility + supply-chain gaps (amends S3 + S12)

> **Superseded-historical [2026-07-10 audit] — provenance only.** This section records the 2026-07-04
> research pass; three of its specific artifacts are OBSOLETE and kept only for the trail: the
> `21/12/9/0` (and `20/12/9/0`) skill counts, the `skills-lock.json` lock name, and the "declare blanket
> `dot_hermes/skills/` symlinks" fan-out instruction. The shipped model superseded all three — one
> `~/.agents/skills` store (31 roster skills) under `dot_agents/custom-skill-lock.json`, with a
> **disjoint** five-profile hermes delivery (`hermesProfiles` store-symlink lane ⟂ `hermesRegistry`
> hub-owned lane), NOT a blanket symlink. The live model is the amended S3 section and the repo
> `CLAUDE.md` "Agent Skills (cross-harness store)" section; read those, not the counts below.

The `~/.agents` store + symlink fan-out + `AGENTS.md`→`CLAUDE.md` model is correct and, in places, ahead
of the ecosystem (AGENTS.md convention, Anthropic's Agent Skills). ~~But **verified on disk**: **21 live
store skills vs 12 committed vs 9 Claude `symlink_` declarations vs 0 Hermes declarations** — a fresh
`chezmoi apply` reproduces only ~9 of 21 skills into Claude and none into Hermes.~~ *(superseded
2026-07-10 — the shipped 31-skill store reproduces every roster skill into every harness it targets; see
the banner above.)*

**Keep/deprecate decision (operator, 2026-07-04): keep ALL 21 — deprecate none** (the operator uses each
at different times; the earlier "overlap" flags were retracted as unfounded — `last30days` [trend
research], `tiktok-crawling` [bulk scrape], and `video-transcript-downloader` [transcripts] do genuinely
different jobs, and the four `hyperframes*` skills are an interdependent suite whose descriptions
cross-delegate, so they are all-or-nothing). The task is therefore purely **reproducibility**, not
culling.

**The 9 uncommitted skills S3 must capture** *(superseded 2026-07-10 — historical: S3 shipped the
31-skill provenance model instead, which subsumed this capture list; see the amended S3 section)*
(committed or install-manifested — verified 2026-07-04):
`chrome-devtools-axi`, `cua-driver`, `elevenlabs`, `gh-axi`, `home-assistant`, `kubernetes-specialist`,
`last30days`, `sql-toolkit`, `tiktok-crawling`. **Note `gh-axi` and `chrome-devtools-axi` are among
them** — the repo's own *preferred* GitHub and browser tools would silently not reproduce on a fresh
machine. Each has a known source (npx-skills / clawhub) captured during this session; S3 records those.

**Changes to apply:**

- **S3** *(these are the 2026-07-04 change requests; the shipped 31-skill model implemented them
  differently — the stale specifics are struck below, the intent held)*: commit the full skill roster (or
  a committed `name→source` install-manifest) so a fresh machine reproduces every skill; make
  `update-skills.sh` **install-capable** (today its loops `[ -d "$STORE/$n" ] || continue` skip anything
  absent → refresh-only); complete the fan-out declarations (declare all store→Claude symlinks;
  ~~add the missing `dot_hermes/skills/` declarations~~ *(superseded: hermes fan-out is the disjoint
  `hermesProfiles` store-symlink lane ⟂ `hermesRegistry` hub lane — NOT blanket `dot_hermes/skills/`
  symlinks)*); resolve the **three-way** fan-out ownership (the ledger names two writers — the third is
  `npx skills … --global`); ~~reconcile the lockfiles (`skills-lock.json` has a stale `moshi-best-practices`
  entry and 12 vs 20 live)~~ *(superseded: the lock is `dot_agents/custom-skill-lock.json`, guarded by the
  five-table roster test)*; add a **supply-chain gate** — pin each vendored git-clone to a commit SHA
  and/or verify `computedHash` before the atomic swap.
- **S12:** specify the global `AGENTS.md` parity **mechanism** (a `.chezmoitemplates` partial included by
  both `~/.claude/CLAUDE.md` and `~/.codex/AGENTS.md`), not just "add parity."

**New features:** committed full roster / install-manifest `[repo]`; single `.chezmoitemplates` partial
for the global ruleset `[dresden]`; SHA-pin + hash-verify vendored skills before swap `[repo]`.
Sources: agents.md, anthropic.com/engineering/…agent-skills, platform.claude.com/docs/…agent-skills/
best-practices, developers.openai.com/codex/skills.
*(verdict: SOUND at the time — the 20/12/9/0 counts were independently re-verified on disk 2026-07-04;
those counts are now superseded-historical, see the R5 banner above.)*

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
<pid> -activeCount <n>`. A tiny wrapper pointing at `relay.sh` (or the SP3 stateless per-event
executable, invoked once per camera/mic event) would turn an
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
  implement under its own spec. Not started; runs after SP2. **Audit directives [audit 2026-07-10]
  (before any modernization — the branch was 69 commits behind / 3 ahead of `origin/main` at audit
  time):** (1) re-check the branch state first; (2) back up both repositories; (3) inventory the live
  Neovim configuration; (4) import the live configuration UNCHANGED first; (5) modernize only through
  later reviewable PRs.
- **SP7 backlog — small chores**, including **P6: install `bandwhich`, `doggo`, `ouch`** ("still valid,
  trivial" — manifest entries + `brew install`), P3 package-manager audit, P5 Determinate Nix review,
  and P8 quick wins — **P8 is UNBLOCKED [audit 2026-07-10]: the SP4 verdict is in (nushell NO-GO,
  operator-ratified 2026-07-09), so shell-config placement = bash.** **Audit directive
  [audit 2026-07-10]: deduplicate the ledger into tracked tasks** — each with current status, severity,
  and dependencies (P12 is already on `main`; obsolete OpenClaw and issue-tracking work closes or
  updates per R3).
  - **`fix/graphify-out-excludes-drop` (moved OUT of Phase E 2026-07-10 — Phase E must complete before
    SP2 closes; this chore is post-cutover by design).** `.gitignore`'s `graphify-out/` entry and
    `treefmt.nix`'s `graphify-out/**` exclude are band-aids, kept because the old global graphify
    post-commit hook still fires in this repo until the opt-out dispatcher (S3) is applied live — the
    LIVE hook is the unmanaged old version with **no** `.githooks/no-graphify` check, so it keeps
    regenerating `graphify-out/` in every worktree until Gate 2's apply replaces it (evidence: the
    plan-amendment worktree carried ~1.2 MB of `graphify-out/` on 2026-07-10). A pre-cutover
    excludes-drop is NOT dormant — `.gitignore`/`treefmt.nix` changes take effect immediately in every
    `main`-derived worktree while the unmanaged hook still fires, recreating untracked source state
    right before the clean-tree cutover (and a post-Gate-2 drop would force re-pinning `$MAIN_SHA`
    mid-cutover). The excludes are pure belt-and-braces with zero cost, so they **stay through
    cutover**; the drop lands here, sequenced: (a) the managed dispatcher + `.githooks/no-graphify`
    marker are deployed AND verified live (Gate 2's apply); (b) existing `graphify-out/` output is
    removed from all worktrees; (c) only then drop both exclusions. D1 Gate 1 independently asserts
    `"$repo/graphify-out"` is absent before the apply (its clean-tree step).
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
1. **Land the Wave-3d OpenClaw cleanup PR (R3)** — removes the `openclaw` package, the AeroSpace F1
   binding, and the docs together — before S12, so S12 documents an OpenClaw-free converged reality.
1. **Land the cutover-tooling PR** — implements the gate runner `scripts/cutover-gate.sh` (owning EVERY
   Phase D command, against the binding 19-item acceptance checklist in the "Cutover tooling PR" section)
   and `scripts/live-reconcile.sh` (`--dry-run`, idempotent, test-driven), so the gates execute
   already-merged, pinned, tested tools. **Before S12** — S12 verifies every command and path against
   the truly final repo, so all implementation PRs, this one included, precede it. (The `graphify-out/`
   excludes-drop is NOT here — it is an SP7 post-cutover chore; see `fix/graphify-out-excludes-drop`.)
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
slice. **[audit 2026-07-10] Every item is attached to a named owner — a pre-cutover PR or a D1 gate —
in the "Phase E → gate attachment" map at the end of Task D1; the per-item owner notes below match that
map (nothing floats unattached). Work whose right timing is post-cutover is NOT Phase E:
`fix/graphify-out-excludes-drop` moved to the SP7 backlog (2026-07-10) for exactly that reason.**

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

The four specialist profiles' `config.yaml` (enablement + `platform_toolsets`) are persisted only as
**untracked encrypted `.age` files** in the primary checkout — verified 2026-07-10 at
`dot_hermes/profiles/private_<profile>/encrypted_private_config.yaml.age` (`private_butters`,
`private_concerned`, `private_elaine`, `private_nicodemus`); codegraph's Hermes-MCP enablement is
likewise untracked (and not yet captured at all). A fresh machine reproduces skill *presence* but not
per-profile *curation* or the MCP wiring. Fix rides S8 (the age machinery): inventory/hash/back up the
four existing untracked captures, then per profile either **commit the existing capture** or
**intentionally recapture newer live state** — plus the root config (already tracked as
`dot_hermes/encrypted_private_config.yaml.age`) and the codegraph MCP config — round-trip verify each,
extend the DR drill.

### fix/codex-agents-parity (→ S12)

Global-rules notes (the Home Assistant pairing line, and any future rule) currently reach Codex only via
a hand-edit to the **untracked `~/.codex/AGENTS.md`**. R5/S12's shared `.chezmoitemplates` partial —
included by both `~/.claude/CLAUDE.md` and `~/.codex/AGENTS.md` — is not built. Fix: implement the parity
partial in S12 and migrate the HA note (and the rest of the global ruleset) into it so both harnesses
share one source.

### fix/live-reconcile-from-scratch

PR #36's live skills convergence (frozen-clone cleanup, per-profile symlink planting, stale hub-install
retirement, Codex-overlay + routing re-assert, live `skillOverrides` merge) was performed **ad-hoc on
the live machine** during review. The reproducible path was drafted as
`.superpowers/sdd/live-reconcile-skills.sh` — **corrected 2026-07-10 [audit → cutover-tooling PR + D1
Gate 3]: `.superpowers/` is gitignored and that script was never tracked. The durable reconcile script
is `scripts/live-reconcile.sh`, and it has a dedicated implementation owner — the pre-cutover
cutover-tooling PR (before S12 in the authoritative order, so S12 verifies the final repo) implements it test-first
(`--dry-run` flag, idempotent, tested), reviewed and merged BEFORE cutover.** Gate 3 does not author it;
Gate 3 only executes that already-merged, pinned tool (`--dry-run` then live) to prove a from-scratch
machine converges identically, after which the ad-hoc live state and the script are reconciled.

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

### fix/pre-commit-path-filter (→ S11, found 2026-07-09 roadmap audit)

The roadmap's S2 design-alternative "pre-commit: skip the bats suite on docs/YAML-only commits (path
filter)" never made it into the S2 plan text or implementation — every commit (including docs-only)
runs the full `just lint-check && just test` (observed live: plan-edit commits run the whole suite).
Friction, not correctness. Fix: a path filter in `.githooks/pre-commit` that skips `just test` (never
the lint gate or gitleaks) when the staged diff touches only docs/markdown. Slot: **S11** (pinned
2026-07-10 — the earlier "or post-SP2" option violated the Phase-E-closes-with-SP2 rule; it is a
two-line hook filter, and S11's git-hygiene PR is the natural home).

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

### fix/skill-architecture-diagram (→ S12, sole owner)

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
