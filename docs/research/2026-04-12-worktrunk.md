# Worktrunk Research Report

**Date:** 2026-04-12
**Repo:** [max-sixty/worktrunk](https://github.com/max-sixty/worktrunk) (4,349 stars, 145 forks,
Rust, MIT/Apache-2.0)
**Docs:** [worktrunk.dev](https://worktrunk.dev/)
**Version at time of research:** 0.37.0

---

## 1. What Worktrunk Is

Worktrunk is a CLI for git worktree management, designed for running AI agents in parallel. It wraps
`git worktree` with three core commands (`wt switch`, `wt list`, `wt remove`) plus workflow automation
(`wt merge`, `wt hook`, `wt step`, `wt config`). Worktrees are addressed by branch name; paths are
computed from a configurable template.

### Install

```bash
brew install worktrunk && wt config shell install   # macOS/Linux
cargo install worktrunk && wt config shell install   # Cargo
sudo pacman -S worktrunk && wt config shell install  # Arch
winget install max-sixty.worktrunk                   # Windows (installs as git-wt)
```

Shell integration is required for `wt switch` to change your working directory.

---

## 2. All Commands

### wt switch

Switch to a worktree; create if needed.

```
wt switch [OPTIONS] [BRANCH] [-- <EXECUTE_ARGS>...]
```

**Flags:**

| Flag | Purpose |
|------|---------|
| `-c, --create` | Create new branch and worktree |
| `-b, --base <BASE>` | Base branch for creation (defaults to default branch) |
| `-x, --execute <CMD>` | Replace wt process with CMD after switching |
| `--` | Pass additional args to executed command |
| `--no-cd` | Skip directory change; hooks still run |
| `--clobber` | Remove stale paths at target |
| `-y, --yes` | Skip approval prompts |
| `--no-hooks` | Skip hook execution |
| `--branches` | Interactive picker: include branches without worktrees |
| `--remotes` | Interactive picker: include remote branches |
| `--format <text\|json>` | Output format |

**Shortcuts:**

| Shortcut | Meaning |
|----------|---------|
| `^` | Default branch (main/master) |
| `@` | Current branch/worktree |
| `-` | Previous worktree (like `cd -`) |
| `pr:{N}` | GitHub PR branch |
| `mr:{N}` | GitLab MR branch |

**Interactive picker** (invoked without arguments): live preview with tabs for HEAD diff, log, changes
since merge-base, remote divergence, and LLM summary. Keybindings: arrows navigate, type to filter,
Enter selects, Alt-c creates, Esc cancels, 1-5 toggle tabs. Unix only.

**Template variables in -x:** `{{ branch }}`, `{{ worktree_path }}`, `{{ base }}`,
`{{ base_worktree_path }}`, with filters like `| sanitize`.

### wt list

```
wt list [--full] [--branches] [--remotes] [--format <table|json>] [--progressive]
```

**Columns:** Branch, Status, HEAD+/- (uncommitted changes), main+/- (commits ahead/behind),
main...+/- (line diffs, `--full` only), Remote+/- (upstream divergence), CI (`--full` only),
URL (from project config), Summary (LLM, `--full` only), Commit hash, Age, Message.

**Status symbols:**

- Working tree: `+` staged, `!` modified, `?` untracked
- State: merge conflicts, rebase/merge in progress, locked, prunable
- Default branch relation: `^` is main, `_` same commit, integrated, ahead, behind, diverged
- Remote: in sync, ahead, behind, diverged

**CI status** (with `--full`): colored dots for passed/running/failed/conflicts/no-ci. Clickable PR
links. Requires `gh` or `glab` CLI authenticated. Results cached 30-60s.

**JSON output** enables scripting:

```bash
# Current worktree path
wt list --format=json | jq -r '.[] | select(.is_current) | .path'

# Branches ahead of main
wt list --format=json | jq '.[] | select(.main.ahead > 0) | .branch'

# Integrated branches safe to remove
wt list --format=json | jq '.[] | select(.main_state == "integrated" or .main_state == "empty") | .branch'
```

### wt merge

Squash, rebase, merge, and clean up in one command.

```
wt merge [TARGET] [OPTIONS]
```

**Pipeline (8 steps):** commit -> squash -> rebase -> pre-merge hooks -> merge (fast-forward) ->
pre-remove hooks -> cleanup -> post-remove + post-merge hooks.

**Key flags:**

| Flag | Effect |
|------|--------|
| `--no-squash` | Preserve full commit history |
| `--no-commit` | Skip committing (requires clean tree) |
| `--no-rebase` | Skip rebase |
| `--no-remove` | Keep worktree after merge |
| `--no-ff` | Create merge commit (semi-linear) |
| `--stage <all\|tracked\|none>` | Control what gets staged |
| `--no-hooks` | Skip all hooks |
| `-y, --yes` | Skip prompts |

### wt remove

```
wt remove [BRANCHES...] [OPTIONS]
```

Defaults to current worktree. Refuses to remove worktrees with uncommitted changes unless `--force`.

**Flags:** `-f, --force` (remove with untracked files), `-D, --force-delete` (delete unmerged
branches), `--no-delete-branch` (preserve branch), `--foreground` (block until done).

**Branch deletion safety:** only deletes branches whose content is already in the default branch.
Handles squash-merge and rebase workflows where commit history differs but file changes match.

**Locked worktrees:** `git worktree lock` prevents removal; shows locked indicator in `wt list`.

### wt step

Individual operations and custom aliases.

**Built-in steps:**

| Step | Purpose |
|------|---------|
| `commit` | Stage and commit with LLM message |
| `squash` | Squash all branch commits into one |
| `rebase` | Rebase onto target branch |
| `push` | Fast-forward target to current |
| `diff` | Show all changes since branching |
| `copy-ignored` | Copy gitignored files between worktrees |
| `eval` | Evaluate a template expression |
| `for-each` | Run command in every worktree |
| `promote` | Swap branch into main worktree |
| `prune` | Remove merged worktrees |
| `relocate` | Move worktrees to expected paths |
| `<alias>` | Run configured alias |

**copy-ignored** eliminates cold starts. Uses reflink (copy-on-write) when available. A 14GB
`target/` directory copies in ~20s with reflink vs ~2m full copy. Filter with `.worktreeinclude`:

```
.env
node_modules/
target/
```

**eval** for shell substitutions:

```bash
curl http://localhost:$(wt step eval '{{ branch | hash_port }}')/health
```

**for-each** runs commands sequentially across all worktrees:

```bash
wt step for-each -- git status --short
wt step for-each -- npm install
```

### wt hook

Manual hook execution.

```bash
wt hook pre-merge              # Run all pre-merge hooks
wt hook pre-merge test         # Run specific named hook
wt hook pre-merge user:        # Run user hooks only
wt hook pre-merge project:test # Run project's "test" hook only
wt hook pre-start --branch=feature/test  # Override template variable
```

### wt config

```bash
wt config create [--project]   # Create config file
wt config show                 # Display resolved config
wt config shell install        # Install shell integration
wt config shell uninstall      # Remove shell integration
wt config state vars set KEY=VAL  # Set per-branch variable
wt config state vars get KEY      # Get per-branch variable
wt config state marker set "X"    # Set branch marker
wt config state logs get --hook=user:post-start:server  # Get hook log path
wt config state default-branch clear  # Clear cached default branch
wt config state clear             # Remove all worktrunk data from .git/
wt config plugins claude install  # Install Claude Code plugin
```

---

## 3. All Configuration Options

### Config file locations

- **User config:** `~/.config/worktrunk/config.toml` (or `$XDG_CONFIG_HOME/worktrunk/config.toml`)
- **Project config:** `.config/wt.toml` (checked into repo; hooks require approval)
- **Approvals:** `~/.config/worktrunk/approvals.toml`

### User config (`~/.config/worktrunk/config.toml`)

```toml
# Worktree path template
worktree-path = "{{ repo_path }}/../{{ repo }}.{{ branch | sanitize }}"
# Alternative layouts:
# worktree-path = "{{ repo_path }}/.worktrees/{{ branch | sanitize }}"
# worktree-path = "~/worktrees/{{ repo }}/{{ branch | sanitize }}"

# LLM commit message generation
[commit.generation]
command = "CLAUDECODE= MAX_THINKING_TOKENS=0 claude -p --no-session-persistence --model=haiku --tools='' --disable-slash-commands --setting-sources='' --system-prompt=''"

[commit]
stage = "all"           # "all" | "tracked" | "none"

[merge]
squash = true           # Squash commits into one
commit = true           # Commit uncommitted changes first
rebase = true           # Rebase onto target before merge
remove = true           # Remove worktree after merge
verify = true           # Run project hooks
ff = true               # Fast-forward merge

[list]
summary = false         # Enable LLM branch summaries
full = false            # Show CI, diffstat, LLM summaries
branches = false        # Include branches without worktrees
remotes = false         # Include remote-only branches
task-timeout-ms = 0     # Timeout per git command (0=disabled)
timeout-ms = 0          # Wall-clock budget (0=disabled)

[switch]
cd = true               # Change directory after switching
[switch.picker]
pager = "delta --paging=never"
timeout-ms = 500        # Wall-clock budget for picker

[step.copy-ignored]
exclude = []            # Additional excludes beyond built-in

# User hooks (no approval needed)
pre-start = "..."
[post-start]
name = "command"

# Aliases
[aliases]
name = "command with {{ branch }} templates"

# Per-project overrides
[projects."github.com/user/repo"]
worktree-path = ".worktrees/{{ branch | sanitize }}"
list.full = true
merge.squash = false
pre-start.env = "cp .env.example .env"
```

### Project config (`.config/wt.toml`)

```toml
# Hooks (require approval on first run)
pre-start = "npm ci"

[post-start]
server = "npm run dev -- --port {{ branch | hash_port }}"

[[pre-merge]]
lint = "npm run lint"

[[pre-merge]]
test = "npm test"

# Dev server URL shown in wt list
[list]
url = "http://localhost:{{ branch | hash_port }}"

# Forge platform override
[forge]
platform = "github"           # or "gitlab"
hostname = "github.example.com"

# Project aliases
[aliases]
deploy = "make deploy BRANCH={{ branch }}"
```

### Environment variable overrides

All config keys map via `WORKTRUNK_` prefix with SCREAMING_SNAKE_CASE:

| Config Key | Env Var |
|-----------|---------|
| `worktree-path` | `WORKTRUNK_WORKTREE_PATH` |
| `commit.generation.command` | `WORKTRUNK_COMMIT__GENERATION__COMMAND` |
| `commit.stage` | `WORKTRUNK_COMMIT__STAGE` |

Special env vars: `WORKTRUNK_BIN`, `WORKTRUNK_CONFIG_PATH`, `WORKTRUNK_SYSTEM_CONFIG_PATH`,
`WORKTRUNK_MAX_CONCURRENT_COMMANDS` (default: 32), `NO_COLOR`, `CLICOLOR_FORCE`.

### Template variables (available in hooks, aliases, path templates)

| Variable | Description |
|----------|-------------|
| `{{ branch }}` | Active branch name |
| `{{ commit }}` | HEAD SHA |
| `{{ short_commit }}` | 7-char SHA |
| `{{ upstream }}` | Tracking branch (if set) |
| `{{ worktree_path }}` | Active worktree directory |
| `{{ worktree_name }}` | Directory name only |
| `{{ base_worktree_path }}` | Other worktree in merge operations |
| `{{ target_worktree_path }}` | Merge destination path |
| `{{ repo_path }}` | Repository root path |
| `{{ primary_worktree_path }}` | Main worktree location |
| `{{ repo }}` | Repository directory name |
| `{{ owner }}` | Primary remote owner path |
| `{{ remote }}` | Primary remote name |
| `{{ remote_url }}` | Remote URL |
| `{{ default_branch }}` | Default branch name |
| `{{ base }}` | Source branch (merge-only) |
| `{{ target }}` | Destination branch (merge-only) |
| `{{ cwd }}` | Hook execution directory |
| `{{ hook_type }}` | e.g., "pre-start" |
| `{{ hook_name }}` | Named command identifier |
| `{{ vars.<key> }}` | Per-branch state variables |

### Template filters

| Filter | Purpose | Output range |
|--------|---------|-------------|
| `sanitize` | Replace `/` and `\` with `-` | filesystem-safe |
| `sanitize_db` | Lowercase, underscores, max 63 chars, hash suffix | database-safe |
| `sanitize_hash` | Filesystem-safe with hash suffix (only when changed) | unique names |
| `hash_port` | Deterministic port from string | 10000-19999 |
| `default(val)` | Fallback value | -- |

Concatenation for composite keys: `{{ (repo ~ '-' ~ branch) | hash_port }}`

### Conditional logic (Jinja2/minijinja)

```toml
sync = "{% if upstream %}git fetch && git rebase {{ upstream }}{% endif %}"
dev = "ENV={{ vars.env | default('development') }} npm start"
```

### JSON context on stdin

Hooks receive all template variables as JSON on stdin for complex logic:

```python
import json, sys
ctx = json.load(sys.stdin)
if ctx['branch'].startswith('feature/'):
    # custom logic
```

---

## 4. Hook System (Complete Reference)

### Hook types and lifecycle

| Event | Pre (blocking) | Post (background) |
|-------|----------------|-------------------|
| switch | `pre-switch` | `post-switch` |
| start | `pre-start` | `post-start` |
| commit | `pre-commit` | `post-commit` |
| merge | `pre-merge` | `post-merge` |
| remove | `pre-remove` | `post-remove` |

- `pre-*` hooks are **blocking** -- failures abort the operation
- `post-*` hooks run **asynchronously** in the background with logged output

### Three configuration formats

**Single command (string):**

```toml
pre-start = "npm install"
```

**Concurrent commands (table):**

```toml
[post-start]
server = "npm run dev"
watch = "npm run watch"
```

**Sequential pipeline (array of tables) -- recommended for pre-* hooks as of 0.37.0:**

```toml
[[post-start]]
install = "npm ci"

[[post-start]]
build = "npm run build"
server = "npm run dev"
```

Pipeline steps execute serially; commands within each step run concurrently.

### Hook execution order during wt switch --create

1. `pre-switch` (blocking)
2. Worktree created at configured path
3. Directory changed
4. `pre-start` (blocking)
5. `post-start` (background) + `post-switch` (background)

### Hook execution order during wt merge

`pre-commit` -> `post-commit` -> `pre-merge` -> `pre-remove` -> `post-remove` + `post-merge`

### Execution contexts

| Hook | When | Runs in |
|------|------|---------|
| `pre-start` | Blocking setup before post-start and --execute | New worktree |
| `post-start` | Background initialization after creation | New worktree |
| `pre-commit` | Validation before squash commit during merge | Source worktree |
| `post-commit` | CI triggers, background tasks | Source worktree |
| `pre-merge` | Tests, builds after rebase, before merge | Source worktree |
| `post-merge` | Deployment, notifications | Target worktree (or primary) |
| `pre-switch` | Before branch resolution | Origin worktree |
| `post-switch` | Fires regardless of outcome | Destination worktree |
| `pre-remove` | Cleanup before deletion | Worktree being removed |
| `post-remove` | Service termination | Primary worktree |

### Security and approvals

Project hooks (`.config/wt.toml`) require explicit approval before first execution. Approval is per
command text; changes require re-approval. Approvals saved to
`~/.config/worktrunk/approvals.toml`.

```bash
wt hook approvals add            # Pre-approve all current project hooks
wt hook approvals clear          # Clear project approvals
wt hook approvals clear --global # Clear all global approvals
```

### Log management

Hook output is logged to `.git/wt/logs/{branch}/{source}/{hook-type}/{name}.log`.

```bash
wt config state logs get --hook=user:post-start:server   # Get log path
tail -f "$(wt config state logs get --hook=user:post-start:server)"  # Tail logs
```

Command audit log: `.git/wt/logs/commands.jsonl` (~2MB max, rotates at 1MB).

---

## 5. LLM Commit Messages

### Configuration by provider

**Claude Code:**

```toml
[commit.generation]
command = "CLAUDECODE= MAX_THINKING_TOKENS=0 claude -p --no-session-persistence --model=haiku --tools='' --disable-slash-commands --setting-sources='' --system-prompt=''"
```

**Codex:**

```toml
[commit.generation]
command = "codex exec -m gpt-5.1-codex-mini -c model_reasoning_effort='low' -c system_prompt='' --sandbox=read-only --json - | jq -sr '[.[] | select(.item.type? == \"agent_message\")] | last.item.text'"
```

**Other tools:** opencode, llm, aichat all supported with their respective command syntax.

### Template customization

```toml
[commit.generation]
command = "llm -m claude-haiku-4.5"
template = """
Write a commit message for this diff. One line, under 50 chars.
Branch: {{ branch }}
Diff:
{{ git_diff }}
"""
```

Available template variables: `{{ git_diff }}`, `{{ git_diff_stat }}`, `{{ branch }}`, `{{ repo }}`,
`{{ recent_commits }}`, `{{ commits }}` (squash), `{{ target_branch }}` (merge).

**Fallback:** when no LLM is configured, worktrunk generates deterministic messages from filenames
(e.g., "Changes to auth.rs & config.rs").

---

## 6. Claude Code Integration

### Plugin installation

```bash
wt config plugins claude install
```

### Three capabilities

1. **Configuration skill** -- Claude Code can help set up LLM commits, hooks, path templates, and
   debug shell integration
2. **Worktree isolation** -- creation/removal routed through `wt` ensuring hooks, naming conventions,
   and lifecycle management are respected
3. **Activity tracking** -- status markers in `wt list` showing whether Claude is actively working
   or waiting for input

### Statusline integration

Add to `~/.claude/settings.json`:

```json
{
  "statusLine": {
    "type": "command",
    "command": "wt list statusline --format=claude-code"
  }
}
```

Displays single-line status with optional context window usage visualization via moon phase gauge.

---

## 7. Community Patterns and Creative Automations

### 7.1. Tmux session per worktree (most relevant for tms users)

From the official [tips & patterns](https://worktrunk.dev/tips-patterns/) page:

```toml
[pre-start]
tmux = """
S={{ branch | sanitize }}
W={{ worktree_path }}
tmux new-session -d -s "$S" -c "$W" -n dev
tmux split-window -h -t "$S:dev" -c "$W"
tmux split-window -v -t "$S:dev.0" -c "$W"
tmux split-window -v -t "$S:dev.2" -c "$W"
tmux send-keys -t "$S:dev.1" 'npm run backend' Enter
tmux send-keys -t "$S:dev.2" 'claude' Enter
tmux send-keys -t "$S:dev.3" 'npm run frontend' Enter
tmux select-pane -t "$S:dev.0"
"""

[pre-remove]
tmux = "tmux kill-session -t {{ branch | sanitize }} 2>/dev/null || true"
```

**Pattern:** Each worktree gets a full tmux session with pre-configured panes running the backend,
frontend, and Claude Code. When the worktree is removed, the tmux session is cleaned up.

### 7.2. Detached tmux agent handoff

From [tips & patterns](https://worktrunk.dev/tips-patterns/):

```bash
tmux new-session -d -s fix-auth-bug \
  "wt switch --create fix-auth-bug -x claude -- 'Fix login timeout to 24 hours'"
```

Fire-and-forget: creates worktree, launches Claude in a detached tmux session, and returns control
immediately. Monitor with `tmux attach -t fix-auth-bug`.

### 7.3. Workmux (git worktrees + tmux windows)

[workmux](https://github.com/raine/workmux) by Raine Virta is a complementary tool that pairs
worktrees with tmux windows:

```yaml
# .workmux.yaml
panes:
  - command: <agent>
    focus: true
  - command: npm install && npm run dev
    split: horizontal
files:
  symlink:
    - .turbo
  copy:
    - .env
```

Agent status appears in the tmux window list. `<agent>` placeholder expands to claude/codex/gemini.

Source: [Introduction to workmux](https://raine.dev/blog/introduction-to-workmux/)

### 7.4. Per-worktree dev server with hash_port

```toml
[post-start]
server = "npm run dev -- --port {{ branch | hash_port }}"

[list]
url = "http://localhost:{{ branch | hash_port }}"

[pre-remove]
server = "lsof -ti :{{ branch | hash_port }} | xargs kill 2>/dev/null || true"
```

Each branch gets a deterministic port (10000-19999). The port is stable across sessions.
The URL appears as a clickable link in `wt list`.

### 7.5. Per-worktree database (Docker)

```toml
[[post-start]]
set-vars = """
wt config state vars set \
  container='{{ repo }}-{{ branch | sanitize }}-postgres' \
  port='{{ ('db-' ~ branch) | hash_port }}' \
  db_url='postgres://user:dev@localhost:{{ ('db-' ~ branch) | hash_port }}/{{ branch | sanitize_db }}'
"""

[[post-start]]
db = """
docker run -d --rm --name {{ vars.container }} \
  -p {{ vars.port }}:5432 \
  -e POSTGRES_DB={{ branch | sanitize_db }} \
  -e POSTGRES_PASSWORD=dev postgres:16
"""

[pre-remove]
db-stop = "docker stop {{ vars.container }} 2>/dev/null || true"
```

Access from shell: `DATABASE_URL=$(wt config state vars get db_url) npm start`

### 7.6. Caddy subdomain routing per worktree

From [Divyendu Singh's blog](https://blog.divyendusingh.com/p/my-git-worktree-workflow-ft-worktrunk)
and [worktrunk tips](https://worktrunk.dev/tips-patterns/):

```toml
[post-start]
server = "npm run dev -- --port {{ branch | hash_port }}"
proxy = """
curl -sf --max-time 0.5 http://localhost:2019/config/ || caddy start
curl -sf http://localhost:2019/config/apps/http/servers/wt || \
  curl -sfX PUT http://localhost:2019/config/apps/http/servers/wt \
    -H 'Content-Type: application/json' \
    -d '{"listen":[":8080"],"automatic_https":{"disable":true},"routes":[]}'
curl -sf -X DELETE http://localhost:2019/id/wt:{{ repo }}:{{ branch | sanitize }} || true
curl -sfX PUT \
  http://localhost:2019/config/apps/http/servers/wt/routes/0 \
  -H 'Content-Type: application/json' \
  -d '{"@id":"wt:{{ repo }}:{{ branch | sanitize }}",
       "match":[{"host":["{{ branch | sanitize }}.{{ repo }}.localhost"]}],
       "handle":[{"handler":"reverse_proxy",
                  "upstreams":[{"dial":"127.0.0.1:{{ branch | hash_port }}"}]}]}'
"""

[pre-remove]
proxy = "curl -sf -X DELETE http://localhost:2019/id/wt:{{ repo }}:{{ branch | sanitize }} || true"

[list]
url = "http://{{ branch | sanitize }}.{{ repo }}.localhost:8080"
```

Each worktree gets `feature-name.myproject.localhost:8080` subdomain routing.

### 7.7. Progressive pre-merge validation

```toml
[[pre-merge]]
lint = "npm run lint"

[[pre-merge]]
typecheck = "npm run typecheck"

[[pre-merge]]
test = "npm test"

[[pre-merge]]
build = "npm run build"
```

Steps run sequentially -- fast checks first, expensive checks last. Failure at any step aborts
the merge.

### 7.8. Move/copy in-progress changes between worktrees

```toml
[aliases]
move-changes = '''
if git diff --quiet HEAD && test -z "$(git ls-files --others --exclude-standard)"; then
  wt switch --create {{ to }};
else
  git stash push --include-untracked --quiet && \
  wt switch --create {{ to }} --execute='git stash pop --index';
fi
'''

copy-changes = '''
if git diff --quiet HEAD && test -z "$(git ls-files --others --exclude-standard)"; then
  wt switch --create {{ to }};
else
  git stash push --include-untracked --quiet && \
  git stash apply --index --quiet && \
  wt switch --create {{ to }} --execute='git stash pop --index';
fi
'''

copy-staged = '''
if git diff --cached --quiet; then
  wt switch --create {{ to }};
else
  p=$(mktemp) && git diff --cached > "$p" && \
  wt switch --create {{ to }} --execute="git apply --index '$p' && rm '$p'";
fi
'''
```

Usage: `wt step move-changes --to=feature-xyz`

### 7.9. Fetch and rebase all worktrees

```toml
[aliases]
up = '''
git fetch --all --prune && wt step for-each -- '
  git rev-parse --verify -q @{u} >/dev/null || exit 0
  g=$(git rev-parse --git-dir)
  test -d "$g/rebase-merge" -o -d "$g/rebase-apply" && exit 0
  git rebase @{u} --no-autostash || git rebase --abort
''''
```

Run with `wt step up`.

### 7.10. Stacked branches

```bash
wt switch --create feature-part2 --base=@    # Branch from current HEAD, not main
```

### 7.11. Target-specific deployment

```toml
post-merge = """
if [ {{ target }} = main ]; then
    npm run deploy:production
elif [ {{ target }} = staging ]; then
    npm run deploy:staging
fi
"""
```

### 7.12. Agent status tracking with markers

```bash
wt config state marker set "WIP"               # Current branch
wt config state marker set "done" --branch feature
```

Markers appear in `wt list` output. The Claude Code plugin sets markers automatically.

### 7.13. Monitor hook logs in real-time

```bash
alias wtlog='f() { tail -f "$(wt config state logs get --hook="$1")"; }; f'
wtlog user:post-start:server
```

### 7.14. Xcode DerivedData cleanup

```toml
[post-remove]
clean-derived = """
grep -Fl {{ worktree_path }} ~/Library/Developer/Xcode/DerivedData/*/info.plist 2>/dev/null | \
while read plist; do
  rm -rf "$(dirname "$plist")"
done
"""
```

### 7.15. Manual commit messages (editor-based)

```toml
[commit.generation]
command = '''
f=$(mktemp); printf '\n\n' > "$f"; sed 's/^/# /' >> "$f";
${EDITOR:-vi} "$f" < /dev/tty > /dev/tty; grep -v '^#' "$f"
'''
```

Opens your editor with the diff as comments for context.

### 7.16. Parallel agent launch pattern

```bash
wt switch -x claude -c feature-a -- 'Add user authentication'
wt switch -x claude -c feature-b -- 'Fix the pagination bug'
wt switch -x claude -c feature-c -- 'Write tests for the API'
```

Or with the recommended shell alias:

```bash
alias wsc='wt switch --create --execute=claude'
wsc feature-a -- 'Add user authentication'
wsc feature-b -- 'Fix the pagination bug'
wsc feature-c -- 'Write tests for the API'
```

### 7.17. .worktreeinclude for selective file copying

```
.env
.dev.vars
node_modules/
target/
```

Files must be both gitignored AND listed in `.worktreeinclude` to be copied by
`wt step copy-ignored`.

### 7.18. incident.io's `w` function pattern

From [incident.io's blog](https://incident.io/blog/shipping-faster-with-claude-code-and-git-worktrees):

```bash
w myproject new-feature              # Create and switch
w myproject new-feature claude       # Create, switch, launch Claude
w myproject new-feature git status   # Run command in worktree context
```

Auto-creates with username prefix, organizes in `~/projects/worktrees/`, auto-completes
repositories and worktrees.

### 7.19. Bare repository layout

```bash
git clone --bare <url> myproject/.git
cd myproject
wt switch --create main
```

Configure equal-level worktrees:

```toml
worktree-path = "{{ repo_path }}/../{{ branch | sanitize }}"
```

---

## 8. Best Practices and Recommended Workflows

### Project setup checklist

1. **Install and configure shell integration:**
   ```bash
   brew install worktrunk && wt config shell install
   ```

2. **Set worktree path template** in `~/.config/worktrunk/config.toml`:
   ```toml
   worktree-path = "{{ repo_path }}/../{{ repo }}.{{ branch | sanitize }}"
   ```

3. **Configure LLM commit messages:**
   ```toml
   [commit.generation]
   command = "CLAUDECODE= MAX_THINKING_TOKENS=0 claude -p --no-session-persistence --model=haiku --tools='' --disable-slash-commands --setting-sources='' --system-prompt=''"
   ```

4. **Create project config** (`.config/wt.toml`, commit to repo):
   ```toml
   [post-start]
   deps = "npm ci"
   copy = "wt step copy-ignored"

   [[pre-merge]]
   lint = "npm run lint"

   [[pre-merge]]
   test = "npm test"
   ```

5. **Add `.worktreeinclude`** for env files and build caches.

### Workflow patterns by task type

**Quick fix:**

```bash
wt switch -c hotfix
# make changes
wt merge main
```

**Feature with PR:**

```bash
wt switch -c feature-auth
# develop...
wt step commit
gh pr create
# after PR merged on GitHub:
wt remove
```

**Parallel agents:**

```bash
alias wsc='wt switch --create --execute=claude'
wsc feature-a -- 'Implement user auth'
wsc feature-b -- 'Add pagination'
wsc feature-c -- 'Write API tests'
# Monitor:
wt list --full
```

**Review a PR:**

```bash
wt switch pr:123
# review...
wt remove
```

### Tips

- Use `wt switch -` to toggle between two worktrees (like `cd -`)
- Use `wt switch ^` to go back to main
- Use `wt list --format=json` for scriptable status queries
- Use `wt step prune` to clean up merged branches
- Use `wt step for-each` for bulk operations across all worktrees
- Lock worktrees with important local state: `git worktree lock ../myproject.feature`
- Use `--no-hooks` when debugging to isolate hook issues
- Use `--yes` in automation/CI scripts

---

## 9. Integration With Your Stack

### Tmux + tms integration

Worktrunk's pre-start hook can create tmux sessions per worktree (see pattern 7.1). For tms
(tmux-sessionizer) users, consider:

- **Option A:** Use worktrunk's `pre-start` hook to register the worktree directory with tms by
  adding it as a bookmark/mark, then `post-remove` to clean up
- **Option B:** Use worktrunk purely for worktree lifecycle and let tms discover worktree
  directories via its configured search paths
- **Option C:** Create a `pre-start` hook that creates a tmux session via tms's expected patterns

The detached tmux session pattern (7.2) is the most natural fit for fire-and-forget agent workflows.

### Neovim

Worktrunk itself does not have a Neovim plugin, but works well alongside:
- [git-worktree.nvim](https://github.com/ThePrimeagen/git-worktree.nvim) or similar for in-editor
  worktree switching
- Telescope/fzf-lua for picking worktrees
- Worktrunk's `wt list --format=json` for building custom pickers

### Chezmoi dotfiles

The worktrunk user config (`~/.config/worktrunk/config.toml`) can be managed by chezmoi. Add it as a
managed file:

```bash
chezmoi add ~/.config/worktrunk/config.toml
```

This ensures your LLM commit config, path templates, aliases, and user hooks are consistent across
machines.

### Claude Code

Install the official plugin (`wt config plugins claude install`) for worktree isolation, activity
tracking, and configuration assistance. The statusline integration shows branch status directly in
Claude Code's UI.

### CI/CD

- Use `pre-merge` hooks as a local CI gate before pushing
- `wt list --full` shows GitHub/GitLab CI status per branch
- `wt list --format=json` enables scripting CI status checks
- `post-merge` hooks can trigger deployments

---

## 10. Files Worktrunk Creates

| Location | Purpose |
|----------|---------|
| `~/.config/worktrunk/config.toml` | User preferences |
| `~/.config/worktrunk/approvals.toml` | Approved hook commands |
| `.config/wt.toml` | Project hooks and config |
| `.worktreeinclude` | Files to copy between worktrees |
| `.git/wt/cache/` | Cached CI status and git results |
| `.git/wt/logs/` | Hook output and command audit logs |
| `.git/wt/trash/` | Staged worktree contents pending deletion |
| Shell config (bashrc/zshrc) | One-line source for shell integration |

Worktrunk does NOT modify `~/.gitconfig`, create global git hooks, or run background processes.

---

## 11. External Resources

### Official

- [worktrunk.dev](https://worktrunk.dev/) -- full documentation
- [GitHub: max-sixty/worktrunk](https://github.com/max-sixty/worktrunk) -- source, issues, releases
- [CHANGELOG](https://github.com/max-sixty/worktrunk/blob/main/CHANGELOG.md)

### Blog posts and tutorials

- [My git-worktree workflow ft. worktrunk, caddy](https://blog.divyendusingh.com/p/my-git-worktree-workflow-ft-worktrunk) -- Divyendu Singh
- [My git-worktree setup using worktrunk and caddy](https://xata.io/blog/my-git-worktree-setup-using-worktrunk-and-caddy) -- Divyendu Singh (Xata repost)
- [Shipping faster with Claude Code and Git Worktrees](https://incident.io/blog/shipping-faster-with-claude-code-and-git-worktrees) -- incident.io
- [Parallel Coding Agents with Git Worktree x tmux](https://medium.com/@sean0628/parallel-coding-agents-with-git-worktree-x-tmux-be2a5a290f18) -- Sho Ito
- [Introduction to workmux](https://raine.dev/blog/introduction-to-workmux/) -- Raine Virta
- [Worktrunk Complete Guide](https://blog.cosine.ren/en/post/git-worktrunk-guide/) -- cosine blog
- [Agentic Workflow with Worktrees](https://yuanchang.org/en/posts/agentic-workflow-with-worktrees/) -- Yuanchang

### Related tools

- [workmux](https://github.com/raine/workmux) -- git worktrees + tmux windows integration
- [git-worktree.nvim](https://github.com/ThePrimeagen/git-worktree.nvim) -- Neovim worktree plugin
- [Claude Code: Best practices for agentic coding](https://www.anthropic.com/engineering/claude-code-best-practices) -- Anthropic

### Videos

- [@DevOpsToolbox's video on Worktrunk](https://youtu.be/WBQiqr6LevQ?t=345)
