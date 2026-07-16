# tmux → herdr Migration + moshi-hook Wiring: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended)
> or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax
> for tracking.

**Goal:** Hard-cutover this chezmoi dotfiles repo from tmux to herdr (preview channel via direct curl
install), wire moshi-hook for mobile agent control, and remove every tmux/sesh/tmux2k/status-hack
artifact in one self-reproducing pass.

**Architecture:** chezmoi declarative dotfiles. Source state under `~/.local/share/chezmoi/`, target
state in `$HOME`. herdr is installed by an idempotent `run_onchange_before_*` chezmoiscript (not brew).
Config lives at `dot_config/herdr/config.toml`. Agent skills are vendored into the repo. Bashrc lands the
user inside the `homelab` workspace on every fresh interactive shell. All tmux/sesh files cold-drop in a
single removal commit *after* herdr is functional, so the user is never stranded multiplexer-less.

**Tech stack:** chezmoi 2.62.3+; Nix flake dev shell (shellcheck, shfmt, mdformat, taplo, jq, yq); herdr
0.7.x preview channel; moshi-hook (Homebrew tap `rjyo/moshi`); KeePassXC for secrets; cargo + rustup for
the herdr-navigator binary; lazy.nvim (for the herdr.nvim plugin entry, out-of-repo).

## Global Constraints

Every task implicitly carries these:

- **chezmoi naming:** `dot_` → `.`, `private_` → permissions, `executable_` → +x, `.tmpl` → Go template.
  `run_once_*` runs once per machine, `run_onchange_*` re-runs when content changes, `run_*` runs every
  apply.
- **OS guard:** all chezmoiscripts wrap darwin-only logic in `{{ if eq .chezmoi.os "darwin" -}}` /
  `{{ end -}}`. Linux-ready by structure.
- **Never run bare `chezmoi apply`** from inside an agent, use
  `chezmoi apply --exclude=templates --force` (skips KeePassXC-touching templates) or apply specific
  non-template files by name.
- **`just l` must pass green** at end of every commit. Pre-commit hook will block otherwise.
- **Conventional Commits** (`feat:`, `fix:`, `chore:`, `docs:`). **NO `Co-Authored-By: Claude` trailers,
  NO "🤖 Generated with Claude Code" footers**. Commits must look as if the user authored them directly.
- **Shell scripts:** `set -euo pipefail`, double-quoted expansions, 2-space indent, case-indent on,
  simplified (`shfmt -i 2 -ci -s`).
- **Use `trash` not `rm`** when removing files at the OS level. For chezmoi-tracked files, deletion is
  `git rm` (chezmoi apply propagates).
- **Plan output is at the spec's pinned path:**
  `docs/superpowers/plans/2026-06-18-tmux-to-herdr-migration-plan.md`.

## File Structure

**Create (new files in chezmoi source):**

- `dot_config/herdr/config.toml`: herdr config (prefix, splits, workspace chords, nav, send-prefix)
- `.chezmoiscripts/run_onchange_before_15-install-herdr.sh.tmpl`: direct curl installer + brew-uninstall
  guard
- `.chezmoiscripts/run_onchange_after_50-install-herdr-navigator.sh.tmpl`:
  `cargo install --git --rev <SHA>` for the Neovim nav helper
- `private_dot_claude/skills/herdr/SKILL.md`: vendored herdr Agent Skill
- `private_dot_claude/skills/moshi/SKILL.md`: vendored Moshi Skill
- (Already on disk via the stash) `.chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl`

**Modify:**

- `.chezmoidata/system_packages_autoinstall.yaml`: remove `tmux` (line ~124) and `sesh` (line ~111); the
  stash already adds the `rjyo/moshi` tap + `moshi-hook` formula + `trusted_taps:`
- `.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl`: stash already adds the trust loop;
  nothing further
- `dot_bashrc.tmpl`: replace the entire tmux autostart block (lines 310-349), remove the
  `tmux-last-proc` precmd (lines 285-301), remove the `TERM='tmux-256color'` export (line 36), remove the
  `tmux-purge-resurrect-session-data` alias (line 192), remove `tmux` from the TUI skip-list (line 271),
  and rewrite line 196, replacing the `alias t='sesh connect uriel'` with a new
  `alias h='herdr workspace create ... homelab ...'` (rename `t` → `h` to match the herdr-era mnemonic;
  `t` was a tmux-era leftover)
- `CLAUDE.md`: stash already adds the moshi-hook setup script to the interactive-apply list; THIS plan
  additionally rewrites the "Tmux Session Management" + "Tmux Window/Pane Status Indicators" + tmux parts
  of "Bashrc Init Ordering" sections, and adds a "Moshi integration" section
- `~/.claude/CLAUDE.md` (global, outside this repo): rewrite the Toolchain "Multiplexer: tmux" line to
  "Multiplexer: herdr"
- `justfile`: add `update-agent-skills` recipe

**Delete (git rm + chezmoi apply propagates removal to $HOME):**

- `dot_tmux.conf`
- `dot_config/sesh/sesh.toml`
- `dot_config/sesh/todoist-project-map.toml`
- `dot_config/sesh/scripts/executable_smart-startup.sh`
- `dot_local/bin/executable_sesh-bootstrap.sh`
- `dot_local/bin/executable_sesh-preview.sh`
- `dot_local/bin/executable_tmux-last-proc.sh`
- `dot_local/bin/executable_tmux-window-emoji.sh`
- `dot_local/bin/executable_tmux-custom-list-keys.sh`
- `dot_local/bin/executable_tmux-refresh.sh`
- `dot_local/bin/executable_claude-restart.sh`
- `Library/LaunchAgents/com.claude.code.plist.tmpl`
- `.chezmoiscripts/run_after_70-install-tmux2k-last-proc.sh.tmpl`
- `.chezmoiscripts/run_onchange_after_60-load-claude-launchagent.sh.tmpl`

**Out-of-repo handoff (NOT a tracked task, called out in Task 7):**

- The user's nvim config (lives outside this chezmoi repo) needs a lazy.nvim entry for
  `devxplay/herdr.nvim` pinned to `<SHA>`. Task 7 will document the snippet; the user adds it manually.

______________________________________________________________________

## Task 1: Pop the moshi-hook stash + commit as one declarative-install unit

**Files:**

- Modify: `.chezmoidata/system_packages_autoinstall.yaml` (already modified in stash)
- Modify: `.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl` (already in stash)
- Modify: `CLAUDE.md` (already in stash)
- Create: `.chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl` (already in stash)

**Interfaces:**

- Consumes: nothing

- Produces: `moshi-hook` is brew-installable; `Moshi :: Pairing Token` KeePassXC entry expected to exist
  at apply time; trust loop pattern available for future taps via `trusted_taps:` YAML field

- [ ] **Step 1: Pop the stash**

```bash
git stash pop stash@{0}
git status -sb
```

Expected: 3 modified + 1 untracked file restored; no merge conflicts (these were stashed pre-rebase from
current HEAD, branch hasn't moved).

- [ ] **Step 2: Render every templated file to confirm Go template syntax + KeePassXC reference parses**

```bash
chezmoi execute-template --no-tty < .chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl > /tmp/render-10.sh
chezmoi execute-template --no-tty < .chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl > /tmp/render-60.sh
head -20 /tmp/render-10.sh /tmp/render-60.sh
```

Expected: both render without errors. `/tmp/render-60.sh` contains the bash script with the keepassxc
password expansion already substituted (KeePassXC unlocked) OR a templating error if locked, in which
case unlock KeePassXC and re-render.

- [ ] **Step 3: Shellcheck the rendered output**

```bash
shellcheck /tmp/render-10.sh /tmp/render-60.sh
```

Expected: no errors. (SC1090/SC1091 globally disabled in `.shellcheckrc`.)

- [ ] **Step 4: Run the full linter**

```bash
just l
```

Expected: all 7 tools green. The lint script renders these templates internally (with `CI=1`) and
shellchecks them.

- [ ] **Step 5: Commit**

```bash
git add .chezmoidata/system_packages_autoinstall.yaml \
        .chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl \
        .chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl \
        CLAUDE.md
git commit -m "feat(moshi): declarative install + setup via brew tap and one-shot pair script

Adds the rjyo/moshi tap and moshi-hook formula to autoinstall YAML, a new
trusted_taps: field, and a pre-bundle trust loop so brew bundle doesn't choke
on the third-party tap. A run_once chezmoiscript pairs moshi-hook with the
mobile app (pulling the token from KeePassXC entry 'Moshi :: Pairing Token'),
runs moshi-hook install, and starts the brew service. Adds the script to the
interactive-apply list in CLAUDE.md because it touches KeePassXC."
```

______________________________________________________________________

## Task 2: Switch herdr from brew to direct curl installer (preview channel)

**Files:**

- Create: `.chezmoiscripts/run_onchange_before_15-install-herdr.sh.tmpl`
- Modify: `.chezmoidata/system_packages_autoinstall.yaml`: remove the `herdr` brew formula entry (it may
  or may not be present; verify)

**Interfaces:**

- Consumes: nothing

- Produces: `herdr` binary on PATH, on preview channel, sourced from direct curl install (not brew)

- [ ] **Step 1: Verify whether `herdr` is currently listed under brew formulae in YAML**

```bash
grep -n 'herdr' .chezmoidata/system_packages_autoinstall.yaml || echo "not present"
```

Expected: either a line number under `formulae:` OR "not present". If present, note the line.

- [ ] **Step 2: If present, remove the herdr line from YAML**

```bash
# Hand-edit .chezmoidata/system_packages_autoinstall.yaml to delete the 'herdr' formula entry.
# Preserve alphabetical ordering of the formulae list.
just y    # yq lint to confirm YAML still parses
```

Expected: `yq` returns no error.

- [ ] **Step 3: Create the curl-installer chezmoiscript**

Create `.chezmoiscripts/run_onchange_before_15-install-herdr.sh.tmpl` with this exact content:

```bash
{{ if eq .chezmoi.os "darwin" -}}
#!/bin/bash

set -euo pipefail

# If brew installed herdr previously (current state on this Mac as of 2026-06-18),
# uninstall it before the curl installer takes over.
if brew list herdr &>/dev/null; then
  echo "Uninstalling brew-installed herdr (preview channel requires direct install)..."
  brew uninstall herdr
fi

# Run the official direct installer (no-op-ish if already present; the installer
# is idempotent for same-version reinstalls).
if ! command -v herdr &>/dev/null; then
  echo "Installing herdr via direct curl installer..."
  curl -fsSL https://herdr.dev/install.sh | sh
fi

# Ensure preview channel is active. The CLI is authoritative; declarative
# [update] channel = "preview" in config.toml is also set (Task 4) but the
# CLI invocation here guarantees the runtime state matches.
if command -v herdr &>/dev/null; then
  current_channel=$(herdr channel show 2>/dev/null | awk '{print $NF}' || echo "unknown")
  if [[ $current_channel != "preview" ]]; then
    echo "Switching herdr to preview channel..."
    herdr channel set preview
  fi
fi
{{ end -}}
```

- [ ] **Step 4: Render + shellcheck the new script**

```bash
chezmoi execute-template --no-tty < .chezmoiscripts/run_onchange_before_15-install-herdr.sh.tmpl > /tmp/render-15.sh
shellcheck /tmp/render-15.sh
```

Expected: clean shellcheck output.

- [ ] **Step 5: Apply the script (non-interactive, does not touch KeePassXC)**

```bash
chezmoi apply --exclude=templates --force .chezmoiscripts/run_onchange_before_15-install-herdr.sh.tmpl
```

Expected: `brew uninstall herdr` runs (current state has herdr brew-installed); curl installer runs;
preview channel set. **Spike #3:** observe whether `herdr channel show` reports `preview` after this.

- [ ] **Step 6: Verify herdr binary and channel**

```bash
which herdr && herdr --version && herdr channel show
```

Expected: path is `~/.herdr/bin/herdr` (or similar non-brew path); version printed; channel reads
`preview`.

- [ ] **Step 7: Run linter**

```bash
just l
```

Expected: all green.

- [ ] **Step 8: Commit**

```bash
git add .chezmoiscripts/run_onchange_before_15-install-herdr.sh.tmpl \
        .chezmoidata/system_packages_autoinstall.yaml
git commit -m "feat(herdr): install via direct curl installer on the preview channel

The preview channel is unavailable on Homebrew installs (verified, 'herdr
channel set preview' errors with 'preview channel is only available for
direct Herdr installs'). This chezmoiscript brew-uninstalls any prior copy,
runs the official curl installer, and sets the channel. Removes herdr from
the brew autoinstall YAML."
```

______________________________________________________________________

## Task 3: Verify the existing rustup bootstrap is sufficient for herdr-navigator builds

**Files:**

- Verify: `.chezmoiscripts/run_once_before_20-install-rustup.sh.tmpl` (exists; current content installs
  rustup if missing, uses `-y` for unattended install, darwin-guarded)

**Interfaces:**

- Consumes: nothing

- Produces: `cargo` on PATH after the user opens a new shell (rustup installer modifies PATH via
  `~/.cargo/env` sourced from shell rc files)

- [ ] **Step 1: Read the existing rustup script**

```bash
cat .chezmoiscripts/run_once_before_20-install-rustup.sh.tmpl
```

Expected: 15-line script with `command -v rustup` guard + `curl https://sh.rustup.rs | sh -s -- -y`.
Confirm matches that shape.

- [ ] **Step 2: Verify cargo is on PATH (rustup may already be installed)**

```bash
command -v cargo && cargo --version
```

If cargo missing: run the rustup script manually via `chezmoi apply` of that one file, then
`source ~/.cargo/env`. If present: skip the install, but check `~/.cargo/env` is sourced from the user's
shell init (search `dot_bashrc.tmpl` for `cargo`).

```bash
grep -n cargo dot_bashrc.tmpl || echo "cargo not sourced from bashrc"
```

Expected: a grep hit OR a note that this needs fixing in a follow-up. **Decision point:** if cargo isn't
on PATH after rustup install, the herdr-navigator build in Task 7 will fail. Fix bashrc sourcing as part
of Task 7 if needed.

- [ ] **Step 3: Note Spike #6 status: Xcode CLT prerequisite**

The current rustup script does NOT verify Xcode Command Line Tools are present (they ship `clang`, which
rustc needs for linking on darwin). On the current machine they're presumably installed (otherwise
nothing would work). For a fresh-machine run, add a prerequisite check in a follow-up commit, out of
scope for this plan if CLT is already present.

```bash
xcode-select -p 2>/dev/null && echo "CLT present" || echo "CLT missing, fix before Task 7"
```

Expected: `/Library/Developer/CommandLineTools` printed.

- [ ] **Step 4: No commit if no changes made**

If the script was sufficient as-is and PATH sourcing is fine, skip to Task 4. Otherwise commit any fix:

```bash
git status -sb
# If changes: git add <files> && git commit -m "fix(bashrc): source ~/.cargo/env so cargo is on PATH"
```

______________________________________________________________________

## Task 4: Track `dot_config/herdr/config.toml` with prefix, base bindings, and crossed splits

**Files:**

- Create: `dot_config/herdr/config.toml`

**Interfaces:**

- Consumes: `herdr` binary on PATH (from Task 2)

- Produces: a parseable herdr config that subsequent tasks (5, 6, 7) extend with workspace chords,
  send-prefix workaround, and Neovim nav bindings respectively

- [ ] **Step 1: Capture the herdr default config as a baseline reference**

```bash
herdr --default-config > /tmp/herdr-default-config.toml
head -50 /tmp/herdr-default-config.toml
```

Expected: TOML output. Read sections to identify the canonical key names: `[update] channel`,
`[keybindings] prefix`, `[keybindings] split_horizontal`, etc.

- [ ] **Step 2: Create the tracked config file**

Create `dot_config/herdr/config.toml` with this exact content (adjust if Step 1 reveals different key
names, herdr's default-config is authoritative):

```toml
# Declarative channel. CLI invocation in run_onchange_before_15-install-herdr.sh.tmpl
# guarantees the runtime state, this entry documents intent.
[update]
channel = "preview"

[keybindings]
prefix = "ctrl+d"

# Workspace picker (already the herdr default, set explicitly for documentation).
goto = "prefix+g"

# Rename current tab.
rename_tab = "prefix+comma"

# Splits: deliberately crossed against herdr's defaults to preserve tmux muscle memory.
# herdr names splits by divider orientation; tmux by motion direction. Opposite words,
# same physical result. DO NOT "fix" the cross. It is the intended mapping.
#   prefix+" → top/bottom stack (tmux: split horizontally with `-`)
#   prefix+% → side-by-side    (tmux: split vertically with `|`)
split_horizontal = "prefix+\""
split_vertical = "prefix+%"

# Workspace sidebar navigation in herdr's built-in navigate mode (NOT a custom
# key table, built-in modes coexist with the no-multi-step-bindings constraint).
navigate_workspace_up = "k"
navigate_workspace_down = "j"
```

- [ ] **Step 3: Lint the TOML**

```bash
just t
```

Expected: `taplo` green.

- [ ] **Step 4: Apply just this config file (safe, no template, no KeePassXC)**

```bash
chezmoi apply --force ~/.config/herdr/config.toml
```

Expected: file written to `~/.config/herdr/config.toml`.

- [ ] **Step 5: Confirm herdr parses the config**

```bash
herdr config validate 2>&1 || herdr --check-config 2>&1 || echo "no validate subcommand, start herdr to test parse"
```

Expected: either a validation pass OR an indication the subcommand doesn't exist. If neither, launch
`herdr` briefly (`herdr &; sleep 1; herdr session list; herdr kill-server`) and confirm no parse error on
startup.

- [ ] **Step 6: Run linter**

```bash
just l
```

Expected: green.

- [ ] **Step 7: Commit**

```bash
git add dot_config/herdr/config.toml
git commit -m "feat(herdr): track ~/.config/herdr/config.toml with prefix=ctrl+d

Sets the prefix, the preview channel declaration, the rename/goto bindings,
the navigate-mode workspace nav keys, and the deliberately crossed splits
that preserve tmux muscle memory (split_horizontal=prefix+\" produces a
top/bottom stack, matching tmux's prefix+\")."
```

______________________________________________________________________

## Task 5: Add the 8 workspace quick-jump chords

**Files:**

- Modify: `dot_config/herdr/config.toml` (append `[[keys.command]]` blocks)

**Interfaces:**

- Consumes: `dot_config/herdr/config.toml` from Task 4

- Produces: 8 prefix+ctrl+\<letter> chords; consumers in later tasks rely on the workspace names
  (`homelab`, `dotfiles`, `casually-concerned`, `Ivy`, `justdavis-ansible`, `essential-feed-case-study`,
  `netpulse`, `plantpulse`) being canonical labels

- [ ] **Step 1: Append the 8 workspace chord blocks to the herdr config**

Append to `dot_config/herdr/config.toml`:

```toml
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
# Workspace quick-jump chords (prefix+ctrl+<letter>)
# All 8 live in a namespace empty in herdr defaults and structurally unlikely to
# be claimed upstream. Each runs `herdr workspace create --cwd <path> --label
# <name> --focus`, which has create-or-focus semantics: first invocation creates
# the workspace, subsequent invocations focus the existing one.
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

[[keys.command]]
key = "prefix+ctrl+h"
type = "shell"
command = "$HERDR_BIN_PATH workspace create --cwd $HOME/workspaces/Ivy/webdavis/homelab --label homelab --focus"
description = "jump to homelab workspace (auto-start)"

[[keys.command]]
key = "prefix+ctrl+."
type = "shell"
command = "$HERDR_BIN_PATH workspace create --cwd $HOME/workspaces/Ivy/webdavis/dotfiles --label dotfiles --focus"
description = "jump to dotfiles workspace (CSI-u; fallback below)"

[[keys.command]]
key = "prefix+."
type = "shell"
command = "$HERDR_BIN_PATH workspace create --cwd $HOME/workspaces/Ivy/webdavis/dotfiles --label dotfiles --focus"
description = "jump to dotfiles workspace (fallback for terminals without CSI-u)"

[[keys.command]]
key = "prefix+ctrl+c"
type = "shell"
command = "$HERDR_BIN_PATH workspace create --cwd $HOME/workspaces/Ivy/casually-concerned --label casually-concerned --focus"
description = "jump to casually-concerned workspace"

[[keys.command]]
key = "prefix+ctrl+i"
type = "shell"
command = "$HERDR_BIN_PATH workspace create --cwd $HOME/workspaces/Ivy --label Ivy --focus"
description = "jump to Ivy vault workspace (also fires on prefix+Tab via ctrl+i alias)"

[[keys.command]]
key = "prefix+ctrl+j"
type = "shell"
command = "$HERDR_BIN_PATH workspace create --cwd $HOME/workspaces/Ivy/karlmdavis/justdavis-ansible --label justdavis-ansible --focus"
description = "jump to justdavis-ansible workspace"

[[keys.command]]
key = "prefix+ctrl+e"
type = "shell"
command = "$HERDR_BIN_PATH workspace create --cwd $HOME/workspaces/Ivy/webdavis/essential-feed-case-study --label essential-feed-case-study --focus"
description = "jump to essential-feed-case-study workspace"

[[keys.command]]
key = "prefix+ctrl+n"
type = "shell"
command = "$HERDR_BIN_PATH workspace create --cwd $HOME/workspaces/Ivy/webdavis/netpulse --label netpulse --focus"
description = "jump to netpulse workspace"

[[keys.command]]
key = "prefix+ctrl+p"
type = "shell"
command = "$HERDR_BIN_PATH workspace create --cwd $HOME/workspaces/Ivy/hobbies/plantpulse --label plantpulse --focus"
description = "jump to plantpulse workspace"
```

- [ ] **Step 2: Lint the TOML**

```bash
just t
```

Expected: green.

- [ ] **Step 3: Apply + restart herdr to pick up the new bindings**

```bash
chezmoi apply --force ~/.config/herdr/config.toml
herdr server restart 2>/dev/null || (herdr kill-server; herdr &)
```

- [ ] **Step 4: Smoke-test 3 chords manually**

In a herdr pane: hit `prefix+ctrl+h` → confirm a `homelab` workspace opens at the expected path. Hit
`prefix+ctrl+i` → confirm `Ivy` opens. Hit `prefix+ctrl+.` → confirm `dotfiles` opens (**Spike #2:** if
it doesn't fire under Ghostty's CSI-u, hit `prefix+.` as fallback). Document outcome in the commit body.

- [ ] **Step 5: Run linter**

```bash
just l
```

Expected: green.

- [ ] **Step 6: Commit**

```bash
git add dot_config/herdr/config.toml
git commit -m "feat(herdr): add 8 workspace quick-jump chords in prefix+ctrl+<letter>

Maps homelab, dotfiles, casually-concerned, Ivy, justdavis-ansible,
essential-feed-case-study, netpulse, and plantpulse to their working
directories. dotfiles has a prefix+. fallback for terminals without CSI-u
keyboard-protocol support. Spike #2 outcome: [record observed behavior of
prefix+ctrl+. under Ghostty + herdr, fires reliably, or fallback used]."
```

______________________________________________________________________

## Task 6: Wire the send-prefix double-tap binding (Ctrl-d EOF preservation) [Spike #1]

**Files:**

- Modify: `dot_config/herdr/config.toml` (append one `[[keys.command]]` block)

**Interfaces:**

- Consumes: herdr config + prefix=ctrl+d from Tasks 4-5

- Produces: shell EOF (Ctrl-d) is reachable from inside herdr panes via a double-tap of Ctrl-d

- [ ] **Step 1: Append the send-prefix binding**

Append to `dot_config/herdr/config.toml`:

```toml
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
# Send-prefix workaround (Ctrl-d EOF preservation)
# Because prefix=ctrl+d, the literal Ctrl-d keystroke is consumed by prefix
# mode. Double-tap Ctrl-d: first press enters prefix mode → second fires this
# binding → herdr CLI injects a literal ctrl+d byte into the focused pane's
# PTY. Conceptually equivalent to tmux's `bind -n send-prefix`.
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

[[keys.command]]
key = "prefix+ctrl+d"
type = "shell"
command = "$HERDR_BIN_PATH pane send-keys $HERDR_ACTIVE_PANE_ID 'ctrl+d'"
description = "double-tap Ctrl-d → send literal Ctrl-d (EOF) to focused pane"
```

- [ ] **Step 2: Lint + apply**

```bash
just t
chezmoi apply --force ~/.config/herdr/config.toml
herdr server restart 2>/dev/null || (herdr kill-server; herdr &)
```

- [ ] **Step 3: Verify Spike #1: does `pane send-keys 'ctrl+d'` trigger shell EOF?**

In a herdr pane running bash with no pending input, hit `Ctrl-d Ctrl-d` (double-tap). Expected: the shell
exits (EOF). If it doesn't: try sending the raw 0x04 byte instead:

```toml
command = "$HERDR_BIN_PATH pane send-keys $HERDR_ACTIVE_PANE_ID $'\\x04'"
```

If even that fails, document the failure mode and fall back to typing `exit`, no commit blocker, but
update the spec spike notes.

- [ ] **Step 4: Run linter**

```bash
just l
```

Expected: green.

- [ ] **Step 5: Commit**

```bash
git add dot_config/herdr/config.toml
git commit -m "feat(herdr): wire send-prefix double-tap for Ctrl-d EOF preservation

prefix=ctrl+d normally swallows literal Ctrl-d. A keys.command binding on
prefix+ctrl+d shells out to 'herdr pane send-keys ctrl+d' so double-tapping
Ctrl-d injects a literal Ctrl-d byte into the focused pane (≈ tmux's
'bind -n send-prefix'). Spike #1 outcome: [record observed EOF behavior;
if the 'ctrl+d' alias does not trigger EOF, the binding instead sends the
raw 0x04 byte, note which form was used]."
```

______________________________________________________________________

## Task 7: Install herdr-navigator (cargo) + bind raw Ctrl-h/j/k/l for Neovim nav

**Files:**

- Create: `.chezmoiscripts/run_onchange_after_50-install-herdr-navigator.sh.tmpl`
- Modify: `dot_config/herdr/config.toml` (append 4 navigation `[[keys.command]]` blocks)

**Interfaces:**

- Consumes: `cargo` on PATH (Task 3); herdr binary + config from Tasks 2 and 4

- Produces: `herdr-navigator` binary at `~/.cargo/bin/herdr-navigator`; raw `Ctrl-h/j/k/l` routed through
  it (Neovim panes do their own movement, herdr panes call back to herdr to move focus)

- [ ] **Step 1: Pin the herdr.nvim commit SHA**

```bash
git ls-remote https://github.com/devxplay/herdr.nvim.git HEAD | awk '{print $1}'
```

Expected: a 40-char SHA. Note it, call it `<SHA>` in the rest of the steps. Use the same SHA for the
cargo install and for the user's lazy.nvim entry (handoff in Step 7).

- [ ] **Step 2: Create the installer chezmoiscript**

Create `.chezmoiscripts/run_onchange_after_50-install-herdr-navigator.sh.tmpl` with exact content
(substitute the real SHA into the `<SHA>` placeholder):

```bash
{{ if eq .chezmoi.os "darwin" -}}
#!/bin/bash

set -euo pipefail

# Pin to a specific upstream commit. devxplay/herdr.nvim is early-stage
# (4 commits, no releases on inspection); pinning by SHA guarantees the
# Neovim plugin and the binary stay in sync.
HERDR_NAV_SHA="<SHA>"

# Bail if cargo isn't on PATH, surface the rustup script as the prerequisite.
if ! command -v cargo &>/dev/null; then
  echo "ERROR: cargo not found on PATH. Run the rustup bootstrap first" >&2
  echo "(.chezmoiscripts/run_once_before_20-install-rustup.sh.tmpl), then" >&2
  echo "source ~/.cargo/env in a new shell." >&2
  exit 1
fi

# Idempotent: only install (or update) if the installed binary's version
# doesn't match the pinned SHA. Cargo doesn't expose the source commit
# easily, so we record it in a sidecar file.
sidecar="$HOME/.cargo/.herdr-navigator-sha"
if [[ -f $sidecar && $(cat "$sidecar") == "$HERDR_NAV_SHA" ]]; then
  echo "herdr-navigator already at pinned SHA $HERDR_NAV_SHA, skipping."
  exit 0
fi

echo "Installing herdr-navigator @ $HERDR_NAV_SHA..."
cargo install \
  --git https://github.com/devxplay/herdr.nvim.git \
  --bin herdr-navigator \
  --rev "$HERDR_NAV_SHA" \
  --locked \
  --force

echo "$HERDR_NAV_SHA" > "$sidecar"
echo "herdr-navigator install complete."
{{ end -}}
```

- [ ] **Step 3: Render + shellcheck**

```bash
chezmoi execute-template --no-tty < .chezmoiscripts/run_onchange_after_50-install-herdr-navigator.sh.tmpl > /tmp/render-50.sh
shellcheck /tmp/render-50.sh
```

Expected: clean.

- [ ] **Step 4: Apply the installer**

```bash
chezmoi apply --exclude=templates --force .chezmoiscripts/run_onchange_after_50-install-herdr-navigator.sh.tmpl
```

Expected: cargo builds + installs the binary. Watch for **Spike #6** failure modes (missing CLT, network
issues, dependency build errors). On success, verify:

```bash
which herdr-navigator && herdr-navigator --help 2>&1 | head -5
```

Expected: binary at `~/.cargo/bin/herdr-navigator`; help output prints.

- [ ] **Step 5: Append the 4 nav bindings to herdr config**

Append to `dot_config/herdr/config.toml`:

```toml
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
# Neovim ↔ herdr seamless pane navigation
# Raw Ctrl-h/j/k/l routes through herdr-navigator. The binary checks whether
# the focused pane is running Neovim (via marker files at
# ~/.cache/herdr.nvim/panes/), if so, it forwards the keystroke; if not, it
# moves focus between herdr panes.
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

[[keys.command]]
key = "ctrl+h"
type = "shell"
command = "$HOME/.cargo/bin/herdr-navigator dispatch left"
description = "seamless pane nav: left (Neovim or herdr)"

[[keys.command]]
key = "ctrl+j"
type = "shell"
command = "$HOME/.cargo/bin/herdr-navigator dispatch down"
description = "seamless pane nav: down (Neovim or herdr)"

[[keys.command]]
key = "ctrl+k"
type = "shell"
command = "$HOME/.cargo/bin/herdr-navigator dispatch up"
description = "seamless pane nav: up (Neovim or herdr)"

[[keys.command]]
key = "ctrl+l"
type = "shell"
command = "$HOME/.cargo/bin/herdr-navigator dispatch right"
description = "seamless pane nav: right (Neovim or herdr)"
```

- [ ] **Step 6: Lint + apply**

```bash
just l
chezmoi apply --force ~/.config/herdr/config.toml
herdr server restart 2>/dev/null || (herdr kill-server; herdr &)
```

- [ ] **Step 7: Out-of-repo handoff: document the lazy.nvim entry the user must add manually**

The herdr.nvim Neovim plugin lives outside this chezmoi repo. The user needs to add this entry to their
lazy.nvim plugin specs (likely under `~/.config/nvim/lua/plugins/` or equivalent):

```lua
{
  "devxplay/herdr.nvim",
  commit = "<SHA>",   -- MUST match HERDR_NAV_SHA in the chezmoi script
  config = function()
    require("herdr").setup()
  end,
}
```

Print this snippet at the end of the commit body and tell the user to apply it manually after the commit
lands. No chezmoi-tracked file change for this step.

- [ ] **Step 8: Commit**

```bash
git add .chezmoiscripts/run_onchange_after_50-install-herdr-navigator.sh.tmpl \
        dot_config/herdr/config.toml
git commit -m "feat(herdr): seamless Neovim<->herdr pane nav via herdr-navigator

Installs the devxplay/herdr.nvim Rust helper binary (pinned to <SHA>) and
binds raw Ctrl-h/j/k/l to 'herdr-navigator dispatch <dir>'. The helper checks
~/.cache/herdr.nvim/panes/ marker files to decide whether to forward the
keystroke to Neovim or move focus between herdr panes.

The matching Neovim plugin entry must be added manually to the user's nvim
config (lives outside this repo):

  { 'devxplay/herdr.nvim', commit = '<SHA>',
    config = function() require('herdr').setup() end }

Pin the same SHA in both places."
```

______________________________________________________________________

## Task 8: Vendor the herdr Agent Skill + add `update-agent-skills` justfile recipe

**Files:**

- Create: `private_dot_claude/skills/herdr/SKILL.md`
- Modify: `justfile`

**Interfaces:**

- Consumes: nothing (independent)

- Produces: agent skill at `~/.claude/skills/herdr/SKILL.md` after apply; `just update-agent-skills`
  recipe (consumed by Task 9)

- [ ] **Step 1: Fetch the upstream herdr Agent Skill**

```bash
curl -fsSL https://raw.githubusercontent.com/ogulcancelik/herdr/master/SKILL.md > /tmp/herdr-SKILL.md
head -20 /tmp/herdr-SKILL.md
```

Expected: markdown file starting with skill metadata.

- [ ] **Step 2: Place it under the chezmoi skill path**

```bash
mkdir -p private_dot_claude/skills/herdr
cp /tmp/herdr-SKILL.md private_dot_claude/skills/herdr/SKILL.md
```

- [ ] **Step 3: Add the `update-agent-skills` recipe to the justfile**

Append to `justfile`:

```makefile
# Refresh vendored agent skills from upstream sources.
# herdr Agent Skill: ogulcancelik/herdr/SKILL.md
# Moshi Skill: installed via 'npx skills add rjyo/moshi-skill', then vendored
update-agent-skills:
  curl -fsSL https://raw.githubusercontent.com/ogulcancelik/herdr/master/SKILL.md \
    > private_dot_claude/skills/herdr/SKILL.md
  @echo "Moshi Skill: run 'npx skills add rjyo/moshi-skill', then copy the"
  @echo "resulting ~/.claude/skills/<moshi-dir>/SKILL.md into"
  @echo "private_dot_claude/skills/moshi/SKILL.md and commit."
```

- [ ] **Step 4: Lint**

```bash
just m   # mdformat checks the vendored SKILL.md
just l   # full sweep including justfile-implicit syntax via Bash interpretation
```

Expected: green. If mdformat reformats the vendored skill, accept it (`mdformat` is non-destructive to
content, only formatting).

- [ ] **Step 5: Apply just the skill file**

```bash
chezmoi apply --force ~/.claude/skills/herdr/SKILL.md
ls -la ~/.claude/skills/herdr/SKILL.md
```

Expected: file copied with `private_` permissions (`-rw-------` or similar).

- [ ] **Step 6: Commit**

```bash
git add private_dot_claude/skills/herdr/SKILL.md justfile
git commit -m "feat(claude): vendor herdr Agent Skill + add update-agent-skills recipe

Pulls ogulcancelik/herdr/SKILL.md into private_dot_claude/skills/herdr/ so
AI agents in herdr panes (HERDR_ENV=1, auto-exported by herdr) get the
terminal-control instructions. The justfile recipe re-pulls from upstream
on demand."
```

______________________________________________________________________

## Task 9: Vendor the Moshi Skill

**Files:**

- Create: `private_dot_claude/skills/moshi/SKILL.md`

**Interfaces:**

- Consumes: `npx skills add rjyo/moshi-skill` installs to `~/.claude/skills/<moshi-dir>/`

- Produces: agent skill at `~/.claude/skills/moshi/SKILL.md` after apply

- [ ] **Step 1: Install the Moshi Skill via npx**

```bash
npx skills add rjyo/moshi-skill
ls ~/.claude/skills/ | grep -i moshi
```

Expected: a moshi-related directory appears under `~/.claude/skills/`. Note its name.

- [ ] **Step 2: Vendor the SKILL.md into the chezmoi tree**

```bash
mkdir -p private_dot_claude/skills/moshi
cp ~/.claude/skills/<moshi-dir>/SKILL.md private_dot_claude/skills/moshi/SKILL.md
```

(Substitute the actual directory name from Step 1.)

- [ ] **Step 3: Lint**

```bash
just m
just l
```

Expected: green.

- [ ] **Step 4: Apply just the skill file**

```bash
chezmoi apply --force ~/.claude/skills/moshi/SKILL.md
ls -la ~/.claude/skills/moshi/SKILL.md
```

Expected: file present with `private_` permissions.

- [ ] **Step 5: Commit**

```bash
git add private_dot_claude/skills/moshi/SKILL.md
git commit -m "feat(claude): vendor Moshi Skill into private_dot_claude/skills/moshi/

Captures the rjyo/moshi-skill SKILL.md (installed via 'npx skills add
rjyo/moshi-skill') so AI agents have the Moshi-side context. Refresh
via 'just update-agent-skills'."
```

______________________________________________________________________

## Task 10: Replace bashrc tmux autostart with herdr homelab landing

**Files:**

- Modify: `dot_bashrc.tmpl`

**Interfaces:**

- Consumes: herdr binary + config (Tasks 2, 4) + the `homelab` workspace chord (Task 5)

- Produces: fresh interactive bash spawns/attaches inside the `homelab` workspace; the new `h=` alias
  jumps to homelab (replaces the tmux-era `t='sesh connect uriel'`)

- [ ] **Step 1: Read the current tmux/sesh block in bashrc**

```bash
sed -n '310,349p' dot_bashrc.tmpl
sed -n '196p' dot_bashrc.tmpl   # the t= alias (to be renamed t → h)
```

Expected: matches the spec's recap: the case statement + the bootstrap branches + the
`alias t='sesh connect uriel'`.

- [ ] **Step 2: Replace the autostart block (lines 310-349)**

Replace lines 310-349 in `dot_bashrc.tmpl` with exactly this block:

```bash
# ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
# ┃    Herdr: Auto-attach Homelab Workspace ┃
# ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛

case $- in
  *i*) ;;      # interactive terminal session, continue
  *) return ;; # non-interactive terminal session, exit
esac

# Skip if herdr isn't installed or already in a pane.
command -v herdr &>/dev/null || return
[[ -n ${HERDR_ENV:-} ]] && return

# Do not interfere with non-interactive remote commands (ssh host 'cmd').
[[ -n ${SSH_ORIGINAL_COMMAND:-} ]] && return

# Skip herdr for VS Code Remote SSH connections.
[[ -n ${VSCODE_INJECTION:-} ]] && return

# Skip herdr for VS Code integrated terminals.
[[ ${TERM_PROGRAM:-} == "vscode" ]] && return

# Create-or-focus the homelab workspace on the default session.
herdr workspace create --cwd "$HOME/workspaces/Ivy/webdavis/homelab" --label homelab --focus
```

- [ ] **Step 3: Rename + retarget the alias (line 196): `t` → `h`**

Replace:

```bash
alias t='sesh connect uriel'
```

with:

```bash
alias h='herdr workspace create --cwd "$HOME/workspaces/Ivy/webdavis/homelab" --label homelab --focus'
```

Rationale: `t` was a tmux-era mnemonic; `h` matches herdr and won't collide with the justfile aliases
(`l/L/s/S/m/n/t/j/y/d/a/c/D`, `h` is free in bash).

- [ ] **Step 4: Render + shellcheck the template**

```bash
CI=1 chezmoi execute-template --no-tty < dot_bashrc.tmpl > /tmp/render-bashrc
shellcheck /tmp/render-bashrc
```

Expected: clean.

- [ ] **Step 5: Lint**

```bash
just l
```

Expected: green. (The lint script renders this template internally.)

- [ ] **Step 6: Apply just bashrc (no template, but Go template renders; safe, no KeePassXC in this
  file)**

```bash
chezmoi apply --force ~/.bashrc
```

Expected: bashrc updated.

- [ ] **Step 7: Smoke test in a fresh shell**

```bash
bash -i -c 'true' 2>&1 | head -20
```

Expected: no errors. Open an actual new terminal, confirm it lands inside the herdr homelab workspace.

- [ ] **Step 8: Commit**

```bash
git add dot_bashrc.tmpl
git commit -m "feat(bashrc): land in herdr homelab workspace on interactive shell

Replaces the tmux/sesh autostart block (lines 310-349) with a single
'herdr workspace create --focus' invocation against the homelab path.
herdr's create-or-focus semantics handle both the first-shell creation
and subsequent attach cases. Renames the t= alias (a tmux-era mnemonic)
to h= and retargets it at the same homelab workspace create-or-focus
command."
```

______________________________________________________________________

## Task 11: Remove claude-restart.sh + com.claude.code LaunchAgent + loader

**Files:**

- Delete: `dot_local/bin/executable_claude-restart.sh`
- Delete: `Library/LaunchAgents/com.claude.code.plist.tmpl`
- Delete: `.chezmoiscripts/run_onchange_after_60-load-claude-launchagent.sh.tmpl`

**Interfaces:**

- Consumes: Happy daemon + moshi-hook (Task 1) cover the mobile bridge, both already exist

- Produces: no com.claude.code LaunchAgent on disk; no `--remote-control` supervision

- [ ] **Step 1: Bootout the LaunchAgent before deleting source files**

```bash
launchctl bootout gui/$(id -u) ~/Library/LaunchAgents/com.claude.code.plist 2>/dev/null || \
  echo "agent not loaded (already booted out or never started)"
launchctl list | grep claude.code && echo "WARN: agent still listed" || echo "agent gone"
```

Expected: "agent gone".

- [ ] **Step 2: Remove the three source files from chezmoi**

```bash
git rm dot_local/bin/executable_claude-restart.sh
git rm Library/LaunchAgents/com.claude.code.plist.tmpl
git rm .chezmoiscripts/run_onchange_after_60-load-claude-launchagent.sh.tmpl
git status -sb
```

Expected: 3 deletions staged.

- [ ] **Step 3: Remove the installed copies from disk**

```bash
trash ~/.local/bin/claude-restart.sh 2>/dev/null || true
trash ~/Library/LaunchAgents/com.claude.code.plist 2>/dev/null || true
ls ~/.local/bin/claude-restart.sh ~/Library/LaunchAgents/com.claude.code.plist 2>&1 | grep -i 'no such'
```

Expected: both "No such file".

- [ ] **Step 4: Lint**

```bash
just l
```

Expected: green. (Fewer files to lint, no failure path here.)

- [ ] **Step 5: Commit**

```bash
git commit -m "chore(claude): remove claude-restart.sh + com.claude.code LaunchAgent

The always-on 'claude --remote-control' supervision is no longer needed.
Happy daemon (Library/LaunchAgents/com.webdavis.happy-daemon.plist.tmpl,
kept) and moshi-hook (Task 1) both bridge already-running agent
sessions, covering the mobile-control use case."
```

______________________________________________________________________

## Task 12: Cold-drop tmux + sesh + tmux2k status hacks + tmux config + YAML entries

**Files:**

- Delete: `dot_tmux.conf`
- Delete: `dot_config/sesh/sesh.toml`
- Delete: `dot_config/sesh/todoist-project-map.toml`
- Delete: `dot_config/sesh/scripts/executable_smart-startup.sh`
- Delete: `dot_local/bin/executable_sesh-bootstrap.sh`
- Delete: `dot_local/bin/executable_sesh-preview.sh`
- Delete: `dot_local/bin/executable_tmux-last-proc.sh`
- Delete: `dot_local/bin/executable_tmux-window-emoji.sh`
- Delete: `dot_local/bin/executable_tmux-custom-list-keys.sh`
- Delete: `dot_local/bin/executable_tmux-refresh.sh`
- Delete: `.chezmoiscripts/run_after_70-install-tmux2k-last-proc.sh.tmpl`
- Modify: `.chezmoidata/system_packages_autoinstall.yaml`: remove `sesh` (line ~111) and `tmux` (line
  ~124) from the formulae list
- Modify: `dot_bashrc.tmpl`: remove `TERM='tmux-256color'` export (~line 36), the
  `tmux-purge-resurrect-session-data` alias (~line 192), tmux-last-proc precmd (~lines 285-301), and tmux
  from the TUI skip-list (~line 271)

**Interfaces:**

- Consumes: herdr autostart in bashrc (Task 10), must be working before this commit

- Produces: zero tmux/sesh artifacts in source or in `$HOME`

- [ ] **Step 1: Delete every tmux/sesh source file in one batch**

```bash
git rm dot_tmux.conf \
       dot_config/sesh/sesh.toml \
       dot_config/sesh/todoist-project-map.toml \
       dot_config/sesh/scripts/executable_smart-startup.sh \
       dot_local/bin/executable_sesh-bootstrap.sh \
       dot_local/bin/executable_sesh-preview.sh \
       dot_local/bin/executable_tmux-last-proc.sh \
       dot_local/bin/executable_tmux-window-emoji.sh \
       dot_local/bin/executable_tmux-custom-list-keys.sh \
       dot_local/bin/executable_tmux-refresh.sh \
       .chezmoiscripts/run_after_70-install-tmux2k-last-proc.sh.tmpl
git status -sb
```

Expected: 11 deletions staged.

- [ ] **Step 2: Remove the brew entries from YAML**

Hand-edit `.chezmoidata/system_packages_autoinstall.yaml` to delete:

- the `- sesh` line under `formulae:`
- the `- tmux` line under `formulae:`

Preserve alphabetical ordering. Verify:

```bash
just y
grep -n 'tmux\|sesh' .chezmoidata/system_packages_autoinstall.yaml || echo "clean"
```

Expected: "clean".

- [ ] **Step 3: Hand-edit `dot_bashrc.tmpl` to strip the four tmux-specific snippets**

Remove these lines (line numbers approximate, search for the strings):

- `export TERM='tmux-256color'` (~line 36) → delete entire line
- `alias tmux-purge-resurrect-session-data=...` (~line 192) → delete entire line
- the `__tmux_last_proc_precmd` function block (~lines 285-301, including the `precmd_functions+=`
  registration) → delete the entire function definition + the `precmd_functions+=` line
- `tmux|` from the TUI skip-list regex (~line 271), change
  `^(vim|nvim|less|man|top|btop|ssh|tmux|claude|fzf)` to
  `^(vim|nvim|less|man|top|btop|ssh|herdr|claude|fzf)`

Verify no tmux references remain:

```bash
grep -n 'tmux\|sesh' dot_bashrc.tmpl || echo "clean"
```

Expected: "clean".

- [ ] **Step 4: Render + shellcheck**

```bash
CI=1 chezmoi execute-template --no-tty < dot_bashrc.tmpl > /tmp/render-bashrc
shellcheck /tmp/render-bashrc
```

Expected: clean.

- [ ] **Step 5: Apply to propagate deletions to $HOME**

```bash
chezmoi apply --exclude=templates --force
# This propagates deletions to $HOME: files removed from source vanish from $HOME.
ls ~/.tmux.conf ~/.config/sesh 2>&1 | grep -i 'no such' || echo "WARN: residue"
```

Expected: "No such file" / directory gone.

- [ ] **Step 6: Remove the brew-installed tmux + sesh binaries**

```bash
brew uninstall tmux sesh 2>&1 | tail -5
command -v tmux sesh 2>&1 | grep -v 'not found' && echo "WARN: binary still on PATH" || echo "clean"
```

Expected: "clean".

- [ ] **Step 7: Optional manual cleanup: TPM plugins under ~/.tmux/plugins/ (not chezmoi-tracked)**

```bash
ls ~/.tmux/plugins/ 2>/dev/null && echo "tmux plugins still on disk, trash if desired"
trash ~/.tmux 2>/dev/null || echo "~/.tmux gone"
```

- [ ] **Step 8: Run linter**

```bash
just l
```

Expected: green.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "chore: cold-drop tmux + sesh + tmux2k status hacks + tmux config

Removes dot_tmux.conf, the sesh config tree, all six tmux/sesh helper
scripts under dot_local/bin/, the tmux2k-last-proc installer chezmoiscript,
the tmux and sesh formulae from autoinstall YAML, and the four
tmux-specific snippets in dot_bashrc.tmpl (TERM export, purge-resurrect
alias, tmux-last-proc precmd, tmux entry in the TUI skip-list, replaced
with 'herdr'). Brew binaries uninstalled separately."
```

______________________________________________________________________

## Task 13: Rewrite tmux sections in CLAUDE.md (repo + global)

**Files:**

- Modify: `CLAUDE.md` (this repo's root)
- Modify: `~/.claude/CLAUDE.md` (global, outside this repo)

**Interfaces:**

- Consumes: the full herdr + moshi stack from Tasks 1-12

- Produces: docs that match the new reality; no stale tmux references

- [ ] **Step 1: Locate the three sections in `CLAUDE.md`**

```bash
grep -n '^### Tmux\|^### Bashrc Init Ordering' CLAUDE.md
```

Expected: line numbers for "Tmux Session Management", "Tmux Window/Pane Status Indicators", and "Bashrc
Init Ordering".

- [ ] **Step 2: Replace "### Tmux Session Management" with "### Herdr Workspace Management"**

Use Edit tool to replace the section's content with:

```markdown
### Herdr Workspace Management

Workspaces (project-anchored tab groups, ≈ tmux sessions) are configured at
`dot_config/herdr/config.toml`. Eight quick-jump chords in the
`prefix+ctrl+<letter>` namespace map to active project paths; see the design spec
at `docs/superpowers/specs/2026-06-18-tmux-to-herdr-migration-design.md` for the
full mapping table. `~/.bashrc` lands a fresh interactive shell inside the
`homelab` workspace on every terminal launch; the other seven workspaces are
on-demand via their jump chords.
```

- [ ] **Step 3: Replace "### Tmux Window/Pane Status Indicators" with "### Herdr Native Status"**

```markdown
### Herdr Native Status

Workspace state (per-pane agent status: blocked / working / done / idle) is
rendered natively by herdr, no third-party plugin or custom script. The sidebar
rolls each workspace up to its most-urgent agent state. Claude Code, Codex,
Cursor, OpenCode, and others are recognized out of the box.
```

- [ ] **Step 4: Rewrite the tmux parts of "### Bashrc Init Ordering"**

In that section, replace tmux references with the herdr equivalent. The autostart block now reads
"create-or-focus homelab workspace via `herdr workspace create --focus`", no `tmux ls` probe, no
`sesh-bootstrap.sh` call. The long-running command notifier and the bash-preexec-before-atuin init
ordering remain (those were not tmux-coupled).

- [ ] **Step 5: Add a new "### Moshi integration" subsection somewhere logical (after the Happy daemon
  section is natural)**

```markdown
### Moshi Integration

Moshi is the user's primary mobile agent bridge (Happy coexists as a secondary
option). The `rjyo/moshi` tap and `moshi-hook` formula are declared in
`.chezmoidata/system_packages_autoinstall.yaml` under a new `trusted_taps:`
field; a pre-bundle trust loop in
`.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl` runs
`brew trust --tap` for each trusted tap before `brew bundle` executes.

One-time setup runs from
`.chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl`: pairs moshi-hook
with the mobile app (token from KeePassXC entry **`Moshi :: Pairing Token`**),
runs `moshi-hook install` to wire agent hooks into Claude Code / Codex /
OpenCode / Gemini / Cursor / Kimi / Qwen / Grok / OMP / Pi, and starts the brew
service.

**Asymmetric herdr integration:** moshi-hook reads `HERDR_ENV`, `HERDR_SESSION`,
and `HERDR_PANE_ID` (which herdr exports natively inside its panes), so no
herdr-side configuration is needed for moshi-hook to operate.
```

- [ ] **Step 6: Update the global `~/.claude/CLAUDE.md` Toolchain line**

```bash
grep -n 'Multiplexer:' ~/.claude/CLAUDE.md
```

Find the line that says "- **Multiplexer:** tmux." and change it to:

```markdown
- **Multiplexer:** herdr.
```

- [ ] **Step 7: Lint**

```bash
just m
just l
```

Expected: green.

- [ ] **Step 8: Commit (this repo's CLAUDE.md only, global is outside the repo)**

```bash
git add CLAUDE.md
git commit -m "docs(claude): rewrite tmux sections for herdr + add Moshi integration

Replaces 'Tmux Session Management' with 'Herdr Workspace Management',
'Tmux Window/Pane Status Indicators' with 'Herdr Native Status', and
strips tmux specifics from 'Bashrc Init Ordering'. Adds a 'Moshi
Integration' section covering the declarative install, trust loop, KeePassXC
pairing, and the asymmetric integration with herdr."
```

Manually verify the global CLAUDE.md edit, it lives outside this repo, no commit here.

______________________________________________________________________

## Task 14: Final lint + spike verification roll-up

**Files:** none

**Interfaces:**

- Consumes: every preceding task

- Produces: a clean lint pass + a recorded outcome for each of the 6 spikes from the spec

- [ ] **Step 1: Full lint sweep**

```bash
just l
```

Expected: green across all 7 linters.

- [ ] **Step 2: Apply everything end-to-end on this machine**

```bash
chezmoi apply --exclude=templates --force
chezmoi diff --exclude=templates
```

Expected: no diff (clean state).

- [ ] **Step 3: Record spike outcomes**

Capture results for each spike in a short note appended to the spec under a new "Spike outcomes" section
(or in this plan's tail, whichever the executor prefers):

1. **Spike #1 (send-keys EOF):** ✅ / ❌, what worked.
1. **Spike #2 (`prefix+ctrl+.` CSI-u):** ✅ / ❌, `prefix+.` fallback used? yes/no.
1. **Spike #3 (`[update] channel = "preview"` declarative):** ✅ / ❌, CLI needed alongside config?
1. **Spike #4 (`moshi .` post-migration):** what happens: opens herdr workspace, errors, or still opens
   tmux?
1. **Spike #5 (full tmux→herdr binding collision sweep):** any per-binding decisions made? (Most tmux
   bindings died with the cold drop; collision concern is mostly hypothetical at this point.)
1. **Spike #6 (rustup bootstrap edge cases):** Xcode CLT present? cargo on PATH after install?

- [ ] **Step 4: Confirm herdr is fully functional end-to-end**

Manual checks:

- Open a new terminal → lands in `homelab` workspace.

- Hit each of the 8 workspace chords → each opens its expected workspace.

- Open Neovim in a herdr pane, split with `prefix+%`, hit `Ctrl-h/j/k/l` → seamless nav between Neovim
  splits and herdr panes.

- Double-tap Ctrl-d in a shell pane → shell exits (EOF preserved).

- Check moshi service:

  ```bash
  brew services list | grep moshi
  moshi-hook status
  ```

  Expected: service running, paired.

- [ ] **Step 5: Commit (only if spike notes were added to a tracked file)**

```bash
git status -sb
# If spike outcomes were appended to the spec or plan:
git add docs/superpowers/specs/2026-06-18-tmux-to-herdr-migration-design.md
git commit -m "docs(superpowers): record spike outcomes from herdr migration plan execution"
```

- [ ] **Step 6: Push the branch + open a PR**

```bash
git push -u origin feat/cli-agent-tracking-workflow
gh pr create --title "feat: migrate from tmux to herdr + wire moshi-hook" --body "$(cat <<'EOF'
## Summary
- Hard cutover from tmux to herdr on the preview channel (direct curl installer, not brew)
- Wires moshi-hook as the primary mobile agent bridge alongside Happy
- Removes claude-restart.sh + com.claude.code LaunchAgent
- 8 workspace quick-jump chords in prefix+ctrl+<letter>; homelab auto-attaches on shell launch
- Seamless Neovim<->herdr pane nav via devxplay/herdr.nvim (Rust helper)
- Vendors herdr Agent Skill + Moshi Skill under private_dot_claude/skills/

## Design spec
docs/superpowers/specs/2026-06-18-tmux-to-herdr-migration-design.md

## Test plan
- [x] just l passes
- [x] All 6 implementation spikes verified, see spike outcomes section
- [x] homelab autostart works on fresh terminal
- [x] All 8 workspace chords functional
- [x] Seamless Neovim<->herdr nav works
- [x] Ctrl-d EOF preserved via send-prefix double-tap
- [x] moshi-hook paired + service running
EOF
)"
```

Per memory: merge convention is `gh pr merge --merge` with subject
`Merge pull request #N from webdavis/<branch> (#N)`.

______________________________________________________________________

## Spike outcomes (filled in during Task 14)

| #   | Spike                                           | Outcome                                                                                                                                                                                                                                                              |
| --- | ----------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `pane send-keys 'ctrl+d'` triggers EOF          | Deferred: requires interactive keyboard verification. User to double-tap Ctrl-d in a herdr-managed shell pane and confirm the shell exits.                                                                                                                          |
| 2   | `prefix+ctrl+.` fires under Ghostty + CSI-u     | Deferred: requires interactive keyboard verification. Both `prefix+ctrl+.` (CSI-u path) and `prefix+.` (fallback) are wired in `dot_config/herdr/config.toml`; user to test which fires under Ghostty's CSI-u mode.                                                 |
| 3   | `[update] channel = "preview"` declarative-only | Resolved: `herdr channel show` returns bare `preview`; the `herdr channel set preview` CLI invocation in `.chezmoiscripts/run_onchange_before_15-install-herdr.sh.tmpl` is the authoritative guarantee. No declarative-only path exists in the herdr config schema. |
| 4   | `moshi .` post-migration behavior               | Deferred: depends on whether the moshi-hook brew install completed (Task 1's `run_once_after_60-moshi-hook-setup.sh.tmpl` is interactive-only). User to run `brew services list \| grep moshi` and `moshi-hook status` after applying that script interactively.    |
| 5   | Full tmux→herdr binding collision sweep         | N/A: Task 12 (tmux/sesh/tmux2k cold drop) is intentionally deferred per user direction. All original tmux and sesh files remain in the source tree. No collision analysis is meaningful until those files are removed; revisit when Task 12 is executed.            |
| 6   | Rustup bootstrap edge cases                     | Resolved: rustup install script is present in `.chezmoiscripts/run_onchange_before_15-install-herdr.sh.tmpl`; `~/.cargo/bin/cargo` is on PATH; Xcode CLT is present at `/Applications/Xcode.app/Contents/Developer`. Non-issue on this machine.                     |

### User Verification Checklist

The following checks require interactive keyboard input and cannot be verified from agent context. After
`chezmoi apply` (with KeePassXC unlocked), confirm:

1. Open a new Ghostty terminal, shell should land in the `homelab` herdr workspace automatically.
1. Hit each of the 8 `prefix+ctrl+<letter>` workspace chords, each should open its expected workspace.
1. Open Neovim in a herdr pane, split with `prefix+%`, then navigate with `Ctrl-h/j/k/l`, confirm
   seamless movement between Neovim splits and herdr panes via devxplay/herdr.nvim.
1. Double-tap Ctrl-d in a shell pane, confirm the shell exits (EOF preserved; Spike #1).
1. Test `prefix+ctrl+.`, confirm it fires under Ghostty's CSI-u mode, or fall back to `prefix+.` (Spike
   #2).
1. Run `brew services list | grep moshi` and `moshi-hook status`, confirm service is running and paired
   (Spike #4; only after `run_once_after_60-moshi-hook-setup.sh.tmpl` has run interactively).
