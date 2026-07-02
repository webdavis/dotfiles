# Repo Modernization Roadmap — Design

**Date:** 2026-07-02
**Status:** approved via brainstorming (this session); covers sub-projects 1–2 in full and defers 3–7 to
their own spec cycles.
**Input:** `docs/plans/2026-07-02-repo-modernization-brief.md` (the full context brief — read it first;
this spec does not duplicate its background).

## Executive summary

The brief's six-plus workstreams are too large for one spec. This roadmap decomposes them into seven
sequenced sub-projects, records the decisions already made with the user, fully designs the first two
(unblocking the dirty tree; the combine-and-split PR workflow), and indexes the rest as future spec
cycles with their banked decisions so nothing has to be re-litigated.

## Decisions log (settled with the user in this brainstorm — do not re-open)

1. **Hermes config encryption: chezmoi-native `age`.** The in-flight work finishes as designed. The
   global toolchain line "File Encryption: git-crypt" stands for general repo file encryption elsewhere;
   for chezmoi-managed secrets in this public repo, `age` (`encrypted_` files, ciphertext always in the
   source tree) is the mechanism.
2. **Notification presence routing:** at the computer → local clickable banner only (terminal-notifier);
   away → phone push via moshi only. moshi is purely the phone channel. Discord `#relay` logs
   unconditionally. Hue pulses only when the user is physically home (a different signal than
   at-computer; Home Assistant is the likely source — final call in the notification-rewrite spec).
3. **nushell is back in scope, contingent on an honest evaluation.** The prior "Shell: bash — locked-in"
   toolchain line and GH issue #5's planned wontfix closure are superseded by explicit user instruction:
   evaluate the migration seriously (criteria in SP4 below) and pursue it if it holds up.
4. **The osquery hands-off constraint is partially lifted:** the *alerting/dispatch design* of the
   three-tier work (PR #25) must be improved during its reimplementation slice. Query/pack *content*
   changes are proposed-and-flagged for user sign-off, not made unilaterally.
5. **Combine-and-split means reimplement-from-main.** The giant integration PR is a frozen live-state
   reference deployed on dresden — never merged. The small PRs are fresh re-implementations from `main`
   that fold in review feedback and slight improvements; git history on `main` is authored by the small
   PRs, not preserved from the feature branches.
6. **Code discipline for all new code:** SOLID; Classist (Detroit-school) Test-Driven Development —
   real collaborators in domain tests, test doubles only at true input/output boundaries (network,
   subprocess, filesystem, clock). This aligns the user's global CLAUDE.md rules with the
   essential-feed-case-study strategy the brief mandates.

## Sub-project sequence

| # | Sub-project | Scope | Spec |
| --- | --- | --- | --- |
| SP1 | Unblock the tree | Finish + commit the Hermes age-encryption work sitting uncommitted | this spec |
| SP2 | Combine & split | Integration PR (#31 + #25) → reimplement from `main` as small PRs → cutover | this spec |
| SP3 | Notification rewrite | New-language pipeline; subsumes hue-pulse improvements (old P7) | own spec |
| SP4 | nushell evaluation → migration | Go/no-go evaluation early; migration (if go) after SP3 lands | own spec |
| SP5 | Thaw install | Trivial standalone PR; slots in during SP2 | none needed |
| SP6 | nvim-overhaul | Re-evaluate v1/v2/v3 specs, then implement | own spec |
| SP7 | Sweep + p-tasks backlog | Small chores as interleaved PRs at the end | backlog list |

The nushell *evaluation* (research only) runs early — during SP2/SP3 — because SP3's shell-hook seam
depends on knowing the shell direction. The *migration* (if go) executes after SP3.

## SP1 — Unblock the tree (Hermes age encryption)

All scaffolding already exists uncommitted: `.chezmoidata/hermes.yaml` (version pin),
`run_onchange_before_25-install-hermes-agent.sh.tmpl` (pinned install),
`run_after_67-hermes-config-migrate.sh.tmpl` (headless migrate), `test/hermes-config-encrypted.sh`
(secret-leak guard; currently skips pre-migration), `test/hermes-config-routes.sh` (route integrity),
plus the gitleaks pre-commit gate, `.gitignore` failsafe block, and `scripts/lint.sh` wiring.

**Operator steps (interactive, KeePassXC unlocked — the agent cannot do these):**

```bash
mkdir -p ~/.config/chezmoi
age-keygen -o ~/.config/chezmoi/key.txt   # prints the public key
chmod 600 ~/.config/chezmoi/key.txt
# Store the FULL key.txt contents in KeePassXC as "chezmoi :: age identity" (crown jewel).
```

**Agent steps (after the key exists):**

1. Edit `.chezmoi.toml.tmpl`: `encryption = "age"` (bare key above the first table), `[age]`
   `identity`/`recipient` (recipient = the public key), `[add] secrets = "error"`; `chezmoi init`;
   verify via `chezmoi dump-config`.
2. Migrate the live config to the pinned schema **before capture** (v24 → v30 at pin time — capture
   order is load-bearing; capturing pre-migrate causes a revert→re-migrate loop every apply):
   `hermes config migrate && hermes gateway restart && hermes doctor`.
3. `git rm` the old mechanism (`dot_hermes/modify_private_config.yaml.tmpl`,
   `private/relay-hermes-route.yq`, `test/relay-hermes-route.sh`) — duplicate-target conflict otherwise.
4. `chezmoi add --encrypt ~/.hermes/config.yaml` → `dot_hermes/encrypted_private_config.yaml.age`.
5. Round-trip verify: `diff <(chezmoi cat ~/.hermes/config.yaml) ~/.hermes/config.yaml` is empty;
   `head -1` of the source file is an age marker, not YAML.
6. `just l && just test` (the two hermes tests flip from skip to enforcing), then commit the whole set
   as logically separate commits (encryption scaffolding; gitleaks gate; docs).
7. Operator afterward: `trash` the world-readable plaintext backups in `~/.hermes/`
   (`config.yaml.bak.*`, `config.yaml.*.backup`) — destructive, per-invocation confirmation.

Also commits in SP1: `docs/superpowers/plans/2026-07-01-dresden-never-sleep-power-policy.md`
(untracked plan doc) and the brief's lint reflow.

## SP2 — Combine & split

### Combine (Approach A: merge-combine)

1. `git checkout -b integration/modernization feat/cli-agent-tracking-workflow` (after SP1 commits).
2. `git merge feat/osquery-alerter-three-tier` — one conflict resolution; known overlap is exactly
   `.chezmoiignore` and `justfile`. PR #25's stale merge-base is irrelevant to a merge.
3. Push; open a **draft PR titled "DO NOT MERGE — integration reference (modernization)"** via
   `gh-axi pr create`. Body links this spec and states the branch's role: frozen live-state reference.
4. dresden keeps its chezmoi source on this branch until cutover. **Freeze policy:** no new feature work
   lands here; only hotfixes needed to keep dresden healthy, and every hotfix must also be folded into
   its corresponding reimplementation slice.

Rejected alternatives: rebase-stacking the 64 osquery commits (history rewrite + conflict churn for a
throwaway branch); skipping the integration PR (contradicts the explicit live-state-artifact
requirement).

### Split (reimplement from `main`)

Rules for every small PR:

- **Self-contained and fully wired** — no dead code, no half-feature waiting on a later PR.
- Branch from current `main`; `just l` + `just test` green; conventional commits; no AI trailers.
- Review feedback folds in before merge — the reimplementation is *allowed to improve* on the
  integration branch's version (that is the point of reimplementing).
- All GitHub operations via `gh-axi`.
- Ordered so dependencies flow infra → features.

**Provisional slice map** (file-level assignment happens in the writing-plans phase, against the real
diff):

1. Docs: briefs + specs (this file, the modernization brief, nvim v3 spec if pulled forward).
2. Lint/test/CI hardening: gitleaks pre-commit gate, `scripts/lint.sh` additions, actionlint (old P9).
3. Skills-store consolidation: `update-skills.sh`, keep-list, `dot_agents/` layout.
4. herdr migration (2–3 PRs: config; smart-nav plugin; bashrc/workspace integration).
5. Tailscale headless daemon.
6. Homebrew weekly-upgrade LaunchAgent.
7. Relay notification pipeline **as deployed** (bash version + its tests) — lands so `main` matches
   dresden's live behavior; SP3 replaces it later through its own PR sequence.
8. Hermes age-encryption (the SP1 work).
9. osquery three-tier alerting — **reimplemented with design improvements** (decision 4). Alerting/
   dispatch redesign is in scope; query/pack content changes are flagged for user sign-off.
10. macOS defaults / system-setup additions.
11. Small chores interleaved (Thaw install = SP5; gitconfig autocorrect = old P12; etc.).

### Cutover

When the slice map is exhausted: dresden's chezmoi source switches back to `main`; full interactive
`chezmoi apply` (KeePassXC unlocked); `just test` + live smoke checks (relay fires, hermes gateway
healthy, osquery alerter behavior verified); then close PRs #31 and #25 and the integration PR with
pointers to the landed slices.

## p-tasks re-evaluation (from `2026-05-15-dotfiles-tasks-design.md`)

| Old phase | Status now | Disposition |
| --- | --- | --- |
| P3 package-manager audit (GH #11) | still valid | SP7 backlog |
| P4 gitleaks pre-commit | **already implemented** (in SP1's uncommitted set) | ships in SP1/SP2 |
| P5 Determinate Nix review (GH #10) | still valid, research-first | SP7 backlog |
| P6 bandwhich/doggo/ouch | still valid, trivial | SP7 backlog |
| P7 hue-pulse improvements | superseded | folds into SP3 (race fix already shipped) |
| P8 quick wins (MANPAGER, fd/FZF, starship) | valid; shell-config placement depends on nushell | SP7, after SP4's go/no-go |
| P9 actionlint | still valid | SP2 slice 2 |
| P12 gitconfig autocorrect | still valid, trivial | SP2 slice 11 or SP7 |
| P13 Tart base image (~25 GB pull) | still valid, deferrable | SP7 backlog, background |
| P10/P11/N1 (OpenClaw queue, mouse, gh-notify webhook) | dead — superseded by relay + SP3 | dropped; close/retitle their Todoist tasks |
| S1–S4 (docs archive, CLAUDE.md audit rule, GH #5/#13 closures) | never executed; S3's "close #5 Nu Shell as wontfix" now **invalid** (decision 3) | SP7: re-rule S1/S2/S4; #5 stays open as the nushell tracking issue; #13 (Zellij) still closes wontfix |

Todoist hygiene is part of SP7: complete/close the dead tasks, re-point the surviving ones at their new
sub-projects.

## Deferred spec index (banked context — future specs start here, not from scratch)

- **SP3 — Notification rewrite.** Banked: presence routing (decision 2); Discord unconditional; Hue
  state-aware and home-gated; the four shipped bash fixes are unverified and get re-derived test-first;
  the spam bug's root cause (transcript-flush race + per-turn Stop-hook noise) needs a structural fix
  proven by a failing-then-green test; language choice fully open (Rust has repo precedent); Classist
  TDD + SOLID + composition-root architecture per the brief's section 8; agent-turn "failure" semantics
  for the red pulse are currently undefined and must be designed; `test/`-discoverable tests via a thin
  `just test` wrapper.
- **SP4 — nushell evaluation.** Go/no-go criteria to verify with evidence, not assume: atuin, starship,
  zoxide, direnv, carapace nushell integration quality; macOS login-shell semantics + the brew-shellenv
  cache analog; reedline keybinding parity with the current bash bindings; native
  `pre_prompt`/`pre_execution` hooks replacing bash-preexec for the notifier; herdr pane/spawn
  compatibility; an incremental migration path (opt-in pane first, cutover last). Output: a written
  recommendation; if go, a migration spec follows after SP3.
- **SP6 — nvim-overhaul.** Re-evaluate v1/v2/reassessment/v3 (v3 + its research doc live only on the
  unpushed `nvim-overhaul` branch — 10 commits at `~/.paseo/worktrees/1sk17y2x/nvim-overhaul`); then
  migrate `~/.config/nvim` (separate repo, 52 Lua files) into `dot_config/nvim/` under chezmoi and
  modernize per the re-evaluated design.
- **SP7 — Sweep + backlog.** The p-tasks table above + findings accumulated while reading every file
  during SP2's split + S1/S2/S4 re-ruling + Todoist hygiene. Ships as small PRs; nothing lands unwired.

## Verification (roadmap-level)

- SP1: `just test` fully green with the hermes tests enforcing; `git status` clean;
  `chezmoi diff` clean after the interactive apply.
- SP2: integration PR open + labeled; every slice PR green on lint/tests and merged after user review;
  cutover checklist passes (apply from `main`, `just test`, live smoke checks); PRs #31/#25 closed.
- Each deferred spec cycle ends with its own verification section; this roadmap only tracks that the
  cycle happened in sequence.
