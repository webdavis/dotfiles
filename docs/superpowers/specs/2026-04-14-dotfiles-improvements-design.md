# Dotfiles Improvements Design Spec

**Date:** 2026-04-14 **Scope:** Sub-projects 1 ("Fix & Tighten") and 2 ("New Tools & Automation") **Out
of scope:** Neovim overhaul (separate sub-project)

______________________________________________________________________

## 1. Atuin

### 1.1 Enable daemon mode

Set `daemon.enabled = true` in `dot_config/atuin/config.toml.tmpl`. The daemon decouples command
recording from PROMPT_COMMAND ordering, using a background process with a hot in-memory search index
(nucleo algorithm).

### 1.2 Fix filter and search modes

- `filter_mode = "host"` for CTRL-R (search this machine, not global)
- `filter_mode_shell_up_key_binding = "session"` (up-arrow searches current session only)
- `search_mode = "prefix"` as default (more predictable; fuzzy available via tab)
- `style = "compact"` for denser results

### 1.3 Migrate to Homebrew

- Add `atuin` to `system_packages_autoinstall.yaml` formulae list
- After brew install, remove `~/.atuin/bin/` (old curl-installed binary; history DB at
  `~/.local/share/atuin/` is untouched)
- Delete `run_once_before_30-install-atuin.sh.tmpl` from chezmoi

### 1.4 Diagnosis during implementation

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

- Remove the sdkman init block from `dot_bashrc.tmpl`
- Delete `~/.sdkman/` directory from disk

### 2.3 Remove skhd

- Remove `skhd --restart-service` reference from the system packages run script
- skhd has been replaced by AeroSpace

### 2.4 Remove bash history

Strip HISTFILE, HISTSIZE, HISTFILESIZE, histappend, and the HISTIGNORE/PROMPT_COMMAND history
flush/reload cycle from `dot_bashrc.tmpl`. Atuin daemon handles all history.

### 2.5 Init ordering comment

Add a comment noting that `atuin init bash` must come after `zoxide init bash` (both modify shell
bindings). This is for the keybinding, not recording (daemon handles recording).

### 2.6 Brew autoupdate idempotency

In the system packages run script, check if autoupdate is already running before restarting.

### 2.7 Bash completions

Add completions for `gh`, `docker`, `kubectl` to `dot_bash_completions`. Currently only SSH hosts are
completed.

### 2.8 Bash bindings fix

Fix em-dash encoding bug in eza commands (~line 96 of `dot_bash_bindings`). Replace em-dash with standard
double-dash.

### 2.9 Shell quality-of-life additions

Add to `dot_bashrc.tmpl`:

**Navigation aliases:**

- `..="cd .."`, `...="cd ../.."`, `....="cd ../../.."`

**Utility functions:**

- `mkd()` -- `mkdir -p "$@" && cd "$_"` (create and enter directory)
- `tmpd()` -- `cd "$(mktemp -d)"` (create and enter temp directory)
- `calc()` -- CLI calculator via `bc -l` (e.g., `calc "2^16"`)
- `timer` -- instant stopwatch: `alias timer='echo "Timer started. Stop with Ctrl-D." && time cat'`
- `getcertnames()` -- show SSL certificate CN and SANs for a domain (useful for HTTPS debugging)

**Network aliases:**

- `pubip='dig +short myip.opendns.com @resolver1.opendns.com'`

**Git safety function:**

- `gitsetoriginnopush()` -- sets push URL to `no_push` on forks to prevent accidental upstream pushes

### 2.10 macOS defaults script

New chezmoi `run_once` script: `.chezmoiscripts/run_once_before_05-macos-defaults.sh.tmpl`

Codifies macOS system preferences reproducibly:

- Disable smart quotes and smart dashes
- Fast key repeat rate and short initial delay
- Avoid creating `.DS_Store` on network and USB volumes
- Show hidden files in Finder
- Show all filename extensions
- Disable auto-correct
- Enable tap-to-click
- Set screenshot format to PNG
- Show path bar and status bar in Finder

Only runs on macOS (`{{ if eq .chezmoi.os "darwin" }}`). Runs once on first apply per machine.

______________________________________________________________________

## 3. Tmux changes

### 3.1 Tmux2k right plugins

Change `set-option -g @tmux2k-right-plugins "network battery cpu ram"` to `"network cpu-temp cpu ram"`.

### 3.2 Migrate tms to sesh

Replace the entire tms section in `dot_tmux.conf` with sesh integration:

- **Fuzzy session picker:** `prefix + o` opens fzf-tmux popup with source cycling (Ctrl+A all, Ctrl+T
  tmux, Ctrl+G configs, Ctrl+X zoxide, Ctrl+F find, Ctrl+D kill), previews via `sesh preview`
- **Window picker:** `prefix + C-w` opens fzf window picker via `sesh window`
- **Last session toggle:** `prefix + \` runs `sesh last` (replaces unused synchronize-panes binding;
  survives session close/detach)
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

- **`detach-on-destroy off`** so closing a session switches to the next one instead of exiting tmux

### 3.3 Remove tms references

- Remove tms keybindings, tms plugin reference, `TMUX_SESSIONIZER` key table
- Update `tmux-refresh.sh` to use `sesh list -c` + `sesh connect` loop instead of `tms start` (bootstrap
  only uriel, openclaw, homelab at startup)

### 3.4 Terminal overrides modernization

- `default-terminal "tmux-256color"` (replace `screen-256color`)
- `terminal-overrides ",xterm-ghostty:RGB"` (replace kitty override + `Tc`)
- Drop the kitty `Ss`/`Se` override (no longer using Kitty)

### 3.5 Extended keys

Enable `extended-keys on` and `extended-keys-format csi-u` so neovim/fzf receive modifier key combos
correctly (tmux 3.5+ feature).

### 3.6 Bug fix

Fix `aggressive-resize` (missing `on` value).

### 3.7 History limit

Raise `history-limit` from 10000 to 50000.

### 3.8 Plugins

- Add `tmux-autoreload` (`b0o/tmux-autoreload`) for auto-reload on config save
- Drop `tmux-copycat` (unmaintained; replicated by tmux 3.5+ built-in search and tmux-fuzzback)

______________________________________________________________________

## 4. Sesh configuration

### 4.1 New file: `dot_config/sesh/sesh.toml`

Chezmoi-managed.

**Global settings:**

```toml
cache = true
separator_aware = true
sort_order = ["tmux", "config", "zoxide"]
dir_length = 1
blacklist = ["popup", "scratch"]
```

### 4.2 Default session

Smart startup script — no auto-open editor/IDE. The `startup_command` calls
`~/.config/sesh/scripts/smart-startup.sh {}` which:

- Shows `git status -sb` under a styled "Git" header
- Shows Todoist tasks for the current session under a "Tasks" header. Uses section-based query:
  `td task list -f "/<session-name>" --limit 5` (sections match git repo names across all 5 Todoist
  projects: tech, karl, career, cc, life). Special case mapping in the script for session names that
  don't match sections (e.g., `casually-concerned` maps to `td task list --project "cc" --limit 5`).
  Skips silently if no matching section.
- Detects project type and shows concise info:
  - `justfile` -> `just --summary` formatted into columns
  - `Makefile` -> target names, capped at 10, columnar
  - `package.json` -> `jq -r '.scripts | keys[]'`, capped at 10
  - `Cargo.toml` -> package name + version
  - `pyproject.toml` -> project name + version
  - Fallback -> `eza --tree --level=1 --icons`
- All sections use ANSI-colored headers and separators
- Total output stays under ~20 lines
- Drops to shell after display

**Preview command:**

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

The bootstrap is called from bashrc's tmux auto-startup block (replacing the current tms logic). When a
new terminal opens and tmux isn't running, bashrc calls the bootstrap script to create the 3 default
sessions, then attaches. This preserves the current behavior of auto-dropping into tmux on terminal open.
The bootstrap script is also called from `tmux-refresh.sh` and the Claude Code LaunchAgent.

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

Chezmoi-managed. Option A coordination: worktrunk owns git worktree lifecycle, sesh owns tmux session
lifecycle. Worktrunk hooks do NOT create tmux sessions.

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
# Rename tmux window to repo/branch on switch for visual context.
# Rename tmux window on worktree switch. Shows repo name on default branch,
# branch name (truncated to 20 chars) on feature branches.
post-switch = """
if [ '{{ branch }}' = '{{ default_branch }}' ]; then
  tmux rename-window '{{ repo }}' 2>/dev/null
else
  tmux rename-window \"$(echo '{{ branch | sanitize }}' | cut -c1-20)\" 2>/dev/null
fi
"""

# Copy whitelisted ignored files to new worktrees.
[post-start]
copy = "wt step copy-ignored"

# Pre-merge validation gate (sequential, fast checks first).
[[pre-merge]]
lint = "just l 2>/dev/null || true"

[[pre-merge]]
test = "just test 2>/dev/null || true"

# Reset window name on removal.
pre-remove = "tmux rename-window '{{ repo }}' 2>/dev/null || true"
```

### 5.3 Aliases

```toml
[aliases]
up = "git fetch --all --prune && wt step for-each -- 'git rev-parse --verify -q @{u} >/dev/null || exit 0; g=$(git rev-parse --git-dir); test -d \"$g/rebase-merge\" -o -d \"$g/rebase-apply\" && exit 0; git rebase @{u} --no-autostash || git rebase --abort'"
```

### 5.4 Claude Code plugin

Run `wt config plugins claude install` during implementation for worktree isolation and activity
tracking.

### 5.5 Shell integration

Run `wt config shell install` during implementation.

______________________________________________________________________

## 6. Espanso migration and reorganization

### 6.1 Migrate to chezmoi

Move `~/Library/Application Support/espanso/` into chezmoi source state at
`Library/Application Support/espanso/`.

### 6.2 File reorganization (7 files)

| File                | Purpose                                                     |
| ------------------- | ----------------------------------------------------------- |
| `autocorrect.yml`   | Bare-word typo fixes (~200 entries)                         |
| `abbreviations.yml` | `;;` word shortcuts                                         |
| `formatting.yml`    | `,,` dates, symbols, formatting (merged from symbols.yml)   |
| `urls.yml`          | `,,` + 3-letter URL shortcuts (renamed from browser.yml)    |
| `identity.yml.tmpl` | `,,` email/name/contact (chezmoi template with KeePassXC)   |
| `prompts.yml`       | `;;` + full-word chatbot prompts (renamed from chatbot.yml) |
| `titles.yml`        | `;;` proper noun capitalization                             |

### 6.3 Files removed

- `_pqi.yml` (old job, healthcare/survey)
- `excel.yml` (only existed to load \_pqi in Excel)

### 6.4 Collision fixes

| Trigger                  | Fix                         |
| ------------------------ | --------------------------- |
| `;;ao` base vs browser   | browser -> `,,azo`          |
| `;;gh` titles vs browser | browser -> `,,ghu`          |
| `;;ed` base vs browser   | browser -> `,,efc`          |
| `;;con` x2 in base       | "conscientious" -> `;;cons` |
| `thye` x2 in base        | Remove duplicate            |

### 6.5 Redundancy cleanup

| Issue                                    | Fix              |
| ---------------------------------------- | ---------------- |
| `rn` (bare) + `;;rn` both -> "right now" | Remove bare `rn` |
| `;;et` + `;;evt` both -> "everything"    | Remove `;;evt`   |

### 6.6 Convention

Two prefixes only: `;;` and `,,`.

- `;;` + 2-3 letters: word abbreviations
- `;;` + full word: chatbot prompts (no collision with short triggers)
- `,,` + 2-3 letters: dates, symbols, formatting
- `,,` + 3 letters: URLs
- `,,` + descriptive: email/identity

### 6.7 Trigger migration

**URLs (browser.yml -> urls.yml):** all `;;` triggers become `,,` + 3 letters.

**Email/identity:** `;;em` -> `,,eml`, `;;ep` -> `,,epl`, `,,em` -> `,,gml`, `,,ep` -> `,,gpl`, `;;wa` ->
`,,wa`, `;;br` -> `,,br`, `;;sd` -> `,,sda`, `,,sd` -> `,,sdn`.

Remove old-job email templates: `;;ff`, `;;0s`, `,,0s`.

**Chatbot (chatbot.yml -> prompts.yml):** triggers stay as `;;` + word.

### 6.8 Sensitive data templating

`identity.yml.tmpl` uses KeePassXC for address, phone number. The credit card trigger (`02667`) is
removed entirely.

### 6.9 New triggers

| Trigger               | Expansion                                                                                                                                         |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| `,,iso`               | ISO date (2026-04-14)                                                                                                                             |
| `,,ts`                | Unix timestamp                                                                                                                                    |
| `,,cb`                | Triple backtick code block with cursor                                                                                                            |
| `;;review`            | "Review this code for bugs, security issues, and readability:"                                                                                    |
| `;;explain`           | "Explain this code step by step:"                                                                                                                 |
| `;;meta`              | "What questions am I not asking that I should be? Define those questions and then answer them."                                                   |
| `;;ty`                | "Thank you"                                                                                                                                       |
| `;;pls`               | "please"                                                                                                                                          |
| `,,sig`               | Full email signature block                                                                                                                        |
| `;;lgtm`              | "Looks good to me"                                                                                                                                |
| `;;wfm`               | "Works for me"                                                                                                                                    |
| `;;afaik`             | "As far as I know"                                                                                                                                |
| `,,tu`                | 👍                                                                                                                                                |
| `;;commits`           | "Each logical unit of work should be its own git commit. No Co-Authored-By lines."                                                                |
| `;;scan`              | "Scan this entire project -- read config files, playbooks, roles, variables, templates, scripts, and any documentation. Do NOT make any changes." |
| `;;specfirst`         | "READ docs/SPEC.md FIRST -- it is the complete technical specification. Follow it precisely."                                                     |
| `;;nochanges`         | "Do NOT make any changes."                                                                                                                        |
| `;;continue`          | "Continue where you left off. The previous model attempt failed or timed out."                                                                    |
| `;;discord`           | "Okay, respond in Discord from now on."                                                                                                           |
| `;;opentosuggestions` | "I'm open to suggestions."                                                                                                                        |
| `;;comprehensive`     | "Take a comprehensive look at this repo and the configuration of each tool."                                                                      |
| `;;reviewproject`     | "Review the current state of this project."                                                                                                       |
| `;;deepresearch`      | "Go do /deep-research on this."                                                                                                                   |
| `;;backup`            | "Make sure to back up my configs and keep working on it until it's up and running."                                                               |

______________________________________________________________________

## 7. Notifications

### 7.1 Long-running command notification (terminal-notifier)

Add to `dot_bashrc.tmpl`:

- `DEBUG` trap records command start time via `$SECONDS`
- On prompt return, check elapsed time
- If > 30 seconds: fire `terminal-notifier` with command name and elapsed time
- Skip if terminal pane is focused

### 7.2 Hue light pulse (10 minute threshold)

If elapsed > 10 minutes, additionally fire a Hue light pulse:

- Success (exit 0): green pulse at 50% brightness, 2 seconds
- Failure (non-zero): red pulse at 50% brightness, 2 seconds
- Save current light state, set color, sleep 2, restore

### 7.3 Hue pulse helper script

New file: `dot_local/bin/executable_hue-pulse.sh`

- Takes exit code as argument
- Uses `openhue` CLI (same API as existing `smart-lights` script)
- Extracts reusable light state save/restore logic from `smart-lights`
- Also add a `--pulse <color>` flag to `smart-lights` for reuse

### 7.4 GH workflow monitoring

Use `gh alias` (no custom script maintenance):

```bash
gh alias set --shell pushwatch '
  git push "$@"
  sleep 3
  run_id=$(gh run list -L 1 --json databaseId --jq ".[].databaseId")
  gh run watch "$run_id" --exit-status >/dev/null 2>&1
  ~/.local/bin/hue-pulse.sh $?
'
```

`gh pushwatch` pushes, watches the workflow, and pulses lights on completion.

______________________________________________________________________

## 8. Shell performance and cleanup

### 8.1 Lazy-load rbenv

Wrap in functions that self-replace on first call:

```bash
ruby()  { unset -f ruby gem rbenv; eval "$(rbenv init -)"; ruby "$@"; }
gem()   { unset -f ruby gem rbenv; eval "$(rbenv init -)"; gem "$@"; }
rbenv() { unset -f ruby gem rbenv; eval "$(rbenv init -)"; rbenv "$@"; }
```

### 8.2 Cargo PATH

Replace `source "$HOME/.cargo/env"` with direct PATH addition (avoids file read).

### 8.3 Karabiner cleanup

Remove commented-out/disabled rules from `dot_config/private_karabiner/private_karabiner.json`.

### 8.4 Delete empty install script

Delete `run_once_before_30-install-atuin.sh.tmpl`.

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
- `column.ui = auto`
- `transfer.fsckObjects = true`
- `pull.rebase = true` (consistent with existing `rebase.autosquash`)
- `help.autocorrect = 1` (auto-corrects typos like `git stauts` -> `git status`)

### 9.2 Consolidate on delta

Replace `diff-so-fancy` with `delta` in `core.pager`. Currently contradictory: `core.pager` uses
diff-so-fancy but `interactive.diffFilter` uses delta. Delta is superior (word-level diff, syntax
highlighting, side-by-side, file navigation).

### 9.3 Fix acp alias

Change `--force` to `--force-with-lease` in the `acp` git alias.

### 9.4 New git aliases

- `undo = reset --soft HEAD~1`
- `unstage = restore --staged`
- `recent = for-each-ref --sort=-committerdate --count=10 --format='%(refname:short)' refs/heads`
- `whoami = !git config --get user.name`
- `find-merge = "!sh -c 'commit=$0 && branch=${1:-HEAD} && (git rev-list $commit..$branch --ancestry-path | cat -n; git rev-list $commit..$branch --first-parent | cat -n) | sort -k2 -s | uniq -f1 -d | sort -n | tail -1 | cut -f2'"`
  (find which merge commit introduced a given commit)
- `show-merge = "!sh -c 'merge=$(git find-merge $0 $1) && [ -n \"$merge\" ] && git show $merge'"` (show
  the merge commit details)
- `pr = "!f() { git fetch -fu ${2:-origin} refs/pull/$1/head:pr/$1 && git checkout pr/$1; }; f"` (fetch
  and checkout a PR by number)
- `go = "!f() { git checkout -b \"$1\" 2>/dev/null || git checkout \"$1\"; }; f"` (create-or-switch
  branch)
- `dm = "!git branch --merged | grep -v '\\*' | xargs -n 1 git branch -d"` (delete all merged branches)
- `fb = "!f() { git branch -a --contains $1; }; f"` (find branches containing a commit)
- `fc = "!f() { git log --pretty=format:'%C(yellow)%h %Cblue%ad %Creset%s%Cgreen [%cn] %Cred%d' --decorate --date=short -S\"$1\"; }; f"`
  (find commits by code change)
- `fm = "!f() { git log --pretty=format:'%C(yellow)%h %Cblue%ad %Creset%s%Cgreen [%cn] %Cred%d' --decorate --date=short --grep=\"$1\"; }; f"`
  (find commits by message)

### 9.5 Inputrc fixes

- Enable `enable-bracketed-paste on` (security)
- Reduce `keyseq-timeout` from 1000 to 200ms (faster vi mode)
- Remove duplicate `show-mode-in-prompt` setting

### 9.6 Starship additions

- Add `nix_shell` module (visual feedback in Nix dev shells)
- Add `direnv` module (shows .envrc loaded/allowed state)
- Add `scan_timeout = 30` and `command_timeout = 500`
- Keep all language modules (needed for reading other repos)

### 9.7 Ghostty improvements

- `clipboard-read = ask` (security: prevents clipboard-sniffing)
- `clipboard-paste-protection = true`
- `shell-integration-features = cursor,sudo,title`
- `window-padding-x = 4`, `window-padding-y = 2`

### 9.8 Bat config

- `--style=numbers,changes,header,grid`
- `--map-syntax "*.tmpl:Bash"`
- `--map-syntax ".envrc:Bash"`
- `--map-syntax "justfile:Makefile"`
- `--pager="less -RFX --mouse"`

______________________________________________________________________

## 10. AI commit messages and local CI

### 10.1 DIY prepare-commit-msg hook

Chezmoi-managed at `dot_config/git/hooks/executable_prepare-commit-msg`. Uses Claude Code CLI in pipe
mode (`claude -p`) -- no API keys, no third-party tools, uses existing Claude Code subscription auth.

```bash
#!/usr/bin/env bash
# Skip for merge commits, amends, or messages passed via -m
[[ -n "$2" ]] && exit 0

diff=$(git diff --cached --diff-algorithm=histogram)
[[ -z "$diff" ]] && exit 0

msg=$(echo "$diff" | CLAUDECODE= MAX_THINKING_TOKENS=0 claude -p \
  --no-session-persistence --model=haiku --tools='' \
  --disable-slash-commands --setting-sources='' \
  --system-prompt='Write a conventional commit message (type: subject). One line, under 72 chars. No explanation.')

[[ -n "$msg" ]] && printf '%s\n\n' "$msg" > "$1"

# Chain to local repo hook if it exists
local_hook="$(git rev-parse --git-dir)/hooks/prepare-commit-msg"
[[ -x "$local_hook" ]] && exec "$local_hook" "$@"
```

Behavior: on `git commit`, reads staged diff, generates conventional commit message via Claude haiku in
\<1 second, prepopulates the editor. User approves, edits, or deletes and writes their own.

### 10.2 Global git hooks directory

Set `core.hooksPath = ~/.config/git/hooks` in gitconfig. The prepare-commit-msg hook and any future
global hooks live here. The hook chains to local `.git/hooks/prepare-commit-msg` if it exists, so
per-repo hooks (like the chezmoi pre-commit lint) still work.

### 10.3 actionlint

Install via brew. Static validator for GH Actions workflow YAML with shellcheck integration. Use before
pushing workflow changes.

### 10.4 act configuration and isolation

No global `~/.actrc` — different projects need different runner configurations. Each project that uses
`act` gets its own `.actrc` in the repo root (which `act` reads automatically). For this chezmoi dotfiles
project, add:

```
# .actrc (chezmoi dotfiles repo)
-P macos-latest=-self-hosted
```

For other projects with Linux workflows, their `.actrc` would contain:

```
-P ubuntu-latest=catthehacker/ubuntu:act-latest
--container-architecture linux/amd64
```

**IMPORTANT: `act -self-hosted` has zero isolation.** It runs directly on your Mac and can modify files,
install packages, and affect system state. Two isolation layers are provided:

**Layer 1: Agent Safehouse (process-level, daily use)**

Install: `brew install eugene1g/safehouse/agent-safehouse`

Usage: `safehouse --add-dirs="$PWD" act -P macos-latest=-self-hosted`

Wraps `act` in a macOS `sandbox-exec` deny-first profile that blocks access to everything outside the
repo directory. Prevents reading secrets, modifying dotfiles, or installing to unexpected locations.
Lightweight and fast.

**Layer 2: Tart VM (full VM isolation, untrusted workflows)**

Install: `brew install cirruslabs/cli/tart`

First-time setup (~25GB disk, user will free storage before this step):

```bash
tart clone ghcr.io/cirruslabs/macos-sequoia-base:latest sequoia-base
```

Per-run (instant APFS copy-on-write clone):

```bash
tart clone sequoia-base ci-run
tart run --dir=repo:$PWD ci-run &
# Wait for VM to boot, then:
tart exec ci-run -- bash -c "cd /Volumes/My\ Shared\ Files/repo && act -P macos-latest=-self-hosted"
tart delete ci-run
```

Full VM isolation via Apple Virtualization.framework. Near-native performance. Free and open source
(Apache 2.0). Max 2 concurrent macOS VMs per host (Apple licensing).

**When to use which:**

- Linux workflows: `act` (Docker containers, isolated by default)
- macOS workflows (trusted, your own repos): `safehouse act -P macos-latest=-self-hosted`
- macOS workflows (untrusted or sensitive): Tart VM

______________________________________________________________________

## 11. Package changes

### 11.1 Add to brew formulae

- `sesh`
- `worktrunk`
- `csvlens`
- `atuin` (migrating from curl install)
- `actionlint`
- `bat-extras` (batman, batgrep, batdiff)
- `hyperfine` (benchmarking)
- `gitleaks` (pre-commit secret scanning)
- `tart` (via `cirruslabs/cli/tart` tap)

### 11.2 Add to brew taps

- `eugene1g/safehouse` (for agent-safehouse)
- `cirruslabs/cli` (for tart)

### 11.3 Add to brew formulae (from taps)

- `agent-safehouse` (from eugene1g/safehouse)

### 11.5 Remove from brew formulae

- `diff-so-fancy` (replaced by delta)

### 11.6 Remove from system

- `tms` (check if brew or cargo install; replaced by sesh)
- `~/.sdkman/` (entire directory)
- `~/.atuin/bin/` (old curl-installed binary)

______________________________________________________________________

## 12. Claude Code improvements

### 12.1 Template the settings file

Convert `dot_claude/settings.json` to `dot_claude/settings.json.tmpl` for path interpolation across
machines (e.g., `{{ .chezmoi.homeDir }}`).

### 12.2 Add settings.local.json

Create `dot_claude/settings.local.json.tmpl` as a placeholder for machine-specific overrides. Claude Code
merges this with `settings.json`, with local taking precedence.

### 12.3 Add deny list

Even with `bypassPermissions`, add explicit denies as a safety net:

- `.env` and `.env.*`
- `secrets/**`
- `credentials.json`
- `.aws/credentials`
- `.ssh/id_*`

### 12.4 Stop hook for Hue lights

Add a Claude Code Stop hook that fires `hue-pulse.sh` when Claude finishes a long task. Reuses the same
`hue-pulse.sh` helper from Section 7.3:

```json
{
  "hooks": {
    "Stop": [{ "hooks": [{ "type": "command", "command": "~/.local/bin/hue-pulse.sh 0" }] }]
  }
}
```

Green pulse when Claude stops (signals "I'm done, come check my work").

### 12.5 Preserve conversation history

Set `cleanupPeriodDays: 36525` (~100 years) to effectively disable automatic cleanup of Claude Code
conversation data.

### 12.6 CLAUDE.md evergreen directive

Add to the top of CLAUDE.md:

> Avoid adding point-in-time content (current sprint goals, active branches, temporary workarounds) that
> wouldn't make sense if multiple workstreams, PRs, or branches were in progress simultaneously. Document
> general principles, workflows, and architecture -- not transient project state.

### 12.7 Global agents directory

Create `private_dot_claude/agents/` in chezmoi to manage reusable Claude Code agent definitions.
Initially empty -- agents added as workflows emerge. The directory structure ensures agents follow the
user across machines.

### 12.8 Claude Code Review GitHub Action

Add `.github/workflows/claude-code-review.yml` for automated PR reviews on every push. Uses
`anthropics/claude-code-action@v1` with:

- Triggers on PR opened/synchronize
- Sticky comment (updates same comment on new pushes, avoids spam)
- Custom prompt for code quality, bugs, performance, security, test coverage
- Restricted tool access (read-only gh commands + pr comment)
- Requires `CLAUDE_CODE_OAUTH_TOKEN` repository secret

### 12.9 `/pr-merge` slash command

Create a global slash command at `private_dot_claude/commands/pr-merge.md` that:

- Squash merges the current PR via `gh pr merge --squash`
- Switches to main branch
- Pulls latest
- Deletes the local feature branch

High-frequency time saver for the merge-and-cleanup workflow.

### 12.10 Notification hook

Add a Claude Code Notification hook alongside the Stop hook. Fires when Claude needs input (permission
prompts, idle waiting). Uses `terminal-notifier` to alert:

```json
{
  "hooks": {
    "Stop": [{ "hooks": [{ "type": "command", "command": "~/.local/bin/hue-pulse.sh 0" }] }],
    "Notification": [{ "hooks": [{ "type": "command", "command": "terminal-notifier -title 'Claude Code' -message 'Needs attention' -sound default" }] }]
  }
}
```

### 12.11 Enable alwaysThinkingEnabled

Set `alwaysThinkingEnabled: true` in Claude Code settings to force extended thinking on every prompt.

### 12.12 gh credential helper

Add `gh auth git-credential` as the HTTPS credential helper for github.com and gist.github.com. Uses the
`helper = ` (empty) pattern to clear any prior helper before setting `gh`:

```gitconfig
[credential "https://github.com"]
    helper =
    helper = !/opt/homebrew/bin/gh auth git-credential
[credential "https://gist.github.com"]
    helper =
    helper = !/opt/homebrew/bin/gh auth git-credential
```

Complements the existing SSH URL rewrites — provides clean HTTPS auth for `gh` operations and any HTTPS
git interactions.

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
