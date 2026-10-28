# Dotfiles Improvements Design Spec (v2)

**Date:** 2026-04-17 **Supersedes:** `docs/superpowers/specs/2026-04-14-dotfiles-improvements-design.md`
**Scope:** Core "fix & tighten" sub-project: shell, tmux, sesh, worktrunk, espanso, configs, Claude
Code, plus template/bootstrap/manifest hygiene. **Out of scope (deferred):** Neovim overhaul, macOS
defaults script, Claude Code Review GH Action, `settings.local.json` templating, theme unification,
launchctl modernization, per-profile shell consolidation.

## What changed from v1

- **Technical corrections**: Claude Code Action v1 input schema was wrong; Stop and UserPromptSubmit
  hooks don't support matchers; Notification hook does (`permission_prompt`, `idle_prompt`,
  `auth_success`, `elicitation_dialog`); hooks receive JSON on stdin, NOT env vars (no
  `CLAUDE_SESSION_ID` exists); `cleanupPeriodDays` **does** exist (v2-early-draft claimed it didn't,
  corrected); `terminal-notifier` abandoned since 2017 → `alerter` via `vjeantet/tap`;
  `b0o/tmux-autoreload` archived Oct 2025; sesh `separator_aware` key not documented; atuin search keys
  are top-level (not nested under `[search]`); macOS `date -Is` isn't supported (use `gdate` or portable
  format).
- **Architectural simplifications**: Espanso 7 files → 5; smart-startup dashboard simplified and made
  opt-in; Hue pulse skips scene save/restore; `settings.local.json.tmpl` cut; Claude Code Review GH
  Action cut; SSH commit signing dropped (GPG+KeePassXC already solves it); KeePassXC preflight script
  cut (the existing `CLAUDE.md` guidance is sufficient); MCP config left unmanaged; **rbenv removed
  entirely** (user doesn't use Ruby, v1 and v2-early lazy-loaded it; v2-final cuts it). Starship's
  `cmd_duration` module added for in-prompt duration display, complementing §7.1's notification triggers.
- **Hardened**: AI commit hook gets 5KB truncation, 4-second timeout, merge/rebase/cherry-pick guards,
  `SKIP_AI_COMMIT=1` escape hatch; Claude Stop hook gated on 5-minute session duration (computed from
  stdin-parsed `session_id`).
- **New scope**: Sections §13 to §21 added: user-customization migration, template hygiene pass, bootstrap
  hardening, shell productivity additions, package manifest cleanup, user-bin script fixes, lint/CI
  expansion, `dot_claude/` surface expansion, passive tmux window/pane status indicators (emoji in window
  list + `last-proc` tmux2k plugin showing the previously-active session's window state).
- **Tart reframed**: Kept, but positioned as general-purpose macOS sandbox (not just `act` Layer 2).
- **Already-wired items flagged**: `direnv` hook, `gh completion`, just/openclaw/git completions are
  already wired in the current `dot_bashrc.tmpl`; v2 notes these as "no action needed" rather than
  treating them as adds.

______________________________________________________________________

## 1. Atuin

### 1.1 Enable daemon mode and sync v2 records

Set in `dot_config/atuin/config.toml.tmpl`:

```toml
[sync]
auto_sync = false
records = true

[daemon]
enabled = true
autostart = true
```

**Daemon:** decouples command recording from `PROMPT_COMMAND` ordering, using a background process with a
hot in-memory search index (nucleo algorithm). `autostart = true` starts the daemon on first Atuin
invocation so new machines don't need a manual launch.

**Records (sync v2):** opt in to the records-based storage protocol now. This will become the default in
a future atuin release; setting it explicitly means the local DB migrates to the v2 format on next run
(non-destructive, atuin handles the conversion). No immediate behavior change while `auto_sync = false`,
but future-proofs the config and positions you to turn sync on later without a schema migration.

Moves the existing top-level `auto_sync = false` under `[sync]` for tidiness. Remove the old top-level
line when adding the table.

### 1.2 Filter and search modes

Atuin search-related keys are **top-level** (not nested under a `[search]` section). Update the existing
top-level settings in `dot_config/atuin/config.toml.tmpl`:

```toml
filter_mode = "host"
filter_mode_shell_up_key_binding = "session"
search_mode = "prefix"
style = "compact"
```

Keep the existing top-level `enter_accept`, `secrets_filter`, and `[ai] enabled = true` entries as-is.
(`auto_sync` moves under `[sync]` per §1.1.)

**Note on `filter_mode = "host"`:** This restricts `Ctrl-R` to the current machine's history even when
sync is enabled. That's the desired behavior for single-machine workflows but silently hides commands
from other synced machines. If cross-machine recall becomes important, switch to `global`.

### 1.3 Migrate to Homebrew

- Add `atuin` to `system_packages_autoinstall.yaml` formulae list.
- After brew install, remove `~/.atuin/bin/` (old curl-installed binary; history DB at
  `~/.local/share/atuin/` is untouched).
- Delete `.chezmoiscripts/run_once_before_30-install-atuin.sh.tmpl`.

### 1.4 Guard unconditional env source

`dot_bashrc.tmpl` currently has `. "$HOME/.atuin/bin/env"` unguarded. On fresh machines (or after §1.3
removes the old directory) this errors. Change to:

```bash
[[ -f "$HOME/.atuin/bin/env" ]] && . "$HOME/.atuin/bin/env"
```

### 1.5 Diagnosis during implementation

Run `atuin history list --cmd-only | head -20` to verify whether the recording gap was a filter issue vs.
a hook issue. The daemon should resolve hook-ordering problems regardless.

______________________________________________________________________

## 2. Bashrc cleanup and reliability

### 2.1 SSH detection

Add `SSH_CONNECTION` detection alongside `MOSH_KEY`:

```bash
if [[ -n "$MOSH_KEY" || -n "$SSH_CONNECTION" ]]; then
  export STARSHIP_CONFIG="$HOME/.config/starship-mosh.toml"
fi
```

### 2.2 Remove SDKMan

- Remove the sdkman init block from `dot_bashrc.tmpl` (including the hidden `curl | bash` auto-installer,
  moving it to a chezmoi script is explicitly rejected; SDKMan is not wanted).
- Delete `~/.sdkman/` directory from disk.

### 2.3 Remove skhd

- Remove `skhd --restart-service` reference from the system packages run script.
- skhd has been replaced by AeroSpace.

### 2.4 Remove bash history

Strip the following from `dot_bashrc.tmpl` (verified lines 41-55):

- `shopt -s histappend`
- `history_file_size=5000000`
- `export HISTSIZE=...`
- `export HISTFILESIZE=...`
- `export HISTFILE="$HOME/.bash_history"`
- `export HISTCONTROL=...`
- The entire `HISTIGNORE` template block (lines 51-55) including the `{{- if (env "CI") }}` /
  `{{- else }}` KeePassXC template. Atuin's `secrets_filter` handles the same job.

Atuin daemon handles all recording; bash's built-in history becomes dead weight.

### 2.5 Init ordering comment

Add a comment noting that `atuin init bash` must come after `zoxide init bash` (both modify shell
bindings). This is for the keybinding, not recording (daemon handles recording).

### 2.6 Brew autoupdate idempotency

In `.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl`, check if autoupdate is already
running before restarting.

### 2.7 Bash completions

**Already wired** (verified in current `dot_bashrc.tmpl`): `gh` (line 93), `just` (via
`$HOME/.bash_just_completions`, line 121), `openclaw` (line 127), SSH hosts (via
`$HOME/.bash_completions`, line 137), git (via `$HOME/.git-completion.bash`, line 91), fzf (line 140). No
action needed for these.

**To add:**

- `docker completion bash`
- `kubectl completion bash`
- `chezmoi completion bash`
- `uv generate-shell-completion bash`

All wrapped in `command -v TOOL &>/dev/null && eval "..."` guards so absent tools don't error.

**Deferred to carapace (§16.3)** for tools with interactive or awkward bash-completion output: rbenv,
deno, bun. Carapace handles these via its universal spec layer, simpler than maintaining per-tool `eval`
glue.

### 2.8 Bash bindings fix

Fix em-dash encoding bug in `dot_bash_bindings` (~line 96). Replace em-dash with standard double-dash
(`--`) in the eza commands.

### 2.9 Shell quality-of-life additions

**Navigation aliases:**

- `..="cd .."`, `...="cd ../.."`, `....="cd ../../.."`

**Utility functions:**

- `mkd()`: `mkdir -p "$@" && cd "$_"`
- `tmpd()`: `cd "$(mktemp -d)"`
- `calc()`: `bc -l <<< "$*"`
- `timer='echo "Timer started. Stop with Ctrl-D." && time cat'`
- `getcertnames()`: SSL certificate CN and SANs for a domain

**Network aliases:**

- `pubip='dig +short myip.opendns.com @resolver1.opendns.com'`

**Git safety function:**

- `gitsetoriginnopush()`: sets push URL to `no_push` on forks to prevent accidental upstream pushes

**Modern tool aliases** (new in v2, see §16 for full list):

- `alias ls='eza --icons'`
- `alias ll='eza -la --git --icons'`
- `alias lt='eza --tree --level=2 --icons --git-ignore'`
- `alias cat='bat -p'`
- `alias du='dust'`
- `alias ps='procs'`
- `alias grep='grep --color=auto'` (fix the existing `--color=always` which breaks pipes)

______________________________________________________________________

## 3. Tmux changes

### 3.1 Tmux2k right plugins

Change `@tmux2k-right-plugins` from `"network battery cpu ram"` to `"last-proc network ram"`.

`last-proc` is a new custom tmux2k plugin defined in §21 that shows the current active window + its
process state from the **previously-active** session (so you can see "uriel:build 🔨" while working in a
different session). cpu-temp and cpu are dropped to make room and because tmux2k's cpu segment is rarely
load-bearing day-to-day; add them back by extending the list if you miss them.

### 3.2 Migrate tms to sesh

Replace the entire tms section in `dot_tmux.conf` with sesh integration:

- **Fuzzy session picker:** `prefix + o` opens fzf-tmux popup with source cycling (Ctrl+A all, Ctrl+T
  tmux, Ctrl+G configs, Ctrl+X zoxide, Ctrl+F find, Ctrl+D kill), previews via `sesh preview`.
- **Window picker:** `prefix + C-w` opens fzf window picker via `sesh window`.
- **Last session toggle:** `prefix + \` runs `sesh last` (replaces unused synchronize-panes binding;
  survives session close/detach).
- **Quick-access key table (SESH):** `prefix + C-o` enters SESH mode, then single letter jumps:

| Key   | Session            |
| ----- | ------------------ |
| u     | uriel              |
| o     | openclaw           |
| h     | homelab            |
| i     | ivy                |
| c     | casually-concerned |
| d     | dotfiles           |
| n     | nvim-config        |
| e     | essential-feed     |
| g     | webdavis-profile   |
| j     | job-hunting        |
| k     | justdavis-ansible  |
| m     | maeve              |
| Space | dresden (claude)   |

- **`detach-on-destroy off`** so closing a session switches to the next one instead of exiting tmux.

### 3.3 Remove tms references

Across the tree:

- **`dot_tmux.conf`:** remove tms keybindings (lines 80-105), `TMUX_SESSIONIZER` key table, the
  `if-shell` curl-installer block (lines 77-78), and the tms-section header/comment.
- **`dot_bashrc.tmpl`:**
  - Remove `export TMS_CONFIG_FILE="$HOME/.config/tms/config.toml"` (line 35).
  - Remove or replace `alias t='tms marks open 0'` (line 160). v2 replacement:
    `alias t='sesh connect uriel'` (preserves the muscle-memory `t` shortcut).
  - Rewrite the tmux auto-startup block (lines 218-233). Current logic uses `tms start` and
    `tms marks open 0`; replace with a call to `~/.local/bin/sesh-bootstrap.sh` (see §4.5) followed by
    `tmux attach || tmux new -s uriel`.
- **`dot_local/bin/executable_tmux-refresh.sh`:** update `verify_required_tools` to check `sesh` instead
  of `tms`; replace the `tms start` call with a loop
  `sesh connect uriel; sesh connect openclaw; sesh connect homelab`.
- **Filesystem:** delete `~/.local/bin/tms` binary and `~/.config/tms/` directory during migration.

### 3.4 Terminal overrides modernization

- `default-terminal "tmux-256color"` (replace `screen-256color`)
- `terminal-overrides ",xterm-ghostty:RGB"` (replace kitty override + `Tc`)
- Drop the kitty `Ss`/`Se` override (no longer using Kitty).
- Also update `TERM=screen-256color` export in bashrc to `TERM=tmux-256color` (or remove, tmux sets it
  automatically inside tmux).

### 3.5 Extended keys

Enable `extended-keys on` and `extended-keys-format csi-u` so neovim/fzf receive modifier key combos
correctly (tmux 3.5+ feature).

### 3.6 Bug fix

Fix `aggressive-resize` (missing `on` value).

### 3.7 History limit

Raise `history-limit` from 10000 to 50000.

### 3.8 Plugins

- Drop `tmux-plugins/tmux-copycat` (unmaintained; replaced by tmux 3.5+ built-in search and
  tmux-fuzzback).

- `b0o/tmux-autoreload` is **archived Oct 2025**. Do NOT add. Instead, bind `prefix + r` to reload:

  ```tmux
  bind-key r source-file ~/.tmux.conf \; display-message "Reloaded ~/.tmux.conf"
  ```

- Resolve `tmux-fingers` duplicate install: currently installed as BOTH a tmux plugin AND a Homebrew
  formula (`morantron/tmux-fingers/tmux-fingers`). Pick the brew formula (prebuilt binary); drop the
  plugin from `dot_tmux.conf`.

______________________________________________________________________

## 4. Sesh configuration

### 4.1 New file: `dot_config/sesh/sesh.toml`

Chezmoi-managed.

**Global settings:**

```toml
cache = true
sort_order = ["tmux", "config", "zoxide"]
dir_length = 1
blacklist = ["popup", "scratch"]
```

**Note:** `separator_aware` is NOT in current sesh docs (v2 correction). Dropped from the config.

### 4.2 Default session

**Simplified** from v1's 30-line script to ~12 lines focused on the two information sources that actually
matter here: git status and Todoist tasks. Additionally, the dashboard is now **opt-in**, not tied to
sesh's `startup_command`. Auto-dashboards in every new session became noisy.

The script at `dot_config/sesh/scripts/executable_smart-startup.sh`:

- Git: `git status -sb` under a styled header if `$dir` is a git repo.
- Tasks: Todoist tasks for current session under "Tasks" header. Uses section-based query:
  `td task list -f "/<session-name>" --limit 5`. Special case for `casually-concerned` →
  `td task list --project "cc" --limit 5`. Skips silently if no matching section.
- **Dropped from v1 scope:** Make/Cargo/pyproject detection. Justfile-only project info preserved.
- Fallback: `eza --tree --level=1 --icons`.
- ANSI-colored headers; output stays under ~15 lines.

**Invocation:** The script is invoked explicitly. Wire it up as a short bash function. Pick a name that
doesn't collide with existing binaries. `sd` is taken by the sed-replacement (§11.1); safe names include
`ss`, `info`, `proj`, or `dash`. Not auto-run on session open.

**Preview command** (still runs on session hover, lightweight):

```toml
[default_session]
preview_command = "td task list -f \"/$(basename {})\" --limit 3 2>/dev/null; echo; eza --tree --level=2 --icons --git-ignore {}"
```

### 4.3 Configured sessions (13)

All 12 existing tms marks plus dresden:

| Name               | Path                                                             |
| ------------------ | ---------------------------------------------------------------- |
| uriel              | ~/workspaces/webdavis/uriel                                      |
| openclaw           | ~/.openclaw                                                      |
| homelab            | ~/workspaces/webdavis/homelab                                    |
| ivy                | ~/workspaces/Ivy                                                 |
| casually-concerned | ~/workspaces/Ivy/Projects/Casually Concerned                     |
| dotfiles           | ~/.local/share/chezmoi                                           |
| nvim-config        | ~/.config/nvim                                                   |
| essential-feed     | ~/workspaces/webdavis/essential-feed-case-study                  |
| webdavis-profile   | ~/workspaces/webdavis/webdavis                                   |
| job-hunting        | ~/workspaces/webdavis/job-hunting                                |
| justdavis-ansible  | ~/workspaces/webdavis/justdavis-ansible                          |
| maeve              | ~/workspaces/webdavis/Maeve                                      |
| dresden            | ~ (disable_startup_command = true, preview = td today --limit 5) |

### 4.4 Wildcard configs

```toml
[[wildcard]]
pattern = "~/workspaces/**"
preview_command = "eza --tree --level=2 --icons --git-ignore {}"

[[wildcard]]
pattern = "~/.config/*"
disable_startup_command = true
preview_command = "eza --tree --level=1 --icons {}"
```

### 4.5 Bootstrap script

`dot_local/bin/executable_sesh-bootstrap.sh` creates only uriel, openclaw, and homelab at startup. Other
sessions created on-demand when keybindings are pressed (`sesh connect` creates if not exists).

The bootstrap is called from bashrc's tmux auto-startup block (replacing the current tms logic), from
`tmux-refresh.sh`, and from the Claude Code LaunchAgent.

### 4.6 Zoxide seeding

During migration, seed zoxide with current tms search paths:

```bash
find ~/.config -maxdepth 2 -type d | xargs -I {} zoxide add {}
find ~/workspaces -maxdepth 2 -type d | xargs -I {} zoxide add {}
find ~/workspaces/webdavis -maxdepth 2 -type d | xargs -I {} zoxide add {}
```

______________________________________________________________________

## 5. Worktrunk configuration

### 5.1 New file: `dot_config/worktrunk/config.toml`

Chezmoi-managed. Coordination: worktrunk owns git worktree lifecycle, sesh owns tmux session lifecycle.
Worktrunk hooks **do not** manipulate tmux, v1's `post-switch`/`pre-remove` tmux-window renames are
dropped (low-value UI and they undermined the "worktrunk doesn't touch tmux" claim).

```toml
worktree-path = "{{ repo_path }}/../{{ repo }}.{{ branch | sanitize }}"

[commit.generation]
command = "CLAUDECODE= MAX_THINKING_TOKENS=0 claude -p --no-session-persistence --model=haiku --tools='' --disable-slash-commands --setting-sources='' --system-prompt=''"

[commit]
stage = "all"

[merge]
squash = true
rebase = true
remove = true
verify = true

[list]
summary = false

[switch.picker]
pager = "delta --paging=never"
```

### 5.2 Hooks

```toml
# Copy whitelisted ignored files to new worktrees.
[post-start]
copy = "wt step copy-ignored"

# Pre-merge validation gate (sequential, fast checks first).
# Array-of-tables form per worktrunk docs, [pre-merge] table form is deprecated.
[[pre-merge]]
lint = "just l 2>/dev/null || true"

[[pre-merge]]
test = "just test 2>/dev/null || true"
```

### 5.3 Aliases

```toml
[aliases]
up = "git fetch --all --prune && wt step for-each -- 'git rev-parse --verify -q @{u} >/dev/null || exit 0; g=$(git rev-parse --git-dir); test -d \"$g/rebase-merge\" -o -d \"$g/rebase-apply\" && exit 0; git rebase @{u} --no-autostash || git rebase --abort'"
```

### 5.4 Claude Code plugin

**Verify before implementing:** The v1 spec referenced `wt config plugins claude install` but this
subcommand is not in the published worktrunk docs. During implementation, run `wt config plugins --help`
first. If the subcommand exists, install it; if not, skip this item and note in the plan.

### 5.5 Shell integration

Run `wt config shell install` during implementation.

______________________________________________________________________

## 6. Espanso migration and reorganization

### 6.1 Migrate to chezmoi

Move `~/Library/Application Support/espanso/` into chezmoi source state at
`Library/Application Support/espanso/`.

### 6.2 File reorganization (5 files)

Simplified from v1's 7 files. Goal: make editable files (`snippets.yml`, `prompts.yml`) small enough for
fast Neovim editing; isolate the huge autocorrect list into two pattern-class files.

| File                           | Purpose                                                                                             |
| ------------------------------ | --------------------------------------------------------------------------------------------------- |
| `autocorrect-contractions.yml` | Missing-apostrophe fixes (dont→don't, wasnt→wasn't, etc.) ~40-60 entries                            |
| `autocorrect-spelling.yml`     | Transpositions, doubled/missing letters, merged-word fixes (teh→the, alot→a lot, etc.) ~150 entries |
| `snippets.yml`                 | All `;;`/`,,` short triggers: abbreviations + urls + formatting + titles (~80-100 entries)          |
| `identity.yml.tmpl`            | KeePassXC-templated sensitive data (address, phone, signing email)                                  |
| `prompts.yml`                  | Long-form AI prompt expansions (different editing cadence)                                          |

**Why this split:** autocorrect-\*.yml files rarely need editing day-to-day (grow one line at a time), so
their size doesn't matter for Neovim perf. `snippets.yml` is the file you edit most often; keeping it
under ~200 lines avoids the current `base.yml` perf issue. `prompts.yml` is isolated because its edits
are of a different nature (long multi-line blocks).

### 6.3 Files removed

- `_pqi.yml` (old job, healthcare/survey).
- `excel.yml` (only existed to load `_pqi` in Excel).

### 6.4 Collision fixes

| Trigger                  | Fix                        |
| ------------------------ | -------------------------- |
| `;;ao` base vs browser   | browser → `,,azo`          |
| `;;gh` titles vs browser | browser → `,,ghu`          |
| `;;ed` base vs browser   | browser → `,,efc`          |
| `;;con` ×2 in base       | "conscientious" → `;;cons` |
| `thye` ×2 in base        | Remove duplicate           |

### 6.5 Redundancy cleanup

| Issue                                   | Fix              |
| --------------------------------------- | ---------------- |
| `rn` (bare) + `;;rn` both → "right now" | Remove bare `rn` |
| `;;et` + `;;evt` both → "everything"    | Remove `;;evt`   |

### 6.6 Convention

Two prefixes only: `;;` and `,,`.

- `;;` + 2-3 letters: word abbreviations
- `;;` + full word: chatbot prompts (no collision with short triggers)
- `,,` + 2-3 letters: dates, symbols, formatting
- `,,` + 3 letters: URLs
- `,,` + descriptive: email/identity

### 6.7 Trigger migration

**URLs (browser.yml → `snippets.yml`):** all `;;` triggers become `,,` + 3 letters.

**Email/identity:** `;;em` → `,,eml`, `;;ep` → `,,epl`, `,,em` → `,,gml`, `,,ep` → `,,gpl`, `;;wa` →
`,,wa`, `;;br` → `,,br`, `;;sd` → `,,sda`, `,,sd` → `,,sdn`.

Remove old-job email templates: `;;ff`, `;;0s`, `,,0s`.

**Chatbot (chatbot.yml → `prompts.yml`):** triggers stay as `;;` + word.

### 6.8 Sensitive data templating

`identity.yml.tmpl` uses KeePassXC for address, phone number. The credit card trigger (`02667`) is
removed entirely.

### 6.9 New triggers

Same set as v1 §6.9, placed in `snippets.yml` or `prompts.yml` per type.

______________________________________________________________________

## 7. Notifications

### 7.1 Long-running command notification

Add to `dot_bashrc.tmpl`. **Important:** atuin already installs a `DEBUG` trap via its bash integration
(bash-preexec). A naked `trap ... DEBUG` will clobber atuin's recording. Instead, register hooks via
`bash-preexec`'s `preexec_functions` and `precmd_functions` arrays (atuin sources bash-preexec
automatically during `atuin init bash`):

```bash
__cmd_notify_preexec() { __cmd_notify_start=$SECONDS; __cmd_notify_name="$1"; }
__cmd_notify_precmd() {
  local exit_code=$?
  [[ -z $__cmd_notify_start ]] && return
  local elapsed=$(( SECONDS - __cmd_notify_start ))
  __cmd_notify_start=""
  # Skip interactive TUIs.
  [[ $__cmd_notify_name =~ ^(vim|nvim|less|man|top|btop|ssh|tmux|claude) ]] && return
  if (( elapsed >= 300 )); then
    alerter --title "Command finished" --message "${__cmd_notify_name%% *} (${elapsed}s)" --sound default 2>/dev/null &
    ~/.local/bin/hue-pulse.sh "$exit_code" 2>/dev/null &
  elif (( elapsed >= 30 )); then
    alerter --title "Command finished" --message "${__cmd_notify_name%% *} (${elapsed}s)" --sound default 2>/dev/null &
  fi
}
preexec_functions+=(__cmd_notify_preexec)
precmd_functions+=(__cmd_notify_precmd)
```

Thresholds:

- **30s:** fire `alerter` (double-dash flags per v26+ Swift rewrite).
- **5 min:** additionally pulse Hue lights (lowered from v1's 10 min, user preference).
- Skip if the command was a known interactive TUI (vim/less/top/ssh/tmux/claude).

**Ordering:** must be registered *after* `atuin init bash` runs (so `preexec_functions` exists). Place
this block near the end of `dot_bashrc.tmpl`, after line 186.

**Terminal-focus check:** v1 mentioned "skip if terminal pane is focused." tmux doesn't expose a reliable
focused-pane env var to bash, so this is dropped in v2; the TUI-command skip list covers the common
cases.

### 7.2 Hue light pulse

If elapsed > 5 minutes, additionally fire a Hue light pulse:

- Success (exit 0): green pulse at 50% brightness, 2 seconds.
- Failure (non-zero): red pulse at 50% brightness, 2 seconds.
- **Simple implementation** (v2): pulse → return to a named scene. No save/restore of current scene. More
  robust than v1's state-capture approach.

### 7.3 Hue pulse helper script

New file: `dot_local/bin/executable_hue-pulse.sh`

- Single positional argument: exit code (0 = green, non-zero = red).
- No threshold logic inside, callers decide when to invoke. The Claude Stop hook (§12.3) wraps this
  script with its own elapsed-time gate; the bashrc command-timer (§7.2) gates by `$SECONDS`.
- Uses `openhue` CLI (same API as existing `smart-lights`).
- Pulse logic:
  1. Set room to green (`#00c96d`) or red (`#ff657a`) at 50% brightness.
  1. `sleep 2`.
  1. Return to named scene (e.g., `"Default"`).
- Also add a `--pulse <color>` flag to `smart-lights` for reuse.

### 7.4 GH workflow monitoring

Use `gh alias` (no custom script maintenance):

```bash
gh alias set --shell pushwatch '
  git push "$@"
  sleep 3
  run_id=$(gh run list -L 1 --json databaseId --jq ".[].databaseId")
  [ -n "$run_id" ] && gh run watch "$run_id" --exit-status >/dev/null 2>&1
  ~/.local/bin/hue-pulse.sh $?
'
```

`gh pushwatch` pushes, watches the workflow, and pulses lights on completion.

______________________________________________________________________

## 8. Shell performance and cleanup

### 8.1 Remove rbenv

User doesn't use Ruby day-to-day; any future Ruby work will go through a Nix flake (pinned per project).
rbenv is dead weight.

**Changes:**

- Delete `eval "$(rbenv init - --no-rehash bash)"` from `dot_bashrc.tmpl` (line 109). Removing this also
  drops `~/.rbenv/shims` from PATH automatically, since it was that eval adding it.
- Remove `rbenv` from `.chezmoidata/system_packages_autoinstall.yaml` formulae list.
- Delete `~/.rbenv/` directory during migration.
- Saves ~50 to 100ms of shell startup time.

(v1 and early-v2 both proposed lazy-loading rbenv across a broader trigger set. Cutting it entirely is
simpler.)

### 8.2 Cargo PATH

Replace `source "$HOME/.cargo/env"` with direct PATH addition:

```bash
[[ -d "$HOME/.cargo/bin" ]] && export PATH="$HOME/.cargo/bin:$PATH"
```

### 8.3 Karabiner cleanup

Remove commented-out/disabled rules from `dot_config/private_karabiner/private_karabiner.json`.

### 8.4 Delete empty install script

Delete `.chezmoiscripts/run_once_before_30-install-atuin.sh.tmpl`.

### 8.5 path_prepend stderr fix

In `dot_bashrc.tmpl`, the existing `path_prepend` function echoes missing-directory warnings to stdout,
which corrupts output for non-interactive bashrc sourcing. Redirect to stderr:

```bash
path_prepend() {
  [[ -d "$1" ]] || { echo "path_prepend: $1 not a directory" >&2; return 1; }
  ...
}
```

______________________________________________________________________

## 9. Config improvements

### 9.1 Git config modernization

Add to `dot_gitconfig.tmpl`:

- `fetch.prune = true`
- `fetch.pruneTags = true`
- `fetch.writeCommitGraph = true`
- `diff.algorithm = histogram`
- `merge.conflictstyle = zdiff3` (upgrade from diff3)
- `rebase.updateRefs = true`
- `rebase.autoStash = true`
- `commit.verbose = true`
- `branch.sort = -committerdate`
- `tag.sort = version:refname`
- `column.ui = auto`
- `transfer.fsckObjects = true`
- `pull.rebase = true`
- `help.autocorrect = prompt` (prompts before executing, safer than v1's `= 1` which auto-runs)
- `core.fsmonitor = true` + `core.untrackedCache = true` (faster `git status` on large repos; Apple
  Silicon-friendly)

### 9.2 Consolidate on delta

Replace `diff-so-fancy` with `delta` in `core.pager`. Currently contradictory: `core.pager` uses
diff-so-fancy but `interactive.diffFilter` uses delta. Delta is superior.

Keep `difftastic` as an on-demand tool (`GIT_EXTERNAL_DIFF=difft git diff`), they have different
strengths.

### 9.3 Fix acp alias

Change `--force` to `--force-with-lease` in the `acp` git alias.

### 9.4 New git aliases

- `undo = reset --soft HEAD~1`
- `unstage = restore --staged`
- `recent = for-each-ref --sort=-committerdate --count=10 --format='%(refname:short)' refs/heads`
- `whoami = !git config --get user.name`
- `find-merge = "!sh -c 'commit=$0 && branch=${1:-HEAD} && (git rev-list $commit..$branch --ancestry-path | cat -n; git rev-list $commit..$branch --first-parent | cat -n) | sort -k2 -s | uniq -f1 -d | sort -n | tail -1 | cut -f2'"`
- `show-merge = "!sh -c 'merge=$(git find-merge $0 $1) && [ -n \"$merge\" ] && git show $merge'"`
- `pr = "!f() { git fetch -fu ${2:-origin} refs/pull/$1/head:pr/$1 && git checkout pr/$1; }; f"`
- `go = "!f() { git checkout -b \"$1\" 2>/dev/null || git checkout \"$1\"; }; f"`
- `dm = "!git branch --merged | grep -v '\\*' | xargs -n 1 git branch -d"`
- `fb = "!f() { git branch -a --contains $1; }; f"`
- `fc = "!f() { git log --pretty=format:'%C(yellow)%h %Cblue%ad %Creset%s%Cgreen [%cn] %Cred%d' --decorate --date=short -S\"$1\"; }; f"`
- `fm = "!f() { git log --pretty=format:'%C(yellow)%h %Cblue%ad %Creset%s%Cgreen [%cn] %Cred%d' --decorate --date=short --grep=\"$1\"; }; f"`

### 9.5 Remove footguns from gitconfig

- Delete the `[filesystem "Oracle Corporation|11.0.5|/dev/mapper/volgroup-home"]` block (lines 60-62).
  This is a JGit-only cache (used by Eclipse/EGit/IntelliJ-bundled git, not the vanilla `git` CLI) from
  an old Linux host. Dead on macOS; vanilla `git` ignores the section entirely.
- Delete the `git u` alias, it force-pushes "Quick save" to the current branch with no confirmation.
  Dangerous. Users who want quick saves can use `git stash` or `git commit --amend`.
- Optionally template the `[difftool "nvimdiff"]` path by OS.

### 9.6 Inputrc fixes

- Enable `enable-bracketed-paste on` (security).
- Reduce `keyseq-timeout` from 1000 to 200ms (faster vi mode).
- Remove duplicate `show-mode-in-prompt` setting (line 81).

### 9.7 Starship additions

- Add `nix_shell` module (visual feedback in Nix dev shells).
- Add `direnv` module (shows .envrc loaded/allowed state).
- Tune `cmd_duration` module: `min_time = 2000` (ms) for in-prompt elapsed display on any command longer
  than 2 seconds. Complements §7.1's notification triggers, starship renders duration in the prompt,
  bash-preexec fires alerter + Hue.
- Add `scan_timeout = 30` and `command_timeout = 500`.
- Keep all language modules (needed for reading other repos).
- Remove the `ruby` module (rbenv cleanup in §8.1 makes Ruby detection pointless).

### 9.8 Ghostty improvements

- `clipboard-read = ask` (security: prevents clipboard-sniffing).
- `clipboard-paste-protection = true`.
- `shell-integration-features = cursor,sudo,title`.
- `window-padding-x = 4`, `window-padding-y = 2`.

### 9.9 Bat config

- `--style=numbers,changes,header,grid`
- `--map-syntax "*.tmpl:Bash"`
- `--map-syntax ".envrc:Bash"`
- `--map-syntax "justfile:Makefile"`
- `--pager="less -RFX --mouse"`

______________________________________________________________________

## 10. AI commit messages and local CI

### 10.1 Hardened prepare-commit-msg hook

Chezmoi-managed at `dot_config/git/hooks/executable_prepare-commit-msg`. Uses Claude Code CLI in pipe
mode (`claude -p`), no API keys, no third-party tools, uses existing Claude Code subscription auth.

**Hardening vs v1:**

1. **Truncate diff to ~5KB** before sending to haiku (huge diffs slow haiku and hurt quality).
1. **4-second timeout** on the `claude -p` call; fall back to empty message on timeout/non-zero exit.
1. **Skip merge/rebase/cherry-pick/amend**, not just `-m`.
1. `SKIP_AI_COMMIT=1` **env escape hatch** for one-off quick commits.
1. **Never blocks the commit**, worst case is an empty prepopulated message.

```bash
#!/usr/bin/env bash
# Global prepare-commit-msg hook, AI-generated conventional commit messages.
# Chains to repo-local .git/hooks/prepare-commit-msg if present.

# Bail early if user opted out, or commit carries a prepared message (-m/-F/merge/squash).
[[ -n "$SKIP_AI_COMMIT" ]] && exit 0
[[ -n "$2" ]] && exit 0

GIT_DIR="$(git rev-parse --git-dir)"

# Bail during in-progress merges, rebases, cherry-picks.
[[ -f "$GIT_DIR/MERGE_HEAD" ]] && exit 0
[[ -f "$GIT_DIR/CHERRY_PICK_HEAD" ]] && exit 0
[[ -d "$GIT_DIR/rebase-merge" || -d "$GIT_DIR/rebase-apply" ]] && exit 0

diff="$(git diff --cached --diff-algorithm=histogram 2>/dev/null | head -c 5000)"
[[ -z "$diff" ]] && exit 0

msg="$(printf '%s' "$diff" | timeout 4 \
  env CLAUDECODE= MAX_THINKING_TOKENS=0 claude -p \
    --no-session-persistence --model=haiku --tools='' \
    --disable-slash-commands --setting-sources='' \
    --system-prompt='Write a conventional commit message (type: subject). One line, under 72 chars. No explanation.' \
    2>/dev/null)"

[[ -n "$msg" ]] && printf '%s\n\n' "$msg" > "$1"

local_hook="$GIT_DIR/hooks/prepare-commit-msg"
[[ -x "$local_hook" ]] && exec "$local_hook" "$@"

exit 0
```

### 10.2 Global git hooks directory

Set `core.hooksPath = ~/.config/git/hooks` in gitconfig. The prepare-commit-msg hook and any future
global hooks live here. The hook chains to local `.git/hooks/prepare-commit-msg` if it exists, so
per-repo hooks (like the chezmoi pre-commit lint) still work.

### 10.3 actionlint

Install via brew. Static validator for GH Actions workflow YAML with shellcheck integration. Use before
pushing workflow changes.

### 10.4 act configuration and Tart general-purpose sandbox

**Reframed from v1:** Tart is positioned as a general-purpose macOS sandbox for testing dotfile rollouts,
new tool installs, or any risky macOS-local change, not just `act` isolation. This makes the 25GB base
image investment pay off across many use cases.

**Per-project `.actrc`:** Each project that uses `act` gets its own `.actrc` in the repo root (which
`act` reads automatically). No global `~/.actrc`, different projects need different runner
configurations. For this chezmoi dotfiles project, add `.actrc` at the repo root:

```
# Local act configuration for this chezmoi dotfiles repo.
-P macos-latest=-self-hosted
```

**`.actrc` placement note (v2 fix):** It lives at the repo root and is a repo file, not a dotfile. Do
**not** manage it via chezmoi; remove the v1 `.chezmoiignore` entry for it.

**Daily use (process sandbox):**

```bash
brew install eugene1g/safehouse/agent-safehouse
safehouse --add-dirs="$PWD" act -P macos-latest=-self-hosted
```

Wraps `act` in a macOS `sandbox-exec` deny-first profile that blocks access outside the repo.

**General-purpose macOS sandbox (Tart):**

Runs entirely in user-space, no root, no daemon. Tart uses Apple's `Virtualization.framework`
(user-accessible on Apple Silicon); VM files live in `~/.tart/`. The only hard cap is Apple's EULA limit
of 2 concurrent macOS VMs per host, which is a licensing constraint, not a privilege one.

```bash
brew install cirruslabs/cli/tart
tart clone ghcr.io/cirruslabs/macos-sequoia-base:latest sequoia-base
```

One base image, used for:

- Clean-machine dotfile rollout testing (`chezmoi apply` against a fresh VM).
- Trying out new tools without polluting the host.
- Reproducing bugs in a known-clean env.
- Occasional "untrusted" `act` workflow runs.

Per-run (instant APFS copy-on-write clone):

```bash
tart clone sequoia-base scratch
tart run --dir=repo:"$PWD" scratch &
# Wait for VM to boot, then (note the single-quoted outer shell to preserve the space in the path):
tart exec scratch -- bash -c 'cd "/Volumes/My Shared Files/repo" && <command>'
tart delete scratch
```

Max 2 concurrent macOS VMs per host (Apple licensing).

______________________________________________________________________

## 11. Package changes

### 11.1 Add to brew formulae (homebrew-core)

- `sesh`
- `worktrunk`
- `csvlens`
- `atuin` (migrating from curl install)
- `actionlint`
- `bat-extras` (batman, batgrep, batdiff)
- `hyperfine` (benchmarking; also used for shell startup profiling in §16)
- `gitleaks`
- `difftastic`
- `moreutils` (sponge, pee, vipe, required by `find-and-remove-json-objects.sh`)
- `sd` (sed replacement)
- `ruff` (Python linter/formatter)
- `hurl` (plain-text HTTP test runner)
- `carapace` (universal shell completion engine, see §16)

### 11.2 Add to brew taps

- `eugene1g/safehouse` (for agent-safehouse)
- `cirruslabs/cli` (for tart)
- `vjeantet/tap` (for alerter, not in homebrew-core)

### 11.3 Add to brew formulae (from taps)

- `agent-safehouse` (from `eugene1g/safehouse`)
- `tart` (from `cirruslabs/cli`)
- `alerter` (from `vjeantet/tap`, replaces terminal-notifier)

### 11.4 Remove from brew formulae

- `diff-so-fancy` (replaced by delta).
- `terminal-notifier` (abandoned; replaced by alerter).
- `hub` (superseded by `gh`; both installed currently).
- `rbenv` (user doesn't use Ruby; any future Ruby goes through a Nix flake, see §8.1).

### 11.5 Remove from system

- `tms` binary at `~/.local/bin/tms` (not brew, not cargo, manual binary).
- `~/.config/tms/` (old config).
- `~/.sdkman/` (entire directory).
- `~/.atuin/bin/` (old curl-installed binary).
- `~/.rbenv/` (entire directory, see §8.1).

### 11.6 Resolve tmux-fingers duplicate

Currently installed via BOTH Homebrew formula (`morantron/tmux-fingers/tmux-fingers`) AND as a tmux
plugin. Keep the brew formula; remove the plugin reference from `dot_tmux.conf`.

______________________________________________________________________

## 12. Claude Code improvements

**Heavily revised from v1** based on Claude Code v1 schema verification.

### 12.1 Template the settings file

Convert `dot_claude/settings.json` to `dot_claude/settings.json.tmpl` for path interpolation
(`{{ .chezmoi.homeDir }}` instead of hardcoded `/Users/stephen/...`).

**Preserve** existing fields from the current settings.json that v1 silently dropped:

- `voiceEnabled: true`
- `skipDangerousModePermissionPrompt: true`
- `statusLine.command` (templated: `{{ .chezmoi.homeDir }}/.claude/statusline-command.sh`)
- `enabledPlugins` (superpowers, document-skills)
- `permissions.defaultMode: "bypassPermissions"` + `permissions.allow: [...]`

### 12.2 Add deny list

Even with `bypassPermissions`, explicit `deny` rules still apply as a safety net:

- `.env` and `.env.*`
- `secrets/**`
- `credentials.json`
- `.aws/credentials`
- `.ssh/id_*`

### 12.3 Stop hook (gated on 5-min session duration)

Stop hook fires `hue-pulse.sh` only if the current session lasted longer than 5 minutes. Implementation:
hook scripts receive their input as **JSON on stdin** (not env vars, there is no `CLAUDE_SESSION_ID` env
var). The `UserPromptSubmit` hook (§20.4) writes `date +%s` to `/tmp/claude-session-$session_id-start` on
first prompt; the Stop hook reads that file and computes elapsed time.

**Stop hook schema (no matcher supported, Stop always fires):**

```json
"Stop": [
  {
    "hooks": [
      {
        "type": "command",
        "command": "~/.local/bin/claude-stop-pulse.sh"
      }
    ]
  }
]
```

Where `dot_local/bin/executable_claude-stop-pulse.sh`:

```bash
#!/usr/bin/env bash
# Stop hook: pulse Hue green if session lasted >5 min.
# Hook input: JSON on stdin with {session_id, transcript_path, cwd, permission_mode, hook_event_name}.

input=$(cat)
session_id=$(printf '%s' "$input" | jq -r '.session_id // empty' 2>/dev/null)
[[ -z "$session_id" ]] && exit 0

start_file="/tmp/claude-session-${session_id}-start"
[[ -f "$start_file" ]] || exit 0

elapsed=$(( $(date +%s) - $(cat "$start_file") ))
rm -f "$start_file"

(( elapsed >= 300 )) && exec ~/.local/bin/hue-pulse.sh 0
exit 0
```

### 12.4 Notification hook (with correct matcher schema)

Notification hooks **do** support matchers on `notification_type`. Use `permission_prompt` to fire only
on actual permission requests:

```json
"Notification": [
  {
    "matcher": "permission_prompt",
    "hooks": [
      {
        "type": "command",
        "command": "alerter --title 'Claude Code' --message 'Needs attention' --sound default"
      }
    ]
  }
]
```

(Note: `alerter` v26+ uses **double-dash** flags, the Swift rewrite; the legacy single-dash form was
dropped. v1 spec used single-dash, fixed in v2.)

### 12.5 `alwaysThinkingEnabled`

Set `alwaysThinkingEnabled: true` in Claude Code settings. Forces extended thinking on every prompt
(typically configured via `/config` but editable directly).

### 12.6 `cleanupPeriodDays`

Claude Code deletes session files older than `cleanupPeriodDays` at startup (default 30 days, minimum 1
day). Setting `cleanupPeriodDays: 36525` (≈100 years) effectively disables cleanup so session history is
preserved indefinitely. Also controls orphaned-subagent-worktree cleanup age.

(Note: v2-early-draft claimed this setting didn't exist. That was wrong, per the Claude Code settings
docs, the setting is valid and takes effect. To fully disable session persistence, use
`CLAUDE_CODE_SKIP_PROMPT_HISTORY` env var or `--no-session-persistence` flag in `-p` mode instead.)

### 12.7 CLAUDE.md evergreen directive

Add to the top of the repo's `CLAUDE.md` (and the global `~/.claude/CLAUDE.md`, see §20):

> Avoid adding point-in-time content (current sprint goals, active branches, temporary workarounds) that
> wouldn't make sense if multiple workstreams, PRs, or branches were in progress simultaneously. Document
> general principles, workflows, and architecture, not transient project state.

### 12.8 `/pr-merge` slash command

Create a global slash command at `private_dot_claude/commands/pr-merge.md` that:

- Squash merges the current PR via `gh pr merge --squash --delete-branch`.
- Switches to main branch.
- Pulls latest.

### 12.9 gh credential helper (in §9 gitconfig)

Covered in §9 (gitconfig modernization). Retained for reference.

### 12.10 **CUT (v2):** `settings.local.json.tmpl`

A chezmoi-managed "machine-local" file is self-contradictory. The actual machine-local
`settings.local.json` is unmanaged and exists on the machine if needed.

### 12.11 **CUT (v2):** Claude Code Review GitHub Action

Low-value on a personal dotfiles repo with few PRs.

### 12.12 **MOVED (v2):** Hue light pulse on Stop hook

Now lives in §12.3 (gated on 5-min session duration).

### 12.13 **FIXED (v2):** Claude Code Action v1 schema

(This item applies only if §12.11 is ever revisited, archived here for reference.) The correct v1 input
schema is:

```yaml
- uses: anthropics/claude-code-action@v1
  with:
    anthropic_api_key: ${{ secrets.ANTHROPIC_API_KEY }}
    prompt: "Review this PR for..."
    claude_args: "--max-turns 10 --model claude-sonnet-4-6"
    # trigger_phrase defaults to "@claude"
```

v1 **removed:** `direct_prompt`, `allowed_tools`, `use_sticky_comment`, `custom_instructions`,
`max_turns`, `model` top-level inputs. Flags now passed via unified `claude_args`.

______________________________________________________________________

## 13. User-customization migration

Migrate existing `~/.claude/` user customizations into chezmoi so they travel across machines.

### 13.1 Custom skills → chezmoi

Current state: four custom skills live at `~/.claude/skills/` but are not under version control:

- `deep-research/`
- `todoist-cli/`
- `web-research-task/`
- `youtube-transcript/`

**Action:** Move each directory into `private_dot_claude/skills/<skill-name>/`. Preserve internal
structure (`SKILL.md`, `scripts/`, `templates/`, etc.). Commit to chezmoi.

### 13.2 Statusline script → chezmoi

Current state: `~/.claude/statusline-command.sh` is unmanaged. Contains hardcoded `/Users/stephen/` paths
referenced by settings.json.

**Action:** Move to `private_dot_claude/executable_statusline-command.sh` (no `.tmpl` needed, it doesn't
interpolate anything at apply time; settings.json will point to
`{{ .chezmoi.homeDir }}/.claude/statusline-command.sh`).

### 13.3 Audit: other user customizations

Before committing the migration, list `~/.claude/{commands,agents,hooks,projects}` for any user-authored
content not covered by §20 and fold it into the migration. Currently all three subdirectories are empty
(plugin-provided content lives elsewhere).

______________________________________________________________________

## 14. Template hygiene pass

Hardcoded `/Users/stephen/...` in several managed files makes them non-portable. Fix in v2.

### 14.1 osquery

- Rename `dot_config/osquery/osquery.conf` → `dot_config/osquery/osquery.conf.tmpl`. Replace hardcoded
  `/Users/stephen/.local/log/osquery` with `{{ .chezmoi.homeDir }}/.local/log/osquery`.
- In `.chezmoiscripts/run_onchange_before_50-setup-osquery.sh.tmpl`, replace hardcoded
  `/Users/stephen/.config/osquery/osquery.conf` with
  `{{ .chezmoi.homeDir }}/.config/osquery/osquery.conf`.

### 14.2 LaunchAgents

`Library/LaunchAgents/*.plist`, three plists, all likely contain hardcoded `/Users/stephen/...` paths.
Audit each, rename to `.tmpl`, interpolate `{{ .chezmoi.homeDir }}` where needed.

### 14.3 `dot_claude/settings.json` statusLine

Covered in §12.1 (templated to `{{ .chezmoi.homeDir }}`).

### 14.4 Git config cleanup

Delete the `[filesystem "Oracle Corporation|11.0.5|/dev/mapper/volgroup-home"]` JGit cache block
(gitconfig lines 60-62), dead on macOS. Covered in §9.5 with more detail.

Optionally template `[difftool "nvimdiff"].cmd` path by OS. Low priority.

### 14.5 Remove `git u` footgun

Covered in §9.5.

### 14.6 `.chezmoiignore` additions

Add `**/.DS_Store` to `.chezmoiignore` (chezmoi currently tries to apply macOS Finder turds from
`Library/.../.DS_Store`).

______________________________________________________________________

## 15. Bootstrap hardening

### 15.1 New: Homebrew install bootstrap

`.chezmoiscripts/run_once_before_00-install-homebrew.sh.tmpl`, ensures Homebrew exists before package
manifest script runs. Currently every downstream script assumes `/opt/homebrew/bin/brew` exists; a fresh
machine without Homebrew fails all of them.

```bash
{{ if eq .chezmoi.os "darwin" -}}
#!/usr/bin/env bash
set -euo pipefail
command -v brew &>/dev/null && exit 0
echo "Homebrew not found. Installing..."
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
{{ end -}}
```

### 15.2 Guard atuin env source

Covered in §1.4.

### 15.3 KeePassXC unlock guidance

No preflight script is added. Chezmoi's own template rendering surfaces the `keepassxc` prompt when the
DB is locked, and `CLAUDE.md` already documents the constraint (never run bare `chezmoi apply` from
Claude Code; use `--exclude=templates` for automation). Just extend `CLAUDE.md` to note that new machines
must run `chezmoi apply` from an interactive terminal with KeePassXC unlocked.

______________________________________________________________________

## 16. Shell productivity additions

### 16.1 Modern tool aliases

(Duplicated in §2.9 for overview; here is the full list.)

In `dot_bashrc.tmpl`:

```bash
# Modern replacements for installed tools.
alias ls='eza --icons'
alias ll='eza -la --git --icons'
alias la='eza -a --icons'
alias lt='eza --tree --level=2 --icons --git-ignore'
alias cat='bat -p'
alias du='dust'
alias ps='procs'
alias grep='grep --color=auto'   # fix --color=always which breaks pipes

# sd not aliased, `sed` and `sd` have different semantics; use sd directly.
```

### 16.2 direnv bash hook (already wired)

direnv is already installed **and** hooked at `dot_bashrc.tmpl` line 96: `eval "$(direnv hook bash)"`.
Verified during the v2 review, no change needed to enable the hook.

Pair with project `.envrc` files containing `use flake` so Nix dev shells auto-activate per-project.
Consider adding a `direnv allow` reminder to the chezmoi-apply workflow so new machines don't silently
skip `.envrc` files after clone.

### 16.3 carapace universal completions

`carapace` provides up-to-date completions for 1000+ tools (including ones where bash-completion@2 is
stale). Install via brew (§11.1) and wire into `dot_bashrc.tmpl`:

```bash
command -v carapace &>/dev/null && source <(carapace _carapace bash)
```

This replaces most per-tool completion lines in §2.7, but keep explicit `gh`, `docker`, `kubectl`, `just`
completions as fallbacks in case carapace lacks a spec.

### 16.4 Shell startup profiling (opt-in)

Add a gated profiling block to `dot_bashrc.tmpl`:

```bash
[[ -n $BASHRC_PROFILE ]] && { PS4='+ $EPOCHREALTIME '; set -x; }
```

Usage: `BASHRC_PROFILE=1 bash -i -c exit 2>/tmp/bashrc.trace`, then inspect the trace. For statistical
benchmarking:

```bash
hyperfine 'bash -i -c exit'
```

______________________________________________________________________

## 17. Package manifest cleanup

(This section consolidates all package-manifest edits from §1.3, §10, §11, §13, §16 into a single source
of truth. The implementation commit order is: this section first, then other sections can assume the
tools exist.)

### 17.1 `.chezmoidata/system_packages_autoinstall.yaml`

**Add to taps:**

- `eugene1g/safehouse`
- `cirruslabs/cli`
- `vjeantet/tap`

**Add to formulae (alphabetical):**

- `actionlint`
- `agent-safehouse`
- `alerter` (from `vjeantet/tap`)
- `atuin`
- `bat-extras`
- `carapace`
- `csvlens`
- `difftastic`
- `gitleaks`
- `hurl`
- `hyperfine`
- `moreutils`
- `ruff`
- `sd`
- `sesh`
- `tart` (from `cirruslabs/cli`)
- `worktrunk`

**Remove from formulae:**

- `diff-so-fancy` (replaced by delta)
- `hub` (superseded by gh)
- `terminal-notifier` (abandoned; replaced by alerter)
- `rbenv` (user doesn't use Ruby, see §8.1)

### 17.2 Verify after apply

Spot-check that everything in the manifest is actually installed:

```bash
comm -23 \
  <(yq '.packages.macos.homebrew.formulae[]' .chezmoidata/system_packages_autoinstall.yaml | sort) \
  <(brew list --formula | sort)
```

Any output lines are formulae in the manifest that aren't installed, indicates a failed or incomplete
`brew bundle`.

______________________________________________________________________

## 18. User-bin script fixes

### 18.1 `dot_local/bin/executable_fetch-gitignore.sh`

**Decision:** Delete. Functionality duplicated by `gh gitignore` alias already configured in
`dot_config/gh/private_config.yml`. `gh gitignore` tracks the upstream repo's default branch
automatically.

### 18.2 `dot_local/bin/executable_find-and-remove-json-objects.sh`

**Fix:**

- Add `moreutils` to formulae (§17.1), provides `sponge`.
- Add `set -euo pipefail`.
- Fix empty-var error messages (currently `printf "Error: $JSON_OBJECT is empty"` prints nothing because
  `$JSON_OBJECT` is the empty var). Use `%q` or quote strings.
- Add `mkdir -p "$backup_dir"` before the copy loop.

### 18.3 `dot_local/bin/executable_gha-notify.sh`

**Fix:** Replace `osascript` notification calls with `alerter` invocations to match the rest of the stack
(§7.1).

### 18.4 `dot_local/bin/executable_osquery-report.sh`

**Fix:**

- Replace hardcoded `$HOME/workspaces/Ivy/Logs/osquery` with an env-configurable `OSQUERY_REPORT_DIR`
  (default: `$HOME/.local/state/osquery-reports`).
- Add `set -euo pipefail`.
- Migrate notification calls to `alerter`.

### 18.5 `dot_local/bin/executable_claude-restart.sh`

**Minor fix:** Replace `sleep 5` with a polling loop that waits for the tmux pane to show the Claude
trust prompt text. Gate with a 30s timeout. Low priority.

______________________________________________________________________

## 19. Lint/CI expansion

### 19.1 Flake dev shell additions

Add to `flake.nix` `buildInputs`:

- `pkgs.taplo` (TOML formatter/linter)
- `pkgs.jq` (JSON validator)
- `pkgs.yq-go` (YAML query tool; useful for validating package manifest)

### 19.2 Lint script extensions

Extend `scripts/lint.sh` with runners for new file types:

- `run_30_taplo`: `taplo fmt --check` across all `*.toml` files. Catches drift in tms, atuin, himalaya,
  aerospace, starship, yt-dlp configs.
- `run_35_jq`: `jq empty < "$f"` across all `*.json` files to verify parseability.
- `run_40_yq`: `yq eval '.' "$f" > /dev/null` on `.chezmoidata/*.yaml` for schema validation.

Wire each into the `all` flag used by `just l`.

### 19.3 Fix nix file glob

`find_nix_files` in `scripts/lint.sh` is hardcoded to `flake.nix`. Change to find all `*.nix` files for
future-proofing:

```bash
find . -type f -name '*.nix' -not -path './.git/*' -not -path './.direnv/*' -print0
```

### 19.4 justfile recipes

Add to `justfile`:

```
diff:
    nix develop .#run --command chezmoi diff --exclude=templates

apply:
    nix develop .#run --command chezmoi apply --exclude=templates --force

check:
    nix develop .#run --command nix flake check --all-systems
```

Naming mirrors the short-letter recipes (`l`, `s`, `S`, `m`, `n`, `h`), these can also get short-letter
variants if desired (e.g., `d`, `a`, `c`). `brew update && upgrade` is NOT added as a recipe because the
system packages autoupdate daemon (§2.6) handles this continuously.

### 19.5 Pre-commit hook integration

Existing: `just h` installs a pre-commit hook running `just l`. After §19.2, `just l` will also validate
TOML/JSON/YAML, so the pre-commit hook catches drift before commit.

______________________________________________________________________

## 20. `dot_claude/` surface expansion

Managed Claude Code customization surface so the full config travels with chezmoi.

### 20.1 Global `CLAUDE.md`

Create `dot_claude/CLAUDE.md` with:

- The evergreen directive from §12.7.
- A `## Toolchain` section listing locked-in choices so future Claude sessions don't re-suggest them
  (e.g., "not migrating to mise, zellij, yazi, lazygit").
- A `## Collaboration style` section capturing high-level preferences.

### 20.2 `private_dot_claude/commands/`, `/pr-merge` (from §12.8)

Already covered.

### 20.3 `private_dot_claude/agents/`, initial agent

Rather than empty `.keep` (v1): add one concrete agent that wraps the chezmoi-apply workflow, since it's
a high-friction repeat. Example `private_dot_claude/agents/chezmoi-apply.md`:

```markdown
---
name: chezmoi-apply
description: Runs `chezmoi apply --exclude=templates --force` and reports any diffs for template files that need KeePassXC-unlocked manual apply.
tools: Bash, Read
---

Use this agent to safely apply chezmoi state without triggering KeePassXC password prompts.
```

(Exact agent body to be refined during implementation, this is a scaffolding placeholder.)

### 20.4 Claude Code hooks: `UserPromptSubmit` + `PreToolUse`

**`UserPromptSubmit` hook**: writes session start marker for the 5-min gated Stop hook (§12.3). Matcher
is not supported on UserPromptSubmit; this fires on every prompt, and the script itself makes the "first
time for this session" check via a sentinel file:

```json
"UserPromptSubmit": [
  {
    "hooks": [
      {
        "type": "command",
        "command": "{{ .chezmoi.homeDir }}/.local/bin/claude-user-prompt-start.sh"
      }
    ]
  }
]
```

`dot_local/bin/executable_claude-user-prompt-start.sh`:

```bash
#!/usr/bin/env bash
# UserPromptSubmit hook: record session start time on first prompt.
input=$(cat)
session_id=$(printf '%s' "$input" | jq -r '.session_id // empty' 2>/dev/null)
[[ -z "$session_id" ]] && exit 0

start_file="/tmp/claude-session-${session_id}-start"
[[ -f "$start_file" ]] || date +%s > "$start_file"
exit 0
```

Context-injection (prepending `git status -sb` to every prompt) is intentionally **out of scope**: easy
to add later as a one-line `printf` to stdout in the same script, but the file would need to exit with
JSON `{"hookSpecificOutput": {"additionalContext": "..."}}` per the hook schema. Defer until needed.

**`PreToolUse` Bash audit hook**: logs every `Bash(*)` invocation to `~/.claude/audit.log` with
timestamp, working directory, and command. Useful when reviewing what Claude did in long sessions:

```json
"PreToolUse": [
  {
    "matcher": "Bash",
    "hooks": [
      {
        "type": "command",
        "command": "{{ .chezmoi.homeDir }}/.local/bin/claude-audit.sh"
      }
    ]
  }
]
```

Hook script parses JSON from stdin (tool input is in `.tool_input.command` for Bash). macOS BSD `date`
lacks `-Is`, so use `gdate` (GNU coreutils, already installed per the package manifest) or a portable
format string:

```bash
#!/usr/bin/env bash
# PreToolUse hook for Bash: append one line per tool call to audit log.
input=$(cat)
cmd=$(printf '%s' "$input" | jq -r '.tool_input.command // ""' 2>/dev/null)
cwd=$(printf '%s' "$input" | jq -r '.cwd // ""' 2>/dev/null)
ts=$(gdate -Is 2>/dev/null || date -u +"%Y-%m-%dT%H:%M:%SZ")
printf '%s\t%s\t%s\n' "$ts" "$cwd" "$cmd" >> "$HOME/.claude/audit.log"
exit 0
```

Non-blocking; always exits 0 so Claude's tool invocation is never held up.

### 20.5 MCP config

**Decision:** Leave MCP configuration **unmanaged** in v2. Per the settings-scope docs, MCP servers are
typically registered via `~/.claude/settings.json` (`mcpServers` field) or a project-local `.mcp.json`.
Both often contain machine-specific paths and OAuth tokens. Auditing those is its own project. If an MCP
server proves generic and stable enough to template, add it in a follow-up.

______________________________________________________________________

## 21. Passive tmux window/pane status indicators

**Goal:** at-a-glance visibility into what every tmux window is doing (building, testing, running an AI
agent, or something else long-running) *without* prefacing commands with wrappers or explicit status
calls. Pairs with §3.1's right-plugin layout change so the status of the previously-active session is
always visible in the bottom-right while you work in a different one.

**Mechanism:** purely passive polling via tmux's existing format-string refresh cycle. Each refresh
(`@tmux2k-refresh-rate`, currently 3s) tmux2k re-runs the format strings; our helpers inspect each pane's
`pane_current_command` and emit an emoji.

### 21.1 Window-emoji helper

New file: `dot_local/bin/executable_tmux-window-emoji.sh`

```bash
#!/usr/bin/env bash
# Output an emoji for the window at $1 (e.g. "uriel:3") based on what's running
# in its active pane. Silent for shells and interactive TUIs.

target="${1:-}"
[[ -z "$target" ]] && exit 0
cmd=$(tmux display-message -p -t "$target" '#{pane_current_command}' 2>/dev/null)

case "$cmd" in
  # Shells and interactive TUIs, silent.
  bash|zsh|fish|sh|dash) ;;
  nvim|vim|vi|view|less|more|man|top|btop|htop|tmux|ssh|mosh|fzf) ;;

  # AI agents.
  claude|codex|aider|goose|cursor-agent) printf '🤖' ;;

  # Test runners.
  pytest|jest|vitest|rspec|mocha|phpunit|tox) printf '🧪' ;;

  # Build tools / task runners / package managers.
  cargo|go|make|gmake|just|webpack|vite|rollup|esbuild|tsc|swift|xcodebuild) printf '🔨' ;;
  docker|nix|nix-build|nixos-rebuild|npm|pnpm|yarn|bun) printf '🔨' ;;
  gradle|mvn|ant|meson|ninja|bazel|buck|cmake) printf '🔨' ;;

  # Everything else that's not a shell, generic long-running.
  *) printf '⏳' ;;
esac
```

**Matching note:** `pane_current_command` gives the foreground process's name, not its full argv. So
`cargo build` and `cargo test` both map to 🔨. This is intentional, the point is coarse "something is
running" signal, not fine-grained classification. If you want `cargo test` → 🧪 later, we'd read full argv
via `ps -o command= -p <pane_pid>`; defer until the simpler version proves insufficient.

### 21.2 Window-list integration

Extend tmux2k's per-window format so the emoji renders after the window index+name:

```tmux
set-option -g @tmux2k-window-list-format "#I #W #(~/.local/bin/tmux-window-emoji.sh '#S:#I')"
```

tmux2k invokes this format per window per refresh. For a typical 10-window setup, that's ~3 forks per
second, trivial overhead for a case-statement shell script.

### 21.3 "Last session" tracking

The `last-proc` right-side plugin needs to know which session you just switched *from*. Use tmux's
`client-session-changed` hook to persist that into a user option:

```tmux
set-hook -g client-session-changed \
  'run-shell "tmux set-option -g @prev-session \"#{hook_session_name}\""'
```

**Verification item for implementation:** tmux format variables available in this hook's context,
`hook_session_name` vs `client_last_session` vs something else. Pick whichever holds the *pre-switch*
session name. `tmux show-hooks` and `tmux list-keys` plus a quick test will confirm.

### 21.4 `last-proc` custom tmux2k plugin

Drop a script at tmux2k's plugin scripts path (exact path to verify, typically
`~/.config/tmux/tmux2k/scripts/<name>.sh` or the installed plugin's `scripts/` directory). The script
name matches the plugin name in `@tmux2k-right-plugins`.

```bash
#!/usr/bin/env bash
# tmux2k custom plugin: last-proc
# Displays "<prev_session>:<window_name> <emoji>" in the right-side status,
# or nothing if no previous session is tracked yet.

prev=$(tmux show-option -gv @prev-session 2>/dev/null)
[[ -z "$prev" ]] && exit 0
tmux has-session -t "$prev" 2>/dev/null || exit 0

win_idx=$(tmux display-message -p -t "$prev:" '#{window_index}' 2>/dev/null)
win_name=$(tmux display-message -p -t "$prev:" '#{window_name}' 2>/dev/null)
emoji=$(~/.local/bin/tmux-window-emoji.sh "$prev:$win_idx")

printf '%s:%s %s' "$prev" "$win_name" "$emoji"
```

Add colors and wire into the right-side plugin list (§3.1):

```tmux
set-option -g @tmux2k-last-proc-colors "cyan black"
set-option -g @tmux2k-right-plugins "last-proc network ram"
```

**Verification items for implementation:**

1. tmux2k's exact plugin-script directory convention (installed via tpm). Inspect
   `~/.tmux/plugins/tmux2k/scripts/` at implementation time to learn the pattern; may need to place the
   script there or at `~/.config/tmux/tmux2k/scripts/`.
1. tmux2k's color-setting naming: `@tmux2k-<name>-colors` is the common pattern but confirm against
   tmux2k's existing plugins.

### 21.5 Ordering and precedence

- The window-emoji helper runs *before* tmux2k's refresh completes; its output is cached by tmux for the
  refresh interval.
- If `@prev-session` is unset (fresh tmux server), `last-proc` outputs nothing, silent degradation.
- No interaction with `automatic-rename` or `allow-rename`, we're not renaming windows, just appending
  computed text to format strings.
- `status-interval` stays at its current value; only tmux2k's refresh rate governs how often the scripts
  run.

______________________________________________________________________

## Non-goals (explicitly excluded)

- Neovim overhaul (separate sub-project)
- GPG agent TTL changes (user wants current 7/28 day TTLs)
- Starship language module pruning (needed for reading other repos)
- Devbox migration (can't replicate current flake.nix)
- Television (fzf integration too deep)
- Sesh replacement of tms marks concept (preserved via SESH key table)
- ntfy.sh mobile push notifications (future consideration)
- Quality triage skill system (future sub-project, after Neovim)
- macOS defaults script (defer until new-machine provisioning)
- Claude Code Review GitHub Action (low value on personal repo)
- SSH commit signing (GPG+KeePassXC already configured and working)
- settings.local.json.tmpl (self-contradictory; real file is unmanaged)
- `mise` (user declined)
- `zellij`, `yazi`, `lazygit` (user declined)
- `ble.sh` (high-risk; defer)
- `jj` / `git-branchless` (defer)
- `sops` / `age` per-project secrets (defer)
- Theme unification across bat/delta/ghostty (defer)
- launchctl `bootout`/`bootstrap` modernization (defer, low value until a failure forces it)
- `dot_bash_profile` / `dot_profile` consolidation (defer)
