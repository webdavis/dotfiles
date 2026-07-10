# Repo Modernization Roadmap — Design

**Date:** 2026-07-02 (extended 2026-07-03 with the full inventory, verified work ledger, SP3 contract,
and testing strategy; re-evaluated same day at max effort — six defects found and corrected, see the
final Verification section)
**Status:** approved via brainstorming; SP1–SP2 designed in full, **SP3 behavior contract approved —
final implementation spec pending** (contract below; open items enumerated in the deferred index),
SP4–SP7 indexed. Extended with a complete feature inventory and a work ledger of 85+ items (≈40 known +
45 sweep-confirmed) so nothing lives only in chat.
**Input:** `docs/plans/2026-07-02-repo-modernization-brief.md` (the full context brief — read it first;
this spec does not duplicate its background). The work ledger below folds in a max-effort, adversarially
verified whole-repo sweep (53 agents, 45 confirmed / 2 refuted findings).

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
   *Refined 2026-07-03 by the SP3 contract below, after live experiments: the "only"s apply to the
   confident cases — gray zone, lock, and probe failure send **both** surfaces; and the v1 home signal
   is the UDR + geofence-Shortcut + gateway-MAC module, with Home Assistant as a later addition, not the
   source.*
3. **nushell is back in scope, contingent on an honest evaluation.** The prior "Shell: bash — locked-in"
   toolchain line and GH issue #5's planned wontfix closure are superseded by explicit user instruction:
   evaluate the migration seriously (criteria in SP4 below) and pursue it if it holds up.
   **RESOLVED 2026-07-09 — NO-GO (see decision 9); GH #5 is closed and bash stays.**
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
7. **macOS system config stays chezmoi-native, not nix-darwin** (banked 2026-07-04 via deep-research).
   S10's `defaults write` + system-setup approach is affirmed: nix-darwin would fork the chezmoi/KeePassXC
   secret model and add a second apply path for a solo maintainer, with no offsetting win at this scale.
   A nix-darwin go/no-go (**SP-nix**) is deferred as a research-first sibling of SP4, not adopted inside
   SP2. Source: github.com/nix-darwin/nix-darwin.
8. **Secrets-at-rest stays chezmoi-native age** (affirmed 2026-07-04 via deep-research vs sops/sops-nix/
   agenix/git-crypt). One X25519 identity restored from KeePassXC is current best practice for a solo
   macOS chezmoi setup; the alternatives each add a second toolchain or a NixOS coupling the repo does
   not want. The migration caveats (multi-recipient for the home server, rotation/DR, git-history) are
   banked as S8 work and the deferred laptop→home-server item below. Source:
   chezmoi.io/user-guide/encryption/age.
9. **nushell migration: NO-GO — operator-ratified 2026-07-09** (resolves decision 3). The honest
   evaluation ran during S4 (report `docs/research/2026-07-09-sp4-nushell-evaluation.md`) and failed on
   three legs: reedline binds one key event per binding — no multi-keystroke chord grammar (reedline
   #69) — which the ~365-chord surface cannot survive; atuin's nushell integration is its weakest (and
   atuin is the thrice-broken subsystem here); and the cost/value fails against a working ~15 ms bash hot
   path. Adjacent shells were surveyed (zsh is the only chord-capable candidate) and the operator chose to
   **stay on bash**. GH #5 is closed; the SP4 slot becomes a bash-setup improvement sub-project, and SP3
   designs its notifier seam against bash-preexec with no shell-portability abstraction.
10. **S7 relay delivery-loss defects are fixed BEFORE merge — operator ruling (R2), 2026-07-10.** This
    reverses the earlier "ship the relay bash exactly as deployed" plan text. The four delivery-loss
    defects (fail-closed idle probe, whole-file `jq -rs` transcript slurp, wedged `mkdir` lock, missing
    flag value) can silently drop notifications, so they are fixed in S7 with characterization tests for
    any retained baseline quirk. SP3 still replaces the whole bash design later; S7 simply must not land a
    known notification-dropping baseline on `main`.

## Sub-project sequence

| # | Sub-project | Scope | Spec |
| --- | --- | --- | --- |
| SP1 | Unblock the tree | Hermes age encryption: scaffolding committed; operator key + capture remain | this spec |
| SP2 | Combine & split | Integration PR (#31 + #25) → reimplement from `main` as small PRs → cutover | this spec |
| SP3 | Notification rewrite | One Rust **stateless per-event executable** (NOT a daemon/service); subsumes hue-pulse improvements (old P7) | behavior contract approved; **final implementation spec pending** |
| SP4 | Bash setup improvement | nushell evaluated **NO-GO 2026-07-09** (GH #5 closed); the vacated slot becomes a bash-setup improvement sub-project | own spec (after SP3) |
| SP5 | Thaw install | Trivial standalone PR; slots in during SP2 | none needed |
| SP6 | nvim-overhaul | Re-evaluate v1/v2/v3 specs, then implement | own spec |
| SP7 | Sweep + p-tasks backlog | Small chores as interleaved PRs at the end | backlog list |
| SP-nix | nix-darwin go/no-go | **Conditional research only** — start triggers: larger Mac fleet / material maintenance failure in the defaults system / a proven design preserving single-apply + secrets model | own spec (deferred) |

**Update 2026-07-09:** the nushell evaluation is RESOLVED — **NO-GO, operator-ratified** (GH #5 closed;
report `docs/research/2026-07-09-sp4-nushell-evaluation.md`). SP3's shell-hook seam is therefore designed
against bash-preexec with no shell-portability abstraction, and the vacated SP4 slot becomes the
bash-setup improvement sub-project (own spec cycle, after SP3). See decision 9 below.

## SP1 — Unblock the tree (Hermes age encryption)

*(Status 2026-07-03: all scaffolding is **committed** on the working branch — `0085c4b`, `2e8ca53`,
`3be7312` — and the tree is clean. What remains is the operator key ceremony plus the agent steps
below.)* The scaffolding: `.chezmoidata/hermes.yaml` (version pin),
`run_onchange_before_25-install-hermes-agent.sh.tmpl` (pinned install),
`run_after_67-hermes-config-migrate.sh.tmpl` (headless migrate), `test/hermes-config-encrypted.sh`
(secret-leak guard; currently skips pre-migration), `test/hermes-config-routes.sh` (route integrity),
plus the gitleaks pre-commit gate, `.gitignore` failsafe block, and `scripts/lint.sh` wiring.

**Operator steps (interactive, KeePassXC unlocked — the agent cannot do these):**

```bash
mkdir -p ~/.config/chezmoi
chezmoi age-keygen --output=$HOME/.config/chezmoi/key.txt   # prints the public key
chmod 600 ~/.config/chezmoi/key.txt
# KeePassXC entry "chezmoi :: Private Key :: age": paste ONLY the AGE-SECRET-KEY-1... line
# into the PASSWORD field (single line, house pattern — same field every other
# template reads via keepassxc(...).Password).
```

(Chezmoi-driven keygen per the official age guide — one toolchain end to end. A key file containing
only the `AGE-SECRET-KEY` line is a complete identity — proven by round trip: identical derived public
key, successful encrypt/decrypt with the stripped file; the `#` comment lines are decoration. The
public key lives in `.chezmoi.toml.tmpl` and is also re-derivable from the secret via
`age-keygen -y`.)

**Agent steps (after the key exists):**

0. **Fix the age-tripwire self-match first** (sweep, high): `test/hermes-config-encrypted.sh:28`'s
   `grep -rlq 'AGE-SECRET-KEY-1'` matches its own source file, so the guard's first *enforcing* run —
   the moment the `.age` file exists — fails every commit with a false leak alarm. Exclude the test
   itself (or split the marker string) before anything below.
0b. **Fresh-machine key restore (so new machines need zero new manual steps):** add a
   `run_once_before` template script that, when `~/.config/chezmoi/key.txt` is missing, writes the
   `chezmoi :: Private Key :: age` entry's **Password** attribute (the single `AGE-SECRET-KEY` line — a
   complete identity, proven by round trip) to the file 0600, via the same `keepassxc(...).Password`
   template call every other secret uses — riding the one DB unlock chezmoi already prompts for and
   caches per apply. Generation happens once ever (the operator ceremony); every future machine
   *restores* the same key. Add the matching line to
   `docs/runbooks/macos-fresh-machine-quickstart.md`.
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
6. `just l && just test` (the two hermes tests flip from skip to enforcing), then commit the capture +
   `.chezmoi.toml.tmpl` change + old-mechanism removal as logically separate commits.
7. Operator afterward: `trash` the world-readable plaintext backups in `~/.hermes/`
   (`config.yaml.bak.*`, `config.yaml.*.backup`) — destructive, per-invocation confirmation.

## SP2 — Combine & split

### Combine (Approach A: merge-combine)

*(Status 2026-07-03: the merge itself already happened — `feat/osquery-alerter-three-tier` was merged
directly into the working branch at `f7220d9` on explicit user instruction; the one predicted conflict
(`.chezmoiignore`) was resolved and the duplicate `justfile` `test` recipe reconciled. The union now
lives on `feat/cli-agent-tracking-workflow`/PR #31.)*

1. `git checkout -b integration/modernization` at the current working-branch head — the union already
   exists there; no further merge needed.
2. Push; open a **draft PR titled "DO NOT MERGE — integration reference (modernization)"** via
   `gh-axi pr create`. Body links this spec and states the branch's role: frozen live-state reference.
3. dresden keeps its chezmoi source on the working branch until cutover. **Freeze policy:** no new
   feature work lands here; only hotfixes needed to keep dresden healthy (the tailscale-monitor and
   credential-perms fixes are the precedent), and every hotfix must also be folded into its
   corresponding reimplementation slice.

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
7. Relay notification pipeline (bash version + its tests). ~~**As deployed** — lands so `main` matches
   dresden's live behavior~~ **superseded 2026-07-10 by decision 10 (R2):** the four delivery-loss
   defects are fixed BEFORE merge (with characterization tests for retained harmless quirks); SP3 still
   replaces the whole design later through its own PR sequence.
8. Hermes age-encryption (the SP1 work).
9. osquery three-tier alerting — **reimplemented with design improvements** (decision 4). Alerting/
   dispatch redesign is in scope; query/pack content changes are flagged for user sign-off.
10. macOS defaults / system-setup additions.
11. Small chores interleaved. *(Annotated 2026-07-10 [audit]: Thaw ships as a **standalone SP5 PR**, not
    inside S11; the old-P12 gitconfig autocorrect is **already on `main`** — no work. S11 itself splits
    into the audit's 7 small PRs; see the SP2 plan's S11 section.)*

### Cutover

**Amended 2026-07-10 [audit]: the cutover is FIVE sequential gates, not one shot** — the authoritative
procedure is the SP2 plan's Task D1
(`docs/superpowers/plans/2026-07-03-sp2-combine-and-split.md`, Phase D): **Gate 1** preflight (fetch and
PIN both remote SHAs; dirty/untracked classification ending in an **empty tree** — kept files move
outside the source tree, must-ship changes are committed before pinning; Hermes-state backup; the
**operator-approved retirement manifest** — desired-state services from the pinned SHA vs live loaded
jobs, since a `launchctl` before/after diff cannot find orphans; and the expected-delta ledger — built
from the immutable recorded-base→integration manifest and classified against the pinned `main` SHA —
that replaced the empty-diff gate); **Gate 2** staged activation **and execution of the approved
retirement** (second remote session; re-verify `origin/main` still equals the pinned SHA, then activate
**attached to `main`** at it, never detached; staged `chezmoi apply`; managed `launchctl bootout`/plist
removal per approved manifest entry; verify remote reachability); **Gate 3** tracked live reconciliation
(the pre-built, pinned `scripts/live-reconcile.sh` from the cutover-tooling PR) **plus post-retirement
verification** (assert against the manifest: approved-retired absent, desired present) and `just test` +
live smoke checks (relay fires, hermes gateway healthy, osquery alerter behavior verified); **Gate 4**
soak of the **final, retired** topology; and **Gate 5** closure-only (re-verify the pin first — if
`main` advanced mid-soak, re-classify and re-soak) — **PRs #25, #31, and the integration reference
PR #32 are closed only after the soak passes**, each with pointers to the landed slices. Retirement
happens in Gates 1–3 (approve / execute / verify), not Gate 5, so the topology soaked is the topology
closed out.

## p-tasks re-evaluation (from `2026-05-15-dotfiles-tasks-design.md`)

| Old phase | Status now | Disposition |
| --- | --- | --- |
| P3 package-manager audit (GH #11) | still valid | SP7 backlog |
| P4 gitleaks pre-commit | **already implemented + committed** (`2e8ca53`) | ships via SP2 slice 2 |
| P5 Determinate Nix review (GH #10) | still valid, research-first | SP7 backlog |
| P6 bandwhich/doggo/ouch | still valid, trivial | SP7 backlog |
| P7 hue-pulse improvements | superseded | folds into SP3 (race fix already shipped) |
| P8 quick wins (MANPAGER, fd/FZF, starship) | **UNBLOCKED** — shell-config placement = bash (nushell NO-GO, decision 9) | SP7 (no longer gated on SP4) |
| P9 actionlint | still valid | SP2 slice 2 |
| P12 gitconfig autocorrect | **already on `main`** (`autocorrect = prompt`, verified 2026-07-09) | no work — done |
| P13 Tart base image (~25 GB pull) | still valid, deferrable | SP7 backlog, background |
| P10/P11/N1 (OpenClaw queue, mouse, gh-notify webhook) | dead — superseded by relay + SP3; **OpenClaw removal ruled (R3, 2026-07-10)** | Wave-3d PR owns package/F1-binding/docs removal; **operator owns the Todoist cleanup** |
| S1–S4 (docs archive, CLAUDE.md audit rule, GH #5/#13 closures) | never executed; GH **#5 is now closed** (nushell NO-GO, decision 9) | SP7: re-rule S1/S2/S4; #13 (Zellij) still open |

Todoist hygiene is part of SP7: complete/close the dead tasks, re-point the surviving ones at their new
sub-projects.

## Deferred spec index (banked context — future specs start here, not from scratch)

- **SP3 — Notification rewrite: behavior contract approved; final implementation spec PENDING**
  [audit 2026-07-10]. The contract section below is approved, but SP3 is **not "designed in full"** — the
  implementation spec still has open items. Superseded banked framing: "spam root cause = transcript-flush
  race" (resolved: `jq -rs` slurp bug, no loop — fire-once already holds) and "language fully open"
  (settled: Rust). **SP3 is a stateless per-event EXECUTABLE, not a daemon/service** (any "service"
  wording is loose). The **UDR client-list probe is RESOLVED** (verified live 2026-07-03, zero open
  questions — see the home-module section) and is **removed** from the open list. Still open for the final
  SP3 spec cycle:
  - **event input schemas** — the hook/CLI event payloads;
  - **per-harness event mapping** — which Claude/Codex/CLI event maps to which class;
  - **native-push ownership (R7)** — disable Claude Code's "Push when Claude decides" and keep the single
    relay-owned route; drive the waiting-on-you vs ready-for-you classes from the **native Notification
    matchers** (`agent_needs_input` / `agent_completed`); confirm native-push coexistence;
  - **per-channel retry and failure behavior** — bounded retry with a loud fallback;
  - **presence thresholds** — the gray-zone idle tuning constants;
  - **lights quiet-window hours**;
  - **migration coexistence and rollback**;
  - **acceptance boundaries** — what the acceptance tests assert, plus the bats-vs-cargo split for the
    thin shell shims (and agent-turn *failure* semantics for the red pulse).

  The four previously-shipped bash fixes are re-derived test-first in Rust; Classist TDD + SOLID +
  composition root per the strategy section; `test/`-discoverable via a thin `just test` wrapper.
- **SP4 — nushell evaluation: RESOLVED — NO-GO, operator-ratified 2026-07-09** (decision 9;
  report `docs/research/2026-07-09-sp4-nushell-evaluation.md`; GH #5 closed). The go/no-go criteria were
  evaluated with evidence — reedline's one-event-per-binding limit (no chord grammar) sank the ~365-chord
  surface, atuin's nushell integration is its weakest, and cost/value failed against the ~15 ms bash hot
  path. The **SP4 slot now becomes a bash-setup improvement sub-project** (own spec cycle, after SP3):
  alias consolidation, an inverted single-table bindings architecture, `fzf` bindings quality/new
  bindings, generated per-binding tests, and a recorded Charm-tools verdict (zero new dependencies now).
  SP3 designs its notifier seam against bash-preexec with no shell-portability abstraction.
- **SP6 — nvim-overhaul.** Re-evaluate v1/v2/reassessment/v3 (v3 + its research doc live only on the
  unpushed `nvim-overhaul` branch — 10 commits at `~/.paseo/worktrees/1sk17y2x/nvim-overhaul`); then
  migrate `~/.config/nvim` (separate repo, 52 Lua files) into `dot_config/nvim/` under chezmoi and
  modernize per the re-evaluated design. **Audit directives [audit 2026-07-10] (`nvim-overhaul` was 69
  commits behind / 3 ahead of `origin/main` at audit time) — before any modernization:** (1) re-check
  the branch state; (2) back up both repositories; (3) inventory the live Neovim configuration;
  (4) import the live configuration UNCHANGED first; (5) modernize only through later reviewable PRs.
- **SP-nix — nix-darwin go/no-go (deferred, research-first).** Sibling of SP4 (nushell): evaluate whether
  nix-darwin should manage macOS system config instead of chezmoi's `defaults`/system-setup scripts.
  Banked prior (decision 7): chezmoi holds for now; this is the formal re-evaluation, cross-referenced to
  P5 (Determinate Nix). Not part of SP2.
- **Age laptop→home-server migration (deferred).** When the always-home Linux server takes the daemon
  role: it generates its own X25519 identity, `.chezmoi.toml.tmpl` switches to a **`recipients` list**
  (both machines) so files encrypt to both. Generalizing the darwin guard on
  `run_once_before_05-restore-age-key` is **blocked on a complete, approved, and tested Linux
  credential-source and identity design** [audit 2026-07-10] — the current macOS paths + KeePassXC
  assumptions do NOT constitute Linux support, so the guard stays until that design exists. Plus the S8
  rotation/DR runbook. Banked from the 2026-07-04 secrets-at-rest research.
- **SP7 — Sweep + backlog.** The p-tasks table above + findings accumulated while reading every file
  during SP2's split + S1/S2/S4 re-ruling + Todoist hygiene. Ships as small PRs; nothing lands unwired.

## Verification (roadmap-level)

- SP1: `just test` fully green with the hermes tests enforcing; `git status` clean;
  `chezmoi diff` clean after the interactive apply.
- SP2: integration PR open + labeled; every slice PR green on lint/tests and merged after user review;
  the five-gate cutover passes (D1 **Gate 1** clean tree + expected-delta ledger against the pinned
  `main` SHA + approved retirement manifest; **Gate 2** pin re-verified, staged apply attached to `main`
  at that SHA + execution of the approved retirement; **Gate 3** tracked reconciliation + manifest-based
  post-retirement verification + `just test` + live smoke checks; **Gate 4** soak of the final topology;
  **Gate 5** pin re-verified, closure only); PRs #25/#31/#32 closed **only in Gate 5, after the soak
  passes**.
- Each deferred spec cycle ends with its own verification section; this roadmap only tracks that the
  cycle happened in sequence.

## Feature inventory (complete)

What exists in the repo today, so the modernization is measured against reality. Grouped by subsystem;
corrected against a completeness critic (folded in the ~11 features an earlier draft missed).

**A. Chezmoi machinery.** Source-state conventions (`dot_`/`private_`/`executable_`/`run_*`/`modify_`/
`symlink_`/`encrypted_`); `.chezmoiversion` ≥ 2.62.3; **the repo IS the source dir** (`.chezmoi.toml.tmpl`
pins `sourceDir`/`workingTree` to `~/workspaces/Ivy/webdavis/dotfiles`, no `~/.local/share/chezmoi`
clone); KeePassXC DB in the iCloud Strongbox folder; `.install-password-manager.sh` read-source-state
pre-hook; `.chezmoiignore` repo-meta + OS gating + `CHEZMOI_SKIP_SYSTEM_PACKAGES` escape hatch; planned
`age` whole-file encryption (SP1). `AGENTS.md` is a symlink to `CLAUDE.md` (one instruction file for all
harnesses).

**B. Package management.** Declarative brew (taps/trusted_taps/formulae/casks/mas) + uv/npm/volta →
generated Brewfile + `brew bundle --cleanup` (`run_onchange_before_10`); bootstrap chain (homebrew
`once_before_00`, rustup `before_20`, composio `once_before_30`, herdr curl-install preview channel
`before_15`, hermes-agent pinned `before_25`); weekly Monday-noon upgrade LaunchAgent + helper (+
tailscaled re-copy); `just brew-upgrade`.

**C. macOS defaults.** Tier 1 (`macos_defaults.yaml` + `after_30`, defaults write + killall); Tier 2
(`macos_system_setup.yaml` + `after_41`, sudo loop, nix-repair LaunchDaemon); CLIs
`macos-defaults-{capture,drift,apply}.sh` + `just D`/`defaults-*`; ByHost support; `drift.sh` needs
`shopt -s lastpipe`.

**D. Shell environment (bash).** `dot_bashrc.tmpl` non-interactive-safe PATH half + interactive guard
half; **login-shell carrier files** `dot_profile` (interactive-gated bashrc source, blocks herdr's
`/bin/sh -lc` from ~90ms init) + `dot_bash_profile`; brew shellenv cache (regen `after_44` + drift
self-heal + test); atuin daemon (LaunchAgent + `--force` + upgrade bounce `after_45` + mtime self-heal);
bash-preexec before atuin; direnv→starship→zoxide→atuin ordering; carapace; herdr auto-attach tail;
`SHELL`→brew bash + `MANPAGER="nvim +Man!"`; starship dual config (full/mosh ASCII); ~500 readline
bindings (`.bash_bindings` vi+emacs) + fzf widgets (`.fzf_bindings`); aliases (`.bash_aliases` + more in
bashrc incl. `rm=trash-put` at `dot_bashrc.tmpl:235`); functions (`.bash_functions`); `.inputrc` vi
mode; long-running-command notifier (≥60s local, ≥300s full fan-out + hue) via bash-preexec.

**E. Notification system (current bash; SP3 rewrites in Rust).** `relay.sh` fan-out (moshi phone / Hermes
→ Discord `#relay` HMAC / terminal-notifier clickable banner; HIDIdleTime gate; 260-char sentence
preview; secrets from 0600 `auth.json`); `relay-agent.sh` (hook JSON → transcript extract → codex summary
w/ loop guard + python fallback); `relay-codex-hooks.sh` (idempotent codex hook merge, `after_72`);
`hue-pulse.sh` (green/red pulse + restore, mkdir lock); `claude-stop-pulse.sh` (≥5min gate);
`claude-user-prompt-start.sh` (marker); Claude hooks in `modify_settings.json` (Stop / Notification
[permission_prompt] / PostToolUse[AskUserQuestion, ExitPlanMode] / PreToolUse[Bash] audit /
UserPromptSubmit).

**F. osquery security system (three-tier: page / digest / log-only).** Root config render (`before_50`):
`osquery.conf` + 4 packs (intrusion-detection, security-policy-regression, installed-software-drift,
agent-attack-surface) + flags + file-path hashing; results-alerter (WatchPaths + 300s, offset state,
allowlist gate, enrich-finding trust check, digest spool); alert-dispatch lib (local alerter always,
CRIT → Hermes `#priority` w/ separate HMAC secret, spool/drain, delivery log); allowlist CLI (sole
writer); digest builder (daily silent summary + rotation); heartbeat (09:00); firewall/gatekeeper 60s
poller; tailscale funnel 60s poller (**now alive** — PATH resolution + fail-loud, fixed this session);
uptime watchdog (15min); pipeline tamper baseline (`pipeline-known-good.sha256`, `after_55`); 6
LaunchAgents + loaders; 87-test bats suite + `lib.bash` harness. `.chezmoidata/osquery.yaml`
digestHour/Minute drive the schedule. **Off-limits to redesign except its alerting/dispatch layer during
SP2 slice 9; query/pack content changes flagged for sign-off.**

**G. Services / LaunchAgents (non-osquery).** atuin daemon (KeepAlive, `--force`); happy daemon
(`start-sync` foreground gotcha); yt-dlp POT provider (node, KeepAlive, ThrottleInterval); homebrew
weekly upgrade (Mon 12:00); update-skills (Mon 04:00 — **currently unloaded, no loader script**).

**H. herdr (multiplexer).** `config.toml` (prefix `ctrl+d`, catppuccin, 8 workspace jump chords, plugin
action bindings, `switch_ascii_input_source_in_prefix` CJK fix); herdr-smart-nav Rust plugin (ctrl-hjkl
nav, `after_57`); herdr-last-workspace Rust plugin (MRU, `after_55`); bashrc auto-attach; native agent
status in sidebar.

**I. Terminal / GUI apps.** Ghostty (`bash -i`, themes, quick-terminal); AeroSpace (hyper-key workspaces,
F-key smart-lights/openhue, terminal-notifier feedback, F1 → OpenClaw `127.0.0.1:18789` — **R3
(2026-07-10): OpenClaw removal ruled; the F1 binding, the `openclaw` package, and the docs are removed by
the Wave-3d cleanup PR, operator owns the Todoist cleanup**); Karabiner
(caps→ctrl/esc, tab→hyper, complex-mod library); espanso (identity from KeePassXC, autocorrect packs,
prompts/snippets, `_pqi.yml` phone quick-replies); smart-lights CLI (openhue room controller);
`aerospace_toggle_app_focus.sh`.

**J. Git tooling.** `dot_gitconfig.tmpl` (GPG signing key ID via KeePassXC, delta + custom theme, gh
credential helper); user-wide hooks (`core.hooksPath`): prepare-commit-msg AI messages (claude sonnet,
`SKIP_AI_COMMIT`), pre-commit dispatcher → `.githooks/pre-commit` (`just lint-check` + `just test` +
gitleaks staged scan); worktrunk (squash+rebase, haiku commit-gen, herdr-mirrored worktree paths).

**K. Agent / AI tooling.** `modify_settings.json` (stable-fields overlay: permissions allow/deny,
bypassPermissions, hooks, statusLine, enabledPlugins, cleanupPeriodDays 36525, autoUpdatesChannel stable,
remoteControlAtStartup, `effortLevel xhigh`); **`modify_private_dot_claude.json`** (a *second* Claude
modify-template — vaults `mcpServers.composio` + `mcpServers.workspace-mcp` secrets into `~/.claude.json`)
+ the Desktop `modify_private_claude_desktop_config.json`; `statusline-command.sh` (Tokyo-Night);
`agents/chezmoi-apply.md`; `commands/pr-merge.md`; skills store `~/.agents/skills` + symlink fan-out
(`~/.claude`, `~/.hermes`) + `update-skills.sh` (idle-gate, npx-dir relocation, portable roster,
skill-lock manifest) + weekly LaunchAgent; vendored skills (herdr, moshi, deep-research, todoist-cli,
lobster) + a stale repo-root `.agents/skills/moshi-best-practices/` (mangled — flagged for removal)
*(superseded 2026-07-10 [audit]: S3 shipped the 31-skill store under `dot_agents/custom-skill-lock.json`
— npx/clawhub/vendored-fork/app-owned lanes + disjoint five-profile hermes delivery; the "skill-lock
manifest"/`skills-lock.json` name and this vendored-set snapshot are historical, see the amended SP2-plan
S3 section)*;
tool-pref `gh-axi` / `chrome-devtools-axi`; `claude-audit.sh`; Moshi pairing + 8-CLI hooks (claude+codex
excluded — relay owns); codex relay hooks.

**L. Secrets.** KeePassXC single source; templates: relay auth.json, hermes .env (Discord/Anthropic/
ElevenLabs/OpenRouter/Tavily + HASS_URL + Browserbase), aws credentials (**now `private_`**), composio,
gogcli, himalaya Proton (**now `private_`**), atuin history_filter, espanso identity, claude_desktop +
`~/.claude.json` MCP secrets, gitconfig signingkey; gnupg (keyboxd, pinentry-mac); gitleaks pre-commit
gate; `.gitignore` failsafes (`*bash_secret*`, hermes plaintext backups, `*key.txt`, `.worktrees/`,
Raycast exports); claude deny-list.

**M. Hermes agent.** Pinned install (`before_25`); config migrate (`after_67`); relay route via yq
modify-template (`after_68` reminder) — **SP1 replaces with age-encrypted config**; gateway
`127.0.0.1:8644` (`/webhooks/relay` + `/webhooks/osquery-priority`).

**N. Tailscale.** Headless tailscaled system daemon (formula, `/Library/Tailscale` state); weekly
re-copy; status reminder (`after_66`); GUI-cask future note; runbook. **Decision history (dated
2026-07-10):** the June `sudo brew services` decision (PR #31, commit `3b43d67`) was **superseded during
execution** — running `brew services` under `sudo` changed ownership of the Cellar/opt/bin/sbin paths to
root and broke the unattended weekly upgrade, so commit `01d15ad` switched to
`tailscaled install-system-daemon` (a root-owned `/usr/local/bin` copy; the formula stays user-owned).
The older June tailscaled spec/plan that still name brew-services as the final decision are superseded by
that discovery (this is the roadmap's own copy of the supersession note; the June docs carry theirs from
PR #38).

**O. Dev / CI.** Nix flake (default + run shells: chezmoi/shellcheck/shfmt/mdformat+gfm/nixfmt/taplo/jq/
yq/bats); `scripts/lint.sh` (priority runners, template rendering, osquery config render); justfile;
`test/` (9 hand-rolled `.sh` + osquery bats, `just test` = sh loop + bats-in-nix); GitHub Actions
`lint.yml` (macos-latest: flake check + `lint.sh -c`); `.editorconfig`/`.shellcheckrc`/`.mdformat.toml`;
`skills-lock.json` *(→ `dot_agents/custom-skill-lock.json` as of 2026-07-10)*; small tool configs
(nix.conf, docker daemon.json, gh config, bat, yt-dlp, ssh config
with Raspberry Pi hosts).

**P. Docs.** `docs/runbooks/macos-fresh-machine-quickstart.md`; `docs/superpowers/{specs,plans}`;
`docs/plans` (briefs); `docs/research`.

## Complete work ledger

Everything to do, merged from the known backlog and the verified sweep. `sev` = high/med/low;
`SP` = target sub-project. `[FIXED]` = done this session. Sweep findings carry file:line; each was read
against source and adversarially verified (2 candidates were refuted and dropped).

### Do these first (sequenced — before any other ledger work)

1. ~~Interactive `chezmoi apply`~~ **DONE 2026-07-03** (three upstream/setup bugs found and fixed en
   route: Homebrew 6.x bundle-cleanup behavior change, moshi re-pair on consumed token + pipefail guard,
   tap re-trust after `brew update`). Credentials at 0600, CLAUDE.md normalized, osquery heartbeat live.
2. ~~UDR API key + client-list probe~~ **DONE 2026-07-03** — results banked in the SP3 home-module
   section above.
3. ~~SP1 key ceremony + agent steps~~ **DONE 2026-07-03** (`c13cc18`, `a0e7d8e`, `3696c92`): encryption
   live, config captured encrypted + round-trip exact, old yq mechanism retired, fresh-machine restore
   script in place, both hermes guards enforcing, full suite green. SP1 is complete.
4. **Next: writing-plans** for SP2 execution. Non-blocking operator odds and ends whenever convenient:
   `trash` the plaintext `~/.hermes/config.yaml.bak.*` backups (destructive — confirm per file);
   `brew uninstall goplaces`; phone-side Focus whitelist + Discord `#relay` mobile mute. (OpenClaw removal
   is **not** an ad-hoc operator action — the Wave-3d cleanup PR owns the `openclaw` package, the
   AeroSpace F1 binding, and the docs together per R3.)

### Fixed this session (baseline — already on the branch)

- `[FIXED]` osquery-tailscale-monitor dead GUI-binary path → PATH resolution + fail-loud + 4 bats (`2f430b3`).
- `[FIXED]` secrets: `private_` on `dot_aws/credentials.tmpl` + `dot_config/himalaya/config.toml.tmpl`
  (were deploying 0644) (`ae02524`) — **operator must interactive-apply to re-deploy at 0600.**
- `[FIXED]` moshi `SKILL.md` frontmatter; justfile bats-in-nix + merge de-dup; memory-lancedb-pro-skill
  removed; deep-research self-referential symlink removed; moshi-hook 0.2.32→0.2.37.
- `[FIXED]` `~/.claude/CLAUDE.md` both-ways fork reconciled in source (`4ac97de`): live-only SOLID/
  Testing/Browser/git-crypt/gh-axi lines folded in; next interactive apply is purely additive on live.

### Bugs — high severity

| Item | Where | SP |
| --- | --- | --- |
| Idle probe fail-**closed** aborts all channels if HIDIdleTime absent (defeats documented fail-open) | `relay.sh:68` | **S7 (blocker)** |
| `jq -rs` whole-file slurp: one half-written trailing line → empty `(main) done`, loses all turns | `relay-agent.sh:17` | **S7 (blocker)** |
| SSH password auth still works — drop-in sets only `PasswordAuthentication no`; `UsePAM yes` + `KbdInteractiveAuthentication` default leave PAM login open (verified live) | `ssh-hardening.sh:13` | SP2/SP7 |
| Age-key tripwire matches **its own source** → the moment SP1 lands, every commit fails a false leak alarm | `test/hermes-config-encrypted.sh:28` | SP1 |
| bats count assertions false-pass on zero (`grep -c` → `"0\n0"` makes `[[ -ne ]]` error silently — re-verified by hand, expected-3-got-0 passes) | `test/osquery-alerter/lib.bash:384` (+418,462,206) | SP2 s2 |

**[R2 / decision 10, 2026-07-10] The four relay delivery-loss defects — the idle-probe fail-closed and
`jq -rs` slurp rows above, plus the `hue-pulse.sh` stale `mkdir` lock and the `relay.sh` flag-parse abort
in the medium/low list below — are retagged from SP3 to **S7 BLOCKERS**: they are fixed in S7 BEFORE it
merges (they silently drop notifications), not deferred. SP3 later re-derives their behavior test-first in
Rust — these four bash fixes are its failure spec (see the SP3 "Bugs the rewrite must not reproduce"
section), so the retag does not remove them from SP3's scope, it just forbids merging S7 with them open.**

### Bugs — medium/low (open)

- `claude-stop-pulse.sh:18` empty/garbage marker → arithmetic abort; marker never cleaned (poisons every
  later Stop) [SP3].
- `hue-pulse.sh` mkdir lock: no stale-lock recovery after SIGKILL (30s spin → silent no-op) [**S7 blocker**].
- `relay.sh` flag parse: value-flag as last arg → `shift 2` abort (breaks "always exits 0") [**S7 blocker**].
- `smart-lights`: `--scene previous` negative index breaks on bash 3.2; getopt errors invisible (exec
  redirect before parse); `-n/--next`/`-l/--last` declared but unhandled; `toggle_power` skips
  `validate_room_id` [SP7].
- `dot_bashrc.tmpl:301` TUI-skip regex unanchored (`topgrade`/`sshpass`/`lessc`/`manage.py` never notify) [SP3].
- `merge.tool = <raw command>` (must be a tool *name*) → `git mergetool` never uses nvim (`dot_gitconfig.tmpl:50`) [SP7].
- `core.excludesfile = ~/.gitignore_global` — file never deployed, doesn't exist → zero global ignores
  (`dot_gitconfig.tmpl:18`) [SP7].
- `find-and-remove-json-objects.sh:40` jq single-quoted string = guaranteed compile error; script has
  zero callers — **remove** [SP7].
- espanso `_pqi.yml` never loads (leading `_`, no `imports:`) → all PCI triggers inert [SP7].
- espanso 9 triggers unreachable (shadowed by shorter prefixes: `;;re` kills `;;review`) [SP7].
- espanso autocorrect fires mid-word without `word:true` → `wonton`→`won'ton` [SP7].
- osquery-heartbeat `RunAtLoad=true` → extra pings on every login/reload, breaks one-per-day contract
  [osquery — flag for sign-off].
- AeroSpace service-mode `join-with` j/k inverted vs every other hjkl binding (`dot_aerospace.toml:326`) [SP7].
- Arc browser hotkey (`dot_aerospace.toml:178`) — Arc never installed; Zen (installed) sits commented below [SP7].
- `.github/workflows/lint.yml:13` nix-installer-action pinned to mutable `@main` (supply-chain) [SP2 s2].
- Pre-commit runs full 87-bats suite on every commit incl. docs-only (friction tax) [SP2 s2].
- Known bug set (unchanged, all SP3/SP2): relay-codex corrupt-json silent skip; `claude-user-prompt-start`
  unvalidated session_id in /tmp path; lint.sh `just <tool>` runs full suite in write mode; CI can't fail
  on format drift; CI runs no tests; update-skills no loader + no log dir; lint template allowlist omits
  ~19; `SKIP_SYSTEM_PACKAGES=0` still skips; `run_after_35` network-on-every-apply + deno/node mismatch;
  `after_41` fragile `{{ if .sudo }}`; `before_10` uv/npm/volta unguarded; `lint.sh:64` `.#adhoc`;
  `after_68` POSTs `{}` every apply; `after_44` mktemp leak; `lint.sh:354` vestigial `-r` crashes under
  `set -u`; dot_fzf_bindings tmux-dead widgets + diff-so-fancy; dot_bash_bindings dup key + nvm/`$blue`;
  hermes `.env` stale `MESSAGING_CWD`; inert `gh hosts.yml.tmpl`; ghostty comment typo.

### Consolidations

- brew-shellenv regen implemented **3×** (`after_44` atomic, bashrc self-heal atomic, `justfile:73`
  **non-atomic** truncate-race) + prefix/path constants dup in the drift test → one deployed helper [SP7].
- macos-defaults trio dup the hardcoded repo path + yq record expr + bool normalize; worktree runs hit
  the **primary** checkout's YAML → shared lib + `chezmoi source-path` [SP7].
- Two herdr plugin build/link scripts ~85% copy-paste (+ unanchored `grep -q "$plugin_id"` link check) →
  `.chezmoitemplates` partial [SP2 s4].
- workspace-mcp server block (with secrets) dup across 2 modify-templates; stdin preamble across 3 [SP7].
- Hermes port `8644` hardcoded in **6** files across 2 config systems → single-source [SP7].
- 5 non-osquery LaunchAgent plists 90% identical → generate from one partial + per-service data [SP7].
- `find` prune set repeated **6×** in lint.sh and already drifted (`.direnv` missing in one) [SP2 s2 /
  treefmt].
- Two owners create `~/.claude/skills` symlinks differently (chezmoi `symlink_` vs update-skills.sh) [SP2 s3].
- Two HMAC/webhook dispatch impls (relay inline python vs osquery dispatch) — respect the deliberate
  two-secret split [SP3 boundary].

### Moves / placement

- `ssh-hardening.sh` wired nowhere → a record in `.chezmoidata/macos_system_setup.yaml` (Tier 2) so a
  fresh machine actually locks sshd [SP2/SP7, security].
- `just brew-upgrade` runs the **source** script while every sibling recipe runs the **deployed** copy →
  point at `~/.local/bin` (`justfile:79`) [SP7].
- Only 2 of ~10 store-skill fan-out symlinks are chezmoi-declared; rest are runtime state → declare all
  so a fresh apply reproduces the skill surface [SP2 s3].
- `aerospace_toggle_app_focus.sh` is the lone snake_case script → rename kebab-case with its binding [SP7].
- atuin `history_filter` (a regex, low-sensitivity) + espanso `identity.yml` (PII) deploy 0644 — consider
  `private_` [SP7, low].

### Removals

- `find-and-remove-json-objects.sh` — dead + syntactically broken (also a Bug above) [SP7].
- git `git://` URL rewrites (github:/gist: shorthands) — GitHub killed the protocol in 2022; they hang
  (`dot_gitconfig.tmpl:103,111,119`) [SP7].
- `~/.bash_just_completions` source (unmanaged, Feb-2024, absent on fresh machine; carapace already
  completes `just`) (`dot_bashrc.tmpl:182`) [SP7].
- atuin `~/.atuin/bin/env` guard — vestigial curl-installer remnant; atuin is brew-managed
  (`dot_bashrc.tmpl:60`) [SP7].
- Linux-branch `.config/yabai` ignore — yabai removed (`6721f7c`); aerospace replaced it (`.chezmoiignore:57`) [SP7].
- Stale repo-root `.agents/skills/moshi-best-practices/` (mangled frontmatter, tmux-era, orphan
  lockfile) — distinct from the fixed `dot_agents/skills/moshi/` [SP2 s3].
- Runbook TCC grants name Hammerspoon + Rectangle — neither installed/managed [SP7 docs].
- justfile tombstone comment; `check` recipe unwired; `dot_config/herdr/.config.toml.swp` stray swap [SP7].

### Design alternatives (creative-liberty proposals — decide during their slice)

- Replace the 510-line hand-rolled `lint.sh` framework with **treefmt-nix** driven from the flake
  (deletes ~450 lines whose bug class bit twice; per-file caching; one exclude list) [SP2 s2].
- Split `run_onchange_before_10` per ecosystem so a one-line npm/uv/volta edit doesn't re-run
  `brew bundle --cleanup` [SP7].
- Wire `~/.codex/hooks.json` via a chezmoi **modify-template** instead of `after_72`'s every-apply script
  (merge becomes visible in `chezmoi diff`; corrupt-json turns loud) [SP3/SP7].
- macOS-native **newsyslog** log rotation, chezmoi-managed (audit.log already 8.7 MB; paseo hit 102 MB) [SP7].
- Generate the 5 non-osquery plists from one `.chezmoitemplates` partial + per-service data [SP7].
- `.chezmoi.toml.tmpl` hardcodes `/Users/stephen` paths in a file meant to render on a *new* machine →
  derive from `.chezmoi.homeDir`/`.chezmoi.username` [SP7, fresh-machine correctness].
- Pre-commit: skip the bats suite on docs/YAML-only commits (path filter) [SP2 s2].

### Installs (new tooling — user directives)

- **ponytail** (DietrichGebert/ponytail, MIT) — a least-code-that-works ruleset plugin. Install per
  harness: Claude Code `/plugin marketplace add DietrichGebert/ponytail` then
  `/plugin install ponytail@ponytail`; Codex `codex plugin marketplace add DietrichGebert/ponytail`;
  Hermes `hermes plugins install DietrichGebert/ponytail --enable`. After the Claude install, promote
  `ponytail@ponytail` into `modify_settings.json`'s `enabledPlugins` stable list so the plugin is
  declarative like the rest. [SP7 / SP2 chores slice]
- **Skills keep-list superseded (2026-07-03):** restored to the store + all three harness fan-outs:
  `conventional-commits` (required commit-policy skill — restored before further implementation, user
  gate), `humanizer`, `video-transcript-downloader`, `hyperframes`/`-cli`/`-media`/`-registry`,
  `website-to-hyperframes` (all from git, `4adcfee`), and `kubernetes-specialist` (re-cloned from its
  roster source `jeffallan/claude-skills`). `cua-driver` was never removed; the `playwright` *plugin*
  remains enabled (assumed to satisfy "all playwright skills" — no standalone playwright skills had
  recorded provenance). Sources user-confirmed and installed 2026-07-03: `last30days` via
  `npx skills add mvanhorn/last30days-skill -g` (canonical name is `last30days`, no `-official`), and
  `tiktok-crawling` via the clawhub CLI (`npx clawhub install romneyda/tiktok-crawling`; frontmatter
  name `tiktok-scraping-yt-dlp`, dir kept as `tiktok-crawling` for clawhub update tracking). Both
  fanned out to Claude + Hermes.

### CLAUDE.md refactors (both memory files — explicit user requirement)

Comprehensively refactor `~/.claude/CLAUDE.md` (source: `private_dot_claude/CLAUDE.md`) and the repo
`CLAUDE.md`, per the memory-file guidance in Kun Chen's "L8 Principal's Agentic Engineering Workflow"
(youtu.be/iQyg-KypKAA @11:26–17:57 — transcript pulled and distilled 2026-07-03) plus Anthropic's own
CLAUDE.md guidance. The binding principles from that source:

1. **Global file = minimal.** It loads into *every* session's system prompt across all projects; excess
   content silently burns tokens. Personal preferences + bias-correction rules only (his is 27 lines).
   The YAGNI/SOLID/TDD/investigate-first rules are exactly the "correct the model's default bias"
   category and stay; operational detail does not.
2. **Project file = the repo's collective learning**: what it is, layout, terminology, how the key
   components work, how to test, conventions — grown incrementally (correction → "remember this into
   the memory file"), not hand-written monoliths.
3. **Conditional information moves to skills/runbooks** and loads on demand: long diagnostic ladders
   (atuin/happy/tailscale), the brew-shellenv three-layer narrative, deep subsystem stories → pointers,
   with the detail in `docs/runbooks/` or a project skill.
4. **One file per scope for all harnesses** — repo `AGENTS.md → CLAUDE.md` symlink already exists; add
   the same parity at the global scope (Codex/other harnesses' global memory path) during the refactor.

**Verified current defects to fix (staleness hotfixes — allowed under the freeze policy, before the
big rewrite):** global file says the prepare-commit-msg hook uses *haiku* (the verified hook runs
`--model=sonnet`) — its `humanizer`/`conventional-commits` references are valid again since the
2026-07-03 restoration (`4adcfee`); repo file's
Testing section predates bats, describes the source state as `~/.local/share/chezmoi/` (this repo IS the
source dir per `.chezmoi.toml.tmpl`), uses a `chezmoi apply ~/.tmux.conf` example (tmux is gone), names
yabai in OS-targeting (removed), and claims only `dot_bashrc.tmpl` is shellcheck-rendered (the lint
allowlist has 12 templates).

**Disposition:** staleness hotfixes land now-ish as a small commit; the **comprehensive rewrites are
their own slice (S12) that runs last of all implementation PRs but PRE-cutover — before Phase D**
[audit 2026-07-10] so the repo file documents the *reimplemented* reality (converged on `main`) rather
than being rewritten twice. Acceptance: global file ≈ preferences/bias rules/toolchain/
destructive gates only, no dead references, global AGENTS.md parity; repo file ≈ identity + commands +
architecture map + conventions with conditional deep-dives extracted to runbooks/skills and **every
factual claim re-verified against the live repo at write time**; both files carry their evergreen-only
header comments forward.

## SP3 — Notification system (Rust): verified behavior contract

**Decision:** one Rust **stateless per-event executable** (NOT a daemon/service — invoked once per event;
see the invocation model below) replaces `relay.sh` + `relay-agent.sh` + `hue-pulse.sh` +
`claude-stop-pulse.sh`. `relay-codex-hooks.sh` (a one-shot config merger) and the bashrc bash-preexec
notifier stay shell (bash — settled: the nushell evaluation resolved NO-GO, decision 9, so there is no
shell-migration contingency). Built + deployed the way the herdr Rust plugins are
(rustup toolchain, `run_onchange_after_*` `cargo build --release --locked`, `target/` gitignored).

### Two event classes (both always reach the user)

- **Waiting-on-you** — blocked / permission prompt / question / plan-ready / idle-prompt. The user is the
  bottleneck.
- **Ready-for-you** — a turn or long command finished; an invitation to re-engage (never assume the user
  has nothing more to add to that session).

### Presence model — **verified empirically this session** (not assumed)

A time-series experiment (1 Hz recorder + labeled physical/remote phases; `round2.csv`) overturned the
naïve "remote input never touches HID" hypothesis and settled the signals:

| Signal | How computed | What it means | Verified result |
| --- | --- | --- | --- |
| **hid-idle** | `ioreg -c IOHIDSystem` HIDIdleTime | seconds since the last **physical** input on dresden's own keyboard/trackpad — remote (mosh/Moshi) input does **not** reset it | Climbed steadily 46→119 s during sustained phone typing → a reliable *physically-at-dresden* clock (Round 1's one-shot "6 s while on phone" was a confound; the time-series settled it). |
| **transport** | `stat -f %a /dev/ttys*` freshness | *which device* is carrying input right now | Ghostty client pty (`ttys000/001`) freshens only on physical typing; the mosh-server pty (`ttys013`) only on phone typing. One `stat` sweep discriminates local vs remote. |
| **phone-attention** | tailscale bytes→`mister` rate | is the Moshi app foregrounded | 0 B/s backgrounded (session still alive), ~150 B/s reading, 1–5 KB/s typing — iOS suspends the app in background, so rate ≈ foreground. |
| **mosh-session** | `pgrep -f 'mosh-server.*MOSHI_CLIENT'` | a phone session exists at all | Reliably visible in argv; but **coexists with physical presence** (a session persists while you sit at dresden) → necessary-not-sufficient for "on phone." |
| **lock** | `IOConsoleLocked` (ioreg) | is the screen locked | Probed, never inferred — verified `false` at 28 min idle, so "idle ⇒ locked" is false on dresden. |

**Ranking (physical wins):** hid-idle fresh → **at dresden** (banner, regardless of any live mosh
session); hid-idle stale + (transport=mosh-pty fresh OR phone-attention high) → **driving via phone**
(push); all stale → **away** (push + lights if home). **Lock overrides everything:** `IOConsoleLocked`
true kills the banner surface, so it's push-only no matter what idle says. **Gray zone, defined:**
hid-idle stale, screen unlocked, and no transport/attention evidence — ambiguous, so send **both**
surfaces (this is the reading-at-the-desk case). **Fail-open:** any probe error → also both. The router
never needs to know the user's location to avoid dropping a notification — gray zone and fail-open
guarantee delivery; presence only optimizes *which* surface.

### Home — its own multi-signal module

Emits `{home, not_home, unknown}` + which signals voted, each with a freshness window: UniFi UDR local
API "is `mister`/`mouse` on home Wi-Fi"; geofence Shortcut writing `home|work|gym` + timestamp over
Tailscale SSH; dresden network identity by **gateway MAC** (subnet numbers lie on foreign networks —
verified: the work hotspot presents a 192.168.x.x that isn't home). Extensible (Home Assistant / mmWave
slot in later). Own test suite.

**UDR probe — verified live 2026-07-03 (zero open questions):** key = KeePassXC
`UniFi :: API Key :: dresden-home-presence` (runtime: a 0600 chezmoi-templated file like relay's
`auth.json`; header `X-API-KEY`, never argv — probe used a curl config file). Flow:
`GET https://192.168.1.1/proxy/network/integration/v1/sites` → `data[0].id` (one site, internalReference
`default`) → `GET .../sites/{id}/clients?limit=200` → array of **currently-connected** clients with
`name`, `type` (WIRELESS/WIRED), `ipAddress`, `macAddress`, `connectedAt` (association freshness —
exactly the vote fields). Self-signed cert (`insecure` for the LAN IP). **Latency — decay measured 2026-07-04:** a 33-minute
gap-ladder test (calls after 30 s/60 s/2 m/5 m/10 m/15 m idle) showed **no decay** — 55–251 ms at every
gap. The one observed 11.4 s call was one-time provisioning of the freshly created API key, not a
recurring cold start. Design consequence: **fire-time probing stands** (no poller/cache needed); probe
with a ~2 s timeout, treat a miss as `unknown` (fail-open). Untested beyond 15 m idle (an hours-scale
decay is unruled-out but inconsequential: home only gates lights, so the worst case is one slow/skipped
pulse after a long quiet period, already covered by fail-open). **Findings:** `mister` present with MAC matching the ARP baseline; the
**Hue bridge appears cross-VLAN (WIRED, 192.168.4.24)** in the same list, so one call doubles as a
bridge-reachable/home-LAN check; `mouse` (watch) was **absent while its owner was home** — watches drop
Wi-Fi for Bluetooth when the phone is near, empirically confirming the watch is a weak voter and the
phone is the strong one.

### Routing, lights, delivery

- **Surfaces:** at-desk → banner (whitelist the notifier app in every Focus mode **except Sleep**; Sleep
  = native quiet hours); away → moshi push (mirrors to Apple Watch); gray zone → both. Catch-up on
  return: first input after a silent window queries herdr for still-blocked agents and sends **one
  clickable push per still-blocked session** (0–2 items, each routes to its pane) — heals missed Focus/
  quiet/away notifications with zero stored state.
- **Lights (home only):** floors 2+3 Hue pulse, worst-color-wins coalescing; green=done / red=fail /
  yellow=waiting; **blue reserved for GitHub PRs.** Delivery-failure signal (distinct kind, not a job
  color): **red alternating between fixture groups** (studio: ceiling HCL1&2 ↔ floor HCL3) + a dresden
  audio chirp — unmistakable "the messenger failed."
- **Summaries everywhere**, with a hard timeout that can never delay or block delivery (worst case: the
  plain snippet). Discord `#relay` always logs (⚠️ prefix for waiting-on-you); operator mutes `#relay`
  mobile notifications to kill the double-push. Mute switch. CLI long-commands are first-class events.
- **Invocation model: stateless per-event CLI.** Hooks and the shell notifier exec the binary once per
  event; probes run at fire time; no daemon, no persistent state (catch-up reads live herdr state, not a
  store). The bashrc long-command notifier re-points its `relay.sh` call at the new binary — its
  thresholds (≥60 s local, ≥300 s full) move into the **stateless per-event executable's** config.
- **Lights quiet window.** Hue has no Focus-mode equivalent, so pulses respect a configured quiet window
  (e.g. mirroring Sleep-Focus hours); anything missed during it is healed by catch-up-on-return.
- **Channels are a pluggable boundary** (moshi = today's phone channel). A future custom iOS app —
  actionable notifications (approve/deny from the lock screen) + native presence reporting — is deferred;
  the $99 dev account + TestFlight-internal path avoids App Store review. Nothing gets locked out.

### The empty-`(main) done` "spam" — resolved

Verified across `hermes/logs/gateway.log` + transcripts: every burst fire mapped **1:1 to a real turn**.
There is **no loop bug** — fire-once-per-completion already holds. The empty body has a **verified,
device-agnostic cause**: the `jq -rs` whole-file slurp aborts on one half-written trailing JSONL line
(reproduced: `Unfinished string at EOF`, exit 5). The Rust port fixes it structurally — line-by-line
parse, skip an unterminated final line.

### Bugs the rewrite must not reproduce (the shell findings are the failure spec)

Fail-**open** presence (not the current fail-closed abort); line-by-line transcript parse; `flock` that
auto-releases on SIGKILL (not a wedged `mkdir` lock); word-boundary command matching; bounded per-channel
retry with a loud fallback (pulse lights if the push channel hard-fails while away); no secret on argv/
env (preserve today's clean stdin/file-path handling — the one thing the audit found already correct).

## Testing & design strategy (essential-feed — binding on *how*, not *what*)

Per the brief, this discipline is **required** for SP3 and all substantial new code; only its translation
into Rust is a design choice. Condensed here so the spec is self-contained; the **unabridged, verbatim**
text (the user's authoritative summary of `essentialdevelopercom/essential-feed-case-study`) lives in the
brief `docs/plans/2026-07-02-repo-modernization-brief.md` §8 and is the binding reference.

> ## Software Testing and Design Strategy
>
> The strategy in this repository is a disciplined, specification-driven approach in which **Test-Driven
> Development is used not just to verify behavior but to *discover* the design**. Tests are written first
> from the specifications (narratives, acceptance criteria, and use cases with explicit happy and sad
> paths), and the modular architecture emerges as a consequence of writing code that is easy to test in
> isolation. TDD applied under **SOLID principles** naturally pushes the system toward small,
> single-responsibility components that depend on abstractions.
>
> The architecture is organized into independent **feature scenes** that share the same layered shape but
> know nothing about each other, with responsibilities separated into layers — networking and persistence
> at the boundaries, a plain domain model in the middle, presentation and UI toward the edge —
> dependencies pointing inward and communicating through abstractions rather than concrete types. Feature
> modules contain no wiring or global state; a **Composition Root** is the single place where concrete
> implementations are instantiated and injected, so each unit is testable with test doubles and the app
> assembles differently for tests than for production.
>
> Tests are split by scope: a broad base of fast isolated **unit tests** (by feature and layer),
> **integration tests** in their own target exercising the real persistence stack, and **end-to-end
> tests** against a live backend isolated separately — clarity of failure and fast feedback, with the
> slow tests quarantined. Unit/presentation logic is tested with **test doubles (spies, stubs) at the
> boundaries** for speed, determinism, and to enforce the abstraction-based design. A small number of
> **acceptance tests** drive the fully composed app through the composition root, swapping only the
> outermost boundaries. **CI enforces all of it on every PR.**
>
> In short: TDD under SOLID *drives out* a modular, layered, dependency-injected design — independent
> scenes wired only at a composition root — rather than testing a pre-existing one; and the strategy
> favors many fast isolated unit tests over slow integrated ones, test doubles at boundaries over real
> infrastructure, a few high-level acceptance tests over brittle end-to-end automation, and continuous
> CI over manual verification.

**Translation to this repo (SP3 and beyond):**

- Tests discover the design: write "does this event route to the phone when away" before the router
  exists, letting the test force presence-detection into its own trait behind a fake.
- Independent scenes, no shared state, wired at a composition root: the *decide what/where* domain logic
  is separate from each delivery mechanism (moshi HTTP, Hermes HTTP + HMAC, local banner, Hue) and each
  presence probe (HID, transport, phone-attention, home module) — concrete impls injected in `main`, fakes
  injected in tests.
- Boundaries get fakes, not real I/O, in the fast suite: no real `curl`/`openhue`/`ioreg`/transcript read
  in unit tests — each behind a trait with a test double.
- Levels by speed, scaled to this repo: broad fast unit base; a few acceptance tests driving the wired
  composition against fake outermost boundaries (fake moshi/Hermes server, fake Hue). A multi-platform
  sanitizer CI matrix is overkill at this scale — say so rather than cargo-cult it; `cargo test` +
  `clippy` + `fmt` wired into `just test`/CI is the right size, and it closes the current gap where **no
  Rust check runs anywhere** and `claude-stop-pulse`'s gate has zero coverage.
- Where bash stays (SP7 fixes, the bashrc notifier), the test framework is **bats**, mirroring the
  `test/osquery-alerter/lib.bash` harness (fixture builders + PATH-shimmed stubs + named assertions), and
  hand-rolled `test/*.sh` convert to bats over time.

## Verification (extended sections)

- **Inventory:** every subsystem A–P traces to real files (spot-checked by a completeness critic; the
  `~/.claude.json` MCP vault, SSH surface, login-shell carriers, and `AGENTS.md` symlink were the gaps it
  caught and are now included).
- **Work ledger:** every sweep row was adversarially verified against source before landing here; 2
  candidates were refuted and excluded. `[FIXED]` rows are committed with passing `just test`.
- **SP3 contract:** the presence table is backed by `round2.csv` (labeled time-series), not assertion;
  each leg (hid-idle climbing under remote typing, per-pty transport fingerprint, tailscale-rate
  attention) was observed live. Open items for the final SP3 spec are enumerated in the deferred index
  (failure semantics + tuning constants + quiet-window hours + test split; the UDR probe is RESOLVED and
  no longer an open item).
- **Re-evaluated 2026-07-03 (max effort), defects found and corrected:** the presence table's hid-idle
  row had inverted the core finding (said "local *or* remote" resets it — the experiment showed remote
  does **not**); SP1/SP2 described already-executed work as pending (scaffolding commits, the `f7220d9`
  merge); decision 2 contradicted the SP3 contract (now annotated); the deferred SP3 bullet still
  carried the superseded flush-race/spam framing; gray zone and lock behavior were referenced but
  undefined (now defined); the invocation model and lights quiet window were unstated (now stated). The
  bats zero-count and CLAUDE.md-fork claims were independently re-verified by hand before acting on
  them; the fork is reconciled (`4ac97de`).
