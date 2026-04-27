# Karl Davis Dotfiles Review

**Repo:** https://github.com/karlmdavis/dotfiles **Branch:** `refine-claude-skills` **Date:** 2026-04-12
**Reviewed by:** Claude Code at Stephen's request

## Executive Summary

Karl's dotfiles repo is a chezmoi-managed setup targeting macOS and Ubuntu, centered around **nushell**,
**Zellij**, **Helix**, and **iTerm2** -- a fundamentally different stack from Stephen's
bash/tmux/neovim/Ghostty setup. The most interesting and transferable elements are:

1. A sophisticated **Claude Code skill and command system** for automated quality triage
1. A **progressive notification escalation system** (desktop -> mobile push via ntfy.sh)
1. **Claude Code hooks** in settings.json for Stop and Notification events
1. A **Claude Code Review GitHub Action** for automated PR reviews
1. Templated `settings.json` and `settings.local.json` for Claude Code
1. A **subagent architecture** pattern using TOON (Token-Oriented Object Notation)
1. Chezmoi **auto-commit and auto-push** for source directory changes
1. **psqlrc** with timing, custom prompts, and pager configuration

______________________________________________________________________

## 1. Claude Code Skills & Commands System (HIGHEST VALUE)

Karl has built an elaborate, multi-layered skill system for Claude Code that Stephen does not have. This
is deployed via chezmoi to `~/.claude/skills/` and `~/.claude/commands/`.

### 1.1 Custom Slash Commands (deployed to `~/.claude/commands/`)

Three global slash commands available in every project:

- **`/quality-triage <scope>`** -- Runs local CI + code review, triages all issues interactively.
  Supports scopes: `everything`, `uncommitted`, `branch`, `branch-dirty`.
- **`/quality-triage-pr`** -- Same concept but for PR feedback: waits for GitHub Actions workflows to
  finish, fetches build logs and review comments, triages interactively.
- **`/pr-merge`** -- Squash merges the current PR via `gh`, switches to main, pulls, deletes local
  branch. Simple but saves time.

**What Stephen could adopt:** The `/quality-triage` pattern of running local CI, parsing results, and
interactively presenting issues with priority ordering. The `/pr-merge` convenience command is trivially
adoptable.

### 1.2 Skill Architecture (10 skills, layered)

Karl organizes skills into three layers:

**Layer 1 -- Parsing (reusable, source-agnostic):**

- `parsing-build-results` -- Extracts failures from raw build logs (Jest, pytest, cargo, tsc, etc.) with
  file:line locations and relatedness analysis (is this failure in a file I changed?)
- `parsing-review-suggestions` -- Structures review feedback from Claude bot comments, GitHub PR reviews,
  and unresolved threads into severity-categorized issues

**Layer 2 -- Fetching (source-specific):**

- `getting-build-results-local` -- Reads project's CLAUDE.md to discover CI commands, runs them
- `getting-build-results-remote` -- Fetches GitHub Actions logs via `gh` API
- `getting-review-local` -- Performs code review of local changes
- `getting-reviews-remote` -- Fetches Claude bot comments, GitHub reviews, unresolved threads using
  sophisticated timestamp filtering (commit push time, not creation time)
- `awaiting-pr-workflow-results` -- Polls GitHub Actions with exponential backoff (up to 20 min)
- `getting-branch-state` -- Single source of truth for branch/PR state (base detection,
  ahead/behind/diverged, changed files)

**Layer 3 -- Orchestration:**

- `getting-feedback-local` -- Combines local build + review in subagent
- `getting-feedback-remote` -- Combines workflow results + reviews in subagent

**Cross-cutting:**

- `addressing-feedback-interactively` -- The main user-facing skill. Presents issues by priority, lets
  user choose commit strategy (incremental/accumulated/manual), works through fixes with verification
  after each change. Detects aligned issues (build failure + review pointing to same root cause) and
  presents them together.
- `using-zellij-docs` -- Forces Claude to read actual config before answering Zellij questions (prevents
  hallucinated keybindings). Not relevant to Stephen's setup but the pattern is good.

**What Stephen could adopt:**

- The layered skill architecture pattern itself
- `getting-branch-state` as a universal utility skill
- `addressing-feedback-interactively` commit strategy chooser (incremental vs accumulated)
- The "always run feedback gathering in subagent to save tokens" pattern
- Relatedness analysis (is this build failure in a file I changed?)

### 1.3 Subagent Architecture

Karl uses a dedicated `quality-data-extractor` agent definition
(`private_dot_claude/agents/quality-data-extractor.md`) with:

- Restricted tool access
- Model set to `sonnet` (cheaper for data extraction)
- Comprehensive system prompt covering TOON format, mise/bun/uv build tool patterns
- Used as the `subagent_type` when spawning Task tool calls

**Key pattern:** Raw logs (50k+ tokens) never enter main conversation context. Subagent extracts
structured TOON summary (2-5k tokens), which is all the main context sees.

### 1.4 TOON (Token-Oriented Object Notation)

Karl uses a custom data format called TOON for structured skill output. Claims ~40% token reduction vs
JSON. Key features:

- YAML-style nested objects
- Arrays with length declarations: `tags[3]: python,testing,automation`
- Tabular arrays with schema: `users[2]{id,name,email}:`
- Uses a PyPI package `toon-format==0.9.0b1` in Python scripts

**What Stephen could adopt:** The general concept of a token-efficient structured format for
skill-to-skill communication, though Stephen could also just use YAML.

### 1.5 Python Skill Scripts

Three Python scripts back the skills, all using `#!/usr/bin/env -S uv run` with inline dependencies (PEP
723 script metadata):

- `check_branch_state.py` -- Git/GH CLI wrapper, outputs TOON or JSON
- `check_pr_workflows.py` -- Polls workflow status with exponential backoff
- `fetch_workflow_logs.py` -- Fetches logs for failed workflow jobs only
- `fetch_pr_reviews.py` -- Fetches Claude bot comments, GH reviews, unresolved threads with GraphQL
  pagination for review threads

All scripts have:

- Proper timeout handling (60s per subprocess)
- Structured error handling with stderr logging
- TypedDict for output type safety
- Comprehensive docstrings

**What Stephen could adopt:** The `uv run` inline dependency pattern for self-contained scripts. The
`check_branch_state.py` and `check_pr_workflows.py` utilities are project-agnostic and useful.

______________________________________________________________________

## 2. Workflow Notification System (wkflw-ntfy V2) (HIGH VALUE)

A full notification system with 20+ composable bash scripts deployed to `~/.local/lib/wkflw-ntfy/`.

### 2.1 Architecture

Unix philosophy design -- each script does one thing:

- **Core:** Config loading, structured logging, environment detection, strategy selection
- **Markers:** Atomic marker files for escalation tracking (mv-based atomic claim)
- **macOS:** Desktop notifications via `terminal-notifier`, window focusing via AppleScript
- **Linux:** Desktop notifications via `notify-send`
- **Push:** Mobile push via ntfy.sh
- **Escalation:** Background workers that wait, check markers, escalate if unacknowledged
- **Hooks:** Integration with Claude Code and nushell

### 2.2 Progressive Escalation

Flow: Desktop notification -> wait 120s -> if marker still exists (user didn't click) -> send mobile push
via ntfy.sh.

Marker-based atomic claim pattern:

1. Create marker file with JSON metadata (session_id, event_type, timestamp, pid, cwd)
1. Desktop notification callback deletes marker on click
1. Background escalation worker sleeps, then attempts `mv marker marker.claimed`
1. Only one process (callback or worker) can successfully `mv` -- kernel guarantees atomicity
1. Worker only sends push if it won the race

### 2.3 Integration Points

**Claude Code hooks** (configured in `settings.json.tmpl`):

```json
"hooks": {
  "Stop": [{"hooks": [{"type": "command", "command": "~/.local/lib/wkflw-ntfy/hooks/claude-stop.sh"}]}],
  "Notification": [{"hooks": [{"type": "command", "command": "~/.local/lib/wkflw-ntfy/hooks/claude-notification.sh"}]}]
}
```

The Stop hook fires when Claude finishes. The Notification hook fires for permission prompts and idle
input. Both parse JSON from stdin, detect environment, choose strategy, and dispatch.

**Nushell hooks** (in config.nu): `pre_execution` hook records command start time, `pre_prompt` hook
calculates duration and calls bash handler if above threshold (90s default). Filters interactive commands
(vim, htop, etc).

**What Stephen could adopt:**

- Claude Code Stop/Notification hooks in settings.json (Stephen doesn't have these)
- ntfy.sh integration for mobile push notifications
- Progressive escalation pattern (desktop -> mobile)
- The composable notification architecture could work with bash/tmux instead of nushell/zellij
- Stephen could add a `PROMPT_COMMAND` hook in bash equivalent to the nushell hooks

### 2.4 Testing

Comprehensive bats test suite (12 test files) with:

- Mock executables for osascript, terminal-notifier, curl, notify-send
- PATH manipulation to inject mocks
- Tests for all core components, markers, platform support, push, escalation

**What Stephen could adopt:** The testing approach with bats + mock executables. Stephen's current
lint.sh doesn't include bash unit tests.

______________________________________________________________________

## 3. Claude Code Configuration (MEDIUM-HIGH VALUE)

### 3.1 Templated settings.json

Karl templates `settings.json` via chezmoi (`private_dot_claude/settings.json.tmpl`) to:

- Inject `$HOME` paths for hook commands
- Inject paths for skill reads

Stephen's `dot_claude/settings.json` is NOT templated. Templating would let Stephen:

- Reference absolute paths portably
- Conditionally include platform-specific permissions
- Pull secrets if needed

### 3.2 settings.local.json

Karl has a separate `settings.local.json.tmpl` for machine-specific overrides. Stephen does not have this
file. It's currently minimal (empty permissions) but the pattern is good for truly local settings.

### 3.3 Comprehensive Permission Allow List

Karl's settings.json includes extensive pre-approved permissions organized by category:

- Git operations (add, rm, log, ls-tree, remote get-url, fetch, pull, commit)
- GitHub CLI (pr list/checks/view/diff/ready/create, run view/list, workflow, issue)
- Unix utilities (mkdir, jq, grep, head, cat, tree, find, awk, tee, sed, cp, ping, xargs, yq)
- Rust tools (cargo, rustc)
- Python tools (uv sync, uv run pytest/black/ruff/mypy)
- Node tools (npm test, npm run type-check, npx tsc, bun)
- Data tools (csvcut, unzip)
- DevOps (ansible-doc, yamllint, aws ec2 describe-\*)
- Mobile (xcrun simctl, lcov)
- Web domains (docs.helix-editor.com, crates.io, docs.rs, github.com, etc.)
- Skills (superpowers:\*)
- Slash commands
- MCP tools

**Deny list:**

```json
"Read(./.env)", "Read(./.env.*)", "Read(./secrets/**)",
"Read(**/credentials.json)", "Read(**/.aws/credentials)", "Read(**/.ssh/id_*)"
```

**What Stephen could adopt:**

- The deny list pattern for sensitive files (Stephen has some but Karl's is more comprehensive)
- Pre-approving more tools to reduce prompting
- The organizational comment structure (Git Operations, GitHub CLI, etc.)

### 3.4 Plugin Configuration

Karl uses the "superpowers" plugin from `obra/superpowers-marketplace`:

```json
"enabledPlugins": {"superpowers@superpowers-marketplace": true}
```

And manages `known_marketplaces.json` for the marketplace source.

Karl's settings also include:

```json
"alwaysThinkingEnabled": true,
"cleanupPeriodDays": 36525
```

The `cleanupPeriodDays` of 36525 (100 years) effectively disables auto-cleanup of conversations.

**What Stephen could adopt:**

- `alwaysThinkingEnabled: true` if not already set
- `cleanupPeriodDays: 36525` to prevent losing conversation history
- Superpowers plugin if useful

______________________________________________________________________

## 4. GitHub Actions (MEDIUM VALUE)

### 4.1 Claude Code Review Action (`claude-code-review.yml`)

Automatically runs Claude Code on every PR (opened/synchronize):

- Uses `anthropics/claude-code-action@v1`
- Sticky comment (updates same comment on new pushes)
- Custom prompt for code quality, bugs, performance, security, test coverage
- Restricted tool access (only gh read commands + pr comment)

**What Stephen could adopt:** This is a drop-in workflow. Stephen would need a `CLAUDE_CODE_OAUTH_TOKEN`
secret. Gets free automated PR reviews on every push.

### 4.2 Claude Code Action (`claude.yml`)

Responds to `@claude` mentions in issues and PR comments:

- Triggers on issue comments, PR review comments, PR reviews, new issues
- Uses `anthropics/claude-code-action@v1`
- Allows Claude to read CI results (`actions: read`)

**What Stephen could adopt:** Same as above -- useful for interacting with Claude directly from GitHub
issues/PRs.

______________________________________________________________________

## 5. Chezmoi Configuration Patterns (MEDIUM VALUE)

### 5.1 Auto-Commit and Auto-Push

Karl's `.chezmoi.toml.tmpl`:

```toml
[git]
    autoCommit = true
    autoPush = true
```

Every `chezmoi apply` automatically commits and pushes changes to the source directory. Stephen does NOT
have this.

**Trade-off:** Convenience vs surprise commits. Karl notes it's "the safer move" for dotfiles. Stephen
might want this for his dotfiles repo.

### 5.2 System Type Prompt

Karl prompts for `systemType` (personal/cms) during `chezmoi init`, setting `.isCMS` variable used
throughout templates. This is similar to Stephen's approach but with a different use case (work vs
personal machine).

### 5.3 Chezmoi Template Reuse

Karl uses `.chezmoitemplates/config.nu` as a shared template included by both macOS and Linux config
paths:

```
{{- template "config.nu" . -}}
```

This avoids duplicating nushell config across the two OS-specific paths. Stephen could use this pattern
for any config that needs platform-specific file paths but identical content.

______________________________________________________________________

## 6. Starship Configuration (LOW-MEDIUM VALUE)

### 6.1 Starship Lite Variant

Karl has TWO Starship configs:

- `starship.toml` -- Full Tokyo Night preset with nerd font icons, git metrics, cmd_duration
- `starship-lite.toml` -- Stripped-down version for terminals without nerd font support (e.g.,
  `TERM=linux` on Ubuntu Server console)

Nushell config auto-selects based on TERM variable.

**What Stephen could adopt:** The lite variant pattern could be useful for Stephen's mosh prompt
situation -- he already has a separate mosh config but the auto-detection pattern is cleaner.

### 6.2 Git Metrics in Prompt

Karl enables `git_metrics` (disabled by default in Starship) showing `+added -deleted` counts in the
prompt.

**What Stephen could adopt:** Enable `git_metrics` in Starship config.

### 6.3 Extended Command Timeout

Karl sets `command_timeout = 2000` (2 seconds, default is 500ms) because "git seems to run a bit too slow
on my work laptop."

______________________________________________________________________

## 7. Git Configuration (LOW VALUE)

### 7.1 git find-merge / show-merge Aliases

Karl has aliases to find which merge commit included a given commit:

```
find-merge = "!sh -c 'commit=$0 && branch=${1:-HEAD} && ...'"
show-merge = "!sh -c 'merge=$(git find-merge $0 $1) && [ -n \"$merge\" ] && git show $merge'"
```

Stephen has more advanced git config (GPG signing, delta, rerere, autosquash) but might find these
aliases useful.

### 7.2 push.autoSetupRemote

Karl has `push.autoSetupRemote = true` which automatically sets up tracking on first push. Stephen likely
already has this but worth verifying.

______________________________________________________________________

## 8. psqlrc (LOW VALUE but UNIQUE)

Karl has a `dot_psqlrc` with:

- Timing always on (`\timing on`)
- Custom prompts with ANSI colors showing timestamp and database name
- Pager configured with `less --chop-long-lines`

Stephen doesn't have a psqlrc. If Stephen uses PostgreSQL, this is worth adopting.

______________________________________________________________________

## 9. Documentation Practices (INFORMATIONAL)

### 9.1 Markdown Style Guide in CLAUDE.md

Karl's CLAUDE.md includes detailed markdown formatting rules:

- One sentence per line (for better git diffs)
- 110-char wrap limit at natural break points
- Specific indent rules for wrapped prose, list items, and checklist items
- All lines end with periods

### 9.2 docs/ Directory Structure

Karl organizes docs into:

- `design-product/` -- Product design specs
- `design-engineering/` -- Architecture and technical decisions
- `implementation-plans/` -- Step-by-step implementation plans
- `analysis/` -- Research and experiments
- `notes/` -- General notes

All dated with `YYYY-MM-DD-short-name` format.

### 9.3 CLAUDE.md "Evergreen" Guidance

Karl's CLAUDE.md begins with:

> **Important**: Keep this file evergreen. Avoid adding point-in-time content (current sprint goals,
> active branches, temporary workarounds) that wouldn't make sense if multiple workstreams, PRs, or
> branches were in progress simultaneously.

This is good advice Stephen could add to his own CLAUDE.md.

______________________________________________________________________

## 10. Things Karl Does NOT Have That Stephen Does

For completeness, areas where Stephen's setup is more advanced:

- **No fzf integration** (Stephen has 800+ lines of fzf customization)
- **No zoxide** (Stephen uses zoxide for directory jumping)
- **No Atuin** (Stephen uses Atuin for shell history)
- **No neovim/vim advanced config** (Karl uses Helix)
- **No window manager** (Stephen has AeroSpace; Karl has i3/sway configs but those appear to be
  Linux-only and basic)
- **No Karabiner** (Stephen has extensive keyboard remapping)
- **No tmux advanced config** (Karl's tmux.conf is basic -- he primarily uses Zellij)
- **No Espanso** (text expansion)
- **No smart home integration** (Stephen has OpenHue)
- **No osquery monitoring**
- **No email client** (Stephen has himalaya)
- **No Nix flake** for reproducible dev environment (Karl uses mise instead)
- **No GPG signing** for git commits
- **No delta** for git diff
- **No rerere/autosquash** git config
- **No sesh/tms** session management
- **No KeePassXC** secrets management in templates

______________________________________________________________________

## Prioritized Recommendations for Stephen

### Tier 1 -- High Impact, Moderate Effort

1. **Add Claude Code hooks** (Stop + Notification) to `settings.json` for task completion awareness
1. **Add ntfy.sh mobile push** notifications for long-running tasks (could reuse Karl's pattern adapted
   for bash/tmux)
1. **Template settings.json** via chezmoi for portable path references
1. **Add Claude Code Review GitHub Action** for automated PR reviews
1. **Add `/pr-merge` slash command** -- simple, high-frequency time saver

### Tier 2 -- Medium Impact, Higher Effort

6. **Build quality triage skills** adapted from Karl's pattern (but for Stephen's bash-centric stack)
1. **Add `cleanupPeriodDays: 36525`** and `alwaysThinkingEnabled: true` to Claude settings
1. **Add deny rules** for .env, credentials, SSH keys in Claude settings
1. **Enable `git_metrics`** in Starship config
1. **Add `push.autoSetupRemote = true`** to gitconfig if not present

### Tier 3 -- Low Impact or Situational

11. **Consider chezmoi auto-commit/auto-push** for the dotfiles repo
01. **Add psqlrc** if using PostgreSQL
01. **Add git find-merge/show-merge aliases**
01. **Add "evergreen" guidance** to CLAUDE.md header
01. **Add starship-lite variant** for degraded terminal situations
