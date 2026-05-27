# Dotfiles Tasks — Design Spec (2026-05-15 audit cycle)

**Date:** 2026-05-15
**Status:** Design approved via prior multi-session brainstorming. Spec drafted for subagent-driven implementation.
**Source plan:** `~/.claude/plans/help-me-configure-https-github-com-nix-c-witty-quill.md` (will be obsolete once this spec is approved).
**Implementation entrypoint:** subagent-driven-development after the writing-plans skill produces a plan from this spec.

---

## Executive Summary

This audit cycle delivers **13 dotfiles improvements** as Todoist tasks in the `#dotfiles` project, plus one-time **setup work** (move completed docs to `docs/archive/`, CLAUDE.md rule, GitHub housekeeping) that completes the audit closeout. The Todoist tasks already exist; this spec is the implementation design for executing them.

**Phase model:**
- **Setup (S1–S4):** Run once, before any implementation phase. `git mv` completed docs to `docs/archive/`, add CLAUDE.md rule to skip the archive when auditing, close GitHub issues, amend the unpushed `Fixes #17` commit. **Two new commits (S1, S2) + GitHub issue closures (S3, no commit) + one amend-in-place (S4).** The "separate logically distinct changes" preference applies to S1/S2.
- **Implementation (P1–P13):** Each phase corresponds to exactly one Todoist task, ordered by priority + dependency. Phase ends with `td task complete id:<id>`.

**Pre-setup ordering:** This spec and the implementation plan (written by `superpowers:writing-plans` after spec approval) are both committed BEFORE S1 runs. The new 2026-05-15 files live in `docs/superpowers/specs/` and `plans/` before any moves happen. `.gitkeep` files are added to eight directories (3 active superpowers subdirs + 5 archive subdirs) during S1 as future-proofing — they coexist with content and survive any future cleanup that empties their parent directory.

**Ordering principle:** p2 → p3 → p4. Within bands, dependencies dictate order — most notably **Phase 1 ships the improved commit-message generator (Task 13)** so every subsequent commit benefits from the better template, and **Phase 2 ships PostgreSQL setup (Task 11)** because daily use begins 2026-05-16.

---

## Goals & Non-Goals

### Goals

1. Execute the 13 audited improvements identified across `docs/research/`, `docs/superpowers/`, and GitHub issues.
2. Mark each task complete in Todoist (`td task complete`) immediately after its phase verification passes.
3. Close stealth-completed GitHub issue #17 via commit-message amend on an existing unpushed commit.
4. Close GitHub issues #5 (Nu Shell) and #13 (Zellij) as wontfix per locked-in toolchain.
5. Reorganize `docs/` so future audits don't re-discover the same items — completed research/plans move to `docs/archive/<mirror-of-original-path>/`, and a CLAUDE.md rule tells agents to skip `docs/archive/` entirely when auditing.

### Non-Goals

- **Creating the Todoist tasks.** Already done (2026-05-15). The IDs are referenced below.
- **Re-auditing archived docs.** They've been audited; the archive directory itself signals "out of scope for audits."
- **Maintaining an audit-outcomes index.** The archive directory + CLAUDE.md rule is enough; an index would duplicate state.
- **Migrating to nix-darwin/sops-nix.** Tracked by separate pre-existing task `6gWP8w7V3R94PRv5`.
- **Building OpenClaw itself.** Tasks 3 and 7 hook into OpenClaw's notification surface; they assume OpenClaw exists and has a webhook/event ingestion path.

---

## Work Units

### Todoist tasks (13)

All in `#dotfiles` project, all `infrastructure` label.

| Phase | Todoist ID | Title | Priority | GH Issue |
| --- | --- | --- | --- | --- |
| P1 | `6gfVJH5P4g4vQ4FM` | Improve `prepare-commit-msg` hook to use conventional-commits | p2 | — |
| P2 | `6gfVJFgXvG9mJ96M` | PostgreSQL workstation setup (daily-use starting 2026-05-16) | p2 | — |
| P3 | `6gfVJCVHWvqJ8Jpv` | Audit chezmoi package-manager automation | p2 | [#11](https://github.com/webdavis/dotfiles/issues/11) |
| P4 | `6gfVJ6W9xxjh9FPM` | Add gitleaks pre-commit secret scanning | p2 | — |
| P5 | `6gfVJ9rXQ85xr7qM` | Review Determinate Nix installer state; plan migration | p2 | [#10](https://github.com/webdavis/dotfiles/issues/10) |
| P6 | `6gfVJCvxV34W3hgM` | Install bandwhich, doggo, ouch CLI tools | p3 | — |
| P7 | `6gfVJGJCfjwCVXQv` | Improve hue-pulse lighting notification system | p3 | — |
| P8 | `6gfVJ8Rfh8ppwpqv` | Adopt remaining quick wins from dotfiles-improvements research | p3 | — |
| P9 | `6gfVJ6w2VHc2w4xv` | Add actionlint to lint suite | p3 | — |
| P10 | `6gfVJ7VwcFQvg7xM` | Notify via `mouse` OpenClaw agent on long-running shell command completion | p3 | — |
| P11 | `6gfVJ9P5vpX64JhM` | Automate gh-notify install + hue-pulse blue + OpenClaw notifications | p3 | [#9](https://github.com/webdavis/dotfiles/issues/9) |
| P12 | `6gfVJ7mrm2259mwM` | Add `help.autocorrect = 1` to gitconfig | p4 | — |
| P13 | `6gfVJ8v6pjjF5Qwv` | Install Tart base macOS VM image | p4 | — |
| **N1** | `6ggHM32c8FXWcqmv` | **Build cross-source notification queue + Cloudflare Tunnel webhook receiver (foundational; blocks P7, P10, P11)** | **p2** | — |

### Pre-existing #dotfiles tasks (NOT in scope for this spec)

- `6gVRJjqWc69XqV75` — Add quarterly cleanup LaunchAgent + reminder script
- `6gVRJmHQ3rWJpCcX` — Clean up chezmoi orphan disk-cleanup docs
- `6gWP8w7V3R94PRv5` — Migrate to nix-darwin + sops-nix on dresden

### Setup work (2 commits, before P1)

| Step | Description | Commit |
| --- | --- | --- |
| S1 | `git mv` 15 research files + 5 superpowers artifacts to `docs/archive/<mirror-of-original-path>/`. Add 8 `.gitkeep` files (3 active superpowers subdirs + 5 archive subdirs); each `.gitkeep` contains a one-line `#` comment explaining why it exists. | `chore(docs): archive completed research and superpowers artifacts (2026-05-15)` |
| S2 | Add to CLAUDE.md (new `### Auditing docs/` subsection): a short rule telling agents to skip `docs/archive/` entirely when auditing `docs/` for actionable items. Open follow-ups live in Todoist (#dotfiles); the archive is the durable lookup. Active dirs to read during an audit: `docs/research/` (excluding `2026-05-01-secrets-management-nix-darwin/` which is tied to Todoist `6gWP8w7V3R94PRv5`) and `docs/superpowers/{plans,specs,audits}/`. | `docs(CLAUDE): skip docs/archive when auditing` |
| S3 | Close GH #5 and GH #13 as wontfix with the rationale comments from the prior plan section A2. | (no commit — `gh issue close` only) |
| S4 | Amend the most recent unpushed 2026-05-05 macos-defaults commit (likely `409dd2a` if it's the topological tail of the cluster, but verify) to add a `Closes #17` trailer. When pushed, GitHub's keyword detection auto-closes #17. **Pre-flight required: confirm the commit is unpushed.** `git log origin/main..HEAD --oneline` must include the commit before amending. | (amend in place — no new commit) |

S4's amend is the most risky setup step — guarded by the unpushed precondition. If any of the 2026-05-05 commits are already pushed, fall back to `gh issue close 17 --comment "..."` (per prior plan section A3).

### Files to `git mv` in S1

**From `docs/research/` → `docs/archive/research/`** (15 files):
- 2026-03-19-bash-preexec-atuin-shell-history.md
- 2026-03-19-chezmoi-tool-installation-automation.md
- 2026-03-21-keeping-imessages-alive-on-locked-mac.md
- 2026-03-22-llm-instruction-following-failures.md
- 2026-04-12-act-runner-isolation.md
- 2026-04-12-devbox-vs-nix-flakes.md
- 2026-04-12-karl-davis-dotfiles-review.md
- 2026-04-12-worktrunk.md
- 2026-04-13-sesh-deep-dive.md
- 2026-04-13-sesh-vs-tms.md
- 2026-04-13-television-vs-fzf.md
- 2026-04-14-act-macos-runners.md
- 2026-04-14-dotfiles-improvements.md
- 2026-04-15-jessfraz-dotfiles-review.md
- 2026-04-26-macos-defaults-management.md

**From `docs/superpowers/` → `docs/archive/superpowers/`** (5 files, mirror dir structure):
- audits/2026-04-28-v2-progress-audit.md
- plans/2026-04-15-dotfiles-improvements.md
- plans/2026-04-19-dotfiles-improvements-v2.md
- specs/2026-04-14-dotfiles-improvements-design.md
- specs/2026-04-17-dotfiles-improvements-v2-design.md

**NOT moved (still active):**
- `docs/research/2026-05-01-secrets-management-nix-darwin/` (tied to open Todoist task `6gWP8w7V3R94PRv5`, currently deferred at p4).
- This spec, once committed: `docs/superpowers/specs/2026-05-15-dotfiles-tasks-design.md`.
- The implementation plan, once committed: `docs/superpowers/plans/2026-05-15-dotfiles-tasks-plan.md`.

**`.gitkeep` placement (8 files, each with a short `#` comment explaining its purpose):**
- `docs/superpowers/audits/.gitkeep` — `# Keeps docs/superpowers/audits/ tracked between audit cycles when no active audit exists.`
- `docs/superpowers/plans/.gitkeep` — `# Keeps docs/superpowers/plans/ tracked between plan cycles when no active plan exists.`
- `docs/superpowers/specs/.gitkeep` — `# Keeps docs/superpowers/specs/ tracked between spec cycles when no active spec exists.`
- `docs/archive/.gitkeep` — `# Keeps docs/archive/ tracked even if all archived files are later removed.`
- `docs/archive/research/.gitkeep` — `# Keeps docs/archive/research/ tracked even if emptied.`
- `docs/archive/superpowers/audits/.gitkeep` — `# Keeps docs/archive/superpowers/audits/ tracked even if emptied.`
- `docs/archive/superpowers/plans/.gitkeep` — `# Keeps docs/archive/superpowers/plans/ tracked even if emptied.`
- `docs/archive/superpowers/specs/.gitkeep` — `# Keeps docs/archive/superpowers/specs/ tracked even if emptied.`

`.gitkeep` files are permanent — they coexist with content.

---

## Phase Ordering Rationale

**Why P1 = Task 13 (commit message generator):** Every phase after P1 produces commits. A better commit-message generator (conventional-commits format, multi-paragraph body with WHY, `Fixes #N` trailer support) means the audit trail for P2–P13 is informative from the start instead of being retrofitted later. Spending the first phase on tooling that compounds is high-leverage.

**Why P2 = Task 11 (PostgreSQL):** The user noted daily use begins 2026-05-16 (the day after spec approval). Time-sensitive. Independent of other phases.

**Why P3 = Task 9 (package-mgr audit) before P4–P5:** The audit will enumerate what's automated and what isn't, which informs the gitleaks pre-commit work in P4 (the hook is added via existing pre-commit hook scaffolding — confirming via the audit that nothing else needs scaffolding first) and might surface other gaps relevant to P5–P13.

**Why P6 (CLI tools) before P7 (hue improvements):** Task 12's ignored-tools audit explicitly references `bandwhich`, which P6 installs. Without P6, P7 references a missing binary.

**P10 and P11 are now independent:** P10's scope changed to "extend `__cmd_notify_precmd` so Bob also notifies on long-running shell command completion" (purely local, no OpenClaw involvement). P11 stays as the gh-notify integration. No dependency between them.

**Why P12 (gitconfig autocorrect) and P13 (Tart pull) last:** Both are p4. P12 is trivial (one-line edit). P13 is a ~25 GB download that needs network — can run in background of other work or be deferred indefinitely if priorities shift.

---

## Per-Phase Detail

Each phase follows the same template:

1. **Pre-flight check** — verify current state matches the assumption in the task description.
2. **Implementation steps** — concrete edits, drawn from the Todoist task description (which lives in Todoist; subagents read it via `td task view id:<id>`).
3. **Verification** — concrete commands to confirm the change works.
4. **Commit** — single commit (or small set) using the improved `prepare-commit-msg` hook from P1.
5. **Closeout** — `td task complete id:<id>`.

The full implementation steps live in the Todoist task descriptions (created 2026-05-15 with verified current-state notes). Subagents pull the description with `td task view id:<id> --json | jq -r '.description'` and execute against that. This spec doesn't duplicate the descriptions — it indexes them.

### Phase 1 — Improve `prepare-commit-msg` hook

- **Todoist:** `6gfVJH5P4g4vQ4FM`
- **Pre-flight:** `git config --global core.hooksPath` returns a path matching `~/.config/git/hooks/`. The existing hook lives there (managed by chezmoi).
- **Why first:** every subsequent phase produces commits that benefit from a better hook.
- **Verification gate:** 3–5 representative commits show conventional-commits format with multi-paragraph body and `Fixes #N` trailers where applicable.
- **Closeout:** `td task complete id:6gfVJH5P4g4vQ4FM`.

### Phase 2 — PostgreSQL workstation setup

- **Todoist:** `6gfVJFgXvG9mJ96M`
- **Pre-flight:** `postgresql@17` already in `.chezmoidata/system_packages_autoinstall.yaml` formulae list (verified 2026-05-15). After `chezmoi apply` (user-side), `command -v psql` succeeds.
- **Verification gate:** psql connects, `\timing` shows on, prompt shows timestamp + DB name, pager works.
- **Closeout:** `td task complete id:6gfVJFgXvG9mJ96M`.

### Phase 3 — Package-manager automation audit

- **Todoist:** `6gfVJCVHWvqJ8Jpv` (closes [#11](https://github.com/webdavis/dotfiles/issues/11))
- **Pre-flight:** Confirm rustup script exists at `.chezmoiscripts/run_once_before_20-install-rustup.sh.tmpl` (already verified 2026-05-15).
- **Verification gate:** Documented list of automated vs. not-automated package managers. Any missing wrappers are either filled in or filed as separate Todoist follow-ups.
- **Closeout:** `td task complete id:6gfVJCVHWvqJ8Jpv`. Final commit includes `Fixes #11` if scope confirmed complete.

### Phase 4 — gitleaks pre-commit hook

- **Todoist:** `6gfVJ6W9xxjh9FPM`
- **Pre-flight:** `gitleaks` in formulae list (verified). `gitleaks --version` succeeds after apply.
- **Verification gate:** Test commit adding a fake API key to a non-tmpl file is blocked; a normal commit is not.
- **Closeout:** `td task complete id:6gfVJ6W9xxjh9FPM`.

### Phase 5 — Determinate Nix installer migration

- **Todoist:** `6gfVJ9rXQ85xr7qM` (closes [#10](https://github.com/webdavis/dotfiles/issues/10))
- **Pre-flight:** Step 1 of the task is research, not implementation. WebFetch the current Determinate docs first.
- **Verification gate:** CI green on a test PR after the workflow change.
- **Closeout:** `td task complete id:6gfVJ9rXQ85xr7qM`. Final commit includes `Fixes #10`.

### Phase 6 — Install bandwhich, doggo, ouch

- **Todoist:** `6gfVJCvxV34W3hgM`
- **Pre-flight:** None of bandwhich/doggo/ouch in autoinstall yaml (verified 2026-05-15 — only `dust` is present).
- **Verification gate:** After yaml edit + apply, `command -v bandwhich doggo ouch` all succeed.
- **Closeout:** `td task complete id:6gfVJCvxV34W3hgM`.

### Phase 7 — Improve hue-pulse lighting notification system

- **Todoist:** `6gfVJGJCfjwCVXQv`
- **Pre-flight:** P6 done (so `bandwhich` is available for the ignored-tools audit). **N1 done** (the consumer-side serialization is the structural fix for the pulse race — sub-task (c) below is replaced by "verify race is gone" rather than subjective tuning).
- **Sub-tasks:** (a) audit + expand the `__cmd_notify_*` ignored-tools list using `atuin search` history; (b) test each added tool >30s without notification firing; (c) **verify the hue snapshot-during-pulse race no longer occurs** because N1's consumer serializes pulses (replaces the original subjective-tuning goal).
- **Verification gate:** (a) skip-list covers actual interactive TUI usage; (b) each added tool tested; (c) two concurrent triggers within 50ms produce sequential pulses with no overlap or bogus restore-state.
- **Closeout:** `td task complete id:6gfVJGJCfjwCVXQv`.

### Phase 8 — Quick wins (bat-extras MANPAGER, fd FZF_DEFAULT, starship git_metrics)

- **Todoist:** `6gfVJ8Rfh8ppwpqv`
- **Pre-flight:** bat-extras + hyperfine already in formulae list (verified).
- **Verification gate:** `man <something>` opens batman; `fzf` lists files via fd; starship prompt shows git +/- counts.
- **Closeout:** `td task complete id:6gfVJ8Rfh8ppwpqv`.

### Phase 9 — actionlint integration

- **Todoist:** `6gfVJ6w2VHc2w4xv`
- **Pre-flight:** actionlint already in formulae list (verified).
- **Verification gate:** `just l` includes actionlint and passes; CI workflow YAML runs it on push.
- **Closeout:** `td task complete id:6gfVJ6w2VHc2w4xv`.

### Phase 10 — Notify via `mouse` OpenClaw agent (single endpoint, three notification types)

- **Todoist:** `6gfVJ7VwcFQvg7xM`
- **Pre-flight (blocking):** the dedicated `mouse` OpenClaw agent (`6gfcXjFrG6q3Pm3v`) and its Discord bot (`6gfcXjRh8vC57g2v`) must exist.

**Single endpoint, payload `type` field switches the message Mouse composes:**

| `type` | Trigger | Payload fields | Mouse composes |
| --- | --- | --- | --- |
| `agent_input_needed` | Claude Code (or any AI agent) waiting for input (permission, idle) | `source_agent` | "Woof! `<source_agent>` is waiting on you" |
| `agent_finished` | Claude Code (or any AI agent) finished a task | `source_agent`, `agent_session`, `tmux_session` (optional), `cwd`, `task_summary`, `success`, `duration_s` | "Woof! `<source_agent>` finished `<task_summary>` in tmux session `<tmux_session>`, dir `<cwd>` — `<success_or_failure>` in `<duration_s>`" (refine after first iter) |
| `command_done` | Regular shell command >3 min | `cmd`, `success`, `duration_s` | "Woof! `<cmd>` completed" (success) / "Woof! `<cmd>` failed after `<duration_s>`" (failure) |

**Plumbing (shared, written once, used by P10 + P11):**
- OpenClaw config: `hooks.enabled = true` + `hooks.token = <kp>` (KeePassXC, distinct from `gateway.auth.token`).
- `hooks.mappings`: ONE mapping — `/hooks/notify` → `action: "agent"`, `agentId: "mouse"`, `deliver: true`.
- Render hooks token to a 0600 file at apply time via chezmoi template (mirrors `dot_aws/credentials.tmpl`).
- Bearer auth on every POST. Async / fire-and-forget.

**Pre-flight (blocking on N1):** the cross-source notification queue + webhook receiver (`6ggHM32c8FXWcqmv`) must exist. All triggers below write to the queue; the consumer dispatches to alerter + hue-pulse + mouse serially.

**Triggers (this task) — all write JSON records to `~/.notify-queue/`, not direct API calls:**
- Extend `__cmd_notify_precmd` in `dot_bashrc.tmpl` so that when duration > 180s, it writes a queue record `{source: "command_done", cmd, success, duration_s, hue_profile: "local"}`.
- Extend Claude Code Notification hook to write `{source: "claude_input", source_agent: "claude-code"}`.
- Extend Claude Code Stop hook to write `{source: "claude_stop", source_agent, agent_session, tmux_session, cwd, task_summary, success, duration_s}`.

The consumer (from N1) handles fan-out: alerter for macOS notification, hue-pulse for the light, mouse for Discord. Sits alongside the existing alerter (≥30s) and hue-pulse (≥5min) — but routed through the queue, so concurrent events no longer race.

**Verification gate:**
- `sleep 200` → alerter fires + Discord message "Woof! sleep 200 completed".
- A Claude Code permission prompt → alerter fires + Discord "Woof! claude-code is waiting on you".
- A Claude Code task completion → Discord message with full context (agent, session, tmux, cwd, summary, duration).

**Out of scope:** Bob. Tailscale. Pi-migration.
**Closeout:** `td task complete id:6gfVJ7VwcFQvg7xM`.

### Phase 11 — GitHub Actions webhook → notification queue (replaces the old gh-notify proposal)

- **Todoist:** `6gfVJ9P5vpX64JhM` (closes [#9](https://github.com/webdavis/dotfiles/issues/9))
- **Pre-flight (blocking on N1):** the queue + Cloudflare Tunnel + webhook receiver scaffolding from `6ggHM32c8FXWcqmv` must exist.
- **Scope correction:** gh-notify was the wrong trigger — it's an interactive ad-hoc `fzf` browser, not a hook. Verified in `docs/research/2026-05-18-github-workflow-notification-trigger.md`. New architecture uses the GitHub `workflow_run` webhook → Cloudflare Tunnel (extends existing clawmail tunnel with a second ingress) → HMAC receiver → notification queue.
- **Sub-tasks:**
  - **(a) Configure GitHub webhooks** per-repo for the user's active repos: `gh api repos/<owner>/<repo>/hooks -X POST` with `events[]=workflow_run`, secret from KeePassXC, content_type=json, url pointing at the tunnel hostname.
  - **(b) Migrate existing `gha-watcher.sh`** to write queue records instead of calling alerter + hue-pulse directly. The watcher stays in place as the missed-event safety net for Mac-asleep scenarios where webhook retries exhaust.
  - **(c) Color extension** of `hue-pulse.sh` (already done as part of N1): `hue-pulse.sh 0 workflow` = purple (`xy 0.2725, 0.1283`, brightness 80), `hue-pulse.sh 1 workflow` = orange (`xy 0.5562, 0.4084`, brightness 100). NOT blue.
  - **(d) mouse Discord notification:** webhook receiver writes a queue record with `source: "github_workflow"` + payload fields `{repo, workflow, conclusion, url}`. Consumer routes to mouse with `type: gh_workflow` (a 4th type extending P10's three). Mouse composes: "Woof! `<repo>/<workflow>` `<conclusion>` — `<url>`" (refine after first iter).
- **Verification gate:** Trigger a workflow run on a test repo. Receiver gets the webhook (verify in receiver log) → queue record appears → consumer dispatches alerter + hue-pulse (orange/purple) + Discord message via mouse.
- **Closeout:** `td task complete id:6gfVJ9P5vpX64JhM`. Final commit includes `Fixes #9`.

### Phase 12 — gitconfig autocorrect

- **Todoist:** `6gfVJ7mrm2259mwM`
- **Pre-flight:** Confirm `[help]` section absent (or present without autocorrect) in current `dot_gitconfig.tmpl`.
- **Verification gate:** `git stauts` auto-runs `git status` after ~0.1s delay.
- **Closeout:** `td task complete id:6gfVJ7mrm2259mwM`.

### Phase 13 — Tart base image pull

- **Todoist:** `6gfVJ8v6pjjF5Qwv`
- **Pre-flight:** `tart` binary on PATH (verified — `tart` in formulae list).
- **Verification gate:** `tart list` shows `sequoia-runner` after the ~25 GB pull completes.
- **Closeout:** `td task complete id:6gfVJ8v6pjjF5Qwv`.

---

## Verification (Spec-Level)

After all phases + setup complete:

1. `td task list --project "dotfiles" --json | jq '.results | length'` shows 16 total (3 pre-existing + 13 new) — but **completed** tasks may filter out depending on td's defaults; check both views.
2. `td task list --project "dotfiles" --filter "search:completed:6gfVJ"` (or equivalent) shows 13 completed.
3. `gh issue view 5`, `gh issue view 13`, `gh issue view 17` all show `state: CLOSED`.
4. `gh issue view 9`, `gh issue view 10`, `gh issue view 11` show CLOSED if their phases ran clean.
5. `ls docs/archive/research/ | wc -l` returns ≥16 (15 archived files + .gitkeep).
6. `ls docs/research/` shows only `2026-05-01-secrets-management-nix-darwin/`.
7. `grep -q 'docs/archive' /Users/stephen/.local/share/chezmoi/CLAUDE.md` finds the new rule.
8. `find docs/superpowers docs/archive -name .gitkeep | wc -l` returns 8.

---

## Open Questions / Gaps

**Gap 1: `2026-04-26-macos-defaults-management.md` not re-verified line-by-line.**
At audit time, this file exceeded the Read tool's 25k-token limit and was not fully re-read. The corresponding implementation is verified done via CLAUDE.md §"macOS Defaults Management" and the 2026-05-05 commit cluster (~40 commits per Agent 3's audit). If future work surfaces inconsistencies between the research and the implementation, read the archived file directly at `docs/archive/research/2026-04-26-macos-defaults-management.md` with `offset`/`limit`.

**Gap 2: OpenClaw notification surface unknown at spec time.**
P10 and P11 both touch OpenClaw's event ingestion. The implementation will need to discover OpenClaw's hook/webhook API as the first step of P10. If OpenClaw doesn't expose a suitable surface, P10 may surface a follow-up Todoist task (add a notification webhook to OpenClaw upstream) rather than completing.

**Gap 3: Phase ordering for P5 (Determinate Nix migration).**
Research-first phase. If the research outcome is "no migration needed — Determinate's current state is fine," P5 closes #10 with a comment rather than a code change. Spec assumes a code change but the closeout works either way.

---

## Implementation Handoff

After user approval of this spec, invoke `superpowers:writing-plans` with this spec as input. The resulting plan should:
- Have one phase per Todoist task (13 phases) + setup phases S1–S4.
- Include `td task complete id:<id>` as the closeout step for each implementation phase.
- Mark phase dependencies explicitly (P6 → P7, P10 → P11, P1 before all others).
- Live at `docs/superpowers/plans/2026-05-15-dotfiles-tasks-plan.md`.

Then `superpowers:subagent-driven-development` runs the plan with review checkpoints between phases.
