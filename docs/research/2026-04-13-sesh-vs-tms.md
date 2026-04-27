# Sesh vs TMS (tmux-sessionizer): Deep Research Comparison

**Date:** 2026-04-13 **Researcher:** Claude Code (Opus 4.6) **Mode:** Standard (6-phase)

______________________________________________________________________

## Executive Summary

This report compares two tmux session managers: **sesh** (by Josh Medeski, written in Go) and
**tms/tmux-sessionizer** (by jrmoulton, written in Rust). Both tools solve the same core problem --
managing tmux sessions around project directories -- but they differ significantly in philosophy, feature
set, and community traction.

**Key findings:**

- **Sesh** has 3.4x the GitHub stars (2,275 vs 666), 2.3x the releases (30 vs 13), and a faster
  development cadence (101 commits since Jan 2025 vs 34 for tms).
- **TMS has a built-in marks system** that directly maps to the user's current workflow (12 numbered
  marks bound to single keystrokes via tmux key tables). Sesh has no equivalent built-in feature.
- **Sesh can replicate the marks pattern** using `sesh connect <name>` with `run-shell` in tmux bindings,
  but this requires manual session definition in `sesh.toml` and separate tmux keybinding configuration
  -- two places to maintain instead of one.
- **Sesh offers richer session configuration** (startup commands, startup scripts, window definitions,
  wildcard patterns, zoxide integration) that tms does not match.
- **For this specific user's workflow**, tms remains the better fit because the marks system is the core
  value proposition, and migrating to sesh would add complexity without proportional gain.

**Recommendation:** Stay with tms. The marks system is purpose-built for the user's keybinding-centric
workflow, and the migration cost to sesh would not be justified by sesh's advantages in areas the user
does not currently need (zoxide integration, session startup scripts, built-in picker).

______________________________________________________________________

## 1. Configuration Structure

### TMS (tmux-sessionizer)

**Config format:** TOML **Config location:** `~/.config/tms/config.toml` (macOS:
`~/Library/Application Support/tms/config.toml`) **Override:** `TMS_CONFIG_FILE` environment variable

**Configuration capabilities:**

```toml
# Directory scanning
bookmarks = ["/path/to/project1", "/path/to/project2"]

[[search_dirs]]
path = "/Users/stephen/workspaces"
depth = 2

# Named sessions (auto-created on `tms start`)
[[sessions]]
name = "uriel"
path = "/Users/stephen/workspaces/webdavis/uriel"

# Marks for instant access by index
[marks]
0 = "/path/to/project-a"
1 = "/path/to/project-b"
```

**Picker UI customization:**

```toml
[shortcuts]
cancel = ["Esc"]
confirm = ["Enter"]

# Color theming for the built-in fuzzy picker
```

TMS configuration is streamlined and focused. The `[marks]` section is the standout feature: a flat
mapping of integer indices to directory paths. The `[[sessions]]` array defines named sessions that
`tms start` bootstraps automatically. The `[[search_dirs]]` array defines where tms scans for git
repositories. There is no support for startup commands, window definitions, or per-session scripts.

**Source:** [tms GitHub README][1], user's live config at
`~/.local/share/chezmoi/dot_config/tms/config.toml`

### Sesh

**Config format:** TOML **Config location:** `$XDG_CONFIG_HOME/sesh/sesh.toml` or
`~/.config/sesh/sesh.toml` **Schema:** JSON Schema available at `sesh.schema.json` in the repository

**Configuration capabilities:**

```toml
# Global settings
cache = false
strict_mode = false
import = ["~/other-config.toml"]
blacklist = ["session-to-hide"]
sort_order = ["tmux", "config", "zoxide"]
dir_length = 1
separator_aware = false

# Default session settings (applied to all sessions unless overridden)
[default_session]
startup_command = "nvim -c ':Telescope find_files'"
preview_command = "ls -la"
windows = ["editor", "server"]

# Named session definitions
[[session]]
name = "Downloads"
path = "~/Downloads"
startup_command = "yazi"
disable_startup_command = false
preview_command = "ls"

[[session.window]]
name = "logs"
path = "~/Downloads/logs"
startup_script = "tail -f *.log"

# Wildcard patterns (glob-based matching)
[[wildcard]]
pattern = "~/workspaces/*"
startup_command = "nvim"
```

Sesh's configuration is substantially richer. It supports:

- **Startup commands** per session (run on creation)
- **Startup scripts** (full bash scripts for complex initialization)
- **Window definitions** with per-window paths and scripts
- **Wildcard patterns** for applying config to directories matching globs
- **Import** for modular config files
- **Blacklisting** to hide sessions from the picker
- **Caching** with stale-while-revalidate for performance
- **tmuxp and tmuxinator** interop fields

**What sesh does NOT have:** Any concept of marks, numbered indices, or instant-access bookmarks in its
configuration.

**Source:** [sesh README][2], [sesh.schema.json][3], [sesh blog post][4]

### Comparison Table: Configuration

| Feature                    | TMS                  | Sesh                |
| -------------------------- | -------------------- | ------------------- |
| Config format              | TOML                 | TOML                |
| Directory scanning         | Yes (`search_dirs`)  | Via zoxide          |
| Named sessions             | Yes (`[[sessions]]`) | Yes (`[[session]]`) |
| Marks / numbered bookmarks | Yes (`[marks]`)      | No                  |
| Startup commands           | No                   | Yes                 |
| Startup scripts            | No                   | Yes                 |
| Window definitions         | No                   | Yes                 |
| Wildcard patterns          | No                   | Yes                 |
| Import/modular config      | No                   | Yes                 |
| Blacklisting               | No                   | Yes                 |
| Caching                    | No                   | Yes                 |
| tmuxp/tmuxinator interop   | No                   | Yes                 |
| JSON Schema for validation | No                   | Yes                 |
| Picker color theming       | Yes                  | Via fzf/television  |

______________________________________________________________________

## 2. UI/UX

### TMS

**Session creation:** `tms` (no subcommand) opens a fuzzy finder over all git repositories found in
configured `search_dirs`. Selecting one creates and attaches to a new tmux session.

**Session switching:** `tms switch` opens a fuzzy finder over active tmux sessions with a preview window
showing the session's windows and panes.

**Window switching:** `tms windows` shows current session's windows in a fuzzy finder.

**Marks (instant access):** `tms marks open <N>` immediately switches to the session at index N. No
picker involved -- it is a direct, instant switch. If the session does not exist, tms creates it from the
path configured in `[marks]`.

**Session lifecycle:**

- `tms start` -- bootstraps all `[[sessions]]` and opens the default
- `tms kill` -- kills current session and jumps to default
- `tms rename` -- renames session and updates working directory
- `tms refresh` -- generates missing worktree windows

**Picker interface:** Built-in Rust fuzzy finder with configurable colors and keyboard shortcuts. Not
pluggable (no fzf/gum integration; the picker is compiled into the binary).

**Source:** [tms README][1], `tms --help` output

### Sesh

**Session creation:** `sesh connect <name-or-path>` creates and connects to a session. If the session
does not exist, it is created automatically with the path resolved via zoxide or config.

**Session switching:** Sesh is designed around picker integration. The canonical workflow uses fzf-tmux,
gum, or television in a tmux popup:

```bash
bind-key "T" display-popup -E -w 40% \
  "sesh connect \"$(sesh list -i | gum filter --limit 1)\""
```

**Built-in picker (v2.23.0+):** `sesh picker` provides a native picker without external dependencies,
added in February 2025.

**Last session:** `sesh last` toggles to the most recently attached session, fixing tmux's broken native
`last-session` command that fails after detach/reattach cycles.

**Advanced picker modes:** The recommended fzf-tmux integration supports multiple source categories
toggled via ctrl-key chords:

- `ctrl-a` -- all sources
- `ctrl-t` -- tmux sessions only
- `ctrl-x` -- configs only
- `ctrl-g` -- zoxide results
- `ctrl-f` -- find (fd) results
- `ctrl-d` -- kill session

**Session lifecycle:**

- `sesh connect` -- create or switch (the only creation command)
- `sesh last` -- toggle to previous session
- `sesh clone` -- clone a git repo and open as session
- No equivalent to `tms start` for bootstrapping multiple sessions
- No equivalent to `tms kill` with automatic fallback

**Source:** [sesh README][2], [sesh blog post][4], [sesh improvements blog][5]

### Comparison Table: UI/UX

| Capability                   | TMS                   | Sesh                           |
| ---------------------------- | --------------------- | ------------------------------ |
| Primary interaction model    | Built-in fuzzy finder | External picker (fzf/gum/tv)   |
| Built-in picker              | Yes (compiled in)     | Yes (since v2.23.0)            |
| Instant session jump (no UX) | `tms marks open N`    | `sesh connect <name>` (manual) |
| Last session toggle          | No                    | `sesh last`                    |
| Session bootstrapping        | `tms start`           | Not available                  |
| Git worktree window creation | Automatic             | Not available                  |
| Kill with fallback           | `tms kill`            | Not available                  |
| Session rename + dir update  | `tms rename`          | Not available                  |
| Zoxide integration           | No                    | Core feature                   |
| Git clone + session          | `tms clone-repo`      | `sesh clone`                   |

______________________________________________________________________

## 3. Community Support

### GitHub Metrics (as of 2026-04-13)

| Metric         | TMS                 | Sesh                 |
| -------------- | ------------------- | -------------------- |
| Stars          | 666                 | 2,275                |
| Forks          | 54                  | 101                  |
| Contributors   | 21                  | 30                   |
| Open issues    | 19                  | 61                   |
| Total releases | 13                  | 30                   |
| Language       | Rust (97.4%)        | Go                   |
| License        | MIT                 | MIT                  |
| Created        | 2022-04-28          | 2023-12-27           |
| Last push      | 2026-02-18          | 2026-03-30           |
| Latest release | v0.5.0 (2025-08-01) | v2.24.2 (2025-02-23) |

### Analysis

**Sesh has stronger community traction** by every metric. It has 3.4x the stars despite being 19 months
younger than tms. Sesh's 30 releases in ~26 months (roughly one every 3.5 weeks) versus tms's 13 releases
in ~46 months (roughly one every 3.5 months) indicates a much faster development cadence.

**Sesh has broader ecosystem support:**

- Raycast extension for macOS ([Raycast Store][9])
- Two Ulauncher extensions for Linux
- Integration guides for fzf, television, gum
- Shell completions for bash, zsh, fish, PowerShell
- Documentation in English and Simplified Chinese
- Featured on the Linkarzu Podcast ([episode][6])
- Blog coverage on Buzzrag ([article][7]) and LinuxLinks ([article][8])

**TMS has more focused but smaller community:**

- Available via Homebrew, Cargo, and Nix
- Shell completions for bash, zsh, fish
- Active contributor community (petersimonsson, junglerobba are recurring contributors)
- Listed on Repology for distribution packaging

**The Session X factor:** The creator of tmux-sessionx (omerxx), another popular tmux session manager,
publicly switched to sesh, calling it superior. This endorsement has driven additional adoption toward
sesh.

**Source:** GitHub API data, [Buzzrag article][7], [Linkarzu podcast][6]

______________________________________________________________________

## 4. Growth Trajectory

### Sesh

- **Commits since Jan 2025:** ~101 (extracted from GitHub API pagination headers)
- **Releases since Jan 2025:** v2.18.2 through v2.24.2 (7 releases in ~14 months)
- **Major features added in 2025:**
  - Built-in picker (`sesh picker`) -- February 2025
  - Session list caching with stale-while-revalidate -- February 2025
  - Wildcard configurations -- February 2025
  - XDG_CONFIG_HOME support -- February 2025
  - Configuration schema (JSON Schema) -- February 2025
  - Manual page generation -- February 2025
  - Separator-aware fuzzy matching -- February 2025
  - Git bare root namer for worktree support -- January 2025
  - Blacklist support across all sources -- December 2024
- **Trajectory:** Sesh had a burst of activity in January-February 2025 with significant feature
  additions. Activity continued through March 2026, but the pace has moderated. The project appears to be
  in a "mature feature" phase, where most core functionality is implemented and development shifts toward
  refinement and bug fixes.

### TMS

- **Commits since Jan 2025:** ~34 (extracted from GitHub API pagination headers)
- **Releases since Jan 2025:** v0.4.5 (March 2025) and v0.5.0 (August 2025)
- **Major features added in 2025:**
  - Marks feature (bookmarking) -- v0.4.5, March 2025
  - Named sessions / open-session command -- v0.4.5, March 2025
  - Jujutsu (jj) VCS support -- v0.5.0, August 2025
  - Migration from git2 to gitoxide -- v0.5.0, August 2025
  - Picker cycling (top-to-bottom wrap) -- v0.4.5, March 2025
- **Trajectory:** TMS has a slower but steady development pace. The marks feature (the user's primary
  value) was added in March 2025, indicating the project is responsive to user needs. The jujutsu support
  in v0.5.0 shows forward-looking development. However, the project has not had a release in over 8
  months (since August 2025), and the last push was February 2026.

### Relative Assessment

Sesh is growing faster in both community size and feature breadth. TMS is a smaller, more focused tool
with a slower but deliberate development cycle. Neither project appears abandoned, but sesh has more
momentum.

**Source:** GitHub API data, [tms releases][10], [sesh releases][11]

______________________________________________________________________

## 5. Plugin/Integration Ecosystem

### TMS Integrations

| Integration       | Details                                                    |
| ----------------- | ---------------------------------------------------------- |
| tmux              | Direct integration via `display-popup` bindings            |
| Git               | Automatic repository scanning and worktree window creation |
| Jujutsu (jj)      | Non-colocated repos and workspaces (v0.5.0+)               |
| Nix               | Flake with overlay for Nix-based installation              |
| Homebrew          | Official formula (`brew install tmux-sessionizer`)         |
| Cargo             | Published crate (`cargo install tmux-sessionizer`)         |
| Shell completions | Bash, Zsh, Fish                                            |

TMS has no plugin system and no external integrations beyond tmux itself. Its value is in being a
self-contained binary that does one thing well.

### Sesh Integrations

| Integration       | Details                                                      |
| ----------------- | ------------------------------------------------------------ |
| tmux              | `run-shell`, `display-popup`, status bar integration         |
| zoxide            | Core dependency for directory discovery                      |
| fzf               | Primary picker integration with multi-mode support           |
| television        | Alternative picker with TUI interface                        |
| gum               | Alternative picker (Charm.sh)                                |
| Raycast           | macOS launcher extension for session access outside terminal |
| Ulauncher         | Linux launcher extensions (2 available)                      |
| tmuxp             | Session definition interop                                   |
| tmuxinator        | Session definition interop                                   |
| Homebrew          | Official tap (`brew install joshmedeski/sesh/sesh`)          |
| Go                | `go install` support                                         |
| Nix               | Package available                                            |
| Conda/mamba/Pixi  | Package available                                            |
| Shell completions | Bash, Zsh, Fish, PowerShell                                  |

Sesh has a significantly broader integration ecosystem, particularly with external picker tools and
desktop launchers. The Raycast integration is notable for macOS users who want session access from
anywhere in the OS, not just within tmux.

**Source:** [sesh README][2], [tms README][1]

______________________________________________________________________

## 6. Keybinding Support -- Critical Analysis

This section addresses the user's primary concern: the ability to bind specific keys to switch to
specific named projects instantly, without a picker.

### Current TMS Workflow

The user's setup uses tmux key tables to create a two-chord keybinding system:

```
Prefix (C-d) -> C-o (enter TMUX_SESSIONIZER mode) -> single letter (jump to project)
```

Example from the user's `dot_tmux.conf`:

```bash
# Enter sessionizer mode
bind-key -N "Tmux Sessionizer Mode" C-o switch-client -T TMUX_SESSIONIZER

# Instant project access via marks
bind-key -T TMUX_SESSIONIZER u display-popup -E "tms marks open 0"   # uriel
bind-key -T TMUX_SESSIONIZER o display-popup -E "tms marks open 1"   # openclaw
bind-key -T TMUX_SESSIONIZER d display-popup -E "tms marks open 5"   # dotfiles
bind-key -T TMUX_SESSIONIZER n display-popup -E "tms marks open 6"   # nvim
# ... 12 marks total, each with a mnemonic single-key binding
```

**What makes this work:**

1. `tms marks open <N>` is a **direct, instant switch**. No picker, no fuzzy finding.
1. If the session does not exist, tms creates it from the configured path in `[marks]`.
1. The mark index-to-path mapping lives in `config.toml`, co-located with other tms config.
1. The tmux keybinding only needs the index number -- it does not need to know the session name or path.
1. Adding a new project requires two changes: add the mark in `config.toml` and add the keybinding in
   `tmux.conf`.

### Can Sesh Replicate This?

**Yes, but with more friction.** Josh Medeski himself confirmed the pattern in [sesh issue #107][12]:

> You could add your own binding to your tmux config:
>
> ```sh
> bind-key 1 run-shell -b "sesh connect 'Downloads'"
> ```
>
> As of right now sesh doesn't make any changes to tmux, it's just a binary you can use to interact with
> tmux.

So the equivalent sesh setup would be:

```bash
# In tmux.conf
bind-key -N "Sesh Mode" C-o switch-client -T SESH_MODE
bind-key -T SESH_MODE u run-shell -b "sesh connect uriel"
bind-key -T SESH_MODE o run-shell -b "sesh connect openclaw"
bind-key -T SESH_MODE d run-shell -b "sesh connect chezmoi"
# ...
```

Combined with `sesh.toml`:

```toml
[[session]]
name = "uriel"
path = "~/workspaces/webdavis/uriel"

[[session]]
name = "openclaw"
path = "~/.openclaw"

[[session]]
name = "chezmoi"
path = "~/.local/share/chezmoi"
```

**Differences from TMS marks:**

| Aspect                      | TMS marks                                | Sesh equivalent                       |
| --------------------------- | ---------------------------------------- | ------------------------------------- |
| Config location             | Single file (`config.toml` `[marks]`)    | Two files (`sesh.toml` + `tmux.conf`) |
| Identifier                  | Integer index                            | Session name string                   |
| Keybinding references       | Index number only                        | Full session name                     |
| Auto-create on first access | Yes (from path in marks config)          | Yes (from path in session config)     |
| Feature is built-in         | Yes (first-class `tms marks` subcommand) | No (manual `run-shell` pattern)       |
| Name changes require        | Just config.toml                         | Both sesh.toml AND tmux.conf          |
| Maximum marks               | Unlimited (integer indices)              | Unlimited (named sessions)            |
| Discovery (list all marks)  | `tms marks list`                         | No equivalent                         |

### Feature Request History in Sesh

Two relevant issues confirm that sesh's maintainer considers marks/shortcuts out of scope:

1. **[Issue #107: "Shortcut per session as configuration"][12]** (May 2024) -- A user requested
   `shortcut = "<C-1>"` in session config. Medeski closed it, pointing to the `run-shell` pattern above.
   His philosophy: "sesh doesn't make any changes to tmux."

1. **[Issue #255: "Add Session Marker System for Activity Tracking"][13]** (May 2025) -- A PR
   implementing a marker system with visual indicators was rejected. Medeski responded: "I don't think
   this approach is quite the right fit for this particular project. The scope feels out of place."

**Conclusion:** Sesh will not add a marks system. The maintainer has drawn a clear boundary: sesh is a
session connection tool, not a tmux configuration manager. Any keybinding-to-session mapping must be done
manually in `tmux.conf`.

**Source:** [sesh issue #107][12], [sesh issue #255][13], [tms README][1], user's `dot_tmux.conf`

______________________________________________________________________

## 7. Detailed Comparison Matrix

| Dimension                      | TMS                        | Sesh                           | Winner |
| ------------------------------ | -------------------------- | ------------------------------ | ------ |
| **Marks / instant keybinding** | Built-in, first-class      | Manual run-shell pattern       | TMS    |
| **Session bootstrapping**      | `tms start` from config    | None                           | TMS    |
| **Git worktree windows**       | Automatic creation         | None                           | TMS    |
| **Session rename + dir**       | `tms rename`               | None                           | TMS    |
| **Kill + fallback**            | `tms kill`                 | None                           | TMS    |
| **Startup commands**           | None                       | Per-session or default         | Sesh   |
| **Startup scripts**            | None                       | Full bash scripts              | Sesh   |
| **Window definitions**         | None                       | Per-session with paths/scripts | Sesh   |
| **Zoxide integration**         | None                       | Core feature                   | Sesh   |
| **Last session toggle**        | None                       | `sesh last` (improved)         | Sesh   |
| **Picker ecosystem**           | Built-in only              | fzf/gum/tv/built-in            | Sesh   |
| **Desktop launcher**           | None                       | Raycast, Ulauncher             | Sesh   |
| **Configuration richness**     | Minimal (focused)          | Rich (flexible)                | Sesh   |
| **Config validation**          | None                       | JSON Schema                    | Sesh   |
| **Tmuxp/tmuxinator interop**   | None                       | Yes                            | Sesh   |
| **Community size**             | 666 stars, 21 contributors | 2,275 stars, 30 contributors   | Sesh   |
| **Release cadence**            | ~1 per 3.5 months          | ~1 per 3.5 weeks               | Sesh   |
| **Setup complexity**           | Low                        | Medium                         | TMS    |
| **Binary size / performance**  | Rust (single binary)       | Go (single binary)             | Tie    |
| **VCS support**                | Git + Jujutsu              | Git only                       | TMS    |

______________________________________________________________________

## 8. Recommendation

### For This User: Stay with TMS

The user's workflow is built around a specific pattern: **tmux key tables + tms marks = instant,
no-picker project switching via mnemonic single-key chords.** This pattern is the central productivity
feature, and tms supports it as a first-class, built-in capability.

**Reasons to stay with TMS:**

1. **The marks system is the killer feature.** It is exactly what the user needs, and it works
   seamlessly. Sesh has no equivalent and the maintainer has explicitly rejected adding one.

1. **Single source of truth.** TMS marks live in `config.toml` alongside other tms configuration. With
   sesh, the same information would be split across `sesh.toml` (session definitions) and `tmux.conf`
   (keybindings), creating a maintenance burden.

1. **`tms start` bootstraps sessions.** The user has `[[sessions]]` configured for uriel, openclaw, and
   Homelab that auto-create on tmux startup. Sesh has no equivalent.

1. **Git worktree auto-windowing.** TMS automatically opens checked-out worktrees as tmux windows. This
   is a unique tms feature with no sesh equivalent.

1. **Migration cost is real.** The user has 12 marks, 3 auto-start sessions, 3 bookmarks, and 4 search
   directories configured. Replicating this in sesh would require redefining all sessions in `sesh.toml`,
   rewriting all keybindings in `tmux.conf` to use `sesh connect` instead of `tms marks open`, and giving
   up `tms start` bootstrapping.

1. **TMS is not abandoned.** The last release (v0.5.0) was August 2025, and the last push was February
   2026\. Development is slower than sesh but ongoing.

**When sesh would be the better choice:**

- If the user wanted per-session startup commands (e.g., auto-run `npm run dev` when opening a project).
- If the user wanted zoxide-powered directory discovery instead of explicit `search_dirs`.
- If the user wanted to launch tmux sessions from Raycast.
- If the user primarily used picker-based (fuzzy finding) session switching rather than instant
  keybinding switching.
- If tms development stalled completely.

### Risk Assessment

| Risk                        | Likelihood | Impact | Mitigation                         |
| --------------------------- | ---------- | ------ | ---------------------------------- |
| TMS development stops       | Low-Medium | Medium | Rust binary; works without updates |
| Sesh adds marks feature     | Very Low   | Low    | Maintainer rejected twice          |
| TMS marks system breaks     | Very Low   | High   | Pin version; contribute fixes      |
| Sesh gains critical feature | Medium     | Low    | Re-evaluate if it happens          |

______________________________________________________________________

## Bibliography

______________________________________________________________________

## Methodology Appendix

### Research Phases Executed

1. **SCOPE:** Decomposed the comparison into 6 dimensions (config, UI/UX, community, growth,
   integrations, keybindings). Identified the marks/keybinding workflow as the critical evaluation axis
   based on the user's existing tms configuration.

1. **PLAN:** Identified primary sources (GitHub repos, READMEs, release notes, issues) and secondary
   sources (blog posts, podcasts, community discussions). Planned triangulation via GitHub API metrics,
   issue tracker analysis, and author statements.

1. **RETRIEVE:** Parallel web searches and fetches across 20+ sources. GitHub API queries for precise
   metrics (stars, forks, contributors, commit counts, release counts). Direct inspection of user's live
   tms config and tmux keybindings. Local `tms` CLI help output.

1. **TRIANGULATE:** Cross-referenced GitHub star counts across multiple sources. Verified marks feature
   rejection via two independent issues (#107 and #255) with author responses. Confirmed `sesh connect`
   direct-switch pattern via author's own code example. Validated release dates against GitHub API.

1. **SYNTHESIZE:** Connected configuration structure, keybinding patterns, and maintainer philosophy into
   a coherent recommendation. Identified the "two-file maintenance" problem as a concrete migration cost
   unique to sesh's architecture.

1. **PACKAGE:** Structured report with comparison tables, code examples from the user's actual config,
   and evidence-cited recommendation.

### Sources Consulted

- 2 GitHub repositories (full README, releases, issues, discussions)
- 4 blog posts / articles
- 1 podcast episode reference
- GitHub API (6 endpoints, multiple queries)
- Local filesystem (user's tms config, tmux.conf)
- Local CLI (`tms --help`, `tms marks --help`, `tms marks list`)
- 2 package registries (crates.io, pkg.go.dev)

### Outline Refinement (Phase 4.5)

Initial outline included "Performance benchmarks" as a section. After Phase 3, this was dropped because
neither tool publishes benchmarks and both are compiled single-binaries with negligible startup times.
The keybinding section was expanded from a subsection to a full dedicated section (Section 6) after
discovering the two rejected sesh issues (#107 and #255), which provided critical evidence for the
recommendation.

[1]: https://github.com/jrmoulton/tmux-sessionizer "tmux-sessionizer GitHub Repository"
[2]: https://github.com/joshmedeski/sesh "sesh GitHub Repository"
[3]: https://github.com/joshmedeski/sesh/blob/main/sesh.schema.json "sesh Configuration Schema"
[4]: https://www.joshmedeski.com/posts/smart-tmux-sessions-with-sesh/ "Smart tmux sessions with sesh - Josh Medeski"
[5]: https://www.joshmedeski.com/posts/i-made-my-favorite-tmux-feature-better-with-sesh/ "I made my favorite tmux feature better with sesh - Josh Medeski"
[6]: https://rss.com/podcasts/linkarzu/2014230/ "Replacing Tmux-Sessionizer with Sesh - Linkarzu Podcast"
[7]: https://buzzrag.com/article/why-developers-are-switching-to-sesh-for-tmux-sessions "Why Developers Are Switching to Sesh - Buzzrag"
[8]: https://www.linuxlinks.com/sesh-smart-terminal-session-manager/ "Sesh: Smart Terminal Session Manager - LinuxLinks"
[9]: https://www.raycast.com/joshmedeski/sesh "Sesh - Raycast Store"
[10]: https://github.com/jrmoulton/tmux-sessionizer/releases "tmux-sessionizer Releases"
[11]: https://github.com/joshmedeski/sesh/releases "sesh Releases"
[12]: https://github.com/joshmedeski/sesh/issues/107 "sesh Issue #107: Shortcut per session as configuration"
[13]: https://github.com/joshmedeski/sesh/issues/255 "sesh Issue #255: Add Session Marker System for Activity Tracking"
