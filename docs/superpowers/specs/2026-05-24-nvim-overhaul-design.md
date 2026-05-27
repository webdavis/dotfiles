# Neovim Config Overhaul Design Spec

**Date:** 2026-05-24
**Status:** Draft — design captured, one decision gate open (see §0). No implementation started.
**Provenance:** Reconstructed from Claude session `c0b61ac3-8dc7-4059-b809-89c9a1ff3434` (nvim work ran
2026-05-15 – 2026-05-18; session recorded against the pre-relocation source path
`~/.local/share/chezmoi`). Nothing from that session was written to disk or implemented — it stopped at
the decision gate in §0.

**Scope:** A single-pass overhaul of the personal Neovim config: bring it under chezmoi management, fix
all known bugs, modernize plugins, improve organization for agents, cut startup time, and make a fresh
machine reach a working editor via `chezmoi apply`.
**Out of scope (deferred to a later session):** the conform.nvim / nvim-lint migration off none-ls
(bump none-ls only here); any change to the tmux/Claude CLI workflow beyond wiring `claudecode.nvim`
with `provider = "none"`.

## Background

The nvim config currently lives in a **standalone git repo**, `git@github.com:webdavis/neovim-config.git`,
checked out at `~/.config/nvim` — it is **not** under this chezmoi source. A fresh machine therefore does
not get a working editor from `chezmoi apply`; it requires a separate manual clone plus a Lazy/Mason
bootstrap. The config has also accumulated 17 confirmed bugs and several abandoned/deprecated plugins,
and startup has regressed from a ~190ms baseline to ~264ms.

Baseline metrics (as surveyed): 83 plugins, 49 personal Lua files (~6,377 lines), Mason 49 packages
(behind latest), `lazyvim.json` `extras: []`, nvim 0.12.2.

### Goal (verbatim intent)

One big improvement pass, ideally in a single session: stay on LazyVim; get the config tracked by
chezmoi so `chezmoi apply` fully sets up nvim on a fresh machine; update all plugins with **no errors on
startup or feature use**; improve code quality and organization (easier for agents to manage); improve
performance and security; migrate to more modern/popular plugins where they exist and add plugins that
boost developer efficiency.

### Approaches considered (chezmoi integration)

| # | Approach | Verdict | Reason |
|---|----------|---------|--------|
| 1 | **Flatten the standalone repo into `dot_config/nvim/`** (this design) | **Chosen** | The `chezmoi.nvim` autocmd presupposes nvim files live in the chezmoi source; one source of truth; `chezmoi apply` sets up a fresh machine. |
| 2 | Keep separate repo, clone via a `run_once` script | Rejected | Two sources of truth; bootstrap still needs network + SSH key at apply time; defeats the "one apply" goal. |
| 3 | git submodule | Rejected | Submodule friction with chezmoi's source-state model; detached-HEAD foot-guns; no real upside over flattening. |

## §0 — Open decision gate (must resolve before implementation)

The session stopped here. Two things to confirm:

1. **The six confident plugin drops** (`boole.nvim`, `telescope.nvim`, `nvim-notify`, `git-blame.nvim`,
   `git-messenger.vim`, `gv.vim`, `cspell.nvim`) — see §2. Confirm or contest each.
1. **gitmoji.nvim — CONDITIONAL.** Drop only if you do **not** type `:sparkles:`-style gitmoji codes in
   commit messages. If you do use them, keep it. **This is the one genuinely unresolved item.**

Everything below assumes the six drops are accepted; gitmoji is marked conditional throughout.

## §1 — Architecture & file layout

Greek-lettered decisions from the session:

| ID | Decision |
|----|----------|
| α | **Flatten** `webdavis/neovim-config` into `dot_config/nvim/` in this chezmoi source. Archive the original repo as `webdavis/neovim-config-archive`. |
| β | **Track `lazy-lock.json`** in chezmoi for reproducible plugin pins. Manual commit on update — no autocmd that rewrites it. |
| γ | **Delete the orphan `lazyvim.json`** — not on a LazyVim distribution; `extras: []` is explicit policy and the file is vestigial. |
| δ | **Lazy-load ~14 eager specs**; target startup **< 150ms** (from the regressed 264ms; ~190ms was the prior baseline). |
| ε | **Audit + fix `custom_api/`** (6 files: init/util/helpers/git/github/delegate, ~890 lines). Keep the module structure. |
| ζ | **Bootstrap script** `.chezmoiscripts/run_onchange_after_80-bootstrap-nvim.sh.tmpl` running `nvim --headless "+Lazy! restore" "+MasonToolsInstallSync" +qa`, wrapped in `timeout 120`, re-running when `lazy-lock.json` or `init.lua` changes. |
| η | **Track `CLAUDE.md` and `.claude/`** so Claude Code sees the conventions on every machine. |
| θ | **Chezmoiignore dev-only files:** `.luacheckrc`, `stylua.toml`, `.prettierignore`, `README.md`. |

## §2 — Plugin changes (net 83 → 75)

### Drops

| Plugin | Reason | Migration / caveat |
|--------|--------|--------------------|
| `boole.nvim` | Abandoned ~3yr; `<C-a>/<C-x>` conflicts with dial (bug #7) | Port its `additions` to dial augends before removing |
| `telescope.nvim` | Vestigial | Remove **after** fzf-lua takes over the octo picker |
| `nvim-notify` | Never invoked; noice prefers `snacks.notifier` | — |
| `git-blame.nvim` | Redundant | → gitsigns `current_line_blame` + gitlinker |
| `git-messenger.vim` | Redundant | → gitsigns `blame_line` |
| `gv.vim` | Zero `:GV` keymaps registered | → `snacks.picker.git_log` |
| `cspell.nvim` | Author-deprecated Dec 2025; never sourced | — |
| `gitmoji.nvim` | **CONDITIONAL** — see §0 | Drop only if not using `:sparkles:` codes |

### Adds

| Plugin | Config | Reason |
|--------|--------|--------|
| `coder/claudecode.nvim` | `provider = "none"` (keep Claude CLI in a tmux pane) | Editor ⇄ Claude integration without launching a second CLI |
| `ibhagwan/fzf-lua` | octo picker backend | New octo picker provider (~3ms startup); the direction LazyVim is heading |

### Keep / bump (not migrate)

- `dial.nvim` — keep, lazy-load; absorb boole augends.
- `nvim-surround` — bump to `^4.0.0`.
- `none-ls` — **bump** for nvim 0.12 fixes (do not migrate to conform/nvim-lint yet — see Out of scope).
- `markview` — keep (over render-markdown).
- `toggleterm` — keep (transitive dependency).
- octo picker backend switches snacks → **fzf-lua** (locked).

## §3 — Bug fixes (17 confirmed)

| # | Severity | Bug | Location |
|---|----------|-----|----------|
| 1 | high | `github.username()` does not exist — runtime error on `<C-g>i` | `git.lua:267` |
| 2 | low | `<C-g>bc` registered twice; first def is dead | `git.lua:466,540` |
| 3 | med | `toggle_runner("OverseerWatchRun")` produces an invalid action name | overseer |
| 4 | high | `git.default_branch(repo)` calling-convention mismatch (string vs table) | `custom_api/git.lua` |
| 5 | med | `delegate.M.setup()` called twice | `delegate.lua:171` + `keymaps.lua:6` |
| 6 | low | Duplicate `checktime` autocmd; options.lua missing buftype guard | `options.lua:116-118` |
| 7 | med | `dial.nvim` vs `boole.nvim` `<C-a>/<C-x>` conflict (non-deterministic) | resolved by dropping boole |
| 8 | med | `parse_branch_line` off-by-one in `message_start_index` — drops first word of commit message on branches with upstreams | `custom_api/git.lua` |
| 9 | med | `vim.cmd("normal! \<Esc>")` sends a literal string, not Escape | `delegate.lua:100` |
| 10 | med | `nvim_win_get_width(0)` evaluated at spec-load time (returns 0) | `harpoon.lua:6` |
| 11 | high | Hardcoded `mkdp_open_ip = "dresden.home.webdavis.io"` — breaks on non-dresden machines | `markdown.lua:314` |
| 12 | **critical** | `mason-lspconfig` `servers` block silently dead — per-server settings (lua_ls `callSnippet`, clangd `--clang-tidy`) never applied | `lsp.lua:50-148` |
| 13 | **critical** | 3 none-ls crashes on nvim 0.12 unfixed in pin (todo_comments parser nil, `supports_method`, `str_byteindex` strict) — you are on 0.12.2 | none-ls pin |
| 14 | high | Overseer v2.0 breakage: `actions`→`keymap`, `bundles` removed, `run_template`→`run_task` | `overseer.lua:83,192-200,254` |
| 15 | low | hlslens deprecation fix available 1 commit ahead of pin (`be2d7b2`) — closes `vim.validate{<table>}` warning | hlslens pin |
| 16 | high | Catppuccin rename `catppuccin`→`catppuccin-nvim` required before bumping past v2.0.0 (`605b460`) | `ui.lua:60` |
| 17 | low | noice `inc_rename = true` references non-installed inc-rename.nvim — dead preset | `noice.lua:36` |

### Additional risks surfaced by the deeper audit (verify during implementation)

- **treesitter `master` → `main` branch migration** — flagged as a critical gap; confirm current branch
  before bumping.
- **`gopls` missing from Mason** — add if Go LSP is wanted.
- **none-ls is the highest-risk single step** — bump and smoke-test in isolation.
- runtimepath bug investigation (open).

## §4 — Implementation sequence

1. **Pre-flight:** commit the dirty `autocmds.lua`; handle `stash@{0}`; verify the treesitter branch.
1. **Flatten** the standalone repo into `dot_config/nvim/` (decision α); archive original.
1. **Bug fixes + modernization** — work the §3 table; do the plugin drops/adds from §2 (telescope last,
   after fzf-lua owns the octo picker; boole after augends ported).
1. **Bootstrap script** (decision ζ).
1. **Productivity additions** and lazy-load pass (decision δ; target < 150ms).
1. Track `lazy-lock.json`, `CLAUDE.md`, `.claude/` (β, η); delete `lazyvim.json` (γ); chezmoiignore
   dev-only files (θ).

## §5 — Success criteria (verbatim)

No startup errors; no feature errors; improved organization for agents; more featureful; faster startup;
workflow-improving plugins added; increased security. Plus: a fresh machine reaches a working editor from
`chezmoi apply` alone.

## §6 — Verification

- `nvim --headless "+checkhealth" +qa` clean (no errors).
- Startup timed (`nvim --startuptime`) under target.
- Each fixed bug exercised (e.g. `<C-g>i` no longer errors; mason per-server settings actually apply).
- `just l` passes on any shell/template/markdown added to this repo.
- A `chezmoi apply --exclude=templates` (or the bootstrap script in isolation) sets up nvim on a clean
  `$HOME` test.
