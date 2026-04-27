# Sesh Deep Research Report

**Date:** 2026-04-13 **Subject:** Comprehensive analysis of sesh (joshmedeski/sesh) for migration from
tms

______________________________________________________________________

## Table of Contents

1. [What Is Sesh](#1-what-is-sesh)
1. [Installation](#2-installation)
1. [Complete Configuration Reference](#3-complete-configuration-reference)
1. [CLI Commands Reference](#4-cli-commands-reference)
1. [Tmux Integration Patterns](#5-tmux-integration-patterns)
1. [Startup Commands and Scripts](#6-startup-commands-and-scripts)
1. [Window Definitions](#7-window-definitions)
1. [Wildcard Configurations](#8-wildcard-configurations)
1. [Preview System](#9-preview-system)
1. [Zoxide Integration](#10-zoxide-integration)
1. [Session Naming Strategy](#11-session-naming-strategy)
1. [Nerd Font Icons](#12-nerd-font-icons)
1. [Picker Options](#13-picker-options)
1. [Raycast Extension](#14-raycast-extension)
1. [Advanced Features](#15-advanced-features)
1. [Migration Plan: tms to sesh](#16-migration-plan-tms-to-sesh)
1. [Recommended sesh.toml for Your Setup](#17-recommended-seshtoml-for-your-setup)
1. [Recommended tmux.conf Changes](#18-recommended-tmuxconf-changes)
1. [Recommended tmux-refresh.sh Rewrite](#19-recommended-tmux-refreshsh-rewrite)
1. [Sources](#20-sources)

______________________________________________________________________

## 1. What Is Sesh

Sesh is a Go-based CLI tool for creating and managing tmux sessions. It is the successor to Josh
Medeski's original bash-based `t-smart-tmux-session-manager` tmux plugin. The rewrite in Go gives it
significantly better performance (compiled binary vs background bash parsing) and enables a richer
feature set.

Key value propositions:

- **Smart session discovery** -- combines zoxide frecency, user-configured sessions, tmuxinator configs,
  and active tmux sessions into a single unified list
- **Per-project configuration** -- define startup commands, windows, and preview commands in `sesh.toml`
  or per-project `.sesh.toml` files imported into the main config
- **Wildcard configs** -- apply settings to any directory matching a glob pattern (e.g.,
  `~/workspaces/**`)
- **Multiple picker UIs** -- built-in TUI picker, fzf, television, or gum
- **Composable** -- works as a simple CLI that pipes into any fuzzy finder or script

Current version: v2.25.0 (as of April 2024 release cycle). 2.3k+ GitHub stars. MIT license.

______________________________________________________________________

## 2. Installation

```bash
brew install sesh
```

Other methods: `go install github.com/joshmedeski/sesh/v2@latest`, AUR (`yay -S sesh-bin`), conda/mamba,
pixi.

**Dependencies:** tmux (required), zoxide (required for directory discovery).

______________________________________________________________________

## 3. Complete Configuration Reference

Config file location: `$XDG_CONFIG_HOME/sesh/sesh.toml` (defaults to `~/.config/sesh/sesh.toml`).

The following is the **complete schema** extracted from the Go source code (`model/config.go`):

### Top-Level Fields

| Field             | Type     | Default                                 | Description                                                                 |
| ----------------- | -------- | --------------------------------------- | --------------------------------------------------------------------------- |
| `cache`           | bool     | false                                   | Enable session list caching with stale-while-revalidate strategy            |
| `strict_mode`     | bool     | false                                   | Reject unknown fields in config (useful for catching typos)                 |
| `import`          | string[] | []                                      | Paths to additional TOML files to merge (supports `~` expansion)            |
| `blacklist`       | string[] | []                                      | Session names to exclude from all listings                                  |
| `sort_order`      | string[] | ["tmux","config","tmuxinator","zoxide"] | Controls source precedence in listings                                      |
| `dir_length`      | int      | 1                                       | Number of directory path components in auto-generated session names (min 1) |
| `separator_aware` | bool     | false                                   | Normalize separators (`-`, `_`, `/`, `\`) to spaces for fuzzy matching      |
| `tmux_command`    | string   | ""                                      | Custom tmux binary path                                                     |

### `[default_session]` Section

Applied to ALL sessions unless overridden per-session or disabled.

| Field             | Type     | Description                                                                                      |
| ----------------- | -------- | ------------------------------------------------------------------------------------------------ |
| `startup_command` | string   | Command run when creating any new session. Supports `{}` placeholder replaced with session path. |
| `preview_command` | string   | Command for session preview. Supports `{}` placeholder.                                          |
| `tmuxp`           | string   | tmuxp config name to use                                                                         |
| `tmuxinator`      | string   | tmuxinator config name to use                                                                    |
| `windows`         | string[] | List of window config names to create (references `[[window]]` entries)                          |

### `[[session]]` Entries

Each defines a named, configured session that appears in `sesh list -c`.

| Field                     | Type     | Description                                                  |
| ------------------------- | -------- | ------------------------------------------------------------ |
| `name`                    | string   | **Required.** Display name for the session (supports emoji). |
| `path`                    | string   | **Required.** Directory path (supports `~` expansion).       |
| `disable_startup_command` | bool     | Skip the `default_session.startup_command` for this session. |
| `startup_command`         | string   | Override startup command for this session. Supports `{}`.    |
| `preview_command`         | string   | Override preview command. Supports `{}`.                     |
| `tmuxp`                   | string   | tmuxp config name                                            |
| `tmuxinator`              | string   | tmuxinator config name                                       |
| `windows`                 | string[] | Window config names to create                                |

### `[[window]]` Entries

Define reusable window layouts. Referenced by name from session or default_session `windows` arrays.

| Field            | Type   | Description                                                       |
| ---------------- | ------ | ----------------------------------------------------------------- |
| `name`           | string | **Required.** Unique identifier (referenced in `windows` arrays). |
| `startup_script` | string | Command sent to the window via `send-keys` after creation.        |
| `path`           | string | Working directory for window (inherits session path if omitted).  |

### `[[wildcard]]` Entries

Apply configuration to any session whose path matches a glob pattern.

| Field                     | Type     | Description                                                     |
| ------------------------- | -------- | --------------------------------------------------------------- |
| `pattern`                 | string   | **Required.** Glob pattern (supports `*`, `/**` for recursive). |
| `startup_command`         | string   | Command for matching sessions. Supports `{}`.                   |
| `disable_startup_command` | bool     | Disable the default startup command for matching sessions.      |
| `preview_command`         | string   | Preview command for matching sessions. Supports `{}`.           |
| `windows`                 | string[] | Window config names to create for matching sessions.            |

### Wildcard Pattern Matching

From the source code, two matching modes:

- `~/projects/*` -- standard `filepath.Match` glob (matches one level)
- `~/workspaces/**` -- recursive prefix match (matches any depth below the prefix)

### Import System

The `import` field merges `[[session]]`, `[[window]]`, and `[[wildcard]]` entries from external files.
This enables per-project `.sesh.toml` files:

```toml
# ~/.config/sesh/sesh.toml
import = [
  "~/workspaces/webdavis/uriel/.sesh.toml",
  "~/workspaces/Ivy/Projects/Casually Concerned/.sesh.toml",
]
```

Each imported file can contain its own `[[session]]`, `[[window]]`, and `[[wildcard]]` blocks.

______________________________________________________________________

## 4. CLI Commands Reference

| Command                   | Aliases | Description                                              |
| ------------------------- | ------- | -------------------------------------------------------- |
| `sesh list`               | `l`     | List sessions from all sources                           |
| `sesh connect <name>`     | `cn`    | Connect to or create a session                           |
| `sesh last`               | `L`     | Switch to the last-used tmux session                     |
| `sesh root`               | `r`     | Print the git root of the current session                |
| `sesh preview <name>`     |         | Show preview for a session                               |
| `sesh window [name]`      | `w`     | List or switch/create windows                            |
| `sesh clone <repo-url>`   | `cl`    | Git clone and connect to the result as a session         |
| `sesh picker`             |         | Built-in interactive session picker                      |
| `sesh completion <shell>` |         | Generate shell completions (bash, zsh, fish, powershell) |

### `sesh list` Flags

| Flag                | Short | Description                                         |
| ------------------- | ----- | --------------------------------------------------- |
| `--tmux`            | `-t`  | Show only tmux sessions                             |
| `--config`          | `-c`  | Show only configured sessions                       |
| `--zoxide`          | `-z`  | Show only zoxide directories                        |
| `--tmuxinator`      | `-T`  | Show only tmuxinator configs                        |
| `--icons`           | `-i`  | Show Nerd Font icons with ANSI color                |
| `--no-color`        | `-n`  | Icons without color (requires `--icons`)            |
| `--json`            | `-j`  | Output as JSON                                      |
| `--hide-attached`   | `-H`  | Hide currently attached session                     |
| `--hide-duplicates` | `-d`  | Hide duplicate entries (keeps first per sort order) |
| `--panes`           | `-p`  | Show panes in current session                       |

### `sesh connect` Flags

| Flag           | Short | Description                                                                   |
| -------------- | ----- | ----------------------------------------------------------------------------- |
| `--switch`     | `-s`  | Switch client instead of attach (for use from outside terminal, e.g. Raycast) |
| `--command`    | `-c`  | Execute a command when creating new session (ignored if session exists)       |
| `--tmuxinator` | `-T`  | Use tmuxinator to start session if it doesn't exist                           |
| `--root`       | `-r`  | Connect to the git root of the given path                                     |

### `sesh picker` Flags

| Flag      | Short | Description          |
| --------- | ----- | -------------------- |
| `--icons` | `-i`  | Show icons in picker |

### `sesh window` Flags

| Flag        | Short | Description                                             |
| ----------- | ----- | ------------------------------------------------------- |
| `--session` | `-s`  | Target a specific session (default: currently attached) |
| `--json`    | `-j`  | Output as JSON                                          |

### Global Flag

| Flag       | Short | Description                  |
| ---------- | ----- | ---------------------------- |
| `--config` | `-C`  | Path to a custom config file |

______________________________________________________________________

## 5. Tmux Integration Patterns

### Pattern 1: Full-Featured fzf Popup (Recommended)

This is the canonical binding from the README. It provides cycling through session sources with keyboard
shortcuts, previews, and session killing:

```bash
bind-key "T" run-shell "sesh connect \"$(
  sesh list --icons | fzf-tmux -p 80%,70% \
  --no-sort --ansi --border-label ' sesh ' --prompt '  ' \
  --header ' ^a all ^t tmux ^g configs ^x zoxide ^d tmux kill ^f find' \
  --bind 'tab:down,btab:up' \
  --bind 'ctrl-a:change-prompt(  )+reload(sesh list --icons)' \
  --bind 'ctrl-t:change-prompt(  )+reload(sesh list -t --icons)' \
  --bind 'ctrl-g:change-prompt(  )+reload(sesh list -c --icons)' \
  --bind 'ctrl-x:change-prompt(  )+reload(sesh list -z --icons)' \
  --bind 'ctrl-f:change-prompt(  )+reload(fd -H -d 2 -t d -E .Trash . ~)' \
  --bind 'ctrl-d:execute(tmux kill-session -t {2..})+change-prompt(  )+reload(sesh list --icons)' \
  --preview-window 'right:55%' \
  --preview 'sesh preview {}'
)\""
```

### Pattern 2: Built-in Picker Popup

Simpler, no fzf dependency. Uses sesh's own Bubble Tea TUI:

```bash
bind-key "K" display-popup -h 90% -w 50% -E "sesh picker -i"
```

### Pattern 3: Gum Filter Popup

Minimal and clean:

```bash
bind-key "K" display-popup -E -w 40% "sesh connect \"$(
  sesh list -i | gum filter --limit 1 --no-sort --fuzzy \
  --placeholder 'Pick a sesh' --height 50 --prompt='  '
)\""
```

### Pattern 4: Television Integration

```bash
bind-key "T" display-popup -E -w 80% -h 70% -d '#{pane_current_path}' -T 'Sesh' tv sesh
```

Use `Ctrl-s` to cycle sources, `Ctrl-d` to kill sessions.

### Pattern 5: Window Navigation

```bash
bind-key "W" run-shell "sesh window \"$(sesh window | fzf-tmux -p 60%,50% --prompt '  ')\""
```

### Pattern 6: Last Session Toggle

Enhanced version of tmux's built-in `switch-client -l` that survives session closures and detach/reattach
cycles:

```bash
bind-key L run-shell "sesh last || tmux display-message -d 1000 'Only one session'"
```

______________________________________________________________________

## 6. Startup Commands and Scripts

### Startup Command Execution Order

When a new session is created, sesh determines the startup command using a strategy chain (in order):

1. **Config strategy** -- if the session matches a `[[session]]` entry with `startup_command`, use it
1. **Wildcard strategy** -- if the session path matches a `[[wildcard]]` with `startup_command`, use it
   (unless `disable_startup_command` is true)
1. **Default config strategy** -- fall back to `[default_session].startup_command` (unless
   `disable_startup_command` is true on the session)

The `{}` placeholder in any startup command is replaced with the session's directory path at runtime.

### Example: Default Opens Neovim

```toml
[default_session]
startup_command = "nvim -c ':Telescope find_files'"
```

Every new session opens Neovim with a file finder -- unless overridden or disabled.

### Example: Per-Session Startup

```toml
[[session]]
name = "Downloads"
path = "~/Downloads"
startup_command = "yazi"

[[session]]
name = "dotfiles"
path = "~/.local/share/chezmoi"
startup_command = "nvim"
disable_startup_command = false
```

### Example: Startup Script via Windows

Sesh v2 removed the `startup_script` field from `[default_session]` and `[[session]]`. The way to run
multi-command startup scripts is through `[[window]]` definitions with `startup_script`:

```toml
[[session]]
name = "joshmedeski.com"
path = "~/c/joshmedeski.com"
windows = ["editor", "dev-server"]

[[window]]
name = "editor"
startup_script = "nvim +GoToFile"

[[window]]
name = "dev-server"
startup_script = "npm run dev"
```

When the session is created, sesh creates a window for each referenced name, sends the `startup_script`
to each window via `tmux send-keys`, then cycles back to the first window.

### Example: External Startup Script

For complex multi-pane layouts, use a shell script as the startup_command:

```toml
[[session]]
name = "project-x"
path = "~/projects/x"
startup_command = "~/.config/sesh/scripts/project-x-setup.sh"
```

Where `~/.config/sesh/scripts/project-x-setup.sh`:

```bash
#!/usr/bin/env bash
tmux split-window -v -l 10 "npm run dev"
tmux select-pane -t :.+
tmux send-keys "nvim +GoToFile" Enter
```

______________________________________________________________________

## 7. Window Definitions

Windows are **globally defined** and **referenced by name** from sessions. This is a reusable pattern.

```toml
# Define reusable windows
[[window]]
name = "editor"
startup_script = "nvim"

[[window]]
name = "server"
startup_script = "npm run dev"
path = "~/projects/api"    # optional: override session path

[[window]]
name = "logs"
startup_script = "tail -f /var/log/app.log"

# Reference windows from sessions
[[session]]
name = "frontend"
path = "~/projects/frontend"
windows = ["editor", "server"]

[[session]]
name = "backend"
path = "~/projects/backend"
windows = ["editor", "logs"]
```

Key behaviors:

- Windows inherit the session's path unless they specify their own `path`
- `startup_script` is sent via `tmux send-keys` (so the command runs in the shell)
- After all windows are created, sesh selects the next window (cycling to window 1)
- Window names must match exactly between `[[window]].name` and the `windows` array entries

______________________________________________________________________

## 8. Wildcard Configurations

Wildcards apply settings to any session created from a matching directory, even zoxide-discovered ones.

```toml
# All projects under ~/workspaces get nvim
[[wildcard]]
pattern = "~/workspaces/**"
startup_command = "nvim"

# Work projects get a different setup
[[wildcard]]
pattern = "~/work/*"
startup_command = "make dev"
windows = ["editor", "server"]

# Disable startup for config directories
[[wildcard]]
pattern = "~/.config/*"
disable_startup_command = true
```

Pattern matching rules (from source):

- `~/projects/*` uses Go's `filepath.Match` -- matches exactly one level (e.g., `~/projects/foo` but NOT
  `~/projects/foo/bar`)
- `~/workspaces/**` uses prefix matching -- matches any depth below `~/workspaces/` (e.g.,
  `~/workspaces/org/repo` matches)

Wildcards also support `preview_command` and `windows` arrays.

______________________________________________________________________

## 9. Preview System

The preview system uses a strategy chain:

1. **Active tmux session** -- captures the current pane content (`tmux capture-pane`)
1. **Config session with preview_command** -- runs the custom command with `{}` path substitution
1. **Config session without preview_command** -- lists the directory
1. **Directory** -- lists the directory contents

### Custom Preview Examples

```toml
[default_session]
preview_command = "eza --tree --level=2 --icons {}"

[[session]]
name = "uriel"
path = "~/workspaces/webdavis/uriel"
preview_command = "git -C {} log --oneline -10"

[[wildcard]]
pattern = "~/workspaces/**"
preview_command = "eza --tree --level=2 --icons {}"
```

The `{}` placeholder is replaced with the session's resolved directory path.

______________________________________________________________________

## 10. Zoxide Integration

Sesh uses zoxide as one of its four session sources. How it works:

- `sesh list -z` lists directories from zoxide's frecency database
- `sesh connect <name>` falls through its strategy chain: tmux pane > tmux session > tmuxinator > config
  session > config wildcard > directory > zoxide query
- When connecting via zoxide, the directory is automatically added to zoxide's database

### Priming Zoxide

If you're migrating from tms and your zoxide database doesn't have your project directories yet:

```bash
# Add all your project directories at once
ls -d ~/workspaces/*/ ~/workspaces/webdavis/*/ | xargs -I {} zoxide add {}

# Or add specific directories
zoxide add ~/workspaces/webdavis/uriel
zoxide add ~/.openclaw
zoxide add ~/.local/share/chezmoi
```

### Hide Duplicates

When a directory appears both as a configured session and a zoxide result, use `--hide-duplicates` (`-d`)
to show only the first occurrence based on `sort_order`:

```toml
sort_order = ["tmux", "config", "zoxide"]
```

With `-d`, configured sessions take precedence over zoxide duplicates.

______________________________________________________________________

## 11. Session Naming Strategy

Sesh auto-generates session names using a hierarchy:

1. **Git bare repository worktrees** -- `repo/worktree-name`
1. **Git repositories** -- `repo/subdirectory/path` (relative to repo root)
1. **Directories** -- last N components of path (controlled by `dir_length`)

The `dir_length` config controls how many path components appear in directory-based names:

```toml
dir_length = 2   # ~/workspaces/webdavis/uriel -> "webdavis/uriel"
dir_length = 1   # ~/workspaces/webdavis/uriel -> "uriel" (default)
```

Session names have dots and colons replaced (tmux restrictions). Characters that tmux cannot handle are
converted to valid alternatives.

______________________________________________________________________

## 12. Nerd Font Icons

When using `--icons` (`-i`), sesh prefixes session names with source-specific Nerd Font glyphs:

| Source                      | Icon     | ANSI Color     |
| --------------------------- | -------- | -------------- |
| tmux (active session)       | (window) | Blue (34)      |
| config (configured session) | (gear)   | Dark gray (90) |
| zoxide (directory)          | (folder) | Cyan (36)      |
| tmuxinator                  | (list)   | Blue (33)      |
| tmux-pane                   | (pane)   | Green (32)     |

Use `--no-color` (`-n`) with `--icons` for icons without ANSI color codes.

______________________________________________________________________

## 13. Picker Options

### Built-in Picker

Sesh includes a Bubble Tea-based TUI picker with:

- Fuzzy filtering (uses `sahilm/fuzzy` library)
- Keyboard navigation: `up`/`down`, `ctrl-j`/`ctrl-k`, `ctrl-u`/`ctrl-d` (half-page)
- `Enter` to select, `Esc`/`Ctrl-C` to cancel
- Source icons with color
- Separator-aware matching (configurable via `separator_aware` in config)
- Async session loading with loading indicator

```bash
bind-key "K" display-popup -h 90% -w 50% -E "sesh picker -i"
```

### fzf Integration

The most powerful option. Supports live-reloading session lists, source filtering, previews, and inline
session killing. See Pattern 1 in section 5.

### Television Integration

Television (`tv`) has built-in sesh support. Provides a modern TUI with live preview.

### Gum Integration

Simplest option. Gum's filter provides basic fuzzy finding. See Pattern 3 in section 5.

______________________________________________________________________

## 14. Raycast Extension

Available at https://www.raycast.com/joshmedeski/sesh (1,908+ installs).

**Features:**

- Search and connect to tmux sessions from Raycast
- Create new sessions from zoxide results
- Launch terminal emulator

**Requirements:**

- tmux must be running before the extension can be used
- sesh CLI must be installed
- Configure your terminal emulator to auto-attach

**Recent update (v2025.10.9):** Added user-configurable PATH variable for finding sesh binary.

______________________________________________________________________

## 15. Advanced Features

### Clone and Connect

Clone a git repo and immediately create a session for it:

```bash
sesh clone https://github.com/user/repo.git
sesh clone https://github.com/user/repo.git --dir ~/workspaces
```

### Root Navigation

Jump to the git root of the current session's directory:

```bash
sesh root    # prints the root session name
```

Useful with `sesh connect --root`:

```bash
sesh connect --root .    # connect to the git root of current directory
```

### Cache System

Enable caching for faster `sesh list` responses:

```toml
cache = true
```

Uses a stale-while-revalidate strategy: returns cached results immediately while refreshing in the
background. Cache is automatically invalidated after `sesh connect`.

### JSON Output

Both `sesh list --json` and `sesh window --json` output structured JSON, useful for scripting:

```bash
sesh list --json | jq '.[].name'
```

### Sort Order Control

Customize which sources appear first in listings:

```toml
sort_order = ["config", "tmux", "zoxide"]   # configs first, then running sessions, then zoxide
```

### Pane Listing

List panes in the current session (useful for pane-level navigation):

```bash
sesh list --panes    # must be inside tmux
```

### Blacklist

Exclude specific session names from all listings:

```toml
blacklist = ["scratch", "temp", "popup"]
```

### Ad-Hoc Command on Connect

Run a one-off command when creating a session (bypasses config):

```bash
sesh connect ~/projects/foo --command "npm run dev"
```

______________________________________________________________________

## 16. Migration Plan: tms to sesh

### What tms Features Map to Sesh

| tms Feature                          | sesh Equivalent                                               |
| ------------------------------------ | ------------------------------------------------------------- |
| `tms` (fuzzy session picker)         | `sesh connect "$(sesh list \| fzf)"` or `sesh picker`         |
| `tms switch`                         | `sesh connect "$(sesh list -t \| fzf)"` (tmux-only list)      |
| `tms start` (bootstrap all marks)    | Script that loops `sesh connect` for each configured session  |
| `tms marks open N`                   | `sesh connect <session-name>` (sessions defined in sesh.toml) |
| `tms windows`                        | `sesh window`                                                 |
| `tms refresh`                        | Custom script (see section 19)                                |
| `search_dirs` (depth-based scanning) | Zoxide (frecency-based, not depth-scanning)                   |
| `bookmarks`                          | `[[session]]` entries in sesh.toml                            |
| `[[sessions]]` (named sessions)      | `[[session]]` entries in sesh.toml                            |
| `[marks]` (numbered quick-access)    | `sesh connect <name>` via tmux `bind-key`                     |

### Your Current tms Marks (12 entries)

| Mark | Path                                              | Proposed sesh session name | Proposed tmux bind |
| ---- | ------------------------------------------------- | -------------------------- | ------------------ |
| 0    | `~/workspaces/webdavis/uriel`                     | `uriel`                    | `u`                |
| 1    | `~/.openclaw`                                     | `openclaw`                 | `o`                |
| 2    | `~/workspaces/webdavis/homelab`                   | `homelab`                  | `h`                |
| 3    | `~/workspaces/Ivy`                                | `ivy`                      | `i`                |
| 4    | `~/workspaces/Ivy/Projects/Casually Concerned`    | `casually-concerned`       | `c`                |
| 5    | `~/.local/share/chezmoi`                          | `dotfiles`                 | `d`                |
| 6    | `~/.config/nvim`                                  | `nvim-config`              | `n`                |
| 7    | `~/workspaces/webdavis/essential-feed-case-study` | `essential-feed`           | `e`                |
| 8    | `~/workspaces/webdavis/webdavis`                  | `webdavis-profile`         | `g`                |
| 9    | `~/workspaces/webdavis/job-hunting`               | `job-hunting`              | `j`                |
| 10   | `~/workspaces/webdavis/justdavis-ansible`         | `justdavis-ansible`        | `k`                |
| 11   | `~/workspaces/webdavis/Maeve`                     | `maeve`                    | `m`                |

### Search Dirs Equivalent

tms uses depth-based `search_dirs` to discover projects. Sesh uses **zoxide** instead. Zoxide ranks by
frecency (frequency + recency), which is arguably better since it surfaces the projects you actually use
rather than scanning directory trees.

To prime zoxide with your search dirs:

```bash
# Add all directories from your tms search_dirs
find ~/.config -maxdepth 2 -type d | xargs -I {} zoxide add {}
find ~/workspaces -maxdepth 2 -type d | xargs -I {} zoxide add {}
find ~/workspaces/webdavis -maxdepth 2 -type d | xargs -I {} zoxide add {}
```

After initial seeding, zoxide learns from your `cd` usage automatically (since zoxide init is in your
bashrc).

### tms start Equivalent

tms `start` bootstraps all marked sessions at tmux startup. Sesh doesn't have a built-in equivalent, but
you can replicate it with a simple script:

```bash
#!/usr/bin/env bash
# sesh-bootstrap.sh -- create all configured sessions
while IFS= read -r session; do
  sesh connect "$session" 2>/dev/null &
done < <(sesh list -c 2>/dev/null)
wait
```

Or more explicitly:

```bash
#!/usr/bin/env bash
# Bootstrap all configured sessions
sessions=(
  "uriel"
  "openclaw"
  "homelab"
  "ivy"
  "casually-concerned"
  "dotfiles"
  "nvim-config"
  "essential-feed"
  "webdavis-profile"
  "job-hunting"
  "justdavis-ansible"
  "maeve"
)

for session in "${sessions[@]}"; do
  sesh connect "$session" &
done
wait
```

______________________________________________________________________

## 17. Recommended sesh.toml for Your Setup

```toml
# ~/.config/sesh/sesh.toml

# -- Global Settings -----------------------------------------------------------
cache = true
separator_aware = true
sort_order = ["tmux", "config", "zoxide"]
dir_length = 1

# Sessions you never want to see
blacklist = ["popup", "scratch"]

# -- Default Session -----------------------------------------------------------
[default_session]
startup_command = "nvim"
preview_command = "eza --tree --level=2 --icons --git-ignore {}"

# -- Configured Sessions (replaces tms marks + sessions) -----------------------

# Mark 0: uriel
[[session]]
name = "uriel"
path = "~/workspaces/webdavis/uriel"

# Mark 1: openclaw
[[session]]
name = "openclaw"
path = "~/.openclaw"

# Mark 2: homelab
[[session]]
name = "homelab"
path = "~/workspaces/webdavis/homelab"

# Mark 3: ivy
[[session]]
name = "ivy"
path = "~/workspaces/Ivy"

# Mark 4: casually-concerned
[[session]]
name = "casually-concerned"
path = "~/workspaces/Ivy/Projects/Casually Concerned"

# Mark 5: dotfiles
[[session]]
name = "dotfiles"
path = "~/.local/share/chezmoi"

# Mark 6: nvim config
[[session]]
name = "nvim-config"
path = "~/.config/nvim"

# Mark 7: essential-feed
[[session]]
name = "essential-feed"
path = "~/workspaces/webdavis/essential-feed-case-study"

# Mark 8: webdavis profile
[[session]]
name = "webdavis-profile"
path = "~/workspaces/webdavis/webdavis"

# Mark 9: job-hunting
[[session]]
name = "job-hunting"
path = "~/workspaces/webdavis/job-hunting"

# Mark 10: justdavis-ansible
[[session]]
name = "justdavis-ansible"
path = "~/workspaces/webdavis/justdavis-ansible"

# Mark 11: maeve
[[session]]
name = "maeve"
path = "~/workspaces/webdavis/Maeve"

# -- Wildcard Configs ----------------------------------------------------------

# All workspaces default to nvim with tree preview
[[wildcard]]
pattern = "~/workspaces/**"
startup_command = "nvim"
preview_command = "eza --tree --level=2 --icons --git-ignore {}"

# Config directories: just list, no editor
[[wildcard]]
pattern = "~/.config/*"
disable_startup_command = true
preview_command = "eza --tree --level=1 --icons {}"

# -- Reusable Window Definitions -----------------------------------------------

[[window]]
name = "editor"
startup_script = "nvim"

[[window]]
name = "server"
startup_script = "npm run dev"

[[window]]
name = "logs"
startup_script = "tail -f /tmp/dev.log"
```

______________________________________________________________________

## 18. Recommended tmux.conf Changes

Replace the tms section in `dot_tmux.conf` with:

```bash
# ┌ sesh (smart session manager) ───────────────────────────┐
# │                                                         │
# │  Ref: https://github.com/joshmedeski/sesh               │
# └─────────────────────────────────────────────────────────┘

# Don't exit tmux when closing the last session.
set -g detach-on-destroy off

# Skip confirmation on pane kill.
bind-key x kill-pane

# Fuzzy session picker (fzf popup with preview, source cycling, session killing).
bind-key -N "Sesh: Open session picker" o run-shell "sesh connect \"$(
  sesh list --icons | fzf-tmux -p 80%,70% \
  --no-sort --ansi --border-label ' sesh ' --prompt '  ' \
  --header ' ^a all ^t tmux ^g configs ^x zoxide ^d tmux kill ^f find' \
  --bind 'tab:down,btab:up' \
  --bind 'ctrl-a:change-prompt(  )+reload(sesh list --icons)' \
  --bind 'ctrl-t:change-prompt(  )+reload(sesh list -t --icons)' \
  --bind 'ctrl-g:change-prompt(  )+reload(sesh list -c --icons)' \
  --bind 'ctrl-x:change-prompt(  )+reload(sesh list -z --icons)' \
  --bind 'ctrl-f:change-prompt(  )+reload(fd -H -d 2 -t d -E .Trash . ~)' \
  --bind 'ctrl-d:execute(tmux kill-session -t {2..})+change-prompt(  )+reload(sesh list --icons)' \
  --preview-window 'right:55%' \
  --preview 'sesh preview {}'
)\""

# Window picker.
bind-key -N "Sesh: Window picker" C-w run-shell "sesh window \"$(
  sesh window | fzf-tmux -p 60%,50% --prompt '  '
)\""

# Last session toggle (survives session close and detach/reattach).
bind-key -N "Sesh: Toggle last session" L run-shell \
  "sesh last || tmux display-message -d 1000 'Only one session'"

# ┌ Sesh Quick-Access Mode (replaces tms marks) ───────────┐
# │                                                         │
# │  prefix + C-o  enters SESH mode                         │
# │  Then press a letter to jump to that session.           │
# └─────────────────────────────────────────────────────────┘
bind-key -N "Sesh Mode" C-o switch-client -T SESH

bind-key -N "Sesh: uriel"              -T SESH u run-shell "sesh connect uriel"
bind-key -N "Sesh: openclaw"           -T SESH o run-shell "sesh connect openclaw"
bind-key -N "Sesh: homelab"            -T SESH h run-shell "sesh connect homelab"
bind-key -N "Sesh: ivy"                -T SESH i run-shell "sesh connect ivy"
bind-key -N "Sesh: casually-concerned" -T SESH c run-shell "sesh connect casually-concerned"
bind-key -N "Sesh: dotfiles"           -T SESH d run-shell "sesh connect dotfiles"
bind-key -N "Sesh: nvim-config"        -T SESH n run-shell "sesh connect nvim-config"
bind-key -N "Sesh: essential-feed"     -T SESH e run-shell "sesh connect essential-feed"
bind-key -N "Sesh: webdavis-profile"   -T SESH g run-shell "sesh connect webdavis-profile"
bind-key -N "Sesh: job-hunting"        -T SESH j run-shell "sesh connect job-hunting"
bind-key -N "Sesh: justdavis-ansible"  -T SESH k run-shell "sesh connect justdavis-ansible"
bind-key -N "Sesh: maeve"              -T SESH m run-shell "sesh connect maeve"
```

Key differences from the tms version:

- Uses `run-shell "sesh connect <name>"` instead of `display-popup -E "tms marks open N"`
- `run-shell` is better than `display-popup -E` for direct session switching because it doesn't flash a
  popup. The popup is only useful for interactive pickers.
- `sesh last` replaces the need for tmux's built-in `switch-client -l` and works more reliably.

______________________________________________________________________

## 19. Recommended tmux-refresh.sh Rewrite

Your `tmux-refresh.sh` currently depends on `tms start`. Here is the equivalent for sesh:

Replace the `launch_tms_sessions` function with:

```bash
launch_sesh_sessions() {
  print_process "info" "Starting all sesh sessions from config..." false

  while IFS= read -r session; do
    sesh connect "$session" 2>/dev/null || true
  done < <(sesh list -c 2>/dev/null)

  print_process "success" " Done."
}
```

And update `verify_required_tools` to check for `sesh` instead of `tms`.

______________________________________________________________________

## 20. Sources

- [GitHub - joshmedeski/sesh](https://github.com/joshmedeski/sesh) -- Official repository and README
- [Smart tmux sessions with sesh | Josh Medeski](https://www.joshmedeski.com/posts/smart-tmux-sessions-with-sesh/)
  -- Primary blog post with config examples
- [I made my favorite tmux feature better with sesh | Josh Medeski](https://www.joshmedeski.com/posts/i-made-my-favorite-tmux-feature-better-with-sesh/)
  -- `sesh last` feature
- [DeepWiki - joshmedeski/sesh](https://deepwiki.com/joshmedeski/sesh) -- Architecture analysis
- [Raycast Store: Sesh](https://www.raycast.com/joshmedeski/sesh) -- Raycast extension
- [Why Developers Are Switching to Sesh | Buzzrag](https://buzzrag.com/article/why-developers-are-switching-to-sesh-for-tmux-sessions)
  -- Community perspective
- [Replacing Tmux-Sessionizer with Sesh | Podcast](https://rss.com/podcasts/linkarzu/2014230/) --
  Linkarzu podcast with Josh Medeski
- [sesh Discussion #230](https://github.com/joshmedeski/sesh/discussions/230) -- Performance comparison
  with t-smart-tmux-session-manager
- [sesh Releases](https://github.com/joshmedeski/sesh/releases) -- Release notes
- [sesh Go Package Docs](https://pkg.go.dev/github.com/joshmedeski/sesh/v2) -- Go API documentation
- Source code: `model/config.go`, `startup/startup.go`, `connector/connect.go`, `previewer/previewer.go`,
  `lister/config_wildcard.go`, `picker/picker.go`, `icon/icon.go`, `namer/namer.go` -- Read directly from
  the repository for schema accuracy
