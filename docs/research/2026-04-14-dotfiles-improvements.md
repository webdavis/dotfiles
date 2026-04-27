# Dotfiles Improvements Research Report

Date: 2026-04-14 Current setup baseline: macOS, chezmoi 2.70.1, tmux 3.6a, git 2.53.0, bash 5.3.9,
Ghostty 1.3.1, starship 1.24.2, fzf 0.71.0, atuin 18.13.3, bat 0.26.1

______________________________________________________________________

## 1. Tmux Improvements

### 1.1 Enable tmux 3.5/3.6 Features

Your tmux is 3.6a but your config does not use several features introduced in 3.5 and 3.6.

**Extended keys (high value).** Enable this so applications (neovim, fzf) receive modifier key combos
that tmux previously swallowed:

```tmux
set-option -g extended-keys on
set-option -g extended-keys-format csi-u
```

If you find some tools misbehave, change `on` to `always` to force-send even when apps do not request
them.

**Theme detection (3.6).** Tmux 3.6 added mode 2031 support to automatically report dark/light theme to
applications. If your Ghostty or neovim supports `COLORFGBG` or OSC 11, tmux will now pass this through
correctly. No config needed, but be aware it exists if you add light theme support.

**Terminal overrides cleanup.** Your current config sets:

```tmux
set-option -g default-terminal "screen-256color"
set-option -ga terminal-overrides ",screen-256color:Tc"
set-option -ga terminal-overrides ",xterm-kitty:Ss=\E[%p1%d q:Se=\E[2 q"
```

Since you use Ghostty (which sets `xterm-ghostty`), update to:

```tmux
set-option -g default-terminal "tmux-256color"
set-option -ga terminal-overrides ",xterm-ghostty:RGB"
```

`tmux-256color` is the correct value for tmux 3.x. `RGB` is preferred over `Tc` (same effect, newer
name). Drop the kitty override since you no longer use Kitty.

**History limit.** You have `history-limit 10000`. Consider raising to `50000` -- with modern memory,
this is negligible and prevents losing scrollback in long sessions. Ghostty also has its own scrollback,
but tmux's matters for copy-mode.

### 1.2 Theme: tmux2k vs. Alternatives

tmux2k is still actively maintained and works well. However, the **catppuccin/tmux** plugin (v2.3.0) has
become the dominant tmux theme in 2025-2026. Since you already use catppuccin palette colors in tmux2k
(`@tmux2k-theme "catppuccin"`), fzf, and likely neovim, switching to the native catppuccin/tmux plugin
would give you:

- First-class Mocha/Macchiato/Frappe/Latte support
- Modular status bar (session, cpu, ram, battery, git, date modules)
- Active upstream with frequent updates
- Built-in support for the same monitoring widgets you get from tmux2k

**Verdict:** Not urgent. tmux2k with catppuccin colors works. But if you ever feel like simplifying,
catppuccin/tmux is the natural successor and has a larger community.

### 1.3 New Plugins Worth Adding

**tmux-autoreload** (`b0o/tmux-autoreload`): Watches `.tmux.conf` and auto-reloads on save. Eliminates
the manual `prefix + R` cycle while iterating on config. Low risk, high convenience.

```tmux
set-option -g @plugin 'b0o/tmux-autoreload'
```

**tmux-notify** (`ChanderG/tmux-notify`): Sends a macOS notification when a long-running process in a
background pane finishes. Useful if you run builds or tests in a split pane.

**Plugins to potentially drop:**

- **tmux-copycat**: This plugin is essentially unmaintained and its regex-search features are now partly
  replicated by tmux's built-in copy-mode search (improved in 3.5+) and by tmux-fuzzback. Consider
  removing it if you do not actively use its pattern shortcuts.

### 1.4 Minor Config Fixes

Your `aggressive-resize` line is missing a value:

```tmux
# Current (missing value):
set-option -g aggressive-resize

# Fix:
set-option -g aggressive-resize on
```

______________________________________________________________________

## 2. Bash Improvements

### 2.1 Bash 5.3 Features to Adopt

You are on Bash 5.3.9, which includes significant new features.

**Forkless command substitution.** The new `${ cmd; }` syntax runs the command in the current shell
context (no subshell fork). For functions that capture output, this can be measurably faster:

```bash
# Old (forks a subshell):
result=$(some_function)

# New bash 5.3 (no fork):
result=${ some_function; }
```

This is most useful in heavily-called functions in your prompt or bindings. Not worth refactoring
everything, but good for new code.

**GLOBSORT variable.** Control how pathname expansion results are sorted:

```bash
# Sort glob results by modification time (newest first):
GLOBSORT='-mtime'

# Sort by size:
GLOBSORT='size'
```

Useful in scripts where you `for f in *.log` and want newest-first ordering without piping through
`ls -t`.

**`source --path` flag.** The `source` (`.`) builtin now accepts `--path PATH` to specify where to look
for sourced files, replacing ad-hoc path construction.

**`read -E` option.** Allows using Readline for input with tab completion in `read` prompts. Useful for
interactive scripts.

### 2.2 Shell Init Performance

Your `.bashrc` runs many `eval "$(tool init bash)"` calls. Each is a subshell + command execution. On
Bash 5.3, you could cache the output:

```bash
# Example: cache starship init (regenerate when starship binary changes)
_starship_cache="$HOME/.cache/starship_init.bash"
if [[ ! -f "$_starship_cache" ]] || [[ "$(command -v starship)" -nt "$_starship_cache" ]]; then
  starship init bash > "$_starship_cache"
fi
source "$_starship_cache"
```

This saves ~50-100ms per `eval` call on cold starts. Whether it is worth the complexity depends on
whether shell startup time bothers you. Your current approach is more maintainable.

### 2.3 Readline / inputrc Improvements

Your inputrc is comprehensive. One addition worth considering:

```inputrc
# Enable bracketed paste (prevents pasted code from executing):
set enable-bracketed-paste on
```

You currently have this set to `off`. Bracketed paste prevents pasted text from being interpreted as
editing commands, which is a security improvement. Modern terminals (including Ghostty) support it. The
main reason to keep it off is if you paste into vi-mode and want immediate execution, but the security
benefit outweighs this.

______________________________________________________________________

## 3. Starship Prompt

### 3.1 Missing Modules Worth Adding

**Nix shell module.** You use Nix flakes and direnv. Add the `nix_shell` module to see when you are
inside a `nix develop` environment:

```toml
[nix_shell]
disabled = false
symbol = " "
style = "fg:#7ebae4 bg:#212736"
format = '[[](fg:prev_bg bg:#212736) $symbol($state )($name)]($style)'
```

**Direnv module.** Shows whether the current directory's `.envrc` is loaded/allowed:

```toml
[direnv]
disabled = false
style = "fg:#b4befe bg:#212736"
format = '[[](fg:prev_bg bg:#212736) direnv ($loaded/$allowed)]($style)'
```

**Container module.** If you work with Docker containers:

```toml
[container]
disabled = false
style = "fg:#89b4fa bg:#212736"
format = '[[](fg:prev_bg bg:#212736) $symbol $name]($style)'
```

### 3.2 Performance Optimization

Use `starship timings` to identify slow modules. Common optimizations:

```toml
# Add scan_timeout to prevent slow scans in large repos:
scan_timeout = 30

# Add command_timeout for external commands:
command_timeout = 500
```

Place rarely-used language modules (Haskell, Scala, PHP) at the bottom of your format string, since
modules are evaluated in order.

### 3.3 Mosh Configuration

You already have `starship-mosh.toml` for Mosh sessions. Make sure it has aggressive timeouts since Mosh
connections can stall on slow commands:

```toml
command_timeout = 200
scan_timeout = 10
```

______________________________________________________________________

## 4. Git Config Improvements

### 4.1 Modern Settings from Core Git Developers

These are settings that Git core developers use themselves but are not yet defaults. Add to your
`dot_gitconfig.tmpl`:

```gitconfig
[column]
  ui = auto

[branch]
  sort = -committerdate

[tag]
  sort = version:refname

[fetch]
  prune = true
  pruneTags = true
  writeCommitGraph = true

[push]
  followTags = true

[diff]
  algorithm = histogram
  mnemonicPrefix = true
  renames = true

[commit]
  verbose = true

[rebase]
  updateRefs = true

[merge]
  conflictstyle = zdiff3

[transfer]
  fsckObjects = true

[receive]
  fsckObjects = true
```

Key explanations:

- **column.ui = auto**: Formats branch/tag listings into columns.
- **branch.sort = -committerdate**: Most recently active branches first.
- **fetch.prune = true**: Automatically removes stale remote-tracking branches.
- **fetch.writeCommitGraph = true**: Writes commit-graph after fetch for faster log traversal.
- **diff.algorithm = histogram**: Better diffs than the default myers algorithm. Recommended by Git core
  developers.
- **diff.mnemonicPrefix = true**: Shows `i/` (index), `w/` (working tree), `c/` (commit) instead of `a/`
  and `b/` prefixes.
- **commit.verbose = true**: Shows the diff in your commit message editor. Helps write better commit
  messages.
- **rebase.updateRefs = true**: When rebasing stacked branches, automatically updates all intermediate
  branch pointers. Huge quality-of-life for stacked PRs.
- **merge.conflictstyle = zdiff3**: You have `diff3`; upgrade to `zdiff3` (available since Git 2.35). It
  is diff3 but with common unchanged lines in the conflict region removed, making conflict markers much
  cleaner.
- **transfer.fsckObjects = true**: Validates objects on push/fetch. Catches corruption early.

### 4.2 Diff Pager: Consolidate on Delta

You currently have both `diff-so-fancy` and `delta` configured:

```gitconfig
[core]
  pager = diff-so-fancy | less --tabs=4 -RFX

[interactive]
  diffFilter = delta --color-only

[delta]
  features = custom
```

This is contradictory: `core.pager` uses diff-so-fancy but `interactive.diffFilter` uses delta. Delta is
superior in 2026:

- Levenshtein-based word-level diff highlighting (more accurate than diff-so-fancy)
- Side-by-side view
- Line numbers
- Syntax highlighting
- `n`/`N` file navigation
- Can emulate diff-so-fancy style if you prefer the look

**Recommendation:** Consolidate on delta:

```gitconfig
[core]
  pager = delta

[interactive]
  diffFilter = delta --color-only

[delta]
  features = custom
  navigate = true
  line-numbers = true
  side-by-side = false
```

You can then remove `diff-so-fancy` from your Brewfile if nothing else uses it.

### 4.3 Background Maintenance

Enable `git maintenance` for your frequently-used repositories. This runs gc, commit-graph writes, and
prefetches in the background via launchd:

```bash
# Run this in each large repo:
cd ~/Projects/uriel && git maintenance start
cd ~/Projects/openclaw && git maintenance start
```

This registers the repo with macOS launchd to run hourly background maintenance. It replaces the
disruptive `gc --auto` that sometimes freezes your terminal.

### 4.4 Aliases Cleanup

Your `acp` alias uses `--force` push, which is dangerous:

```gitconfig
acp = "!acp() { git add ${@} && git commit --amend --no-edit && git push --force; }; acp"
```

Consider adding `--force-with-lease` instead:

```gitconfig
acp = "!acp() { git add ${@} && git commit --amend --no-edit && git push --force-with-lease; }; acp"
```

`--force-with-lease` refuses to push if the remote has commits you have not fetched, preventing you from
accidentally overwriting a collaborator's work.

______________________________________________________________________

## 5. Ghostty Terminal Improvements

### 5.1 New Features to Enable (Ghostty 1.2-1.3)

**Scrollback search** (1.3): Already available via `Cmd+F`. No config needed.

**Quick terminal sizing** (1.2): Configure the size of the quick terminal (toggled via
`ctrl+grave_accent`):

```
quick-terminal-size = 80%,50%
```

**Shell integration features.** Ghostty auto-injects shell integration for bash. Verify you have these
features active:

```
shell-integration = detect
shell-integration-features = cursor,sudo,title
```

The `sudo` feature preserves Ghostty integration inside sudo sessions. The `title` feature sets the
terminal title based on the running command.

**Background blur** (macOS 26+): If you want a modern translucent look:

```
background-opacity = 0.92
background-blur = 20
```

### 5.2 Font Configuration

You use Menlo at size 13. You have Fira Code Nerd Font installed (in your Brewfile). If you want
ligatures and nerd font icons natively:

```
font-family = "FiraCode Nerd Font"
font-size = 13
font-thicken = true
font-feature = -liga    # Keep this if you prefer no ligatures
```

### 5.3 Clipboard Security

Add explicit clipboard policy:

```
clipboard-read = ask
clipboard-write = allow
clipboard-paste-protection = true
```

`clipboard-read = ask` prompts you before any program reads your clipboard (prevents clipboard-sniffing
attacks). `clipboard-paste-protection` warns before pasting text that contains potentially dangerous
characters (newlines that could execute commands).

### 5.4 Window Padding

Add some breathing room:

```
window-padding-x = 4
window-padding-y = 2
window-padding-balance = true
```

______________________________________________________________________

## 6. AeroSpace Improvements

### 6.1 Workspace-to-Monitor Assignment

If you use multiple monitors, pin workspaces to specific displays:

```toml
[workspace-to-monitor-force-assignment]
"1:Main" = 'main'
"2:Work" = 'main'
"3:Finances" = 'secondary'
"4:Relax" = 'secondary'
```

### 6.2 Focus-Follows-Mouse

You already have `on-focused-monitor-changed = ['move-mouse monitor-lazy-center']`. Consider also adding
focus-follows-mouse for within-monitor navigation if you use mouse alongside keyboard:

```toml
# Uncomment if you want focus-follows-mouse:
# on-focus-changed = ['move-mouse window-lazy-center']
```

### 6.3 Window Detection Rules

Add rules for common apps you likely use:

```toml
[[on-window-detected]]
    if.app-id = 'com.apple.finder'
    check-further-callbacks = true
    run = ['layout floating']

[[on-window-detected]]
    if.app-id = 'com.apple.ActivityMonitor'
    check-further-callbacks = true
    run = ['layout floating']

[[on-window-detected]]
    if.app-id = 'com.mitchellh.ghostty'
    check-further-callbacks = true
    run = ['move-node-to-workspace "1:Main"']

[[on-window-detected]]
    if.app-id = 'company.thebrowser.Browser'  # Arc
    check-further-callbacks = true
    run = ['move-node-to-workspace "2:Work"']
```

### 6.4 Sticky Windows

Sticky windows (windows visible on all workspaces) are still not implemented as of April 2026. This is
tracked as [issue #2](https://github.com/nikitabobko/AeroSpace/issues/2). No workaround available.

______________________________________________________________________

## 7. Bat Improvements

### 7.1 Configuration Enhancements

Update your bat config:

```
--theme="Catppuccin Mocha"
--italic-text=always
--style=numbers,changes,header,grid
--map-syntax "*.tmpl:Bash"
--map-syntax "*.conf:INI"
--map-syntax ".envrc:Bash"
--map-syntax "justfile:Makefile"
--pager="less --RAW-CONTROL-CHARS --quit-if-one-screen --mouse"
```

Key additions:

- **Catppuccin Mocha theme**: Matches your overall color scheme. Install from
  [catppuccin/bat](https://github.com/catppuccin/bat).
- **--style=numbers,changes,header,grid**: Shows line numbers, git change markers, filename headers, and
  grid lines. More informative than the default.
- **--map-syntax for tmpl files**: Your `.tmpl` files will get bash highlighting instead of plain text.
- **--pager with mouse support**: Enables mouse scrolling in bat output within tmux.

### 7.2 Shell Integration

Set `BAT_PAGER` in your bashrc to decouple bat's pager from the global `$PAGER`:

```bash
export BAT_PAGER="less -RFX --mouse"
```

Use `batgrep` (from `bat-extras`) for syntax-highlighted grep results. Install via:

```bash
brew install bat-extras
```

This gives you `batgrep`, `batdiff`, `batman` (manpage viewer with bat), and `batpipe`.

______________________________________________________________________

## 8. Security Hardening

### 8.1 Pre-commit Secret Scanning

Add Gitleaks as a pre-commit hook. It scans staged changes in milliseconds and blocks commits containing
secrets (API keys, tokens, passwords):

```bash
brew install gitleaks
```

Add to your existing pre-commit hook (or create a new one):

```bash
# In .git/hooks/pre-commit or via your justfile hook:
gitleaks git --staged --no-banner
```

For your chezmoi repo specifically, create a `.gitleaks.toml` to allowlist your `.tmpl` files that
contain `keepassxc` template calls (which look like secrets but are not):

```toml
[allowlist]
  paths = [
    '''.*\.tmpl$'''
  ]
```

### 8.2 Bracketed Paste (inputrc)

As noted in Section 2.3, enable `enable-bracketed-paste on` in your inputrc. This prevents pasted text
from being interpreted as commands, which is a common attack vector.

### 8.3 SSH Key Rotation

Review your `private_dot_ssh` directory. If any keys are RSA, consider rotating to Ed25519:

```bash
ssh-keygen -t ed25519 -C "your-email@example.com"
```

### 8.4 GPG Subkey Expiration

Your git config references a GPG signing subkey. Ensure your subkeys have expiration dates set (1-2
years). Unexpiring keys are a risk if compromised. Check with:

```bash
gpg --list-keys --keyid-format long
```

### 8.5 Chezmoi Encryption

Consider using `age` encryption (built into chezmoi 2.70+) for files that currently rely solely on
`.gitignore` for protection. Age encryption means even if someone gains access to your source state,
encrypted files remain protected:

```toml
# In .chezmoi.toml.tmpl:
[age]
  identity = "~/.config/chezmoi/key.txt"
  recipient = "age1..."
```

Files prefixed with `encrypted_` in the source state will be decrypted on apply. This is complementary to
your KeePassXC integration -- use KeePassXC for secrets injected into templates, and age for entire files
that should be encrypted at rest.

______________________________________________________________________

## 9. Chezmoi Improvements

### 9.1 Hooks (Now Stable)

Chezmoi 2.46+ promoted hooks to stable. You can run commands before/after specific chezmoi operations:

```toml
# In .chezmoi.toml.tmpl:
[hooks.apply.pre]
  command = "echo"
  args = ["Applying chezmoi changes..."]

[hooks.apply.post]
  command = "terminal-notifier"
  args = ["-title", "Chezmoi", "-message", "Apply complete"]
```

### 9.2 External Resources via .chezmoiexternal

If you are not already using `.chezmoiexternal.toml`, it can replace manual plugin management. For
example, managing TPM and tmux plugins:

```toml
[".tmux/plugins/tpm"]
    type = "git-repo"
    url = "https://github.com/tmux-plugins/tpm.git"
    refreshPeriod = "168h"
```

This is cleaner than the `if-shell` auto-install block in your tmux.conf. Chezmoi handles cloning and
updating.

### 9.3 Multiple Externals to Same Target

Chezmoi 2.70 added support for multiple externals to the same target in one `.chezmoiexternal` file. This
simplifies managing plugin directories.

### 9.4 Re-add --recursive

Chezmoi 2.46+ added `chezmoi re-add --recursive`, which re-adds all managed files in a directory. Useful
after bulk-editing target files.

### 9.5 Template Optimization

Cache expensive template function calls. In your `dot_bashrc.tmpl`, the `keepassxc` call happens at apply
time, but if you have multiple templates calling the same entry, consider using `$variables` to avoid
repeated calls:

```
{{ $histignore := (keepassxc "Dotfiles (bashrc) :: HISTIGNORE Regex").Password -}}
export HISTIGNORE="$HISTIGNORE:{{ $histignore }}"
```

______________________________________________________________________

## 10. Missing Tools

### 10.1 High-Value Additions

**bat-extras** (formula: `bat-extras`): Provides `batgrep`, `batdiff`, `batman`, `batpipe`. `batman`
replaces your `MANPAGER="nvim +Man!"` with a faster, syntax-highlighted pager that does not require
loading neovim. `batgrep` combines ripgrep + bat for highlighted search results.

**hyperfine** (formula: `hyperfine`): Benchmarking tool. Useful for comparing shell init times, script
performance, etc. Made by the same author as `bat` and `fd`.

```bash
# Example: benchmark shell startup
hyperfine --warmup 3 'bash -i -c exit'
```

**gitleaks** (formula: `gitleaks`): Secret scanner for git repos. See Section 8.1.

**doggo** or **dog** (formula: `doggo`): Modern DNS lookup tool. Replaces `dig` with colored,
human-readable output.

**bandwhich** (formula: `bandwhich`): Real-time network bandwidth monitoring by process. Shows which
processes are using network and how much, unlike `iftop` which only shows connections.

**ouch** (formula: `ouch`): Universal archive decompressor. Handles tar, zip, 7z, gz, bz2, xz, zstd, etc.
with a single command: `ouch decompress file.tar.gz`.

### 10.2 Tools You Already Have but May Be Underusing

**eza**: You have it installed and use it in bash bindings. Consider adding `--git` flag to your eza
aliases to show git status per file:

```bash
alias ll='eza -ahlrs date --git --icons'
```

**fd**: You have it installed. Make sure fzf uses it as its default command (faster than rg for file
listing):

```bash
export FZF_DEFAULT_COMMAND='fd --type f --hidden --follow --exclude .git'
export FZF_CTRL_T_COMMAND="$FZF_DEFAULT_COMMAND"
export FZF_ALT_C_COMMAND='fd --type d --hidden --follow --exclude .git'
```

**procs**: You have it installed. Consider aliasing: `alias ps='procs'`

**dust**: You have it installed. Consider aliasing: `alias du='dust'`

### 10.3 Tools to Skip

- **lazygit**: User has explicitly declined this.
- **yazi**: User has explicitly declined this.
- **mise**: User has explicitly declined this.
- **Zellij**: User is committed to tmux.

______________________________________________________________________

## 11. Atuin Improvements

### 11.1 Enable Daemon Mode

Your current config has `daemon.enabled = false`. The daemon has stabilized significantly in 18.13.
Benefits:

- Hot in-memory search index (faster than SQLite queries)
- Uses nucleo (same algorithm as fzf) for search
- Syncs records in background so remote machines stay current

```toml
[daemon]
enabled = true
```

### 11.2 Daemon-Fuzzy Search

The new `daemon-fuzzy` search mode is faster and more accurate than the default fuzzy search:

```toml
search_mode = "fuzzy"
search_mode_shell_up_key_binding = "fuzzy"
```

With the daemon enabled, fuzzy search uses the in-memory index automatically.

### 11.3 AI Suggestions

You have `ai.enabled = true`. To use it, press `?` on an empty prompt, type a natural language
description, and press Enter to execute or Tab to edit. This is functional as of 18.13.

______________________________________________________________________

## Summary: Priority-Ranked Action Items

### High Priority (immediate improvements, low effort)

1. **Git config modernization**: Add `column.ui`, `branch.sort`, `fetch.prune`,
   `diff.algorithm = histogram`, `merge.conflictstyle = zdiff3`, `rebase.updateRefs`, `commit.verbose`
   (Section 4.1)
1. **Consolidate on delta** over diff-so-fancy (Section 4.2)
1. **Tmux terminal overrides**: Switch to `tmux-256color` and `xterm-ghostty:RGB` (Section 1.1)
1. **Enable tmux extended-keys** (Section 1.1)
1. **Enable bracketed paste** in inputrc (Section 2.3 / 8.2)
1. **Ghostty clipboard security**: Add `clipboard-read = ask` (Section 5.3)
1. **Fix tmux aggressive-resize** missing value (Section 1.4)
1. **Use --force-with-lease** instead of --force in git acp alias (Section 4.4)

### Medium Priority (meaningful improvements, moderate effort)

9. **Add starship nix_shell and direnv modules** (Section 3.1)
1. **Bat config overhaul**: Catppuccin theme, syntax mappings for .tmpl files, style options (Section
   7.1)
1. **Enable Atuin daemon mode** (Section 11.1)
1. **Install gitleaks** for pre-commit secret scanning (Section 8.1)
1. **Install bat-extras** for batman, batgrep (Section 10.1)
1. **Install hyperfine** for benchmarking (Section 10.1)
1. **Use fd as FZF_DEFAULT_COMMAND** instead of rg (Section 10.2)
1. **tmux-autoreload plugin** (Section 1.3)
1. **Ghostty quick-terminal-size** and shell-integration-features (Section 5.1)
1. **Chezmoi .chezmoiexternal** for TPM management (Section 9.2)

### Low Priority (nice to have, or higher effort)

19. **Consider dropping tmux-copycat** (Section 1.3)
01. **Raise tmux history-limit** to 50000 (Section 1.1)
01. **Bash 5.3 forkless substitution** in new code (Section 2.1)
01. **Cache shell init evals** for faster startup (Section 2.2)
01. **AeroSpace workspace-to-monitor assignment** if multi-monitor (Section 6.1)
01. **AeroSpace app-to-workspace rules** (Section 6.3)
01. **Chezmoi hooks** for apply notifications (Section 9.1)
01. **Age encryption** for sensitive files (Section 8.5)
01. **git maintenance start** on large repos (Section 4.3)
01. **Ghostty background blur** for aesthetic (Section 5.1)
01. **Install bandwhich, doggo, ouch** (Section 10.1)

______________________________________________________________________

## Sources

- [tmux 3.6 CHANGES](https://raw.githubusercontent.com/tmux/tmux/3.6/CHANGES)
- [tmux 3.5 new features](https://linuxiac.com/tmux-3-5-terminal-multiplexer/)
- [How Core Git Developers Configure Git](https://blog.gitbutler.com/how-git-core-devs-configure-git)
- [New gitconfig for 2025 (ekzhang)](https://gist.github.com/ekzhang/ed9e4b36cee96f9431deaeeb342a31f7)
- [Optimizing Your Git Config](https://weirdion.com/posts/2025-04-12-optimizing-your-git-config-my-developer-setup/)
- [Git 2.52 release](https://about.gitlab.com/blog/whats-new-in-git-2-52-0/)
- [Ghostty 1.3.0 release notes](https://ghostty.org/docs/install/release-notes/1-3-0)
- [Ghostty config reference](https://ghostty.org/docs/config/reference)
- [Ghostty config guide for macOS 2026](https://scopir.com/posts/best-ghostty-terminal-config-themes-macos-2026/)
- [Bash 5.3 new features](https://www.phoronix.com/news/GNU-Bash-5.3)
- [Bash 5.3 developer guide](https://medium.com/@heinancabouly/bash-5-3-is-here-the-shell-update-that-actually-matters-97433bc5556c)
- [Starship prompt guide 2026](https://viadreams.cc/en/blog/starship-prompt-guide/)
- [Starship configuration](https://starship.rs/config/)
- [Catppuccin tmux plugin](https://github.com/catppuccin/tmux)
- [tmux-autoreload](https://github.com/b0o/tmux-autoreload)
- [AeroSpace guide](https://nikitabobko.github.io/AeroSpace/guide)
- [AeroSpace sticky windows issue](https://github.com/nikitabobko/AeroSpace/issues/2)
- [bat GitHub](https://github.com/sharkdp/bat)
- [chezmoi release history](https://www.chezmoi.io/reference/release-history/)
- [chezmoi age encryption](https://www.chezmoi.io/user-guide/encryption/age/)
- [chezmoi externals](https://www.chezmoi.io/user-guide/include-files-from-elsewhere/)
- [Dotfiles secret management](https://dotfiles.io/en/guides/secret-management/)
- [Gitleaks](https://github.com/gitleaks/gitleaks)
- [Delta diff pager](https://github.com/dandavison/delta)
- [git maintenance documentation](https://git-scm.com/docs/git-maintenance)
- [Atuin v18.13 release](https://blog.atuin.sh/atuin-v18-13/)
- [Atuin daemon documentation](https://docs.atuin.sh/cli/reference/daemon/)
- [Best terminal tools 2026](https://dev.to/raxxostudios/best-terminal-tools-for-developers-in-2026-4jn1)
- [Modern CLI tools 2026](https://nexasphere.io/blog/modern-cli-tools-developers-2026)
- [tmux plugins list](https://github.com/tmux-plugins/list)
- [awesome-tmux](https://github.com/rothgar/awesome-tmux)
