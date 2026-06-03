# Neovim Config Overhaul Design Spec — v3

**Date:** 2026-06-03
**Status:** Active design. Supersedes `2026-06-02-nvim-overhaul-design-v2.md` (v2).
**Provenance:** This version adds a first-principles redesign of `custom_api/` and resolves the
`delegate.lua` question with research. Inputs: the brainstorming session of 2026-06-03; the plugin
reassessment in `2026-06-02-nvim-overhaul-reassessment.md`; and the agent-integration research in
`docs/research/2026-06-03-nvim-coding-agent-integration.md`. Nothing here is implemented yet.

**What v3 changes vs v2 (everything else in v2 still stands):**

1. **`custom_api/` gets a real design** (§1), replacing v2's one-line decision ε ("audit + fix, keep
   module structure"). The headline: the broken name *reflection* is removed, not patched; "SOLID" is
   evaluated through a test-driven-development lens (where does testability force a seam?), not by
   splitting files for tidiness.
1. **`delegate.lua` is retired entirely** (§2) — deleted, not normalized — in favor of an external
   Neovim Model Context Protocol (MCP) server that exposes the live buffer to the agent.
1. **`claudecode.nvim` is dropped from the plan** (§3). v2 added it; the agent-integration research shows
   it serves selection-send, not the live-current-buffer workflow actually wanted, so it is redundant.

Carried forward from v2 unchanged: the corrected framing (vanilla `lazy.nvim`, not LazyVim; the
`defaults.lazy=false` startup lever), the 14-item bug table in v2 §3, architecture decisions α–δ/ζ–θ,
the two blockers, and the safe implementation sequence (amended in §5 below).

## §1 — `custom_api/` redesign

Six modules today: `init`, `util`, `helpers`, `git`, `github`, `delegate` (~890 lines). `delegate` leaves
(§2). This section covers the rest.

### 1.1 Error tracing — remove the reflection, don't fix it

**Problem (verified).** `helpers.wrap` labels error notifications by introspecting the wrapped function's
name: `debug.getinfo(fn, "n").name` (`helpers.lua:44`). A passed-in function *value* carries no name in
Lua — the name is inferred from a call site, which `wrap` doesn't have — so this returns `"anonymous"`
**every time**, on Lua 5.5, LuaJIT, and Neovim's builtin Lua (reproduced 2026-06-03). Every wrapped
function reports as `custom_api.git.anonymous`, `custom_api.github.anonymous`, etc.

**Root cause.** The need for reflection is self-inflicted: `wrap` made *every* API function also
responsible for *notifying the user*, so it had to discover which function failed. Remove that
responsibility and the reflection requirement disappears.

**Target shape (the beaten path for Lua, confirmed against `lazy.nvim`'s `lazy.core.util`).** Two failure
modes, kept separate:

- **Expected/operational failure** (not a git repo, not logged into `gh`): a *result*, not an error.
  Return `nil, message` — the established Lua convention (as in `io.open`). The caller decides what to do.
- **Unexpected failure** (a bug): `error(msg, level)`, caught at a boundary with
  `xpcall(fn, debug.traceback)`. The traceback reports file:line:function accurately via *stack-level*
  introspection — `debug.getinfo(level, "Sl")` — which works, unlike function-value name lookup.

So:

- API functions in `git`/`github`/`util` return `(value, err)` and **never call `vim.notify`**.
- A single thin boundary helper at the keymap/command layer (the `lazy.nvim` `try` shape: an *explicit*
  context label + `xpcall` + traceback) presents errors. The label is **explicit data**, passed in or
  taken from the registration key — never reflected.
- `debug.getinfo(fn, …)` is deleted from the codebase.

```lua
-- boundary helper, called from the keymap/command layer (sketch, not final code)
-- M.try(fn, { label = "git.default_branch" })  -- explicit label, no reflection
local function try(fn, opts)
  local ok, result = xpcall(fn, function(err)
    vim.notify(("[%s] %s\n%s"):format(opts.label, err, debug.traceback("", 2)),
      opts.level or vim.log.levels.ERROR, { title = "custom_api" })
    return err
  end)
  return ok and result or nil
end
```

This also retires the `wrap` soft-error scanner (`helpers.lua:56-70`) and its bug: it searches all return
values for "first string" but then notifies `results[2]` regardless, mishandling `latest_commit`'s
`(hash, summary, body, errmsg)` 4-tuple. Standardizing on `(value, err)` removes the loop and the bug.

### 1.2 SOLID, evaluated through the test-driven-development lens

The goal is not more files. It is genuine adherence to the principles, the way test-first development
would expose it. The implementation/testing is the user's to do; v3 prescribes the *seams* a test-first
pass would force, and only those.

| If you wrote this test… | What the current design forces | Principle | Prescribed seam |
|---|---|---|---|
| `default_branch` returns `"main"` when `origin/main` exists | Can't — it calls real `git` through a hard-wired `util.run_shell_command` | **dependency inversion** | Make the shell runner an *injected* dependency, so git/`gh` functions are testable without a live repo |
| `default_branch` falls back to the GitHub API when no local default | Needs a *different* fake (`curl`/`gh`) **and** owner+name | **single responsibility** | Move the GitHub-API fallback into `github` (where `repo()` supplies owner/name). This also fixes bugs #4/#4b |
| `parse_branch_line` keeps the first commit word when an upstream is present | Pure function — trivially testable | — | No seam; it just needs a test. Test-first would have caught bug #8 |
| requiring `delegate` does **not** register keymaps | Impossible — `require` triggers `setup()` (`delegate.lua:171`) | **single responsibility / no import side-effects** | Moot once `delegate` is deleted (§2); the principle still governs any module: no side-effects on `require` |
| `map` sets `silent=false` for a `:Ex` command rhs | Needs to fake `vim.keymap.set` — a different collaborator than string-trimming | **single responsibility** | `map` and `overseer_runner` are genuinely separate units from string helpers (different test doubles) — separate them on that basis, not for tidiness |

The pure helpers (`trim`, `sanitize`, `normalize`, `convert_remote_protocol`, `parse_branch_line`,
`extract_upstream`, `is_current_branch`, `normalize_branch`) are already single-purpose. They need
*tests*, not restructuring.

### 1.3 KISS / correctness

- Standardize the `(value, err)` contract across `git`/`github` (see §1.1); `latest_commit`'s 4-tuple
  return becomes `(result_table_or_nil, err)`.
- Drop the redundant closure in `map` (`util.lua:141-147`): it re-dispatches string-vs-function rhs that
  `vim.keymap.set` already handles natively.
- Fold in the runtime bug fixes from v2 §3 that live in these modules: #1 (`git.lua:267` →
  `account().username`), #4 + the new #4b (`custom_api/git.lua:231-232`, `:247`), #8
  (`custom_api/git.lua:101-112` off-by-one), #9 (`delegate.lua:100` literal Escape — moot if delegate is
  deleted), #5 (double `setup` — moot likewise).
- Align error message text with parameter names (`latest_commit` says "project" for field `repo_name`;
  `url` says "user" for `account_name`).

### 1.4 What v3 does **not** prescribe

No test code (the user will do test-driven development). No new abstraction beyond the injected-runner
seam and the git/GitHub split. No renaming of the public `custom_api` surface beyond the casing fix
(`copy_URL_to_clipboard` → consistent snake_case) — flagged for the user to confirm, per the rename rule.

## §2 — Agent integration: retire `delegate.lua`

`delegate.lua` hand-rolls "send code context to a CLI in a tmux window" via `tmux send-keys` (brittle:
1.5 s `defer_fn`, shell-escaping, the literal-Escape bug #9, and `setup()` as a side-effect of `require`,
bug #5). The desired workflow, as stated: *keep a Neovim buffer open → prompt the agent → the agent
auto-reads the buffer in its current state **without requiring a save** → and can update it in process.*

**Decision: delete `delegate.lua`; adopt an external Neovim MCP server.** Reasoning (full analysis and
citations in `docs/research/2026-06-03-nvim-coding-agent-integration.md`):

- Reading **unsaved, in-memory** buffer content and writing back into the live buffer can only be done
  over Neovim's remote-procedure-call (RPC) API. Disk-based approaches (`@path` mentions, the agent's own
  file reads) only ever see *saved* state, so they cannot satisfy the no-save-required requirement. This
  is the floor — not over-engineering.
- An MCP server exposes the live buffer to the agent (running in its tmux pane) as a resource/tool. The
  agent pulls current buffer state on demand and edits it in place. No per-prompt send gesture, no
  `send-keys`, agent-agnostic.

**Recommended server: `paulburgess1357/nvim-mcp`** (v1.0.0, May 2026; Python; native msgpack-RPC
auto-discovery, no plugin; "edit buffers in memory… full undo support"). Fallback:
`bigcodegen/mcp-neovim-server` (315★, more eyes, but staler release and needs `nvim --listen` +
`NVIM_SOCKET_PATH`). All viable options are single-author projects — the inherent maturity risk of this
niche in mid-2026; revisit before relying on it long-term.

**Required operating rule (the real gotcha).** With unsaved buffer edits in play, the agent must edit
through the MCP buffer tools, **not** its native disk Write — a disk write would collide with the dirty
buffer. Encode this as a `CLAUDE.md` rule for the project: "for files open in Neovim, use the nvim MCP
tools, not Write." `paulburgess1357`'s full-undo support is the safety net if violated.

**Setup (one-time):** register the MCP server in the Claude CLI config (`~/.claude.json` or project
`.mcp.json`); ensure Neovim exposes its RPC socket (default, or `--listen`). Keep the user's existing
yank-filepath keymap as-is.

**Future hedge, not now:** the Agent Client Protocol (ACP, Zed/JetBrains; registry live Jan 2026) is the
standards-track cross-agent option, but it is architecturally inverse to this setup — the editor must own
the agent subprocess over stdio, pulling the agent out of the tmux pane. Reconsider only if cross-agent
portability becomes a priority.

## §3 — Plugin-list delta vs v2 §2

- **Remove `coder/claudecode.nvim` from the adds.** It provides selection-send, not the live-current-
  buffer workflow (§2); redundant with the MCP server and the existing yank keymap. Dropping it also
  removes the `snacks`-dependency-under-`provider=none` question entirely.
- **`delegate.lua` removed** from `custom_api` (§2) — a deletion, not a plugin change.
- **`fzf-lua`**: unchanged from v2 — still optional, gated on the octo-picker sub-decision (default:
  stay on snacks).
- Net effect on v2's "83 → 75" count: one fewer *add* (no claudecode.nvim), so the target trends one
  lower; recompute after the drops land. No bug-table changes.

## §4 — Verification additions

Beyond v2 §7, the `custom_api` redesign adds (the user implements the tests):

- A headless Lua test target the runtime-only bugs can't escape: `extract_upstream` on `[origin/main]`
  retains the first commit word (#8); `default_branch` with an injected fake runner returns `"main"` /
  falls back correctly (#4/#4b); the boundary `try` reports an explicit label + traceback and never
  `"anonymous"`; `git.account().username` resolves (#1).
- Manual check of the agent loop: open a buffer, make an **unsaved** edit, prompt the agent, confirm it
  reads the in-memory change and can write back via the MCP tool without a prior `:w`.

## §5 — Amended implementation sequence

v2 §5 stands, with these slots:

1. Pre-flight + flatten (v2 §5 steps 1-3) — unchanged.
1. **`custom_api` error-tracing redesign** (§1.1): introduce the boundary `try`, convert `git`/`github`
   to `(value, err)`, delete `wrap`'s reflection + soft-error loop. Own commit.
1. **`custom_api` seams** (§1.2-1.3): inject the shell runner; move the GitHub fallback into `github`
   (fixes #4/#4b); separate `map`/`overseer_runner` from string helpers; drop the redundant `map`
   closure; remaining `custom_api` bug fixes (#1, #8). Own commit(s).
1. **Delete `delegate.lua`** (§2); update `init.lua`, `keymaps.lua:6`, and the `which-key`
   `<leader>d` group; configure `paulburgess1357/nvim-mcp` + the `CLAUDE.md` edit rule. Own commit.
1. Remaining v2 work (per-server LSP fix #12, other bug fixes, coupled drops, lazy-load pass, bootstrap)
   — unchanged, minus the `claudecode.nvim` add.

## §6 — Open items

- **octo picker** (from v2 §0): snacks vs fzf-lua — default snacks (drops telescope with zero net add).
- **`copy_URL_to_clipboard` casing** rename — confirm before applying (rename rule).
- **MCP server long-term** — all candidates single-author; re-evaluate before depending on it.
