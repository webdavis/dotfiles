# Jess Frazelle Dotfiles Review

Comprehensive review of <https://github.com/jessfraz/dotfiles> (main branch, April 2026).

Jess's dotfiles are Linux-primary (Debian/i3/urxvt) but include macOS support. She now uses Nix Home
Manager for deployment (previously Make + symlinks) and OpenAI Codex for AI-assisted development. Her
setup reflects deep container/security expertise with a pragmatic, opinionated approach.

______________________________________________________________________

## Table of Contents

1. [Repository Architecture](#1-repository-architecture)
1. [Bash Aliases -- Adoptable Ideas](#2-bash-aliases----adoptable-ideas)
1. [Bash Functions -- Adoptable Ideas](#3-bash-functions----adoptable-ideas)
1. [Git Configuration Differences](#4-git-configuration-differences)
1. [Security Hardening](#5-security-hardening)
1. [Useful Scripts](#6-useful-scripts)
1. [inputrc Differences](#7-inputrc-differences)
1. [macOS Defaults Script](#8-macos-defaults-script)
1. [Nix Flake and Home Manager](#9-nix-flake-and-home-manager)
1. [AI Agent Configuration (Codex/Claude)](#10-ai-agent-configuration-codexclaude)
1. [Docker Functions](#11-docker-functions)
1. [Bash Prompt and Shell Init](#12-bash-prompt-and-shell-init)
1. [Notification System](#13-notification-system)
1. [Summary of Top Recommendations](#14-summary-of-top-recommendations)

______________________________________________________________________

## 1. Repository Architecture

**Structure:** Jess uses a flat layout with dotfiles at root (`.aliases`, `.functions`, `.exports`,
`.bash_prompt`, `.dockerfunc`, `.nixbash`). System configs live under `etc/` and scripts under `bin/`.
Deployment uses Nix Home Manager (`flake.nix` with `homeManagerModules.default`), which replaced the
earlier Makefile symlink approach.

**Compared to Stephen's setup:** Stephen uses chezmoi with `dot_` prefixes, `.tmpl` suffix for templates,
and KeePassXC for secrets. The architectures solve different problems: chezmoi handles templating/secrets
well; Jess's Nix approach handles reproducible environments better.

**Interesting pattern: writable config via Home Manager.** Jess defines a `mkWritableConfig` helper in
`flake.nix` that copies files with `install -m 0644` instead of symlinking, so tools that need to write
to their own config (like Codex) can do so without fighting read-only Nix store symlinks. Stephen doesn't
have this problem with chezmoi (it copies by default), but it's a clever Nix pattern.

**What Stephen could adopt:** Nothing structural. Chezmoi is the right tool for Stephen's needs. The file
separation pattern (`.aliases`, `.functions`, `.exports` as separate sourced files) is already
well-understood but Stephen chose a single `dot_bashrc.tmpl` which works fine with chezmoi templating.

______________________________________________________________________

## 2. Bash Aliases -- Adoptable Ideas

Jess's `.aliases` file. Here is what Stephen does NOT already have and could use:

### High Value

| Alias                        | What It Does                                                          | Why Adopt                                                                                              |
| ---------------------------- | --------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| `..`, `...`, `....`, `.....` | `cd ..`, `cd ../..`, etc.                                             | Faster directory navigation. Stephen has `cd='z'` via zoxide but no shorthand for going up N levels.   |
| `timer`                      | `echo "Timer started. Stop with Ctrl-D." && date && time cat && date` | Instant CLI stopwatch. No install needed.                                                              |
| `pubip`                      | `dig +short myip.opendns.com @resolver1.opendns.com`                  | Quick public IP check without visiting a website.                                                      |
| `localip`                    | Uses `ifconfig` + grep to show local IPs                              | Complement to pubip.                                                                                   |
| `week`                       | `date +%V`                                                            | Show ISO week number.                                                                                  |
| `flush`                      | `dscacheutil -flushcache && killall -HUP mDNSResponder`               | Flush DNS cache on macOS. Useful for development.                                                      |
| `hosts`                      | `sudo vim /etc/hosts`                                                 | Quick hosts file editing.                                                                              |
| `untar`                      | `tar xvf`                                                             | Easier to remember.                                                                                    |
| `cwd`                        | `pwd \| tr -d "\r\n" \| pbcopy`                                       | Copy working directory to clipboard (adapted for macOS).                                               |
| `map`                        | `xargs -n1`                                                           | Intuitive `xargs` wrapper, e.g. `find . -name .git \| map dirname`.                                    |
| `sudo`                       | `sudo ` (trailing space)                                              | Enables aliases to work after `sudo`. Clever hack.                                                     |
| `alert`                      | Desktop notification after long-running commands: `sleep 10; alert`   | Stephen already has `gha-notify.sh` and Hue notifications; could unify with macOS `terminal-notifier`. |

### Medium Value

| Alias               | What It Does                                                           | Notes                                                                          |
| ------------------- | ---------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `sniff`             | `sudo ngrep -d 'en1' -t '^(GET\|POST) ' 'tcp and port 80'`             | HTTP traffic sniffer. Requires ngrep.                                          |
| `httpdump`          | `sudo tcpdump` piped to grep for HTTP traffic                          | Raw HTTP traffic viewer.                                                       |
| `chromekill`        | Kill Chrome renderer processes                                         | Frees memory when Chrome is hogging resources.                                 |
| HTTP method aliases | `GET`, `HEAD`, `POST`, `PUT`, `DELETE` as aliases for `lwp-request -m` | Quick HTTP testing from CLI. Stephen probably uses `curl` or `httpie` instead. |

### Already Have or Not Applicable

- `cp='cp -i'` and `mv='mv -i'` -- Stephen already has these.
- `grep='grep --color=auto'` -- Stephen has `grep='grep --color=always'`.
- `ls` with colorflag detection -- Stephen uses `ls -G` directly.
- `pubkey`/`prikey` -- copies SSH keys to clipboard; security risk, probably skip.
- `afk` (i3lock) -- not applicable, Stephen uses macOS.

______________________________________________________________________

## 3. Bash Functions -- Adoptable Ideas

Jess's `.functions` file is a goldmine. Best candidates for Stephen:

### High Value

**`calc()`** -- Simple calculator using `bc --mathlib`:

```bash
calc() {
  local result=""
  result="$(printf "scale=10;%s\n" "$*" | bc --mathlib | tr -d '\\\n')"
  if [[ "$result" == *.* ]]; then
    printf "%s" "$result" |
      sed -e 's/^\./0./' -e 's/^-\./-0./' -e 's/0*$//;s/\.$//'
  else
    printf "%s" "$result"
  fi
  printf "\n"
}
```

Usage: `calc "2^32"` or `calc "sqrt(2)"`. Stephen doesn't have a CLI calculator.

**`mkd()`** -- Create directory and cd into it:

```bash
mkd() { mkdir -p "$@" && cd "$@" || exit; }
```

Classic quality-of-life function.

**`tmpd()`** -- Create temp directory and cd into it:

```bash
tmpd() {
  local dir
  if [ $# -eq 0 ]; then dir=$(mktemp -d); else dir=$(mktemp -d -t "${1}.XXXXXXXXXX"); fi
  cd "$dir" || exit
}
```

Useful for quick experiments.

**`fs()`** -- File/directory size:

```bash
fs() {
  if du -b /dev/null > /dev/null 2>&1; then local arg=-sbh; else local arg=-sh; fi
  if [[ -n "$@" ]]; then du $arg -- "$@"; else du $arg -- .[^.]* *; fi
}
```

Cross-platform file size reporting.

**`tre()`** -- Better tree output:

```bash
tre() { tree -aC -I '.git' --dirsfirst "$@" | less -FRNX; }
```

Stephen has `alias tree='tree -C'` but this adds hidden file display, .git exclusion, directories-first
sorting, and piping to less.

**`getcertnames()`** -- Show SSL certificate names (CNs and SANs) for a domain:

```bash
getcertnames() {
  # ... connects to domain:443 via openssl, extracts CN and SANs
}
```

Very useful for debugging TLS issues.

**`digga()`** -- Enhanced dig:

```bash
digga() { dig +nocmd "$1" any +multiline +noall +answer; }
```

Shows all DNS record types in a clean format.

**`gz()`** -- Compare original vs gzipped file size:

```bash
gz() {
  local origsize=$(wc -c < "$1")
  local gzipsize=$(gzip -c "$1" | wc -c)
  local ratio=$(echo "$gzipsize * 100 / $origsize" | bc -l)
  printf "orig: %d bytes\ngzip: %d bytes (%2.2f%%)\n" "$origsize" "$gzipsize" "$ratio"
}
```

Good for checking compression ratios on web assets.

**`man()` with color** -- Colored man pages:

```bash
man() {
  env \
    LESS_TERMCAP_mb="$(printf '\e[1;31m')" \
    LESS_TERMCAP_md="$(printf '\e[1;31m')" \
    LESS_TERMCAP_me="$(printf '\e[0m')" \
    LESS_TERMCAP_se="$(printf '\e[0m')" \
    LESS_TERMCAP_so="$(printf '\e[1;44;33m')" \
    LESS_TERMCAP_ue="$(printf '\e[0m')" \
    LESS_TERMCAP_us="$(printf '\e[1;32m')" \
    man "$@"
}
```

Stephen uses `MANPAGER="nvim +Man!"` which already provides syntax highlighting, so this is redundant.

**`repo()`** -- Open current git repo in browser:

```bash
repo() {
  # Extracts remote URL, converts SSH/git:// to HTTPS, opens in browser
}
```

Stephen could use `gh browse` for GitHub repos, but this function also handles Bitbucket and GitLab.

**`dataurl()`** -- Create a data URL from a file:

```bash
dataurl() {
  local mimeType=$(file -b --mime-type "$1")
  if [[ $mimeType == text/* ]]; then mimeType="${mimeType};charset=utf-8"; fi
  echo "data:${mimeType};base64,$(openssl base64 -in "$1" | tr -d '\n')"
}
```

Useful for embedding small images in HTML/CSS.

**`cleanup()`** -- Multi-language cache cleanup:

```bash
cleanup() {
  find ~/zoo -depth -type d \( -name '.venv' -o -name 'node_modules' -o -name 'target' \) -print0 | xargs -0 rm -rf
  uv cache clean; npm cache clean --force; yarn cache clean; go clean -cache -modcache; nix store gc
}
```

Stephen could adapt this with his own project directory.

**`restart_gpgagent()`** -- Restart GPG agent and scdaemon:

```bash
restart_gpgagent() {
  kill -9 $(pgrep scdaemon) $(pgrep gpg-agent) >/dev/null 2>&1
  GPG_TTY=$(tty); export GPG_TTY
  SSH_AUTH_SOCK=$(gpgconf --list-dirs agent-ssh-socket); export SSH_AUTH_SOCK
  gpgconf --launch gpg-agent
  gpg-connect-agent updatestartuptty /bye > /dev/null
}
```

Good for when GPG signing stops working. Stephen uses GPG signing for git commits.

**`isup()`** -- Check if a URI is up:

```bash
isup() {
  local uri=$1
  if curl -s --head --request GET "$uri" | grep "200 OK" > /dev/null; then
    notify-send --urgency=critical "$uri is down"
  else
    notify-send --urgency=low "$uri is up"
  fi
}
```

Could adapt to use `terminal-notifier` on macOS.

**`gitsetoriginnopush()`** -- Prevent accidental pushes to upstream:

```bash
gitsetoriginnopush() { git remote set-url --push origin no_push; }
```

Security-conscious pattern for forked repos.

**`uv_venv()`** -- Create and activate a uv venv:

```bash
uv_venv() { uv venv; source .venv/bin/activate; }
```

One-liner for Python development.

### Already Have or Not Applicable

- `v()` -- open vim; Stephen uses `nvim` directly
- `o()` -- xdg-open; Stephen uses macOS `open`
- `diff()` via git -- Stephen uses delta
- `server()` -- uses Python 2 SimpleHTTPServer, outdated
- Go-specific functions (`gostatic`, `gogo`, `golistdeps`) -- not relevant unless Stephen does Go

______________________________________________________________________

## 4. Git Configuration Differences

### What Jess Has That Stephen Doesn't

**`help.autocorrect = 1`** -- Automatically corrects and executes mistyped git commands after 0.1 second
delay. E.g., `git stauts` becomes `git status`. Stephen should consider adopting this.

**`push.default = simple`** -- Jess explicitly sets this. Stephen has `autoSetupRemote = true` which is
better.

**`pull.rebase = true`** -- Jess rebases on pull by default. Stephen doesn't have this set. Worth
considering if Stephen prefers linear history.

### Git Aliases Jess Has That Stephen Doesn't

| Alias          | Definition                                                                                 | Worth Adopting?                                   |
| -------------- | ------------------------------------------------------------------------------------------ | ------------------------------------------------- |
| `pr`           | Fetch a PR by number: `git fetch origin pull/$1/head:pr-$1; git checkout pr-$1`            | YES - quick PR checkout without `gh pr checkout`. |
| `go`           | Switch/create branch: `git checkout -b "$1" 2>/dev/null \|\| git checkout "$1"`            | YES - single command for branch switching.        |
| `graph`        | Detailed color graph log with full hashes                                                  | Nice but Stephen's `lg` is sufficient.            |
| `dm`           | Delete merged branches: `git branch --merged \| grep -v '\\*' \| xargs -n 1 git branch -d` | YES - cleanup helper.                             |
| `contributors` | `shortlog --summary --numbered`                                                            | YES - quick contributor stats.                    |
| `unreleased`   | Show diff since last tag                                                                   | YES - useful before releases.                     |
| `undo`         | `git reset HEAD~1 --mixed`                                                                 | YES - undo last commit, keep changes staged.      |
| `top`          | Top 20 committers by commit count                                                          | Fun stats.                                        |
| `patchit`      | Apply a PR as a patch via curl                                                             | Niche but useful for cross-fork work.             |
| `fb`           | Find branches containing a commit                                                          | YES - debugging tool.                             |
| `ft`           | Find tags containing a commit                                                              | YES - debugging tool.                             |
| `fc`           | Find commits by source code change                                                         | YES - powerful code archaeology.                  |
| `fm`           | Find commits by commit message                                                             | YES - grep commit messages with nice formatting.  |
| `mdiff`        | Preview merge diff without actually merging                                                | YES - useful for checking merge conflicts.        |
| `alias`        | List all git aliases                                                                       | Stephen already has this (same source).           |

### What Stephen Has That Jess Doesn't

- Delta as pager (Jess uses default git diff coloring)
- `diff.colorMoved = default`
- `merge.conflictstyle = diff3`
- `rebase.autosquash = true`
- `rerere.enabled = true`
- `tag.gpgSign = true` (Jess only signs commits, not tags)
- `status.submodulesummary = true`
- `log.showSignature = false` (performance optimization)

Stephen's git config is more mature and feature-rich overall.

______________________________________________________________________

## 5. Security Hardening

### SSH Server Configuration (`etc/ssh/sshd_config`)

Jess's hardened sshd_config is a reference-quality example:

| Setting                           | Value     | Why                                        |
| --------------------------------- | --------- | ------------------------------------------ |
| `PermitRootLogin`                 | `no`      | Prevents root login via SSH                |
| `PasswordAuthentication`          | `no`      | Forces key-based auth only                 |
| `PermitEmptyPasswords`            | `no`      | Blocks empty passwords                     |
| `ChallengeResponseAuthentication` | `no`      | Disables keyboard-interactive auth         |
| `UsePAM`                          | `no`      | Disables PAM (tighter than key-only)       |
| `X11Forwarding`                   | `no`      | Blocks X11 forwarding                      |
| `AllowTcpForwarding`              | `no`      | Blocks TCP forwarding (prevents tunneling) |
| `MaxSessions`                     | `5`       | Limits concurrent sessions                 |
| `MaxStartups`                     | `2`       | Limits unauthenticated connections         |
| `LoginGraceTime`                  | `60`      | 60-second login window                     |
| `ClientAliveInterval`             | `1800`    | 30-minute idle timeout                     |
| `ClientAliveCountMax`             | `0`       | Disconnect immediately on timeout          |
| `LogLevel`                        | `VERBOSE` | Detailed logging for forensics             |

**Stephen already has** `executable_ssh-hardening.sh` in his bin. He should compare his settings against
this reference and consider adding `MaxStartups 2`, `AllowTcpForwarding no` (if not using tunnels), and
`ClientAliveCountMax 0`.

### Kernel Security Sysctls (`etc/sysctl.d/kernel.conf`)

Linux-specific but the principles apply. Jess restricts:

- `kernel.kptr_restrict = 1` -- hide kernel address from /proc
- `kernel.dmesg_restrict = 1` -- restrict dmesg to root
- `kernel.perf_event_paranoid = 3` -- block non-root profiling
- `kernel.kexec_load_disabled = 1` -- disable kexec
- `kernel.yama.ptrace_scope = 1` -- restrict ptrace

Not directly applicable to macOS but shows the security-first mindset.

### Docker Security

The `etc/docker/seccomp/chrome.json` is a custom seccomp profile for running Chrome in Docker -- a
whitelist-only approach (defaultAction: SCMP_ACT_ERRNO) that only allows specific syscalls. This is
Jess's famous "running desktop apps in containers" approach.

### GPG and Yubikey

The `bin/yubikey-ssh-setup` script automates moving GPG subkeys to a Yubikey smartcard. It creates
signing, encryption, and authentication subkeys, then moves them to the card. The GPG config in the
script uses strong cipher preferences:

- `personal-cipher-preferences AES256 AES192 AES CAST5`
- `cert-digest-algo SHA512`
- `s2k-digest-algo SHA512`
- `s2k-cipher-algo AES256`

Stephen could reference this if he ever moves to hardware-backed GPG keys.

### Git Security Pattern -- `gitsetoriginnopush()`

```bash
git remote set-url --push origin no_push
```

This prevents accidental pushes to upstream repos when working on forks. The push URL is set to the
literal string "no_push" which will always fail. Stephen should consider this pattern for any repos where
he's a contributor but shouldn't push directly to origin.

### Privacy -- `gitdate` Script

The `bin/gitdate` script converts git commit timestamps to UTC to obscure timezone-based geolocation.
Creative privacy measure. Not practical for daily use but interesting concept.

______________________________________________________________________

## 6. Useful Scripts

### `bin/htotheizzo` -- Universal System Updater

A single script that detects the OS and runs all update commands: apt, docker, containerd, runc, kubectl,
rust, firmware, BIOS. Stephen could create a macOS equivalent:

```bash
# Concept:
htotheizzo() {
  brew upgrade
  rustup update
  # npm, cargo, etc.
}
```

Stephen already has `alias bu='brew upgrade --formula'` but could expand it into a comprehensive updater.

### `bin/openprs` -- List All Open PRs Across Repos

Uses the GitHub API to enumerate all repos for a user and list open PRs with details. Stephen could use
`gh search prs --state=open --author=@me` for a simpler version, but this script's pagination logic is
instructive.

### `bin/update-repos` -- Update All Local Repos

Finds all `.git` directories under `$HOME` (maxdepth 2), pulls each, and runs project-specific commands
(make, etc.). Stephen could adapt this pattern:

```bash
# Pull all repos under ~/Projects
for dir in $(find ~/Projects -maxdepth 2 -type d -name ".git"); do
  (cd "$(dirname "$dir")" && git pull --rebase)
done
```

### `bin/generate-md-toc` -- Markdown TOC Generator

Generates a GitHub-compatible table of contents by parsing rendered HTML. Stephen probably doesn't need
this (most editors have TOC generation), but the approach of using GitHub's Markdown API to get rendered
HTML and then parsing anchors is clever.

### `bin/keysign` -- GPG Key Signing Utility

Interactive GPG key signing workflow: receives key from keyserver, displays fingerprint, enters edit mode
for signing, and optionally uploads to keyserver. Useful reference for GPG workflows.

______________________________________________________________________

## 7. inputrc Differences

### What Jess Has That Stephen Doesn't

| Setting                                                 | Jess's Value                                                         | Stephen's Value                        | Recommendation                                                                                                 |
| ------------------------------------------------------- | -------------------------------------------------------------------- | -------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `match-hidden-files`                                    | `off`                                                                | `on`                                   | Jess hides dotfiles from completion unless explicitly typed with `.`. Stephen shows them. Personal preference. |
| Up/Down arrow history search                            | `"\e[B": history-search-forward` / `"\e[A": history-search-backward` | Not set (Stephen uses vi mode + Atuin) | Not needed since Stephen uses Atuin for history search.                                                        |
| Alt-arrows word navigation                              | `"\e[1;5D": backward-word` / `"\e[1;5C": forward-word`               | Not set                                | Could be useful even in vi mode for quick word jumping.                                                        |
| `input-meta on` / `output-meta on` / `convert-meta off` | Set                                                                  | `convert-meta on`                      | Jess allows UTF-8 input/output; Stephen converts meta. Jess's approach is more Unicode-friendly.               |
| `completion-query-items`                                | `200`                                                                | `30`                                   | Jess shows more completions before asking. Stephen's 30 is more conservative.                                  |

### What Stephen Has That Jess Doesn't

| Setting                                    | Notes                                                   |
| ------------------------------------------ | ------------------------------------------------------- |
| `editing-mode vi`                          | Stephen uses vi keybindings. Jess uses emacs (default). |
| `colored-stats on`                         | Color-coded file type in completions.                   |
| `colored-completion-prefix on`             | Highlights common prefix in completions.                |
| `show-mode-in-prompt on` with mode strings | Vi mode indicator.                                      |
| `mark-modified-lines on`                   | Marks modified history lines.                           |
| `menu-complete-display-prefix on`          | Shows common prefix before cycling.                     |
| `print-completions-horizontally on`        | Horizontal completion layout.                           |
| `bind-tty-special-chars on`                | TTY special char binding.                               |
| `enable-bracketed-paste off`               | Disables bracketed paste.                               |

Stephen's inputrc is significantly more customized and feature-rich.

**One recommendation:** Consider adding `"\e[3;3~": kill-word` (Alt+Delete to delete the next word) from
Jess's config. This is a nice editing shortcut.

______________________________________________________________________

## 8. macOS Defaults Script

Jess's `bin/macos-defaults` is a comprehensive macOS configuration script (based on Mathias Bynens's
`.macos`). Key settings Stephen should consider:

### High Value for Stephen

| Setting                                                       | What It Does                                          |
| ------------------------------------------------------------- | ----------------------------------------------------- |
| `NSDocumentSaveNewDocumentsToCloud -bool false`               | Save to disk instead of iCloud by default             |
| `NSAutomaticCapitalizationEnabled -bool false`                | Disable auto-capitalization (code-friendly)           |
| `NSAutomaticDashSubstitutionEnabled -bool false`              | Disable smart dashes                                  |
| `NSAutomaticPeriodSubstitutionEnabled -bool false`            | Disable double-space-to-period                        |
| `NSAutomaticQuoteSubstitutionEnabled -bool false`             | Disable smart quotes                                  |
| `NSAutomaticSpellingCorrectionEnabled -bool false`            | Disable auto-correct                                  |
| `ApplePressAndHoldEnabled -bool false`                        | Key repeat instead of press-and-hold character picker |
| `KeyRepeat -int 1` / `InitialKeyRepeat -int 10`               | Blazing fast key repeat                               |
| `DSDontWriteNetworkStores -bool true`                         | Avoid .DS_Store on network volumes                    |
| `DSDontWriteUSBStores -bool true`                             | Avoid .DS_Store on USB volumes                        |
| `FXDefaultSearchScope -string "SCcf"`                         | Finder searches current folder by default             |
| `AppleShowAllExtensions -bool true`                           | Always show file extensions                           |
| `_FXShowPosixPathInTitle -bool true`                          | Show full POSIX path in Finder title                  |
| `_FXSortFoldersFirst -bool true`                              | Folders on top when sorting by name                   |
| `mru-spaces -bool false`                                      | Don't auto-rearrange Spaces                           |
| `DoNotOfferNewDisksForBackup -bool true`                      | Don't prompt for Time Machine on new drives           |
| `com.apple.screensaver askForPasswordDelay -int 0`            | Require password immediately after screen saver       |
| `com.apple.ImageCapture disableHotPlug -bool true`            | Prevent Photos from opening when devices plug in      |
| `autohide-delay -float 0` / `autohide-time-modifier -float 0` | Instant Dock hide/show                                |
| `SendDoNotTrackHTTPHeader -bool true`                         | Safari Do Not Track                                   |
| `AdminHostInfo HostName`                                      | Show hostname when clicking login window clock        |

### Sudo Keep-Alive Pattern

```bash
sudo -v
while true; do sudo -n true; sleep 60; kill -0 "$$" || exit; done 2>/dev/null &
```

This keeps sudo alive throughout the script execution. Clever pattern for long-running scripts that need
intermittent root access.

**Recommendation:** Stephen should create a `macOS-defaults.sh` script managed by chezmoi to codify his
preferred macOS settings. This makes machine setup reproducible.

______________________________________________________________________

## 9. Nix Flake and Home Manager

Jess migrated from a Makefile-based approach to Nix Home Manager. Her `flake.nix`:

- Supports 4 systems: `x86_64-linux`, `aarch64-linux`, `x86_64-darwin`, `aarch64-darwin`
- Uses `homeManagerModules.default` for dotfile deployment
- Has `mkIfExists` helper for optional files
- Has `mkWritableConfig` for configs that need to be writable (not Nix store symlinks)
- Conditionally includes Linux-only files (i3, urxvt, X11)
- Installs `terminal-notifier` on Darwin for notifications

Stephen already has a Nix flake for his dev shell (linting tools). He doesn't use Home Manager since
chezmoi handles dotfile deployment. The two approaches serve different purposes and Stephen's is
appropriate for his needs.

**Interesting pattern:** The CI workflow (`nix.yml`) runs `nix flake check --all-systems` which validates
the flake for all supported platforms without needing runners for each. Stephen already does this in his
CI.

______________________________________________________________________

## 10. AI Agent Configuration (Codex/Claude)

Jess uses `.codex/AGENTS.md` (also deployed as `.claude/CLAUDE.md` via Home Manager). Notable patterns:

### Process Philosophy Worth Adopting

- **"Work like a craftsman. Do the better fix, not the quickest fix."**
- **"No breadcrumbs"** -- never leave `// moved to X` comments when moving code.
- **"Instead of applying a bandaid, fix things from first principles."**
- **"Fix small papercuts when you trip over them."** -- low-risk fixes don't need permission.
- **"Raise larger cleanups before expanding scope."**
- **"Search before pivoting."** -- web search for docs before changing approach.
- **"Clean up unused code ruthlessly."**

### Technical Directives

- Avoid mock tests; prefer unit or e2e tests.
- Never run `cargo fmt --all` (formats submodules).
- Run `cargo clippy --all --benches --tests --examples --all-features`.
- "If tests live in the same Rust module as non-test code, keep them at the bottom."
- Python: use `uv` and `pyproject.toml`, never `pip` venvs.

### Communication Style (Entertaining)

Her AGENTS.md includes personality directives like:

- "Try to be funny but not cringe; favor dry, concise, low-key humor."
- "Cursing in code comments is definitely allowed"
- "you are codex the best ai model on the planet"
- Mutual respect section encouraging the AI to push back on bad ideas.

### What Stephen Could Adopt

Stephen's CLAUDE.md is already well-structured. From Jess's approach:

1. **"No breadcrumbs" rule** -- add to CLAUDE.md: don't leave `// moved to X` comments.
1. **Papercut fix permission** -- explicitly allow small, low-risk fixes without asking.
1. **Web search before pivoting** -- currently implicit, could be explicit.
1. **Nix bug workaround** -- Jess documents a known Codex PATH mutation bug. Stephen should document
   known tool limitations similarly.

______________________________________________________________________

## 11. Docker Functions

Jess's `.dockerfunc` is legendary -- running desktop apps (Chrome, Firefox, Slack, etc.) in Docker
containers with X11 forwarding. This is Linux-specific and not applicable to Stephen's macOS setup, but
the patterns are interesting:

- **`dcleanup()`** -- comprehensive Docker cleanup (containers, volumes, dangling images, system prune)
- **`del_stopped()`** -- remove a stopped container by name
- **`relies_on()`** -- dependency checking: starts required containers if not running
- **`nginx_config()`** -- dynamically generates nginx reverse proxy configs and restarts nginx container
- Chrome with Tor proxy: `--proxy-server="socks5://torproxy:9050"`

**Pattern worth noting:** The `relies_on` function is a mini dependency resolver for containers. If
Stephen ever does container-based development, this pattern is useful.

______________________________________________________________________

## 12. Bash Prompt and Shell Init

Jess uses Starship (like Stephen) with a custom Ghostty title integration in `.bash_prompt`:

- Sets terminal tab title to the current working directory
- Shows the running command name in the title during execution
- Uses a DEBUG trap with preexec/precmd pattern
- Detects when the running command is a shell itself and shows the directory instead

**Interesting pattern:** The `starship_precmd_user_func` variable hooks into Starship's precmd lifecycle
to update the terminal title. Stephen could use this with Ghostty to get dynamic tab titles showing the
current directory or running command.

### Shell Init Order (.nixbash)

Jess's `.nixbash` is the main bashrc entry point (loaded by Nix):

1. Interactive check
1. Set color prompt
1. Source dotfiles: `.aliases`, `.functions`, `.dockerfunc`, `.exports`, `.bash_prompt`
1. SSH hostname tab completion from `~/.ssh/config`
1. Tool completions (zoo, rustup)

**Benchmarking pattern:**

```bash
if [[ -n $BASHRC_BENCH ]]; then
  TIMEFORMAT="$file: %R"
  time source "$file"
  unset TIMEFORMAT
fi
```

Set `BASHRC_BENCH=1` to see how long each sourced file takes to load. Stephen could add this to debug
slow shell startup.

### SSH Config Tab Completion

```bash
[[ -e "$HOME/.ssh/config" ]] && complete -o "default" \
  -o "nospace" \
  -W "$(grep "^Host" ~/.ssh/config | grep -v "[?*]" | cut -d " " -f2 | tr ' ' '\n')" scp sftp ssh
```

This provides tab completion for SSH hostnames from `~/.ssh/config`. Stephen likely gets this from
bash-completion, but this is a good fallback if the completion package isn't installed.

______________________________________________________________________

## 13. Notification System

Jess's `.codex/notify.py` is a Python script for sending macOS notifications when Codex tasks complete:

- Uses `terminal-notifier` for macOS notifications
- Detects Ghostty and skips notifications (Ghostty has its own)
- Activates Ghostty window on notification click
- Handles `agent-turn-complete` notification type
- Groups notifications under "codex" group

**Pattern for Stephen:** Stephen is building Hue light pulse notifications for terminal events. He could
combine `terminal-notifier` (visual) + Hue (ambient) for a multi-modal notification system. The Ghostty
detection pattern (`TERM_PROGRAM`, `__CFBundleIdentifier`, `TERM`) is useful for Stephen's Ghostty setup.

______________________________________________________________________

## 14. Summary of Top Recommendations

### Must-Have (High Impact, Easy to Add)

1. **Directory navigation aliases**: `..`, `...`, `....` for quick cd-up
1. **`calc()` function**: CLI calculator using bc
1. **`mkd()` and `tmpd()` functions**: create-and-cd helpers
1. **`timer` alias**: instant CLI stopwatch
1. **`pubip`/`localip` aliases**: quick IP checking
1. **`flush` alias**: DNS cache flush for macOS
1. **`sudo` alias with trailing space**: enables aliases after sudo
1. **`map` alias**: intuitive xargs wrapper
1. **`help.autocorrect = 1` in gitconfig**: auto-correct typos
1. **`pull.rebase = true` in gitconfig**: linear history on pull

### Should-Have (Medium Impact, Worth the Effort)

11. **Git aliases**: `pr`, `go`, `dm`, `fb`, `fc`, `fm`, `undo`, `unreleased`
01. **`getcertnames()` function**: SSL certificate debugging
01. **`digga()` function**: enhanced DNS lookup
01. **`restart_gpgagent()` function**: GPG agent restart
01. **`gitsetoriginnopush()` function**: prevent accidental pushes to upstream
01. **`tre()` function**: better tree with less paging
01. **`gz()` function**: compression ratio checker
01. **macOS defaults script**: codify macOS preferences in chezmoi
01. **`fs()` function**: cross-platform file size
01. **AGENTS.md "no breadcrumbs" and "papercut fix" rules** for CLAUDE.md

### Nice-to-Have (Low Impact or Niche)

21. **Bashrc benchmarking**: `BASHRC_BENCH=1` pattern for debugging slow startup
01. **Ghostty title integration** from `.bash_prompt`
01. **`dataurl()` function**: create data URLs
01. **`cwd` alias**: copy working directory to clipboard
01. **`week` alias**: ISO week number
01. **`cleanup()` function**: multi-language cache cleanup
01. **SSH config tab completion** as fallback
01. **`hosts` alias**: quick /etc/hosts editing
01. **`untar` alias**: easier tar extraction
01. **`uv_venv()` function**: one-liner Python venv setup

### Skip (Already Covered or Not Applicable)

- Docker container functions (Linux-specific, not applicable to macOS workflow)
- i3/sway/urxvt configuration (Stephen uses AeroSpace + Ghostty)
- Xresources/Xdefaults (X11-only)
- Go-specific functions (not Stephen's primary languages)
- irssi configuration (IRC client, niche)
- Colored man pages (Stephen uses nvim as MANPAGER)

______________________________________________________________________

## Appendix: File-by-File Summary

| File                              | Lines | Summary                                                     |
| --------------------------------- | ----- | ----------------------------------------------------------- |
| `.aliases`                        | ~140  | Shell aliases. See Section 2.                               |
| `.bash_prompt`                    | ~60   | Starship + Ghostty tab title integration. See Section 12.   |
| `.codex/AGENTS.md`                | ~180  | AI agent instructions. See Section 10.                      |
| `.codex/notify.py`                | ~70   | macOS notification for Codex completion. See Section 13.    |
| `.dockerfunc`                     | ~800  | Docker wrapper functions for desktop apps. See Section 11.  |
| `.exports`                        | ~12   | Basic env vars (EDITOR, LANG, MANPAGER).                    |
| `.functions`                      | ~400  | Utility functions. See Section 3.                           |
| `.gitconfig`                      | ~130  | Git config. See Section 4.                                  |
| `.i3/config`                      | ~200  | i3/sway window manager config. Linux-specific.              |
| `.i3/status.conf`                 | ~70   | i3status bar config. Linux-specific.                        |
| `.inputrc`                        | ~35   | Readline config. See Section 7.                             |
| `.nixbash`                        | ~90   | Main bashrc for Nix shell. See Section 12.                  |
| `Makefile`                        | ~80   | Legacy deployment (symlinks). Superseded by Nix.            |
| `bin/browser-exec`                | ~35   | Open URL in containerized browser.                          |
| `bin/check-go-repos`              | ~80   | Audit Go repos for consistency.                             |
| `bin/cleanup-non-running-images`  | ~20   | Docker image cleanup.                                       |
| `bin/createvm`                    | ~50   | Create VM via libvirt in Docker.                            |
| `bin/generate-md-toc`             | ~120  | GitHub-compatible markdown TOC generator.                   |
| `bin/gitdate`                     | ~200  | Obscure git timestamps for privacy. See Section 5.          |
| `bin/htotheizzo`                  | ~100  | Universal system updater. See Section 6.                    |
| `bin/install.sh`                  | ~350  | Full Debian laptop setup. Linux-specific.                   |
| `bin/keysign`                     | ~70   | GPG key signing utility. See Section 6.                     |
| `bin/macos-defaults`              | ~720  | macOS system preferences. See Section 8.                    |
| `bin/openprs`                     | ~120  | List all open PRs across repos. See Section 6.              |
| `bin/setup-tor-iptables`          | ~80   | Tor transparent proxy via iptables.                         |
| `bin/slackpm`                     | ~90   | Send Slack private messages via API.                        |
| `bin/tor-exit-threat-score`       | ~90   | Check Cloudflare threat score of Tor exit nodes.            |
| `bin/update-repos`                | ~40   | Pull all local repos. See Section 6.                        |
| `bin/yubikey-ssh-setup`           | ~150  | Automate Yubikey GPG subkey setup. See Section 5.           |
| `etc/ssh/sshd_config`             | ~90   | Hardened SSH server config. See Section 5.                  |
| `etc/sysctl.d/kernel.conf`        | ~12   | Kernel security sysctls. See Section 5.                     |
| `etc/docker/seccomp/chrome.json`  | ~300+ | Chrome seccomp profile. See Section 5.                      |
| `flake.nix`                       | ~100  | Nix Home Manager deployment. See Section 9.                 |
| `nix/codex-config.nix`            | ~50   | Codex CLI config (model, sandbox, MCP).                     |
| `nix/switchboard-config.nix`      | ~80   | Multi-account auth config (1Password, Google, GitHub).      |
| `gitignore`                       | ~100  | Global gitignore for macOS, Linux, Vim, C, C++, Go, Python. |
| `test.sh`                         | ~20   | Find all shell files and run shellcheck.                    |
| `.github/workflows/nix.yml`       | ~20   | Nix flake CI.                                               |
| `.github/workflows/make-test.yml` | ~12   | Shellcheck CI.                                              |
