---
title: Dotfiles v2 progress audit
date: 2026-04-28
status: final
audience: Stephen (chezmoi maintainer)
scope: Verify each task in `docs/superpowers/plans/2026-04-19-dotfiles-improvements-v2.md` against the live state of the chezmoi source tree on `main`. Identify gaps, post-v2 follow-ons, and the worktree branch's status.
---

# Dotfiles v2 Progress Audit — 2026-04-28

## Executive summary

The v2 plan is **substantially complete on `main`**: 39 of 40 tasks verified DONE,
1 gap confirmed (E2 — likely closeable as superseded by the ambient GHA watcher),
1 conflict-with-user-preference noted. The worktree branch
`dotfiles-v2-implementation` was stale (0 commits ahead of main, 60 behind);
removed and branch deleted on 2026-04-28. Three post-v2 threads remain
unstarted: **macOS defaults management** (research complete, no implementation),
the **GitHub-webhook-service** (3-path comparison documented in conversation
2026-04-25; no path selected, no plan yet — replaces the current ambient GHA
watcher), and the **Neovim overhaul** (no spec, plan, or research yet).

## Phase-by-phase status

| Task  | § ref       | Status | Evidence                                                                                                                  |
| ----- | ----------- | ------ | ------------------------------------------------------------------------------------------------------------------------- |
| A1    | §15.1       | DONE   | `.chezmoiscripts/run_once_before_00-install-homebrew.sh.tmpl`                                                             |
| A2    | §17         | DONE   | `.chezmoidata/system_packages_autoinstall.yaml`; recent updates `c625d35`, `96f9910`, `ce5e52c`                           |
| A3    | §11         | DONE   | Operational; CLAUDE.md documents the workflow                                                                             |
| B1    | §6          | DONE   | `82cf777` daemon LaunchAgent, `c27d54e` history_filter, `9844f72` self-healing socket                                     |
| B2    | §9.6        | DONE   | `dot_inputrc:30,38,66` (bracketed-paste, keyseq-timeout=200, show-mode-in-prompt)                                         |
| B3    | §9.7        | DONE   | `dot_config/starship.toml:36,37,218,224` (scan_timeout=30, command_timeout=1500, `[nix_shell]`, `[direnv]`)               |
| B4    | §9.8        | DONE   | `dot_config/ghostty/config`                                                                                               |
| B5    | §9.9        | DONE   | `dot_config/bat/config` v2-additions block (`--style`, `--map-syntax`, `--pager`)                                         |
| B6    | §11         | DONE   | `.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl`                                                          |
| B7    | §8.3        | DONE   | `dot_config/private_karabiner/`                                                                                           |
| C1    | §2, §8, §16 | DONE   | Many commits; carapace loads in bashrc per CLAUDE.md (covers §2.7 universal completions for docker/kubectl/chezmoi/uv)    |
| C2    | §3, §21     | DONE   | `dot_tmux.conf:199,207,220,221` (extended-keys csi-u, aggressive-resize, history-limit=50000)                             |
| C3    | §9          | DONE   | `dot_gitconfig.tmpl:55-58,89` (rebase autoStash + updateRefs, autocorrect=prompt); `917da86` gpg.program pin              |
| D1    | —           | DONE   | `dot_config/sesh/sesh.toml`; this session's preview wiring (`34c1b46`)                                                    |
| D2    | —           | DONE   | `dot_local/bin/executable_sesh-bootstrap.sh`                                                                              |
| D3    | —           | DONE   | `dot_config/worktrunk/config.toml`                                                                                        |
| D4    | §5          | DONE   | `abeb834` (this session's split-by-purpose), `964e863`, `f5fa5b5`                                                         |
| E1    | §7.2-7.3    | DONE   | `00e670b`, `a1e40b6`, `09e6d18`; `dot_local/bin/executable_hue-pulse.sh`                                                  |
| E2    | §7.4        | GAP    | **No `pushwatch` reference anywhere.** Likely superseded by ambient GHA watcher (`a6a1df2`).                              |
| E3    | —           | DONE   | 5 hooks present: `claude-audit.sh`, `claude-restart.sh`, `claude-stop-pulse.sh`, `claude-user-prompt-start.sh`, `hue-pulse.sh` |
| E4    | §21         | DONE   | `4334107`, `c88196b`; tmux-window-emoji + tmux-last-proc plugin                                                           |
| E5    | §10.1       | DONE   | Chezmoi-managed at `dot_config/git/hooks/executable_prepare-commit-msg` (committed `e6df212`); timeout bumped 4s → 10s on 2026-04-28 after observing claude haiku consistently took ~4s and the prior timeout truncated 100% of calls |
| F1    | §18         | DONE   | `dot_local/bin/*fetch-gitignore*` absent                                                                                  |
| F2-F5 | §18         | DONE   | All 4 scripts present and lint-clean (this session ran shellcheck/shfmt 3× green)                                         |
| G1    | §14.1       | DONE   | `30e896f refactor(osquery): template homeDir + configurable report dir`                                                   |
| G2    | §14.2       | DONE   | All 5 plists are `.tmpl`: `com.claude.code`, `com.webdavis.atuin-daemon`, `com.webdavis.gha-watcher`, `com.webdavis.osquery-report`, `com.webdavis.yt-dlp-pot-provider` |
| G3    | §14.6       | DONE   | `96be864 chore(chezmoiignore): ignore **/.DS_Store`                                                                       |
| H1    | §12, §14.3  | DONE   | Evolved to modify-template approach; `169df6b`, `b132707`, `5cc4e1d`                                                      |
| I1    | §13         | DONE   | `44877e8` migrate 4 skills, `1337576` add conventional-commits + humanizer                                                |
| I2    | §13         | DONE   | `636907b feat(claude): manage statusline-command.sh in chezmoi`                                                           |
| I3    | §20         | DONE   | `e3223f0 feat(claude): global CLAUDE.md, /pr-merge command, chezmoi-apply agent` plus refinements                         |
| J1-J3 | §19         | DONE   | `2ec6c9f` adds taplo/jq/yq runners + justfile recipes; all 7 lint checkers green                                          |
| K1    | §8.1        | DONE   | `tms`, `rbenv` binaries absent; no `rbenv`/`sdkman`/`nvm` references in `dot_bashrc.tmpl`/`dot_profile`/`dot_bash_profile`/`dot_bash_bindings` |
| L1    | —           | DONE   | `4639612 docs(CLAUDE): v2 overhaul` plus subsequent refinements                                                           |
| L2    | —           | DONE   | `just l` ran cleanly 3× during this session                                                                               |

## Confirmed gaps

### E2 — `gh pushwatch` alias (§7.4)

Missing entirely. No file or alias mentions `pushwatch`. The functionality
appears **superseded** by the ambient GHA watcher
(`a6a1df2 feat(notifications): ambient GitHub Actions watcher with hue-pulse +
alerter`), which monitors GitHub Actions runs continuously rather than on-demand
after a push.

**Decision needed:** close as superseded (recommended) or implement as planned.

### ~~E5 — Prepare-commit-msg hook not chezmoi-managed~~ (resolved on inspection)

Initial finding incorrect. The hook **is** chezmoi-managed at
`dot_config/git/hooks/executable_prepare-commit-msg` (committed in `e6df212
feat(git-hooks): add hardened prepare-commit-msg via Claude haiku`). The
`find` that returned only `.git/hooks/prepare-commit-msg.sample` missed it
because the file was transiently absent from the working tree — restored via
`cp` on 2026-04-28.

A real sub-issue did exist: the hook's `timeout 4` budget was too tight (claude
haiku consistently lands at ~4.0s in this environment, so the hook truncated
100% of calls and produced empty messages). Fixed in same session by bumping
to `timeout 10`.

## Conflicts with stated user preference

### C3 §9.4 git aliases

`dot_gitconfig.tmpl:204-207` adds `undo`, `unstage`, `recent`, `whoami`. User
stated 2026-04-28 they don't use git aliases. These are dead config from the v2
plan, not user-driven. Safe to remove if a leaner gitconfig is preferred. Not
urgent.

## Worktree branch status (resolved 2026-04-28)

`.worktrees/dotfiles-v2-implementation/` was **0 commits ahead, 60 behind**
main with 17 uncommitted modifications under `private_dot_claude/skills/*`
that diverged from main's content (main had the `humanizer` skill — ~519
lines — that the worktree didn't, plus newer `todoist-cli`,
`web-research-task`, `youtube-transcript` SKILL.md). Diff confirmed main's
content was strictly fresher.

Resolution: `git worktree remove --force` + `git branch -D
dotfiles-v2-implementation` (was at `f64cd50`). Divergent uncommitted edits
discarded; nothing of value lost.

## Post-v2 follow-on threads

### macOS defaults management

`docs/research/2026-04-26-macos-defaults-management.md` (82KB, 2 days old).
Research is complete and ends in concrete recommendations. No implementation
yet. Three artifacts to create:

- `.chezmoidata/macos_defaults.yaml` — declarative defaults catalog (mirrors the existing `system_packages_autoinstall.yaml` pattern)
- `.chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl` — runner that reads the YAML and applies via `defaults write`
- `dot_local/bin/executable_macos-defaults-drift.sh` — drift detector wired into the justfile as `just defaults-drift`

None of the three exist on main. Next step: brainstorm to lock scope (which
~30 settings to track in the starter list), then plan via
`superpowers:writing-plans`, then execute.

### Neovim overhaul

The v2 plan explicitly defers (`docs/superpowers/plans/2026-04-19-dotfiles-improvements-v2.md:3464`):

> Editor: Neovim (overhaul is a separate sub-project — out of scope for most
> work here).

No spec, plan, or research exists. Brainstorming session is the entry point.
Open questions for that session:

- Current pain points with `~/.config/nvim`
- Target feature set: LSP, plugin manager (lazy.nvim?), AI integration with Claude, theme, completion, debugging, etc.
- Migration from current config vs. greenfield rewrite
- Whether the new config lives in this chezmoi repo (under `dot_config/nvim/`) or stays in its own dedicated repo

## Recommended sequence

1. **Decide E2's fate** — close as "superseded by ambient GHA watcher
   (`a6a1df2`)" if you're satisfied with the polling implementation, OR queue
   the webhook-replacement work for after macOS defaults (your original
   sequencing per the 2026-04-27 message: *"v2 carry-overs → macOS defaults →
   atuin server + GitHub-webhook-service"*). ~5 min decision.
2. **Optional micro-cleanup** — drop the unused git aliases from
   `dot_gitconfig.tmpl:204-207` if a leaner gitconfig is preferred. ~5 min.
3. **macOS defaults management** — brainstorm → spec → plan → execute,
   anchored on `docs/research/2026-04-26-macos-defaults-management.md`. Half-day.
4. **GitHub-webhook-service** — pick a tunnel path (Smee.io / Tailscale Funnel
   / Cloudflare Tunnel; full comparison at the 2026-04-25T11:55Z message in
   transcript `7f0c819f-38a8-4ce0-a456-95a6ec7722ba.jsonl`), brainstorm to
   spec, plan, execute. Replaces the current `gha-watcher` polling
   implementation: delete `dot_local/bin/executable_gha-notify.sh`,
   `dot_local/bin/executable_gha-watcher.sh`, and
   `Library/LaunchAgents/com.webdavis.gha-watcher.plist.tmpl` once the webhook
   path lands. ~Half-day to a day.
5. **Self-hosted atuin server** — separate but adjacent to the webhook
   service. Untracked draft already at
   `~/workspaces/webdavis/Homelab/docs/PLAN-ATUIN-SERVER.md`; pair with
   architecture doc `~/workspaces/webdavis/Homelab/docs/plans/remote-access-architecture.md`.
   Sequenced alongside webhook work per your stated order.
6. **Neovim overhaul** — brainstorm session as the entry point. No
   spec/plan/research yet. Multi-session sub-project.
