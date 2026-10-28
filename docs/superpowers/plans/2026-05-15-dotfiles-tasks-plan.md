# 2026-05-15 Dotfiles Tasks: Implementation Plan

> **SUPERSEDED (2026-07-10, operator ruling R3).** OpenClaw was removed from the fleet and replaced by
> Hermes. The OpenClaw tasks in this plan (B1 create the mouse agent, B2 wire the Discord bot, P10 notify
> via the mouse OpenClaw agent, and the OpenClaw half of P11) MUST NOT be executed. This plan is retained
> only as a historical record of the audit cycle, never as an actionable instruction to reinstall or
> reconfigure OpenClaw.

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Execute the 13-phase audit cycle defined in `docs/superpowers/specs/2026-05-15-dotfiles-tasks-design.md`, plus the setup/cleanup phases (S1-S5) and Mouse-blocker phases (B1-B3), marking each Todoist task complete on phase closeout.

**Architecture:** Each phase corresponds to one Todoist task in the `#dotfiles` project. Per-phase implementation details live in the Todoist task description (single source of truth, fetched at phase start via `td task view`). The plan provides the phase ordering, dependencies, verification gates, and closeout steps. Setup phases (S0-S4) and Mouse-blocker phases (B1-B3) are detailed inline since they don't have Todoist tasks. **Closeout housekeeping** for completed docs uses `git rm` (history retains the originals); the audit-outcomes index from S2 is the durable lookup point.

**Tech Stack:** chezmoi 2.62.3+, bash 5.3, Nix (upstream via NixOS/nix-installer), Tailscale (already installed), Todoist `td` CLI, GitHub `gh` CLI, OpenClaw (local gateway, future Pi migration).

**Source spec:** `/Users/stephen/.local/share/chezmoi/docs/superpowers/specs/2026-05-15-dotfiles-tasks-design.md`

---

## Phase Ordering Summary

```
S0 → S1 → S2 → S3              (setup: rebase + amend, archive, CLAUDE.md rule, GH closes)
                  → B1 → B2     (Mouse agent, Mouse Discord bot, gates P10, P11)
                  → B3          (rescue gateway decommission, independent)
                  → P1          (commit-msg generator, improves all downstream commits)
                  → P2          (PostgreSQL, time-sensitive)
                  → P3 → P4     (package audit, gitleaks)
                  → P6          (CLI tools: bandwhich/doggo/ouch)
                  → P7          (hue-pulse improvements, depends on P6)
                  → P8 → P9     (quick wins, actionlint)
                  → P10 → P11   (Mouse OpenClaw notif → gh-notify reuses plumbing)
                  → P12 → P13   (autocorrect, Tart image)
```

**P5 (Determinate Nix migration) is already shipped and closed in Todoist as of 2026-05-17** (commits `58bbb7d` + `6a3da6f` on `main`). It is retained in the plan only as a placeholder for the `Closes #10` trailer that lands once HEAD is pushed; no implementation work remains. **P2 (PostgreSQL) is postponed** per user, likely to land when agents migrate to the NUC server.

---

## Setup Phases

### S0: Amend unpushed commits with `Closes #<n>` trailers (USER ACTION)

**Why:** GitHub auto-closes issues when commits containing `Closes #N` / `Fixes #N` are pushed to the default branch. Two commits need amending:
- Current HEAD~2 (fork-migration): swap existing `Solves #10` → `Closes #10`.
- `f24ef50` (last 2026-05-05 macos-defaults commit): add `Closes #17`.

**Files:**
- No file changes; git commit-message metadata only.

**Steps:**

- [ ] **Step 1: Confirm state**

```bash
git log origin/main..HEAD --oneline | head -10
git log -1 --format=%B f24ef50
```

Expected: f24ef50 message ends without `Closes #17`. Current chain has `Solves #10` somewhere in HEAD~2.

- [ ] **Step 2: User runs the rebase (`! ` prefix; auto-mode classifier blocks rebase-on-main from automation)**

```bash
! set -e
ORIG=$(git log -1 --format=%B f24ef50)
git checkout --detach f24ef50
SKIP_AI_COMMIT=1 git commit --amend --no-verify -m "${ORIG}

Closes #17"
NEW=$(git rev-parse HEAD)
git checkout main
SKIP_AI_COMMIT=1 git rebase --onto "$NEW" f24ef50 main
NEW58=$(git log --grep="migrate installer from DeterminateSystems" --format=%H -1)
ORIG2=$(git log -1 --format=%B "$NEW58" | sed 's/Solves #10/Closes #10/')
git checkout --detach "$NEW58"
SKIP_AI_COMMIT=1 git commit --amend --no-verify -m "$ORIG2"
NEW2=$(git rev-parse HEAD)
git checkout main
SKIP_AI_COMMIT=1 git rebase --onto "$NEW2" "$NEW58" main
git log --grep='Closes #' --oneline
```

Expected: `git log` output shows both commits with `Closes #10` and `Closes #17` trailers.

- [ ] **Step 3: No commit needed (rebase rewrites in place).**

---

### S1: `git mv` completed docs to `docs/archive/` + add `.gitkeep` future-proofing

**Files:**
- Move (`git mv`): 15 files from `docs/research/*.md` → `docs/archive/research/*.md` (top-level only; NOT the `2026-05-01-secrets-management-nix-darwin/` subdir).
- Move (`git mv`): 5 files from `docs/superpowers/{audits,plans,specs}/2026-04-*` → `docs/archive/superpowers/{audits,plans,specs}/`.
- Create: 8 `.gitkeep` files (3 active superpowers subdirs + 5 archive subdirs), each containing a short `#` comment explaining its purpose.

**Steps:**

- [ ] **Step 1: Create destination directories**

```bash
cd /Users/stephen/.local/share/chezmoi
mkdir -p docs/archive/research docs/archive/superpowers/audits docs/archive/superpowers/plans docs/archive/superpowers/specs
```

- [ ] **Step 2: `git mv` research files**

```bash
for f in 2026-03-19-bash-preexec-atuin-shell-history.md \
         2026-03-19-chezmoi-tool-installation-automation.md \
         2026-03-21-keeping-imessages-alive-on-locked-mac.md \
         2026-03-22-llm-instruction-following-failures.md \
         2026-04-12-act-runner-isolation.md \
         2026-04-12-devbox-vs-nix-flakes.md \
         2026-04-12-karl-davis-dotfiles-review.md \
         2026-04-12-worktrunk.md \
         2026-04-13-sesh-deep-dive.md \
         2026-04-13-sesh-vs-tms.md \
         2026-04-13-television-vs-fzf.md \
         2026-04-14-act-macos-runners.md \
         2026-04-14-dotfiles-improvements.md \
         2026-04-15-jessfraz-dotfiles-review.md \
         2026-04-26-macos-defaults-management.md; do
  git mv "docs/research/$f" "docs/archive/research/$f"
done
```

- [ ] **Step 3: `git mv` superpowers artifacts**

```bash
git mv docs/superpowers/audits/2026-04-28-v2-progress-audit.md docs/archive/superpowers/audits/
git mv docs/superpowers/plans/2026-04-15-dotfiles-improvements.md docs/archive/superpowers/plans/
git mv docs/superpowers/plans/2026-04-19-dotfiles-improvements-v2.md docs/archive/superpowers/plans/
git mv docs/superpowers/specs/2026-04-14-dotfiles-improvements-design.md docs/archive/superpowers/specs/
git mv docs/superpowers/specs/2026-04-17-dotfiles-improvements-v2-design.md docs/archive/superpowers/specs/
```

- [ ] **Step 4: Add 8 `.gitkeep` files with explanatory `#` comments**

```bash
printf '# Keeps docs/superpowers/audits/ tracked between audit cycles when no active audit exists.\n' > docs/superpowers/audits/.gitkeep
printf '# Keeps docs/superpowers/plans/ tracked between plan cycles when no active plan exists.\n' > docs/superpowers/plans/.gitkeep
printf '# Keeps docs/superpowers/specs/ tracked between spec cycles when no active spec exists.\n' > docs/superpowers/specs/.gitkeep
printf '# Keeps docs/archive/ tracked even if all archived files are later removed.\n' > docs/archive/.gitkeep
printf '# Keeps docs/archive/research/ tracked even if emptied.\n' > docs/archive/research/.gitkeep
printf '# Keeps docs/archive/superpowers/audits/ tracked even if emptied.\n' > docs/archive/superpowers/audits/.gitkeep
printf '# Keeps docs/archive/superpowers/plans/ tracked even if emptied.\n' > docs/archive/superpowers/plans/.gitkeep
printf '# Keeps docs/archive/superpowers/specs/ tracked even if emptied.\n' > docs/archive/superpowers/specs/.gitkeep
git add docs/superpowers/audits/.gitkeep docs/superpowers/plans/.gitkeep docs/superpowers/specs/.gitkeep \
        docs/archive/.gitkeep docs/archive/research/.gitkeep \
        docs/archive/superpowers/audits/.gitkeep docs/archive/superpowers/plans/.gitkeep docs/archive/superpowers/specs/.gitkeep
```

- [ ] **Step 5: Verify state**

```bash
ls docs/research/                                                # only 2026-05-01-secrets-management-nix-darwin/
ls docs/archive/research/ | wc -l                                # 16 (15 archived files + .gitkeep)
ls docs/archive/superpowers/{audits,plans,specs}/                # each contains its archived file + .gitkeep
find docs/superpowers docs/archive -name .gitkeep | wc -l        # 8
```

- [ ] **Step 6: Commit**

```bash
git commit -m "chore(docs): archive completed research and superpowers artifacts (2026-05-15)

git mv 15 dated research files + 5 v1/v2 superpowers artifacts to
docs/archive/ preserving the original directory structure. Add 8 .gitkeep
files (3 active superpowers subdirs + 5 archive subdirs) with explanatory
# comments so the directories survive any future cleanup that empties them.

CLAUDE.md rule (added in S2) tells agents to skip docs/archive/ entirely
when auditing docs/ for actionable items."
```

---

### S2: Add CLAUDE.md rule to skip `docs/archive/` when auditing

**Files:**
- Modify: `CLAUDE.md`, add a new subsection under `## Architecture` titled `### Auditing docs/`.

**Steps:**

- [ ] **Step 1: Read CLAUDE.md to find insertion point**

```bash
grep -n '^##' CLAUDE.md
```

Pick the location after the `### macOS Defaults Management` subsection or wherever fits the existing structure.

- [ ] **Step 2: Insert the new subsection**

Add this block:

```markdown
### Auditing docs/

When asked to audit `docs/` for actionable items, **skip `docs/archive/` entirely**, files there have already been audited; treat the archive as out-of-scope. Open follow-ups live in Todoist (`#dotfiles` project). The audit-target directories are `docs/research/` (excluding tied-to-task subdirectories like `2026-05-01-secrets-management-nix-darwin/`) and the active `docs/superpowers/{plans,specs,audits}/`.
```

- [ ] **Step 3: Verify lint passes**

```bash
just l
```

Expected: all checks `✅`.

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md
git commit -m "docs(CLAUDE): skip docs/archive when auditing

Add the convention so future audits short-circuit the archived material.
Files in docs/archive/ have been audited; open follow-ups live in Todoist
(#dotfiles project)."
```

---

### S3: Close GitHub issues #5 and #13 as wontfix

**Files:** None (gh CLI only; no commits).

**Steps:**

- [ ] **Step 1: Close #5 (Bash → Nu Shell)**

```bash
gh issue close 5 --comment "Closing as wontfix. Bash is a locked-in toolchain choice per the dotfiles architecture (atuin daemon, bash-preexec, DEBUG-trap sequencing all depend on bash specifics). See CLAUDE.md § 'Bashrc Init Ordering' for the architectural commitments."
```

- [ ] **Step 2: Close #13 (Zellij exploration)**

```bash
gh issue close 13 --comment "Closing as wontfix. Tmux is a locked-in toolchain choice. The sesh + worktrunk + tpm + tmux2k stack is mature and integrated; migration cost outweighs the benefit. See CLAUDE.md § 'Tmux Session Management' for the architectural commitments."
```

- [ ] **Step 3: Verify**

```bash
gh issue view 5 --json state -q .state
gh issue view 13 --json state -q .state
```

Expected: both output `CLOSED`.

---

## Mouse-Blocker Phases (B1-B3)

These three blockers are prerequisites for P10 and P11. B3 is independent housekeeping and can run anywhere in the sequence after S3.

### B1: Create the `mouse` OpenClaw agent

**Todoist:** `6gfcXjFrG6q3Pm3v`

**Files:** Depend on OpenClaw's agent-definition layout (likely `~/.openclaw/agents/mouse/` or similar, verify against bob's existing layout at `~/workspaces/webdavis/uriel/agents/bob/`).

**Steps:**

- [ ] **Step 1: Fetch task description**

```bash
td task view id:6gfcXjFrG6q3Pm3v --json | jq -r '.description'
```

- [ ] **Step 2: Inspect bob's agent layout as template**

```bash
ls -la ~/workspaces/webdavis/uriel/agents/bob/
cat ~/workspaces/webdavis/uriel/agents/bob/*.md 2>/dev/null | head -40
```

- [ ] **Step 3: Create the mouse agent**

Mirror bob's directory layout at `~/workspaces/webdavis/uriel/agents/mouse/`. Agent persona: a brief, dog-themed notifier. System prompt should accept a JSON payload with `type` ∈ `{agent_input_needed, agent_finished, command_done, gh_notification}` (to be extended in P11) plus context fields and emit "Woof! ..."-style messages.

- [ ] **Step 4: Register mouse with the OpenClaw gateway**

Use whatever registration mechanism bob uses. Confirm via `openclaw agent list` (or equivalent) that `mouse` appears.

- [ ] **Step 5: Smoke test**

Invoke mouse directly with a sample payload (CLI or HTTP) and confirm it produces a "Woof! …" message.

- [ ] **Step 6: Commit (if any tracked files changed)**

```bash
git status -s
# Stage relevant files; example:
git add <agent-related-files>
git commit -m "feat(openclaw): add mouse agent for long-task notifications"
```

- [ ] **Step 7: Mark Todoist task complete**

```bash
td task complete id:6gfcXjFrG6q3Pm3v
```

---

### B2: Create Discord bot tied to mouse

**Todoist:** `6gfcXjRh8vC57g2v`

**Files:** Depends on OpenClaw + Discord plumbing; may include a chezmoi-managed token file (KeePassXC entry).

**Steps:**

- [ ] **Step 1: Fetch task description**

```bash
td task view id:6gfcXjRh8vC57g2v --json | jq -r '.description'
```

- [ ] **Step 2: Create the Discord bot via Discord Developer Portal**

Create a new bot application. Capture: bot token, application ID, intent flags. Store the bot token in KeePassXC as a new entry.

- [ ] **Step 3: Bind the bot to mouse**

Configure OpenClaw to associate the bot with the `mouse` agent. If OpenClaw has a `channels.discord` config block, populate it with the bot token (pulled from KeePassXC via chezmoi template at apply time, NOT committed in plain text). Pattern: `dot_openclaw/channels/discord.yaml.tmpl` or equivalent, mirror existing OpenClaw config templating.

- [ ] **Step 4: Smoke test**

Send a test invocation through mouse. Confirm the message lands in your chosen Discord channel/DM.

- [ ] **Step 5: Commit**

```bash
git status -s
git add <relevant config templates>
git commit -m "feat(openclaw): wire mouse agent to Discord bot

Discord bot token vaulted in KeePassXC; chezmoi template renders the
OpenClaw discord channel config at apply time."
```

- [ ] **Step 6: Mark Todoist task complete**

```bash
td task complete id:6gfcXjRh8vC57g2v
```

---

### B3: Decommission `~/.openclaw-rescue` gateway (delete butters too)

**Todoist:** `6gfcXm9XfvqjV9Fv`

**Files:** Backup of `~/.openclaw-rescue` to `~/workspaces/backups/`; removal of any chezmoi templates referencing the rescue gateway path.

**Steps:**

- [ ] **Step 1: Fetch task description**

```bash
td task view id:6gfcXm9XfvqjV9Fv --json | jq -r '.description'
```

- [ ] **Step 2: Confirm no live references**

```bash
grep -rIl 'openclaw-rescue\|butters' /Users/stephen/.local/share/chezmoi/ 2>/dev/null | head -10
launchctl list | grep -i openclaw-rescue
```

If any references exist, remove or update them in their own commit BEFORE proceeding.

- [ ] **Step 3: Stop the rescue gateway**

If a LaunchAgent runs it: `launchctl bootout gui/$(id -u)/<label>`. Otherwise stop the process directly.

- [ ] **Step 4: Back up `~/.openclaw-rescue` per convention**

```bash
TS=$(date +"%Y-%m-%dT%H-%M-%S")
mkdir -p ~/workspaces/backups/${TS}.openclaw-rescue.backup
cp -R ~/.openclaw-rescue/ ~/workspaces/backups/${TS}.openclaw-rescue.backup/
```

- [ ] **Step 5: Remove the rescue gateway**

Per CLAUDE.md "trash > rm" preference:

```bash
trash ~/.openclaw-rescue
```

- [ ] **Step 6: Commit any chezmoi cleanup**

```bash
git status -s
git add <changes>
git commit -m "refactor(openclaw): decommission rescue gateway and butters agent

backed up to ~/workspaces/backups/<ts>.openclaw-rescue.backup before removal.
butters deleted per user's 'for now' instruction; reconstitute from backup
if needed later."
```

- [ ] **Step 7: Mark Todoist task complete**

```bash
td task complete id:6gfcXm9XfvqjV9Fv
```

---

## Implementation Phases (P1-P13)

Each phase below ends with `td task complete id:<id>`. Detailed implementation steps live in the Todoist task description, fetched at phase start via `td task view`. The plan shows the phase scaffolding (fetch description, verification gate, closeout).

### P1: Improve `prepare-commit-msg` hook to use conventional-commits

**Todoist:** `6gfVJH5P4g4vQ4FM`

**Why first:** every subsequent phase produces commits. Better generator = better audit trail from phase 2 onward.

**Files (per task description):**
- Modify: `private_dot_config/git/hooks/prepare-commit-msg` (chezmoi-managed; verify path with `chezmoi managed | grep prepare-commit-msg`).
- Verify: `~/.claude/skills/conventional-commits/` exists.

**Steps:**

- [ ] **Step 1: Fetch full description**

```bash
td task view id:6gfVJH5P4g4vQ4FM --json | jq -r '.description'
```

- [ ] **Step 2: Execute the steps in the description.**

Inline highlights from the description:
1. Verify `git config --global core.hooksPath` points to `~/.config/git/hooks/`.
2. Switch the prompt to invoke `/conventional-commits` skill.
3. Request type/scope/subject/body/footer per Conventional Commits.
4. Preserve `SKIP_AI_COMMIT=1` bypass + merge/rebase/cherry-pick bail.
5. Maybe expand the 5KB truncation if 10s haiku timeout still tolerates.

- [ ] **Step 3: Verification gate**

Make 3 to 5 representative commits (could be the subsequent setup commits from this plan!) and inspect the generated messages. Confirm conventional-commits format with multi-paragraph body + `Fixes #N` trailer support.

- [ ] **Step 4: Commit the hook change**

```bash
git add private_dot_config/git/hooks/prepare-commit-msg
git commit -m "<conventional-commits-formatted message>"
```

- [ ] **Step 5: Mark Todoist task complete**

```bash
td task complete id:6gfVJH5P4g4vQ4FM
```

---

### P2: PostgreSQL workstation setup

**Todoist:** `6gfVJFgXvG9mJ96M`

**Status:** POSTPONED per user (2026-05-17), likely to land when agents migrate to the NUC server. Detailed steps retained below for when work resumes; do not execute during this cycle unless user re-prioritizes.

**Why second (when active):** time-sensitive for workstation daily use.

**Files (per task description):**
- Modify: `.chezmoidata/system_packages_autoinstall.yaml` (confirm `postgresql@17` present).
- Create: `dot_psqlrc`.
- Modify: `dot_bashrc.tmpl` (add `PSQL_EDITOR`, `PSQL_PAGER` if desired).

**Steps:**

- [ ] **Step 1: Fetch description**

```bash
td task view id:6gfVJFgXvG9mJ96M --json | jq -r '.description'
```

- [ ] **Step 2: Execute the steps**

Inline highlights:
1. Verify `postgresql@17` in yaml + on PATH after `chezmoi apply`.
2. Create `dot_psqlrc` with `\timing on`, colored prompt with timestamp+db name, pager via `less --chop-long-lines` or `bat`.
3. Optional: `~/.pgpass` template if secrets needed (KeePassXC entry).

- [ ] **Step 3: Verification gate**

```bash
psql --version
psql -c '\conninfo'   # against a test DB if available
```

Connect to a DB, confirm `\timing` shows on, prompt shows timestamp + DB name, pager works.

- [ ] **Step 4: Commit**

Conventional-commits format (P1 generator should handle this).

- [ ] **Step 5: Mark Todoist task complete**

```bash
td task complete id:6gfVJFgXvG9mJ96M
```

---

### P3: Audit chezmoi package-manager automation

**Todoist:** `6gfVJCVHWvqJ8Jpv` (closes #11)

**Steps:**

- [ ] **Step 1: Fetch description**

```bash
td task view id:6gfVJCVHWvqJ8Jpv --json | jq -r '.description'
```

- [ ] **Step 2: Execute**

Inline highlights: enumerate `.chezmoiscripts/` automated package managers (Homebrew, rustup, composio, yt-dlp, osquery already automated). Report gaps. Per user, ignore cargo packages; only verify rust toolchain install (already done at `run_once_before_20-install-rustup.sh.tmpl`).

- [ ] **Step 3: Verification gate**

A documented gap list in the audit-outcomes file (update S2's index with the findings) or a new follow-up Todoist task per gap.

- [ ] **Step 4: Commit any new wrapper scripts**

Conventional-commits format. Include `Closes #11` if scope confirmed complete; else comment on the issue with scope reduction.

- [ ] **Step 5: Mark Todoist task complete**

```bash
td task complete id:6gfVJCVHWvqJ8Jpv
```

---

### P4: Add gitleaks pre-commit secret scanning

**Todoist:** `6gfVJ6W9xxjh9FPM`

**Files (per task description):**
- Create: `.gitleaks.toml` (allowlist `*.tmpl`).
- Modify: pre-commit hook to call `gitleaks git --staged --no-banner`.

**Steps:**

- [ ] **Step 1: Fetch description**

```bash
td task view id:6gfVJ6W9xxjh9FPM --json | jq -r '.description'
```

- [ ] **Step 2: Execute**

- [ ] **Step 3: Verification gate**

```bash
# Test commit attempt with a fake API key in a non-tmpl file should be blocked:
echo 'AWS_SECRET=AKIAIOSFODNN7EXAMPLE' > /tmp/test-leak.txt
cp /tmp/test-leak.txt .
git add test-leak.txt
git commit -m "test"   # should be REJECTED by gitleaks
git reset HEAD test-leak.txt
rm test-leak.txt
```

- [ ] **Step 4: Commit**

Conventional-commits format.

- [ ] **Step 5: Mark Todoist task complete**

```bash
td task complete id:6gfVJ6W9xxjh9FPM
```

---

### P5: Determinate Nix migration, DONE (closed 2026-05-17)

**Todoist:** `6gfVJ9rXQ85xr7qM` (closed 2026-05-17; closes #10 on push)

**Status:** COMPLETE. Three commits already on `main`; Todoist task closed via this cycle's housekeeping:
- `58bbb7d chore(nix): migrate installer from DeterminateSystems to NixOS fork`
- `6a3da6f feat(nix): add self-healing nix-installer repair LaunchDaemon`
- `3426adc feat(nix): track user-level nix.conf with flakes + nix-command enabled`

**Residual work (one-shot, after S0 push):** confirm GH #10 auto-closes when the `Closes #10` trailer (added in S0) reaches `main`. If the trailer didn't get added during S0, fall back to `gh issue close 10 --comment "Closed by 58bbb7d + 6a3da6f + 3426adc on main."`. No Todoist closeout needed, task already closed.

---

### P6: Install bandwhich, doggo, ouch CLI tools

**Todoist:** `6gfVJCvxV34W3hgM`

**Steps:**

- [ ] **Step 1: Fetch description**

```bash
td task view id:6gfVJCvxV34W3hgM --json | jq -r '.description'
```

- [ ] **Step 2: Execute**

Add `bandwhich`, `doggo`, `ouch` to `.chezmoidata/system_packages_autoinstall.yaml` formulae list (alphabetical). Optional alias `dig=doggo` in `dot_bash_aliases`.

- [ ] **Step 3: Verification gate**

```bash
chezmoi apply --exclude=templates
command -v bandwhich doggo ouch
```

Expected: all three on PATH.

- [ ] **Step 4: Commit**

Conventional-commits format.

- [ ] **Step 5: Mark Todoist task complete**

```bash
td task complete id:6gfVJCvxV34W3hgM
```

---

### P7: Improve hue-pulse lighting notification system

**Todoist:** `6gfVJGJCfjwCVXQv`

**Depends on:** P6 (bandwhich must be installed for ignored-tools audit).

**Steps:**

- [ ] **Step 1: Fetch description**

```bash
td task view id:6gfVJGJCfjwCVXQv --json | jq -r '.description'
```

- [ ] **Step 2: Execute three sub-tasks**

(a) Audit + expand ignored-tools list in `__cmd_notify_*` framework (`dot_bashrc.tmpl`).
(b) Test every added tool: run interactively >30s and confirm no notification.
(c) Iterate on `~/.local/bin/hue-pulse.sh` color/timing.

- [ ] **Step 3: Verification gate**

User-subjective approval that pulse behavior feels right + skip-list covers actual usage.

- [ ] **Step 4: Commit (likely multiple, separate commits for skip-list vs pulse-tuning).**

- [ ] **Step 5: Mark Todoist task complete**

```bash
td task complete id:6gfVJGJCfjwCVXQv
```

---

### P8: Adopt remaining quick wins from dotfiles-improvements research

**Todoist:** `6gfVJ8Rfh8ppwpqv`

**Steps:**

- [ ] **Step 1: Fetch description**

```bash
td task view id:6gfVJ8Rfh8ppwpqv --json | jq -r '.description'
```

- [ ] **Step 2: Execute the four sub-items**

- bat-extras → switch `MANPAGER` in bashrc to `batman`.
- `fd` as FZF_DEFAULT_COMMAND in bashrc.
- starship `git_metrics` in `dot_config/starship.toml`.
- (hyperfine already in yaml, just confirm.)

- [ ] **Step 3: Verification gate**

`man <topic>` opens batman; `fzf` lists files via fd; starship prompt shows +/- diff counts.

- [ ] **Step 4: Commit (probably one commit per sub-item).**

- [ ] **Step 5: Mark Todoist task complete**

```bash
td task complete id:6gfVJ8Rfh8ppwpqv
```

---

### P9: Add actionlint to lint suite

**Todoist:** `6gfVJ6w2VHc2w4xv`

**Files:**
- Modify: `flake.nix` (add `actionlint` to `baseShell.buildInputs`).
- Modify: `scripts/lint.sh` (add actionlint call).
- Modify: `justfile` (add `a` recipe).
- Modify: `.github/workflows/lint.yml` (add actionlint step).

**Steps:**

- [ ] **Step 1: Fetch description**

```bash
td task view id:6gfVJ6w2VHc2w4xv --json | jq -r '.description'
```

- [ ] **Step 2: Execute**

- [ ] **Step 3: Verification gate**

```bash
just l    # actionlint runs and passes
just a    # standalone actionlint recipe
```

- [ ] **Step 4: Commit**

- [ ] **Step 5: Mark Todoist task complete**

```bash
td task complete id:6gfVJ6w2VHc2w4xv
```

---

### P10: Notify via mouse OpenClaw agent (3 notification types)

**Todoist:** `6gfVJ7VwcFQvg7xM`

**Depends on:** B1 (mouse agent exists) and B2 (Discord bot exists).

**Files (per task description):**
- OpenClaw config: `hooks.enabled = true`, `hooks.token`, `hooks.mappings` with `/hooks/notify` → `mouse`.
- chezmoi template for hooks token (KeePassXC entry).
- Modify: `dot_bashrc.tmpl` (extend `__cmd_notify_precmd` to POST type=`command_done` at >180s).
- Modify: Claude Code Notification + Stop hooks (in `private_dot_claude/modify_settings.json` or wherever they're configured) to POST type=`agent_input_needed` and type=`agent_finished`.

**Steps:**

- [ ] **Step 1: Fetch description**

```bash
td task view id:6gfVJ7VwcFQvg7xM --json | jq -r '.description'
```

- [ ] **Step 2: Execute the four steps in the description**

1. OpenClaw `hooks.*` config + mapping.
2. Render hooks token to 0600 file via chezmoi template.
3. Bashrc `__cmd_notify_precmd` extension.
4. Claude Code hooks extension.

- [ ] **Step 3: Verification gate**

```bash
sleep 200      # → alerter fires + Discord "Woof! sleep 200 completed"
# Trigger a Claude Code permission prompt → Discord "Woof! claude-code is waiting on you"
# Complete a Claude Code task → Discord with full context (agent, session, tmux, cwd, summary, duration)
```

- [ ] **Step 4: Commit (multi-commit; separate logical changes).**

- [ ] **Step 5: Mark Todoist task complete**

```bash
td task complete id:6gfVJ7VwcFQvg7xM
```

---

### P11: Automate gh-notify install + hue-pulse blue + mouse Discord notification

**Todoist:** `6gfVJ9P5vpX64JhM` (closes #9)

**Depends on:** P10 (shared `/hooks/notify` endpoint + mouse + Discord bot).

**Steps:**

- [ ] **Step 1: Fetch description**

```bash
td task view id:6gfVJ9P5vpX64JhM --json | jq -r '.description'
```

- [ ] **Step 2: Execute three sub-tasks**

(a) chezmoi script `run_once_after_<NN>-install-gh-extensions.sh.tmpl` for gh-notify, pinned to a specific upstream commit, idempotent.
(b) Extend `hue-pulse.sh` to accept a color argument; gh-notify watcher fires `hue-pulse.sh blue` on new notifications.
(c) gh-notify watcher POSTs to `/hooks/notify` with `type=gh_notification`. Extend mouse's switch to compose: "Woof! New GitHub notification: <title> in <repo>".

- [ ] **Step 3: Verification gate**

Trigger a fake GitHub notification (assign yourself, comment on a PR, etc.). Confirm: blue hue pulse + Discord message via mouse.

- [ ] **Step 4: Commit with `Closes #9` trailer**

- [ ] **Step 5: Mark Todoist task complete**

```bash
td task complete id:6gfVJ9P5vpX64JhM
```

---

### P12: Add `help.autocorrect = 1` to gitconfig

**Todoist:** `6gfVJ7mrm2259mwM`

**Files:**
- Modify: `dot_gitconfig.tmpl`, add `[help]` section.

**Steps:**

- [ ] **Step 1: Fetch description**

```bash
td task view id:6gfVJ7mrm2259mwM --json | jq -r '.description'
```

- [ ] **Step 2: Edit `dot_gitconfig.tmpl`**

Add:
```gitconfig
[help]
  autocorrect = 1
```

- [ ] **Step 3: Verification gate**

```bash
chezmoi apply ~/.gitconfig    # interactive, KeePassXC unlock needed
git stauts                     # → auto-corrects to `git status` after ~0.1s
```

- [ ] **Step 4: Commit**

- [ ] **Step 5: Mark Todoist task complete**

```bash
td task complete id:6gfVJ7mrm2259mwM
```

---

### P13: Install Tart base macOS VM image

**Todoist:** `6gfVJ8v6pjjF5Qwv`

**Steps:**

- [ ] **Step 1: Fetch description**

```bash
td task view id:6gfVJ8v6pjjF5Qwv --json | jq -r '.description'
```

- [ ] **Step 2: Pull the base image (one-time, ~25 GB)**

```bash
tart clone ghcr.io/cirruslabs/macos-sequoia-base:latest sequoia-runner
```

- [ ] **Step 3: Verification gate**

```bash
tart list   # shows sequoia-runner
```

- [ ] **Step 4: Document the spin-up pattern in CLAUDE.md or a runbook**

If desired:

```bash
tart clone sequoia-runner act-run
tart run act-run --dir=repo:$PWD --no-graphics
```

- [ ] **Step 5: Commit (only if docs/runbook updated; binary image isn't tracked).**

- [ ] **Step 6: Mark Todoist task complete**

```bash
td task complete id:6gfVJ8v6pjjF5Qwv
```

---

## Final Verification

After all phases complete:

- [ ] **A. Todoist tasks all closed**

```bash
td task list --project "dotfiles" --json | jq '.results | length'
```

Expected: 3 open (the 3 pre-existing tasks: quarterly cleanup, orphan docs, nix-darwin migration which is deferred at p4 as of 2026-05-17) PLUS however many of P2 / B1-B3 remain explicitly postponed. All 13 P-cycle implementation tasks should be marked complete; the 16th (P5) is already closed as of 2026-05-17.

- [ ] **B. GitHub issues all addressed**

```bash
for n in 5 9 10 11 13 17; do
  echo "#$n: $(gh issue view $n --json state -q .state)"
done
```

Expected: #5, #10, #13, #17 CLOSED. #9 CLOSED after P11 push. #11 CLOSED after P3 if scope confirmed complete; else OPEN with scope-reduction comment.

- [ ] **C. Directory hygiene**

```bash
ls docs/research/                                                 # only 2026-05-01-secrets-management-nix-darwin/
git log --diff-filter=D --name-only --pretty=format: <S1-sha> | grep -c '^docs/'   # 20
find docs/superpowers -name .gitkeep | wc -l                      # 3
test -f docs/superpowers/audits/2026-05-15-dotfiles-audit-outcomes.md && echo "OK"
grep -q 'audit-outcomes' CLAUDE.md && echo "OK"
```

- [ ] **D. CI green**

After push: `gh run list --workflow=lint.yml --limit 3` shows green.

- [ ] **E. Final commit (sweep)**

If any stray uncommitted changes from the iteration, commit them with a `chore(cycle): close 2026-05-15 audit cycle` summary.

---

## Out of Scope (Future Cycles)

- Migrate to nix-darwin (existing Todoist `6gWP8w7V3R94PRv5`).
- Migrate secrets from KeePassXC to sops-nix (covered by the nix-darwin task).
- npm/pipx broader autoinstalls (deferred unless gaps surface in P3).
- Pi-side OpenClaw gateway migration (architecturally noted; needs its own cycle).
- macOS-defaults-management.md re-verification (deferred unless discrepancies surface).
