# Design: tmux → herdr migration + moshi-hook integration

- **Branch:** `feat/cli-agent-tracking-workflow`
- **Date:** 2026-06-18
- **Status:** Design (brainstorming complete; pending writing-plans → executing-plans)

## Goal

Migrate this chezmoi dotfiles repo from tmux to herdr (a terminal multiplexer with native AI-agent
awareness) AND wire moshi-hook (the AI-agents → Moshi iPhone app bridge) into the new stack. Hard
cutover for tmux→herdr, kept as simple as possible. The aim is a self-reproducing setup: a fresh
`chezmoi apply` on a clean Mac brings herdr up on the preview channel with the keybindings, workspaces,
and agent integration documented below. Linux-ready (the curl installer + chezmoi sources port cleanly).

## herdr concept model (use these terms consistently)

- **session** = persistent background server (≈ tmux SERVER). One default session; survives disconnect.
  Named sessions (`herdr session attach <name>`) isolate bigger setups, not used here.
- **workspace** = per-project tab group anchored to a directory (≈ tmux SESSION). The sidebar shows
  workspaces as "spaces" and rolls each up to its most-urgent agent state.
- **tab / pane** = windows / panes (real PTYs).
- **agent state** = each agent pane carries a semantic state (blocked / working / done / idle); Claude
  Code, Codex, OpenCode, Cursor, etc. are recognized out of the box.

The 8 projects below are 8 WORKSPACES inside the one default session. Use "space"/"workspace"
interchangeably for projects in human-facing text; `workspace` in CLI/config; reserve "session" for the
server.

## moshi ↔ herdr integration (verified, asymmetric)

- **moshi-hook IS herdr-aware natively:** reads `HERDR_ENV`, `HERDR_SESSION`, `HERDR_PANE_ID` (set by
  herdr in panes). The `moshi-hook context` subcommand explicitly supports `tmux`, `herdr`, `zellij`,
  or `shell`. Binary string inspection confirms.
- **herdr is NOT moshi-aware:** `/docs/preview/integrations/` lists AI agents only, no moshi mention.
- **Consequence:** no herdr-side config is required for moshi-hook to work. The single `HERDR_ENV=1`
  signal that herdr exports natively in its panes double-serves both moshi-hook's context detection
  and the herdr Agent Skill's gate.
- `moshi-hook install` writes agent hooks into Claude Code, Codex, OpenCode, Gemini, Cursor, Kimi,
  Qwen, Grok, OMP, Pi (`--target` flag for subset; default = all).
- moshi-hook is the user's primary remote-terminal / agent-bridge tool; Happy daemon coexists.

## Context, current state (verified; herdr 0.7.0 installed)

- chezmoi dotfiles repo. tmux with prefix Ctrl-d, tmux2k theme, ~10 TPM plugins, sesh session manager
  (14 sessions in `dot_config/sesh/sesh.toml`), and a custom status hack
  (`executable_tmux-last-proc.sh` + `executable_tmux-window-emoji.sh` +
  `.chezmoiscripts/run_after_70-install-tmux2k-last-proc.sh.tmpl`).
- Session bootstrap: `~/.local/bin/sesh-bootstrap.sh` (auto-creates uriel/openclaw/homelab) called from
  `dot_bashrc.tmpl` (~lines 311-349, including a `sesh connect uriel` autostart and a `t=` alias),
  `tmux-refresh.sh`, and the Claude Code LaunchAgent.
- `~/.local/bin/claude-restart.sh` + `Library/LaunchAgents/com.claude.code.plist.tmpl` supervise an
  always-on `claude --remote-control` tmux session.
- Happy daemon (`Library/LaunchAgents/com.webdavis.happy-daemon.plist`) bridges Claude Code sessions to
  Happy mobile/web, STAYS, coexists with moshi-hook.
- Bashrc keepers (NOT tmux-coupled): the long-running-command notifier
  (`__cmd_notify_preexec`/`__cmd_notify_precmd`) and the bash-preexec-before-atuin init ordering.

## Already in-flight on this branch (uncommitted)

These edits are already on disk from earlier brainstorming work, they implement the moshi-hook
declarative install and remain consistent with the design below:

- `.chezmoidata/system_packages_autoinstall.yaml`, adds `rjyo/moshi` tap, `rjyo/moshi/moshi-hook`
  formula, and a new `trusted_taps:` field listing `rjyo/moshi`.
- `.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl`, pre-bundle trust loop that runs
  `brew tap` + `brew trust --tap` for every entry in `trusted_taps`.
- `.chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl`, one-time pair + install +
  `brew services start` (pulls pairing token from KeePassXC entry `Moshi :: Pairing Token`).
- `CLAUDE.md`, adds the new chezmoiscript to the interactive-apply list.

## Locked decisions

### Install & update channel (herdr)

- **Current state on this Mac:** herdr 0.7.0 is **already brew-installed**. The migration must
  actively uninstall the brew copy (`brew uninstall herdr`) before the curl installer runs, this is
  not a hypothetical / first-machine path.
- Switch from Homebrew to the direct curl installer
  (`curl -fsSL https://herdr.dev/install.sh | sh`) BECAUSE the preview channel is **unavailable on
  Homebrew installs** (verified, `herdr channel set preview` errors with *"preview channel is only
  available for direct Herdr installs"*).
- Remove `herdr` from `.chezmoidata/system_packages_autoinstall.yaml`.
- Add a chezmoi `run_onchange_before_*` script that, idempotently on every run:
  1. If `brew list herdr` succeeds → `brew uninstall herdr` (handles the current state above AND any
     future fresh machine where someone brew-installs herdr before applying dotfiles).
  2. Run the curl installer (`curl -fsSL https://herdr.dev/install.sh | sh`).
  3. Ensure the preview channel is active.
- Channel: **preview**. Prefer setting it declaratively via `[update] channel = "preview"` in the
  tracked config; CLI fallback is `herdr channel set preview` (whichever works on direct install, see
  spike below).
- Update cadence: **manual** (`herdr update`), never on every `chezmoi apply`.

### Config (herdr)

- Track config at `dot_config/herdr/config.toml`, seeded from `herdr --default-config` then customized.

### Prefix & keybindings (real herdr action names)

- `prefix = "ctrl+d"` (herdr default is `ctrl+b`).
- `goto = "prefix+g"` (already herdr's default, the workspace picker; primary space switcher).
- `rename_tab = "prefix+comma"`.
- **Splits deliberately crossed** to preserve tmux muscle memory:
  - `split_horizontal = "prefix+\""` (top/bottom stack)
  - `split_vertical = "prefix+%"` (side-by-side)
  - Reason: herdr names splits by *divider orientation*; tmux by *motion direction*. Opposite words,
    same physical result. The cross matches what `prefix+"`/`prefix+%` do in tmux today.
- `navigate_workspace_up = "k"`, `navigate_workspace_down = "j"`, local keys inside herdr's
  built-in navigate mode (a stock mode like copy mode; not a user-defined key table, so this does not
  conflict with the multi-step-keybindings constraint below).
- Keep herdr's default `prefix+h/j/k/l` (`focus_pane_*`) as fallback pane focus; raw `Ctrl-h/j/k/l`
  is owned by herdr.nvim (see Neovim section).
- Map every remaining tmux binding best-effort during implementation, calling out every collision for
  per-binding user decision.

### Send-prefix workaround (Ctrl-d EOF)

Because `prefix = ctrl+d`, the literal Ctrl-d (shell EOF) keystroke is otherwise consumed by prefix
mode. Workaround:

```toml
[[keys.command]]
key = "prefix+ctrl+d"
type = "shell"
command = "$HERDR_BIN_PATH pane send-keys $HERDR_ACTIVE_PANE_ID 'ctrl+d'"
description = "double-tap Ctrl-d → send literal Ctrl-d (EOF) to focused pane"
```

Mechanism: double-tap `ctrl+d` → first press enters prefix mode → second fires this binding → the
herdr CLI injects a literal `ctrl+d` byte into the focused pane's PTY. Conceptually equivalent to
tmux's `bind -n send-prefix`. **Implementation spike** (below): verify `pane send-keys "ctrl+d"`
actually triggers EOF in the shell.

### Workspaces (8), quick-jump chords

All chords live in the `prefix+ctrl+<letter>` namespace, currently empty in herdr defaults and
structurally unlikely to be populated upstream (a breaking change for any existing user). Each
workspace gets one `[[keys.command]]` entry running `herdr workspace create --cwd <path> --label
<name> --focus` (create-or-focus semantics).

| Workspace | Chord | Path |
|---|---|---|
| homelab | `prefix+ctrl+h` | `~/workspaces/Ivy/webdavis/homelab` *(auto-start)* |
| dotfiles | `prefix+ctrl+.` → `prefix+.` fallback | `~/workspaces/Ivy/webdavis/dotfiles` |
| casually-concerned | `prefix+ctrl+c` | `~/workspaces/Ivy/casually-concerned` |
| Ivy | `prefix+ctrl+i` | `~/workspaces/Ivy` |
| justdavis-ansible | `prefix+ctrl+j` | `~/workspaces/Ivy/karlmdavis/justdavis-ansible` |
| essential-feed-case-study | `prefix+ctrl+e` | `~/workspaces/Ivy/webdavis/essential-feed-case-study` |
| netpulse | `prefix+ctrl+n` | `~/workspaces/Ivy/webdavis/netpulse` |
| plantpulse | `prefix+ctrl+p` | `~/workspaces/Ivy/hobbies/plantpulse` |

Notes:

- `prefix+ctrl+.` depends on the CSI-u keyboard protocol in Ghostty + herdr's support for it. If it
  doesn't fire reliably during testing, fall back to `prefix+.`.
- Ctrl-letter byte aliases (`ctrl+i`=Tab, `ctrl+m`=Enter, etc.) are accepted side-effects: e.g.
  `prefix+ctrl+i` for Ivy also fires on `prefix+Tab` (overriding herdr's default `cycle_pane_next`);
  user accepted the loss (still has `prefix+h/j/k/l` direct pane focus).
- The workspace set is the **active standalone-repo set** plus the Ivy vault root: own git repo →
  dedicated workspace; vault-subfolder area (resources/fitness/career-campaign) → reach via the Ivy
  workspace, no dedicated chord.

### Auto-start

- ONLY the `homelab` workspace auto-creates on login; a fresh interactive bash lands inside homelab (in
  the one default session). The other 7 are on-demand via their jump keybindings.
- Update the bashrc autostart block (replace `sesh connect uriel` with the herdr homelab equivalent)
  and the `t=` alias.

### Neovim ↔ herdr pane navigation

Adopt **`devxplay/herdr.nvim`** for seamless raw `Ctrl-h/j/k/l` across nvim splits and herdr panes,
the only known solution. It's early-stage (4 commits, no releases, 3 stars on inspection) → pin to an
exact commit SHA.

Requirements:

- Rust helper binary `herdr-navigator` installed via
  `cargo install --git https://github.com/devxplay/herdr.nvim.git --bin herdr-navigator --rev <SHA> --locked`
- Neovim plugin via lazy.nvim: `{ "devxplay/herdr.nvim", commit = "<SHA>" }`
- Four `[[keys.command]]` entries binding raw `ctrl+h/j/k/l` to
  `herdr-navigator dispatch {left|down|up|right}`, plus split-key routing and fallback keys per the
  plugin README.
- Marker files at `~/.cache/herdr.nvim/panes/` (managed by the plugin/binary).

**Rust toolchain bootstrap:** a new chezmoi `run_once_before_05-install-rustup.sh.tmpl` installs rustup
via `curl https://sh.rustup.rs | sh -s -- -y --no-modify-path` if `cargo` is missing. Then
`run_onchange_after_*-install-herdr-navigator.sh.tmpl` runs the `cargo install --git --rev` at the
pinned SHA. This is the **one accepted bit of complexity** in an otherwise simple migration.

### Agent skills (herdr Agent Skill + Moshi Skill)

Both skills are **vendored** into the chezmoi tree alongside a `just` recipe to refresh from upstream
on demand.

- **herdr Agent Skill**, copy of upstream
  `https://github.com/ogulcancelik/herdr/blob/master/SKILL.md` into
  `private_dot_claude/skills/herdr/SKILL.md`. Teaches AI agents (with `HERDR_ENV=1` set, which herdr
  exports natively in its panes) to use the `herdr` CLI for terminal control.
- **Moshi Skill**, installed by `npx skills add rjyo/moshi-skill`, then vendor the resulting
  `~/.claude/skills/...` file into `private_dot_claude/skills/moshi/SKILL.md` for reproducibility.
- `just update-agent-skills`, one recipe that re-pulls both upstream skill files.
- `HERDR_ENV` is set automatically by herdr inside its panes (verified via binary inspection); no
  manual shell export is needed.

### Moshi (mobile agent bridge)

Recap of the already-in-flight implementation (above):

- `rjyo/moshi` tap + `moshi-hook` formula declared in `.chezmoidata/system_packages_autoinstall.yaml`.
- `brew trust --tap rjyo/moshi` automated via the new trust loop in
  `.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl` (new `trusted_taps:` YAML field).
- One-time setup: `.chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl` runs
  `moshi-hook pair --token <keepassxc>`, `moshi-hook install`, `brew services start moshi-hook`.
- KeePassXC entry **`Moshi :: Pairing Token`** must exist with the Password field set to the pairing
  token from the Moshi iPhone app (Settings → Integrations). This script is on the interactive-apply
  list in CLAUDE.md.

### claude-restart.sh + com.claude.code LaunchAgent → REMOVE

- DECISION: remove `~/.local/bin/claude-restart.sh`,
  `Library/LaunchAgents/com.claude.code.plist.tmpl`, and any loader chezmoi script.
- Rationale: the always-on Claude Desktop registration that script provided is not actively used by
  this user. Mobile agent control is covered by Happy + moshi-hook (both bridge already-running agent
  sessions); persistent `claude --remote-control` supervision is no longer needed.

### Happy daemon → KEEP (coexist with moshi-hook)

- `Library/LaunchAgents/com.webdavis.happy-daemon.plist` + the
  `run_onchange_after_62-load-happy-daemon-launchagent.sh.tmpl` loader + the `happy` npm package all
  STAY untouched.
- moshi-hook is the primary bridge, but Happy runs alongside; user picks the mobile UI per session.

### Multi-step keybindings (key tables), NOT AVAILABLE (known constraint)

- Verified via comprehensive source-code review (parser at `src/config/keybinds.rs:813,983`,
  `Mode` enum at `src/app/state.rs:748-769`, plugin v1 manifest schema) and a full issues /
  discussions / PRs sweep: herdr is structurally single-prefix + single-chord-per-binding. No custom
  modes, no leader sequences, no plugin extension point for keybindings. Discussions #596 (named
  prefixes) and #599 (repeatable bindings) sit with no maintainer engagement; the plugin v1 CHANGELOG
  explicitly defers runtime action registration and native non-terminal plugin UI.
- Implication: tmux-style `prefix → C-o → letter` key tables CANNOT be replicated. All workspace
  jumps are flat single chords (resolved above).

### `moshi .` post-migration behavior, implementation spike

- `moshi .` today opens a tmux session per `moshi-hook help`.
- Spike during implementation: verify whether the command auto-detects `HERDR_ENV` and spawns a herdr
  workspace, or whether it stays tmux-only.
- If herdr-aware: keep the shortcut.
- If tmux-only: retire it (don't depend on the shortcut after tmux is gone).

### herdr server pre-warm, NOT NEEDED

- Plain `herdr` (or any `[[keys.command]]` that runs the CLI) auto-creates the default session if
  none exists.
- bashrc's homelab autostart on first interactive shell is sufficient.
- No dedicated `herdr server` pre-warm LaunchAgent.

### Copy-mode parity

- Accept herdr-native copy mode: `prefix+[` to enter; `h/j/k/l` / `w/b/e` / `{`/`}` movement; `v` or
  Space to select; `y` or Enter to copy; `q` or Esc to leave.
- `prefix+e` = `edit_scrollback` (herdr default).
- tmux-thumbs and tmux-fuzzback stay cold-dropped (no herdr equivalent; feature loss accepted per the
  hard-cutover scope).

### Docs (CLAUDE.md updates)

- Rewrite dotfiles CLAUDE.md sections: "Tmux Session Management", "Tmux Window/Pane Status
  Indicators", and the tmux-specific parts of "Bashrc Init Ordering". Add a "Moshi integration"
  section covering the install + setup script + the asymmetric integration shape.
- Rewrite the global `~/.claude/CLAUDE.md` Toolchain line that lists tmux as "locked-in" → herdr.

## Removals (hard cutover, no dead code/packages)

- `tmux`, `tmux2k`, and ALL TPM plugins cold-dropped with no replacement: `tpm`, `tmux-sensible`,
  `tmux-resurrect`, `tmux-continuum`, `tmux-yank`, `tmux-fzf-url`, `tmux-thumbs`, `tmux-fuzzback`,
  `tmux-sessionist`, `tmux-floax`.
- `sesh` and its whole surface: `dot_config/sesh/` (`sesh.toml`, `sesh-preview.sh`,
  `smart-startup.sh`, `todoist-project-map.toml`, `scripts/`), `~/.local/bin/sesh-bootstrap.sh`.
- The 3 status-hack files + `~/.local/bin/executable_tmux-custom-list-keys.sh` +
  `~/.local/bin/executable_tmux-refresh.sh`.
- `dot_tmux.conf` + `dot_tmux/`; tmux/sesh entries in `.chezmoidata/system_packages_autoinstall.yaml`;
  tmux bashrc snippets; related `.chezmoiignore` patterns.
- `~/.local/bin/claude-restart.sh` + `Library/LaunchAgents/com.claude.code.plist.tmpl` + their loader.

## Sources (cited; verify herdr/moshi claims against these, not training data)

- herdr preview docs: `herdr.dev/docs/preview/*` (quick-start, configuration with
  `#custom-command-keybindings`, keyboard, cli-reference, persistence-remote, agents, integrations,
  agent-skill, install, session-state, plugins, socket-api, concepts).
- `github.com/ogulcancelik/herdr`, source code review (single-prefix model confirmed).
- `github.com/devxplay/herdr.nvim`, Neovim pane-nav plugin.
- `getmoshi.app`, Moshi homepage (herdr explicitly supported).
- `moshi-hook help` output + binary string inspection (`HERDR_ENV` / `HERDR_SESSION` /
  `HERDR_PANE_ID` confirmed).
- Local dotfiles repo, source of truth for what must be replicated.
- Captured real default config at `/tmp/herdr-default-config.toml` (herdr 0.7.0).

## Constraints

- Keep it simple; the herdr.nvim Rust helper is the only accepted complexity.
- Follow chezmoi conventions (`dot_` / `private_` / `executable_` / `.tmpl` prefixes, OS guards,
  `.chezmoiignore` for source-only files). Linux-ready structure. `just l` must pass.
- Don't modify third-party source. Conventional Commits; **NO** Claude/Anthropic co-author trailers.
- Scope is strictly **terminal multiplexer + moshi**. Don't expand to adjacent cleanup.

## Open implementation questions (verification spikes for the plan phase)

1. **`pane send-keys 'ctrl+d'` injects EOF**: wire the send-prefix workaround, double-tap Ctrl-d,
   verify the shell exits. If the byte isn't interpreted as EOF, find an alternative (e.g. send the
   raw `0x04` byte) or fall back to using `exit`.
2. **`prefix+ctrl+.` fires reliably in Ghostty + herdr**: depends on CSI-u keyboard protocol; if it
   doesn't fire, fall back to `prefix+.` for dotfiles.
3. **`[update] channel = "preview"` honored on direct install** vs. requiring `herdr channel set
   preview` CLI invocation.
4. **`moshi .` post-migration behavior** (above).
5. **Full tmux→herdr keybinding collision sweep**: walk the entire `dot_tmux.conf` binding list,
   surface every collision against herdr defaults + the configured set, decide per-binding before
   implementation.
6. **Rust toolchain bootstrap edge cases**: what if `cargo install` fails on a clean machine without
   build dependencies (e.g., missing Xcode CLT)? Document the fallback / prerequisite check in the
   bootstrap script.

## Definition of done

- herdr is the default multiplexer on this Mac: preview channel via direct install, prefix Ctrl-d, the
  8 workspace jumps wired, `homelab` auto-opening on terminal launch, seamless Neovim↔herdr nav via
  herdr.nvim, herdr Agent Skill + Moshi Skill both vendored and wired.
- moshi-hook is paired, hooks installed (default agent set), brew service running, silently picking
  up `HERDR_ENV` from herdr panes.
- All tmux/sesh/tmux2k/TPM/hack-script cruft removed; `claude-restart.sh` + `com.claude.code`
  LaunchAgent removed; Happy daemon stack untouched (kept).
- Send-prefix double-tap binding works (Ctrl-d EOF preserved).
- CLAUDE.md (repo + global) updated. `just l` green. Sources structured Linux-ready.
- The eventual implementation plan lands at
  `docs/superpowers/specs/2026-06-18-tmux-to-herdr-migration-plan.md`.

## Proposed commit sequence (the writing-plans phase will refine)

1. `feat(herdr): switch from brew to direct curl installer (preview channel)`, chezmoi script +
   YAML removal of `herdr`.
2. `feat(rust): bootstrap rustup if missing`, prereq for herdr-navigator build.
3. `feat(herdr): track ~/.config/herdr/config.toml in chezmoi`, seeded from `herdr --default-config`,
   `prefix=ctrl+d` + the locked keybindings.
4. `feat(herdr): add the 8 workspace quick-jump chords`, `[[keys.command]]` entries.
5. `feat(herdr): wire the send-prefix double-tap binding`, Ctrl-d EOF workaround.
6. `feat(herdr): install + pin devxplay/herdr.nvim and the navigator binary`, chezmoi script,
   lazy.nvim entry, four routing bindings.
7. `feat(claude): vendor the herdr Agent Skill into private_dot_claude/skills/herdr/SKILL.md` + a
   `just update-agent-skills` recipe.
8. `feat(claude): vendor the Moshi Skill into private_dot_claude/skills/moshi/SKILL.md`.
9. `feat(bashrc): land in herdr homelab on interactive shell`, replace `sesh connect uriel`; update
   the `t=` alias.
10. `chore: remove claude-restart.sh + com.claude.code LaunchAgent`, Happy + moshi cover the
    bridging.
11. `chore: remove tmux + tmux2k + TPM plugin set + sesh + 3 hack scripts + tmux config`, cold
    cutover.
12. `docs(claude): rewrite tmux sections in CLAUDE.md → herdr`, repo + global.
13. `chore: run just l`, verify everything lints.

(The moshi-hook brew install + trust loop + setup script + CLAUDE.md interactive-apply addition are
already on this branch as uncommitted edits; the plan can group them as their own commit early in the
sequence.)
