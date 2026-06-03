# Neovim Config Overhaul Design Spec — v2

**Date:** 2026-06-02
**Status:** Active design. Supersedes `2026-05-24-nvim-overhaul-design.md` (v1).
**Provenance:** v1 was reassessed on 2026-06-02 by a nine-agent adversarial workflow against the live
config (frozen at commit `7681750`, 2026-02-24) and the June 2026 plugin ecosystem. Findings recorded in
`2026-06-02-nvim-overhaul-reassessment.md`. This v2 folds those corrections in; the reassessment doc is
the audit trail. Nothing here is implemented yet.

**Scope:** A single-pass overhaul of the personal Neovim config: bring it under chezmoi management, fix
the confirmed bugs, modernize plugins, improve organization for agents, cut startup time, and make a
fresh machine reach a working editor via `chezmoi apply`.
**Out of scope:** the conform.nvim / nvim-lint migration off none-ls; any change to the tmux/Claude CLI
workflow beyond wiring `claudecode.nvim` with `provider = "none"`.

## What changed from v1 (read this first)

Three framing errors in v1 are corrected here, because they invalidate v1's rationales:

1. **This is not a LazyVim distribution.** The `LazyVim/LazyVim` import is commented out
   (`lua/config/lazy.lua:34`); there are zero active `lazyvim.plugins` references. The config is vanilla
   `lazy.nvim` with borrowed LazyVim patterns. v1's "stay on LazyVim" goal and anything reasoning from
   "LazyVim will default X" are struck. `lazyvim.json` and `lazyvim_*` augroups are vestigial starter
   scaffolding (delete per decision γ).
1. **The startup lever is `defaults.lazy = false`** (`lua/config/lazy.lua:41`), which forces *all* ~39
   plugin specs eager — not "~14 eager specs" as v1 claimed. The fix is to flip that flag and add
   per-spec triggers (decision δ), and the < 150 ms target is **non-binding** (it sits below the never-
   achieved ~190 ms baseline). The only hard criterion is "measurably faster."
1. **Two v1 bugs are dead and two are softened** — see §3. A new bug was found (`custom_api/git.lua:247`).

## Background

The nvim config is a **standalone git repo**, `git@github.com:webdavis/neovim-config.git`, checked out
at `~/.config/nvim` — **not** under this chezmoi source (`~/workspaces/Ivy/webdavis/dotfiles`, verified
via `chezmoi source-path`). A fresh machine therefore gets no working editor from `chezmoi apply`.

Baseline metrics: 83 plugin pins (`lazy-lock.json`), ~39 plugin spec files, 6 `custom_api/` modules
(~890 lines), nvim 0.12.2, `lazyvim.json` `extras: []`, startup regressed ~190 ms → 264 ms.

### Approaches considered (chezmoi integration)

| # | Approach | Verdict | Reason |
|---|----------|---------|--------|
| 1 | **Flatten the standalone repo into `dot_config/nvim/`** (this design) | **Chosen** | One source of truth; `chezmoi apply` sets up a fresh machine. (v1 cited the `chezmoi.nvim` autocmd as the reason — that rationale is **struck**: the autocmd watches `~/.local/share/chezmoi`, not the relocated source, and has `watch=false`.) |
| 2 | Keep separate repo, clone via a `run_once` script | Rejected | Two sources of truth; bootstrap still needs network + SSH at apply time. |
| 3 | git submodule | Rejected | Submodule friction with chezmoi's source-state model; detached-HEAD foot-guns. |

## §0 — Decisions

**Resolved:**

- **gitmoji.nvim → drop** (user confirmed: does not type `:sparkles:` codes).
- **All eight drops are safe** (reassessment validated each against the June ecosystem). The six v1
  "confident" drops stand.

**One open sub-decision (does not block the plan):**

- **octo.nvim picker backend.** v1's "switch snacks → fzf-lua because that's where LazyVim is heading"
  is **false** (LazyVim's octo extra tries telescope→fzf-lua→snacks; snacks is the bundled default). The
  live config already uses `picker = "snacks"` (`git.lua:1170`) and already depends on snacks.
  **Recommendation: stay on snacks** — this still removes telescope with **zero net plugin add** and
  contradicts v1's "net 83→75 (+fzf-lua)" framing in your favor. Choose fzf-lua only if you specifically
  want its speed. Default assumed below: **snacks**.

## §1 — Architecture & file layout (α–θ, corrected)

| ID | Decision | Rework folded in from reassessment |
|----|----------|------------------------------------|
| **α** | Flatten `webdavis/neovim-config` → `dot_config/nvim/`; archive original as `…-archive` | Justify on "one source of truth," **not** the inert `chezmoi.nvim` autocmd. Script removal of the nested `~/.config/nvim/.git` to avoid dual-VCS state. Sequencing is a **blocker** — see §4.1. |
| **β** | Track `lazy-lock.json` for reproducible pins | After `:Lazy update/restore`, run `chezmoi re-add ~/.config/nvim/lazy-lock.json`. **Set `checker.enabled = false`** (`lazy.lua:48-49`) — the background update checker rewrites the lock and fights chezmoi (§4.4). |
| **γ** | Delete orphan `lazyvim.json` | Low stakes; tie to the "not actually LazyVim" cleanup. |
| **δ** | Lazy-load + faster startup | **Primary lever = flip `defaults.lazy = false` → `true`** (`lazy.lua:41`) plus per-spec `event/ft/keys/cmd`. < 150 ms is **non-binding**; measure with `--startuptime`. |
| **ε** | Audit + fix `custom_api/`, keep module structure | Sound. Ship with a headless smoke test (the runtime-only bugs are invisible to luacheck — §4.8). |
| **ζ** | Bootstrap script `run_onchange_after_80-bootstrap-nvim.sh.tmpl` | **Reworked — blocker (§4.1).** Cold-path long/no timeout; drive Mason only via `MasonToolsInstallSync` with `run_on_start = false` (kill the `run_on_start=true`/`start_delay`/`debounce` race at `lsp.lua:212-226`); verify expected binaries and **exit non-zero on incompleteness** so `run_onchange` re-triggers; document network/SSH + cargo prerequisites; `{{ if eq .chezmoi.os "darwin" }}` guard. |
| **η** | Track `CLAUDE.md` + `.claude/` | Nested `CLAUDE.md` tracks fine. **Carve out `.config/nvim/.claude/settings.local.json`** — it is a permissions allowlist and must not land in a public repo. Confirm before propagating the allowlist machine-wide. |
| **θ** | Chezmoiignore dev-only files | **Bare patterns are target-root-anchored** (verified) — they would *not* ignore files under `dot_config/nvim/`. Use **path-anchored** entries: `.config/nvim/.luacheckrc`, `.config/nvim/stylua.toml`, `.config/nvim/.prettierignore`, `.config/nvim/README.md`, and the two v1 missed: `.config/nvim/docs/`, `.config/nvim/.github/`. |

## §2 — Plugin changes

### Drops (8) — most are transitive deps, so each is an atomic multi-edit

Removing a drop's spec block alone is a **no-op or an error**: `lazy.nvim` force-installs anything still
named in a `dependencies` list, and removing a dep a consumer still references errors at use-time. Land
each as one cohesive commit.

| Plugin | How loaded | Atomic edit | Migration target (verified) |
|--------|-----------|-------------|------------------------------|
| `cspell.nvim` | none-ls source | remove source `lsp.lua:247` | none (author-deprecated) |
| `gitmoji.nvim` | blink-cmp source | 3 edits/1 commit: dep `blink-cmp.lua:114` + provider `:261-263` + `sources.default` `:279` | none |
| `nvim-notify` | noice dep | remove dep; noice uses `snacks.notifier` | snacks.notifier |
| `gv.vim` | fugitive dep | remove from fugitive deps `git.lua:255` | `snacks.picker.git_log` |
| `git-messenger.vim` | explicit/dep | remove spec | gitsigns `blame_line` (already at `git.lua:199`) |
| `git-blame.nvim` | explicit | remove spec | **feature loss — flag (§4.10)**; `current_line_blame` must be *added* to gitsigns opts (`git.lua:60`); commit-URL keymaps have no gitlinker equivalent |
| `telescope.nvim` | octo dep | **last**: decide octo picker (§0) → remove dep `git.lua:1161` + delete standalone block `git.lua:1142-1155` | snacks (default) or fzf-lua |
| `boole.nvim` | explicit spec | **last**: port `<C-a>/<C-x>` augends to a new dial spec first, then remove | dial.nvim augends |

### Adds

| Plugin | Config | Note |
|--------|--------|------|
| `coder/claudecode.nvim` | `provider = "none"` | Healthy; `provider="none"` valid and endorses the tmux-pane workflow. **Pin a commit** — only tag is v0.3.0 (Sept 2025) while main advances daily. |
| `ibhagwan/fzf-lua` | octo picker backend | **Only if §0 chooses fzf-lua over snacks.** Otherwise omit (snacks already present). |

### Keeps / bumps (corrected)

- **`defaults.lazy`** flip is the real perf change (δ), not a plugin swap.
- **`nvim-surround`** → `^4.0.0` (current v4.0.5).
- **`none-ls`** — **no forced bump.** Bug #13 is refuted (the pinned commit `0b45795` already contains
  the 0.12 guards, verified on disk). none-ls publishes **no tags**; if ever bumped, update the lock to a
  main HEAD and record `0b45795` as the rollback anchor. Not part of this overhaul's required work.
- **catppuccin** — keep `name = "catppuccin"`. The v2.0.0 breaking change is the **colorscheme name**:
  change only `vim.cmd.colorscheme(...)` → `"catppuccin-nvim"` at `ui.lua:60`, and only when bumping the
  pin past `605b460`. Bufferline path (`ui.lua:70`) is already v2-correct.
- **nvim-treesitter** — already fully on the `main` branch (`treesitter.lua:153,195`); v1's "master→main
  critical gap" is **done, strike it**. New fact: nvim-treesitter was **archived 2026-04-03** — the pin
  keeps working but receives no upstream fixes; long-term path is nvim 0.12's builtin treesitter. Flag,
  no action. Verify `nvim-treesitter-context` (still on `master`) renders under 0.12.2 during any bump.
- **`gopls`** — still absent from Mason; add only if you want Go LSP (user preference).
- Keep `dial.nvim`, `markview`, `toggleterm`.

## §3 — Bug fixes (the work list)

13 confirmed + 1 newly found. Refuted/softened items noted at the bottom so they are not re-litigated.

| # | Severity | Bug | Location | Fix |
|---|----------|-----|----------|-----|
| 1 | high | `github.username()` is nil → runtime error on `<C-g>i` | `git.lua:267` | read `account().username` (cf. `git.lua:25`) |
| 4 | high | `default_branch` expects table, gets string | `custom_api/git.lua:231-232`; caller `git.lua:997-998` | pass `{repo=…}` or read the string directly |
| 4b | high | **NEW:** `string.format("…/%s/%s…", repo)` — two `%s`, one arg → raises whenever the GitHub-API fallback runs | `custom_api/git.lua:247` | supply both owner+name args; fix with #4 |
| 11 | high | hardcoded `mkdp_open_ip = "dresden.home.webdavis.io"` | `markdown.lua:314` | derive host or make configurable |
| 12 | **critical** | mason-lspconfig v2.1.0 never reads the `servers` block — lua_ls/clangd settings silently dropped | `lsp.lua:50-148` | move per-server config to `vim.lsp.config('lua_ls', {…})` / `vim.lsp.config('clangd', {…})`; leave mason-lspconfig for `ensure_installed` + `automatic_enable` only |
| 3 | medium | `toggle_runner("OverseerWatchRun")` → invalid action name | `overseer.lua:413-414`→`292` | bind `<M-[>` to `vim.cmd("OverseerWatchRun")` (`overseer.lua:253`) |
| 5 | medium | `delegate.setup()` called twice (benign) | `delegate.lua:171` + `keymaps.lua:6` | drop the auto-call or the explicit one |
| 8 | medium | `parse_branch_line` off-by-one drops first commit word when an upstream is present | `custom_api/git.lua:101-112` | return `i`, not `i+1` |
| 9 | medium | `vim.cmd("normal! \<Esc>")` sends literal text (on-disk has doubled backslash) | `delegate.lua:100` | use `nvim_replace_termcodes` / a real Escape |
| 2 | low | `<C-g>bc` mapped twice; first def dead | `git.lua:464-475` (dead) + `538-543` | delete the dead mapping |
| 6 | low | duplicate `checktime` autocmd, ungrouped/unguarded | `options.lua:116-118` vs `autocmds.lua:15-22` | remove the options.lua copy |
| 10 | low | `nvim_win_get_width(0)` evaluated at spec-load (returns 0) | `harpoon.lua:6` | make `opts` a function |
| 15 | low | hlslens `validate()` deprecation fix is exactly 1 commit ahead | pin `4254054` → `be2d7b2` | bump pin +1 |
| 16 | low (conditional) | catppuccin colorscheme rename | `ui.lua:60` | **only** when bumping past `605b460` — see §2 |
| 17 | low | noice `inc_rename=true` references uninstalled plugin (harmless) | `noice.lua:36` | set false or install inc-rename.nvim |

**Refuted — do not fix:** **#7** (dial binds nothing; `boole.lua:5-6` is the only binder — no live
`<C-a>/<C-x>` conflict; resolved for free when boole is dropped) and **#13** (none-ls 0.12 crashes —
guards present at the pinned commit, verified on disk).
**Softened:** **#14** Overseer is medium, not high — `run_template` is a working deprecated alias,
`bundles`/`log` are dead-but-harmless config, and `actions`→`keymap` never happened (that sub-claim was
wrong); clean up at `overseer.lua:83,192-200,219-229,254` opportunistically.

## §4 — Blockers & risks (prioritized)

**Blockers — resolve before any implementation:**

1. **Bootstrap reliability (ζ).** `timeout 120` cannot clone ~75 repos + build/download ~30 Mason tools
   (incl. `tree-sitter-cli` via cargo, `codelldb`); a SIGTERM mid-install still trips the `run_onchange`
   hash → "done." Make the script detectably-incomplete (verify binaries, exit non-zero), cold-path
   timeout, sync-only Mason, OS-guarded.
1. **Flatten over live work = data loss (α).** `~/.config/nvim` has uncommitted `lua/config/autocmds.lua`
   **and** a `stash@{0}` (WIP overseer keymaps). Hard-sequence: backup → commit `autocmds.lua` →
   pop/commit `stash@{0}` → push to the archive remote → copy working tree into source → `rm -rf` nested
   `.git`. Skipping this is irreversible.

**Important:**

3. **`checker.enabled = true` fights β** (`lazy.lua:48-49`) — set false or document the drift workflow.
1. **Bug #12 fix method is the riskiest to get wrong** — moving the `servers` table without adopting the
   `vim.lsp.config(...)` API reproduces the silent failure. Assert: open a `.c` file →
   `:lua =vim.lsp.get_clients()[1].config.cmd` shows `--clang-tidy`/`--header-insertion=iwyu`.
1. **No test catches the runtime-only bugs** (#1, #4, #4b, #8, #9, #12) — they pass luacheck and a clean
   startup while broken (exactly how #12 survived a year frozen). Add a headless Lua test
   (`nvim --headless -l test.lua`) and wire it to `just` / the bootstrap.

**Nice-to-have:**

6. **git-blame drop loses features** (§2) — `<C-g>By` copy-SHA and `<C-g>Bo/BO` commit-URL have no
   gitlinker equivalent. Flag to user; if retained, remap to `gitsigns.blame_line({full=true})` + a
   custom commit-URL action.

## §5 — Implementation sequence (safe order)

1. **Backup** `~/.config/nvim` → `~/workspaces/backups/<ts>.nvim-config.backup/`.
1. **Drain VCS state:** commit `autocmds.lua`; pop/commit `stash@{0}`; push to `webdavis/neovim-config`
   (becomes the archive).
1. **Flatten** into `dot_config/nvim/`; `rm -rf` nested `.git`; set `checker.enabled = false`; add
   path-anchored chezmoiignores + the `.claude/settings.local.json` carve-out; delete `lazyvim.json`.
1. **Per-server LSP fix (#12)** via `vim.lsp.config`, with the `cmd` assertion. Own commit.
1. **`custom_api/` fixes** (#1, #4, #4b, #8, #9) + the headless Lua test. Own commit.
1. **Remaining low-risk bug fixes** (#2, #3, #5, #6, #10, #11, #15, #17). Grouped sensibly.
1. **Coupled drops** as atomic per-plugin commits (cspell, gitmoji, nvim-notify, gv.vim, git-messenger,
   git-blame [with flagged loss]); **telescope last** after the octo picker is decided; **boole last**
   after the dial-augend spec is written.
1. **Adds:** `claudecode.nvim` (pinned commit, `provider="none"`); fzf-lua only if §0 chose it;
   `nvim-surround` `^4`; catppuccin rename only if bumping past v2.0.0.
1. **Lazy-load pass:** flip `defaults.lazy = true` + per-spec triggers; measure `--startuptime`.
1. **Bootstrap script** (hardened ζ); test against a clean `$HOME`.
1. **Optional:** `gopls`; opportunistic Overseer cleanup (#14).

## §6 — Success criteria

No startup errors; no feature errors; improved organization for agents; measurably faster startup;
workflow-improving plugins added; increased security; and a fresh machine reaches a working editor from
`chezmoi apply` alone.

## §7 — Verification

- `nvim --headless "+checkhealth" +qa` clean.
- Headless Lua test green: `extract_upstream` on `[origin/main]` keeps the first commit word;
  `default_branch` with a string arg + the no-upstream fallback path both run without raising;
  `delegate` sends a real Escape; clangd `cmd` carries its flags.
- `nvim --startuptime` below the pre-change number.
- Each high/critical bug exercised live (`<C-g>i` no longer errors; per-server LSP settings apply).
- `just l` passes on any shell/template/markdown added to this repo.
- `chezmoi apply --exclude=templates` (or the bootstrap in isolation) reaches a working editor on a clean
  `$HOME`.
