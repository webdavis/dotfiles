# Television vs fzf: Deep Research Comparison

**Date:** 2026-04-13 **Mode:** Standard (6-phase pipeline) **Research question:** Should a power user
with deep fzf integration migrate to or complement with Television?

______________________________________________________________________

## Executive Summary

Television (tv) is a Rust-based fuzzy finder (5.6k GitHub stars, 84 contributors, v0.15.5) that
introduces a channel-based architecture for structured fuzzy finding. fzf is the dominant terminal fuzzy
finder (79.4k stars, 320 contributors, v0.71.0) with 12+ years of maturity and an unmatched ecosystem.

**Bottom line:** For a user with 850 lines of custom fzf bindings, deep tmux integration (tmux-fzf-url,
tmux-fuzzback), and established muscle memory, **Television does not justify migration**. It is a
well-designed tool solving a different problem -- batteries-included fuzzy finding with minimal
configuration -- but it cannot match fzf's composability, scripting depth, or ecosystem breadth.
Television may be useful as a **lightweight complement** for a narrow set of use cases (channel browsing
for discovery, cross-shell consistency), but replacing fzf would mean rebuilding 850 lines of custom
functionality from scratch with a less capable action/binding system.

______________________________________________________________________

## 1. Architecture

### fzf: Unix Composability Model

fzf follows the Unix philosophy: it reads lines from stdin, presents a fuzzy-matching interface, and
writes selected lines to stdout. This makes it a universal building block:

```
git branch | fzf --preview 'git log --oneline {}' | xargs git checkout
```

Key architectural properties:

- **Stateless filter:** Input piped in, selection piped out. No internal data model.
- **Event-action binding system:** 250+ bindable actions (`--bind`) turn fzf into a framework for
  building interactive TUI applications (calculators, clipboard managers, LLM chat interfaces).
- **Dynamic reloading:** The `reload` action refreshes candidate lists without restarting, enabling
  two-phase filtering (e.g., ripgrep for initial search, fzf for refinement).
- **Mode switching:** `transform` actions with environment variable access allow toggling between
  different filtering strategies within a single invocation.
- **`become()` action:** Replaces the fzf process with another command, enabling seamless
  selector-to-editor workflows.

### Television: Channel-Based Model

Television inverts the composability model. Instead of piping data through an external filter, it
internalizes data sources as "channels" -- TOML-configured structs that implement an `OnAir` trait:

```toml
[metadata]
name = "tldr"
description = "Search TLDR pages"

[source]
command = "tldr --list"

[preview]
command = "tldr {0}"
```

Key architectural properties:

- **Built-in data sources:** Channels for files, text search, git repos, git branches, git logs,
  environment variables, aliases, docker containers, and more. No shell scripting required.
- **Channel transitions:** Switch between channels mid-session (e.g., start in git-repos, transition to
  files within selected repo, then to text search within those files).
- **Source cycling:** Multiple source commands per channel, switchable via Ctrl+S.
- **Async preview:** Previews render asynchronously via tokio, with built-in syntax highlighting.
- **Cable channels:** User-defined channels via TOML files in `~/.config/television/cable/`.

### Architecture Verdict

fzf is a **composable primitive** -- a building block for constructing arbitrary interactive workflows
via shell scripting. Television is a **self-contained application** -- a batteries-included fuzzy finder
with structured extensibility. These are fundamentally different philosophies.

For a user who has already invested in building custom fzf workflows (git checkout, commit selection,
file browsing with preview, directory navigation, bookmarking), fzf's architecture is the right fit.
Television's channel system would require re-implementing each workflow as a TOML channel definition,
losing the full power of shell scripting in the process.

______________________________________________________________________

## 2. Performance

### Matching Engine

| Aspect    | fzf                                                    | Television                          |
| --------- | ------------------------------------------------------ | ----------------------------------- |
| Language  | Go                                                     | Rust                                |
| Matcher   | Custom (fzf algorithm, unchanged since 2016)           | Nucleo (from Helix editor)          |
| Threading | Shared work queue, linear scaling with cores (v0.71.0) | Tokio async runtime, multi-threaded |

**Nucleo vs fzf algorithm:** The Nucleo matcher uses a two-matrix scoring approach (m and p scores) with
single-character lookahead, providing theoretically better edge-case handling than fzf's greedy approach.
However, as fzf's maintainer junegunn noted: "at the current level of performance, any performance
gain...will be of marginal benefit to users." Both are fast enough that the difference is imperceptible
for typical workloads (thousands to tens of thousands of items).

### Startup and Responsiveness

| Metric               | fzf                                                  | Television                                            |
| -------------------- | ---------------------------------------------------- | ----------------------------------------------------- |
| Startup              | Near-instant (single Go binary)                      | Near-instant (single Rust binary)                     |
| Input responsiveness | Smooth cursor movement                               | Reported lag/jumpiness even at --frame-rate=60        |
| Preview debouncing   | Built-in, prevents excessive updates while scrolling | Async but reported to block interaction in some cases |
| Large file handling  | Handles 100MB+ files with minor degradation          | Reported to "completely fail" on 100MB text files     |

### Performance Verdict

fzf is the more battle-tested performer. Television's Rust/Nucleo foundation is theoretically strong, but
real-world reports indicate responsiveness issues (cursor lag, preview blocking) and scalability problems
with large datasets. fzf's recent v0.71.0 release added 1.26x-1.84x filtering performance improvements
through better thread scheduling. For a power user who processes large git histories and grep results,
fzf's proven performance is a significant advantage.

______________________________________________________________________

## 3. Configuration

### fzf

Configuration is environment-variable-driven:

- `FZF_DEFAULT_COMMAND` -- default input source
- `FZF_DEFAULT_OPTS` -- default options (colors, layout, keybindings, preview)
- `FZF_DEFAULT_OPTS_FILE` -- file-based configuration (newer feature)
- Per-binding overrides via shell functions and `--bind` flags

The user's current setup demonstrates fzf's configuration depth: 850 lines covering Catppuccin theming,
bat-powered previews, vi-mode and emacs-mode key bindings, tmux split/window integration, git operations
(checkout, commit selection, file-at-commit browsing), directory navigation (cwd, home, root, parent,
git-project, bookmarks), and fuzzy grep with line-number navigation.

### Television

Configuration is TOML-based:

- `~/.config/television/config.toml` -- global settings (tick rate, default channel, UI scale,
  orientation, theme, keybindings, shell integration)
- `~/.config/television/cable/*.toml` -- custom channel definitions
- Built-in themes: dracula, nord-dark, catppuccin, gruvbox variants, solarized, tokyonight, rose-pine
- Shell integration triggers: map shell commands to channels (e.g., `git` commands trigger git-branch
  channel, `cd` triggers directory channel)

### Configuration Verdict

Television's TOML configuration is more structured and accessible for simple use cases. fzf's
environment-variable and shell-function approach is more powerful but requires deeper shell scripting
knowledge. For the user's level of customization (850 lines of bash with helper functions, readline
bindings, tmux integration), fzf's configuration model is the only viable option -- Television's TOML
channels cannot express the same level of programmatic control (conditional logic, readline manipulation,
`__insert_text_at_cursor`, dynamic command construction).

______________________________________________________________________

## 4. UI/UX

| Feature               | fzf                                                  | Television                                     |
| --------------------- | ---------------------------------------------------- | ---------------------------------------------- |
| Preview support       | Via `--preview` flag + external commands (bat, etc.) | Built-in with syntax highlighting              |
| Multi-select          | Yes (Tab/Shift-Tab, `--multi`)                       | Yes (Tab)                                      |
| Layout modes          | Fullscreen, `--height`, `--popup` (tmux/Zellij)      | Landscape, portrait, configurable scale        |
| Theming               | 20+ color attributes via `--color`                   | Named themes (catppuccin, dracula, nord, etc.) |
| Preview debouncing    | Built-in                                             | Async but less mature                          |
| Word wrapping         | Yes (v0.68.0, list + preview)                        | Yes (v0.14.5, preview panel)                   |
| Popup/overlay         | tmux popup + Zellij floating pane                    | No equivalent                                  |
| Status indicators     | Header, footer, info line                            | Status bar, help panel                         |
| Copy to clipboard     | Via `execute` binding                                | Ctrl+Y built-in                                |
| Remote control picker | N/A (manual mode switching)                          | Ctrl+T channel picker                          |
| Frecency sorting      | N/A                                                  | Yes (v0.15.0)                                  |

### UI/UX Verdict

Television provides a more polished out-of-the-box experience with named themes, built-in previews, and
channel switching. fzf offers more granular control over every visual element and integrates more
naturally with tmux popups. The user already has Catppuccin theming, bat previews, and sophisticated
keybindings configured for fzf -- Television's UI conveniences provide no incremental value.

______________________________________________________________________

## 5. Integration Ecosystem

### fzf Ecosystem (Massive)

**Shell integration:**

- Bash, Zsh, Fish -- first-class support
- Ctrl+T (file finder), Ctrl+R (history), Alt+C (directory changer)
- Fuzzy completion for files, processes, hostnames, environment variables

**Tmux integration:**

- `fzf-tmux` wrapper / `--popup` flag for tmux popup windows
- tmux-fzf-url -- fuzzy-open URLs from terminal scrollback
- tmux-fuzzback -- fuzzy-search terminal scrollback buffer
- tmux-fzf -- manage tmux sessions/windows/panes via fzf
- forgit -- interactive git commands via fzf

**Editor integration:**

- fzf.vim / fzf-lua (Neovim) -- deeply integrated file/buffer/grep/command finders
- telescope.nvim was inspired by fzf's paradigm

**Third-party tools:**

- Hundreds of tools pipe through fzf: zoxide, autojump, pass, bitwarden-cli, kubectl, docker, etc.
- Any tool that outputs text can be piped through fzf

### Television Ecosystem (Growing)

**Shell integration:**

- Bash, Zsh, Fish, Nushell, PowerShell
- Ctrl+T (smart autocomplete), Ctrl+R (history search)
- Context-aware channel triggering (typing `cd` activates directory search)

**Editor integration:**

- tv.nvim -- Neovim plugin (by the author)
- tv.vim -- Vim plugin
- VSCode extension
- Zed integration

**No tmux-specific plugins exist.** Television does not have equivalents for tmux-fzf-url, tmux-fuzzback,
or tmux-fzf.

### Ecosystem Verdict

fzf's ecosystem is overwhelmingly larger. The user relies on tmux-fzf-url and tmux-fuzzback -- neither
has a Television equivalent. The hundreds of tools that pipe through fzf (zoxide, forgit, etc.) have no
Television counterparts. Television's editor integrations are growing but are far less mature than
fzf.vim/fzf-lua. Television's broader shell support (Nushell, PowerShell) is irrelevant for a bash+tmux
user.

______________________________________________________________________

## 6. Community and Maturity

| Metric          | fzf                     | Television                                    |
| --------------- | ----------------------- | --------------------------------------------- |
| GitHub stars    | 79,400                  | 5,600                                         |
| Contributors    | 320                     | 84                                            |
| First release   | 2013                    | Late 2024                                     |
| Age             | 12+ years               | ~1.5 years                                    |
| Latest version  | 0.71.0 (Apr 4, 2026)    | 0.15.5 (Apr 8, 2026)                          |
| Release cadence | Monthly (consistent)    | Monthly (consistent, sometimes more frequent) |
| Pre-1.0         | No (mature, stable API) | Yes (API still evolving)                      |
| Commits         | 3,588                   | ~1,500 (estimated)                            |
| Language        | Go                      | Rust                                          |
| License         | MIT                     | MIT                                           |

### Maturity Assessment

fzf is one of the most successful developer tools on GitHub. Its API is stable, its maintainer (junegunn)
is deeply committed, and breaking changes are exceedingly rare. Television is pre-1.0, actively adding
20+ channels per release, and its configuration format is still evolving. The rapid development pace is a
double-edged sword: features arrive quickly, but the upgrade path may require configuration changes.

Television shows healthy project trajectory: consistent monthly releases, growing contributor base, and
the author is responsive to community feedback. However, it is fundamentally a young project that has not
yet proven long-term stability.

______________________________________________________________________

## 7. Unique Features

### What Television Can Do That fzf Cannot

1. **Channel transitions:** Navigate through a chain of data sources (git-repos -> files -> text) within
   a single session, with results flowing between stages. fzf requires separate invocations or complex
   `reload` scripting.

1. **Declarative channel definitions:** Define new fuzzy-finding workflows in TOML without shell
   scripting. Lower barrier to entry for simple use cases.

1. **Context-aware shell integration:** Automatically selects the appropriate channel based on the
   current command being typed (e.g., `cd` triggers directory search, `git checkout` triggers branch
   search). fzf's Ctrl+T always uses the same finder.

1. **Frecency sorting:** Built-in frecency-based result ordering (v0.15.0). fzf has no native frecency
   support.

1. **Built-in channel browsing:** Ctrl+T opens a channel picker -- effectively a "meta-fuzzy-finder" for
   choosing what to fuzzy-find.

1. **Community channel repository:** `tv update-channels` pulls community-contributed channels. fzf has
   no equivalent package system.

### What fzf Can Do That Television Cannot

1. **Event-action binding framework:** 250+ actions bindable to keys and events. Build calculators,
   clipboard searchers, LLM chat interfaces. Television's action system is limited to executing external
   commands on selection.

1. **`become()` action:** Replace the fzf process with another command, enabling seamless transitions
   (e.g., fzf becomes vim at the selected file and line). No Television equivalent.

1. **Dynamic reload with mode switching:** Toggle between ripgrep-powered search and fzf fuzzy matching
   within a single session using `transform` actions and environment variables. Television has source
   cycling but not this level of conditional logic.

1. **tmux popup integration:** `--popup` launches fzf in a tmux popup or Zellij floating pane. Television
   has no terminal multiplexer integration.

1. **Readline integration:** fzf's shell bindings manipulate the readline buffer directly
   (`READLINE_LINE`, `READLINE_POINT`). The user's 850-line config extensively uses this for
   `__insert_text_at_cursor`. Television's shell integration is simpler (Ctrl+T/Ctrl+R only).

1. **Massive third-party ecosystem:** tmux-fzf-url, tmux-fuzzback, forgit, zoxide integration, fzf.vim,
   fzf-lua, and hundreds more. Television has a fraction of this.

1. **Proven scalability:** Handles 100MB+ files, millions of lines. Television has reported failures on
   large datasets.

1. **Preview debouncing:** Mature handling of rapid scrolling through results without preview lag.

______________________________________________________________________

## 8. Migration Analysis: Specific to Your Setup

### What Would Be Lost in Migration

Your 850-line `dot_fzf_bindings` file implements the following workflows that have **no Television
equivalent**:

| Workflow                                              | Lines        | Television Equivalent                         |
| ----------------------------------------------------- | ------------ | --------------------------------------------- |
| `__insert_text_at_cursor` (readline manipulation)     | Core helper  | None -- Television cannot manipulate readline |
| `__build_edit_command` (dynamic command construction) | 50+          | Partial -- TOML actions are static            |
| Ctrl+T f/F/h/ (file opening in cwd/home/root)         | 40           | Built-in files channel (partial)              |
| Ctrl+G Ctrl+G (fuzzy grep + open at line)             | 30           | Built-in text channel (partial, no line jump) |
| Ctrl+G Ctrl+F / Ctrl+G ff (git file selection)        | 30           | Built-in git-files channel (partial)          |
| Ctrl+G h (git commit hash selection)                  | 40           | Built-in git-log channel (partial)            |
| Ctrl+G kk/kc (git checkout branch/commit)             | 50           | Built-in git-branch channel (partial)         |
| Ctrl+G fc (file from previous commit)                 | 50           | No equivalent                                 |
| Ctrl+T dd/dg/d/ (directory navigation)                | 50           | Built-in dirs channel (partial)               |
| Ctrl+T dp (parent directory navigation)               | 30           | No equivalent                                 |
| Ctrl+T Ctrl+B / Ctrl+T b (bookmarking)                | 30           | No equivalent                                 |
| Tmux split/window/new-tab actions via key modifiers   | Core feature | No equivalent                                 |
| Catppuccin theming                                    | 10           | Built-in catppuccin theme                     |
| bat preview with empty/binary file detection          | 10           | Built-in preview (different)                  |

**Critical gap:** Your bindings use tmux-aware key modifiers (Ctrl+X splits vertically, Ctrl+V splits
horizontally, Ctrl+T opens new window) on virtually every fzf operation. Television has no tmux awareness
at all.

### What Would Be Gained

- Frecency sorting on file/directory results
- Channel transitions (start in git-repos, narrow to files, then text)
- Context-aware Ctrl+T (different behavior for `cd` vs `vim` vs `git checkout`)
- Named theme support without hex color codes
- Community channel repository

### Tmux Plugin Dependencies

| Plugin        | fzf Required | Television Alternative |
| ------------- | ------------ | ---------------------- |
| tmux-fzf-url  | Yes          | None                   |
| tmux-fuzzback | Yes          | None                   |

Both plugins are hard dependencies on fzf. Migration would mean losing URL-opening and scrollback-search
functionality entirely.

______________________________________________________________________

## 9. Recommendation

### Primary Recommendation: Do Not Migrate

Television does not offer enough advantage to justify migrating from a mature, deeply customized fzf
setup. The specific reasons:

1. **Investment protection:** 850 lines of battle-tested, tmux-integrated fzf bindings represent
   significant engineering effort. Television cannot replicate this functionality due to architectural
   differences (no readline manipulation, no tmux awareness, limited action system).

1. **Ecosystem lock-in:** tmux-fzf-url and tmux-fuzzback have no Television equivalents. These are daily
   workflow tools.

1. **Performance risk:** Television has reported issues with large files and input responsiveness that
   fzf does not have.

1. **Maturity gap:** Television is pre-1.0 and ~1.5 years old. fzf is 12+ years mature with a stable API.
   Configuration breakage during upgrades is a real concern with Television.

1. **fzf continues to improve:** v0.71.0 added Zellij support, 1.8x performance improvements, and
   cross-reload item tracking. The project is not stagnating.

### Secondary Recommendation: Consider as a Narrow Complement

Television could complement fzf for two specific use cases:

1. **Discovery/exploration:** When you want to browse through various data sources (docker containers,
   tailscale nodes, environment variables) without writing shell scripts, `tv` with its channel picker is
   faster to set up than a new fzf pipeline.

1. **Quick one-off searches:** `tv text` for ripgrep-powered text search with built-in preview is
   arguably faster to invoke than constructing an fzf pipeline from scratch, though your Ctrl+G Ctrl+G
   binding already does this.

Installation would be trivial (`brew install television`) and would not conflict with fzf. The `tv`
binary is independent. However, do not configure Television's shell integration (Ctrl+T/Ctrl+R) as it
would conflict with your existing fzf bindings.

### If You Do Install Television

```bash
brew install television
# Do NOT run: eval "$(tv init bash)"  -- this would conflict with fzf's Ctrl+T/Ctrl+R
# Use tv directly: tv files, tv text, tv git-repos, tv env
```

Add to `.chezmoidata/system_packages_autoinstall.yaml` under `formulae` only if you find yourself using
it regularly after a trial period.

______________________________________________________________________

## Bibliography

### Primary Sources

1. Television GitHub Repository. alexpasmantier/television. https://github.com/alexpasmantier/television
   Accessed 2026-04-13. v0.15.5, 5.6k stars, 84 contributors.

1. Television Documentation. alexpasmantier. https://alexpasmantier.github.io/television/ Accessed
   2026-04-13. Official docs covering channels, configuration, shell integration.

1. fzf GitHub Repository. junegunn/fzf. https://github.com/junegunn/fzf Accessed 2026-04-13. v0.71.0,
   79.4k stars, 320 contributors.

1. fzf Advanced Usage. junegunn/fzf. https://github.com/junegunn/fzf/blob/master/ADVANCED.md Accessed
   2026-04-13. Event-action binding framework, dynamic reload, become action.

1. fzf Release Notes: 0.71.0. junegunn. https://junegunn.github.io/fzf/releases/0.71.0/ Apr 4, 2026.
   Performance improvements, Zellij integration, --popup rename.

### Community Discussions

6. "Television: Fast general purpose fuzzy finder TUI." Hacker News.
   https://news.ycombinator.com/item?id=42651487 Jan 2025. Performance reports, fzf comparison,
   large-file failure reports.

1. "tv isn't intended to be a direct competitor to fzf." Hacker News.
   https://news.ycombinator.com/item?id=42654835 Jan 2025. Author's positioning of Television relative to
   fzf.

1. "Thoughts about Nucleo?" junegunn/fzf Discussion #3491.
   https://github.com/junegunn/fzf/discussions/3491 Performance comparison between Nucleo and fzf's
   matching algorithm.

1. Television Ratatui Forum Discussion.
   https://forum.ratatui.rs/t/television-a-general-purpose-fuzzy-finder-tui/141 Community feedback on UI
   design, documentation, and architecture.

### Reviews and Blog Posts

10. "I Switched From Fzf to Television: Declarative Channels for Cross-Shell Fuzzy Finding." GnixAij.
    https://vluv.space/television/ Multi-shell user perspective; highlights cross-shell consistency as
    primary advantage.

01. "Television: a modern fuzzy finder for the terminal." Xin Fu.
    https://imfing.com/til/television-modern-fuzzy-finder-terminal/ Overview of channels system and Zed
    integration.

01. "Television - Fuzzy Finder for Files, Text, Git Logs and More." Kostiantyn Lysenko.
    https://lysenko.dev/posts/2025-02-television-fuzzy-finder/ Brief introduction to channels and
    installation.

01. "Television - A fuzzy finder with channels." Fryboyter.
    https://fryboyter.de/en/television-a-fuzzy-finder-with-channels/ Independent review (access
    restricted during research).

### Technical References

14. Television on crates.io. https://crates.io/crates/television Rust package metadata, dependency graph.

01. Television on lib.rs. https://lib.rs/crates/television Rust ecosystem listing.

01. tmux-fzf-url. wfxr/tmux-fzf-url. https://github.com/wfxr/tmux-fzf-url fzf-dependent tmux plugin for
    URL extraction.

01. tmux-fuzzback. roosta/tmux-fuzzback. https://github.com/roosta/tmux-fuzzback fzf-dependent tmux
    plugin for scrollback search.

______________________________________________________________________

## Methodology Appendix

### Research Pipeline

This report followed the standard 6-phase deep research methodology:

1. **SCOPE:** Defined comparison axes (architecture, performance, configuration, UI/UX, ecosystem,
   community, unique features) with special attention to migration feasibility for a power user with 850
   lines of custom fzf configuration.

1. **PLAN:** Identified primary sources (GitHub repos, official docs, release notes) and secondary
   sources (Hacker News discussions, blog reviews, crate registries). Planned triangulation across author
   claims, community reports, and independent reviews.

1. **RETRIEVE:** Parallel web searches (8 queries) and web fetches (12 URLs) covering both tools. Local
   file analysis of user's `dot_fzf_bindings` (850 lines), `dot_bashrc.tmpl`, and `dot_tmux.conf` to
   understand integration depth.

1. **TRIANGULATE:** Cross-referenced performance claims against community reports (Hacker News users
   contradicted some of Television's speed claims). Verified feature lists against official
   documentation. Confirmed ecosystem gap (no tmux plugins for Television) across multiple sources.

1. **SYNTHESIZE:** Connected architectural differences (composable primitive vs. self-contained app) to
   practical migration implications. Identified the readline manipulation gap as the critical blocker.

1. **PACKAGE:** Structured report with executive summary, detailed comparison tables, migration analysis
   specific to user's setup, and actionable recommendation.

### Outline Refinement (Phase 4.5)

After triangulation, the original outline was adapted to add a dedicated "Migration Analysis" section
(Section 8) that maps the user's specific fzf bindings to Television equivalents. This emerged as the
most decision-relevant section after discovering that Television's architectural limitations (no readline
manipulation, no tmux awareness) make migration infeasible regardless of feature comparisons.

### Source Quality

- 17 sources consulted across 5 source types (repositories, documentation, community forums, blog posts,
  package registries)
- Core claims verified across 3+ independent sources
- Author claims (Television "batteries-included," "fast") partially contradicted by community reports
  (input lag, large-file failures)
- No sponsored or promotional content identified in sources
