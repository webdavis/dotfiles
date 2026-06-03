<!-- Reassessment of 2026-05-24-nvim-overhaul-design.md, produced 2026-06-02 by a 9-agent
adversarial workflow (Opus 4.8). Bug #13 was resolved empirically AFTER synthesis: the live
none-ls pin 0b45795 contains all three 0.12 guards -> REFUTED (no crash). Treat #13 as settled. -->

# REVISED Assessment — May 2026 Neovim Overhaul Spec

**Reassessed:** 2026-06-02 against the frozen live config (commit `7681750`, 2026-02-24) and the June 2026 plugin ecosystem.
**Spec under review:** `/Users/stephen/workspaces/Ivy/webdavis/dotfiles/docs/superpowers/specs/2026-05-24-nvim-overhaul-design.md`

## 1. Verdict

**Patch, do not rewrite — but the patches are substantial and two of them invalidate the spec's framing.** The bug audit holds up well: **13 of 17 bugs confirmed as-stated** (1, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 15, 17), **2 refuted** (7, 13), and **2 changed to conditional/forward-looking** (14 downgraded high→medium; 16 is a pre-upgrade precondition, not a live defect). The plugin work is mostly sound — all 8 drops are still safe — but **2 plugin claims are stale or false**: the octo→fzf-lua "direction LazyVim is heading" rationale is verified false, and the treesitter `master→main` "critical gap" is already done (and treesitter was archived 2026-04-03, a new fact the spec couldn't know). Two **deeper root-cause errors** undercut spec framing: (a) the config is **not a LazyVim distribution** (import commented at `lua/config/lazy.lua:34`), so "stay on LazyVim" and the fzf-lua rationale rest on a framework that doesn't run; (b) the startup-perf decision misdiagnoses the lever — the regression is `defaults.lazy = false` (`lazy.lua:41`), not "~14 eager specs." Net: keep the spec's structure and decisions, but revise it in place with corrected rationales, two new bug rows, and a hardened bootstrap. **One blocker-grade gap** (fresh-machine bootstrap reliability) and **one data-loss blocker** (un-sequenced flatten over live dirty + stashed work) must be resolved before any implementation.

## 2. Bug re-verification

| # | Title | Verdict | Severity (re-assessed) | Current location |
|---|-------|---------|------------------------|------------------|
| 1 | `github.username()` nil — runtime error on `<C-g>i` | **confirmed** | high | `git.lua:267` (value really lives at `account().username`, cf. `git.lua:25`) |
| 2 | `<C-g>bc` registered twice; first def dead | **confirmed** | low | `git.lua:464-475` (dead) + `538-543` (wins) |
| 3 | `toggle_runner("OverseerWatchRun")` → invalid action name | **confirmed** | medium | `overseer.lua:413-414`→`292`; meant `vim.cmd("OverseerWatchRun")` (`overseer.lua:253`) |
| 4 | `git.default_branch(repo)` string-vs-table mismatch | **confirmed** | high | `custom_api/git.lua:231-232`; caller `git.lua:997-998` — **plus a second crash, see §5** |
| 5 | `delegate.M.setup()` called twice | **confirmed** | medium | `delegate.lua:171` + `keymaps.lua:6` (benign — re-registers identically) |
| 6 | Duplicate `checktime` autocmd; no buftype guard | **confirmed** | low | `options.lua:116-118` (ungrouped, unguarded) vs `autocmds.lua:15-22` (grouped, guarded) |
| 7 | dial vs boole `<C-a>/<C-x>` conflict | **refuted** | low (cleanup, not conflict) | dial is a **bare spec** (`init.lua:5`), binds nothing; only boole binds the keys (`boole.lua:5-6`). No live contention. |
| 8 | `parse_branch_line` off-by-one drops first commit word | **confirmed** | medium | `custom_api/git.lua:101-112` (returns `i+1`); only fires when an upstream is present |
| 9 | `vim.cmd("normal! \<Esc>")` sends literal, not Escape | **confirmed** | medium | `delegate.lua:100` (on-disk has doubled backslash — more clearly wrong than spec quotes) |
| 10 | `nvim_win_get_width(0)` evaluated at spec-load | **confirmed** | low | `harpoon.lua:6` (static table literal; fix = make `opts` a function) |
| 11 | Hardcoded `mkdp_open_ip = "dresden.home.webdavis.io"` | **confirmed** | high | `markdown.lua:314` (verbatim) |
| 12 | mason-lspconfig `servers` block silently dead | **confirmed** | **critical** | `lsp.lua:50-148`; mason-lspconfig **v2.1.0** never reads `servers`; needs `vim.lsp.config(...)` — see §5 |
| 13 | 3 none-ls nvim-0.12 crashes unfixed in pin | **refuted** | — (n/a at pin) | Two contradictory analyses — **see note below** |
| 14 | Overseer v2.0 breakage (`actions`→`keymap`, `bundles`, `run_template`) | **changed** | high → **medium** | None is a hard error: `run_template` is a working deprecated alias; `bundles`/`log` are dead config; `actions` was *never* renamed to `keymap` (that sub-claim is wrong). `overseer.lua:83,192-200,219-229,254` |
| 15 | hlslens deprecation fix 1 commit ahead | **confirmed** | low | pin `4254054`; fix `be2d7b2` (`validate()` deprecation) is exactly +1 |
| 16 | Catppuccin `catppuccin`→`catppuccin-nvim` rename | **changed** | low (conditional) | Current pin `ce8d176` is **pre-v2.0.0**; `name="catppuccin"` is correct *today*. It is an **upgrade precondition**, not a live bug — and the required edit is `ui.lua:60` (colorscheme call), **not** `name` at `ui.lua:7` (README keeps `name`). |
| 17 | noice `inc_rename=true` references uninstalled plugin | **confirmed** | low | `noice.lua:36`; inc-rename.nvim not declared/installed — dead preset, harmless |

> **Bug #13 conflict — must resolve before acting.** The two inputs disagree on the *same pinned commit*. The BUGS entry says the pin is `0b45795` (2025-12-29) and **already contains** all three 0.12 fixes (cites HEAD commits `3ac8b7b`, `d595330` and a guarded `todo_comments.lua:15-25`), so the crashes **do not reproduce** → refuted. The PLUGINS entry says the same `0b45795` pin **predates** the fixes (`#341` 2026-04-29, `#338` 2026-04-19, `supports_method` 2026-04-24) → real, critical. These cannot both be true. **Action: re-derive empirically before any bump** — `cd ~/.local/share/nvim/lazy/none-ls.nvim && git log --oneline -1` to confirm the live commit, then `git log --oneline | grep -E '341|338|str_byteindex|supports_method'` to see whether the fixes are ancestors of that commit, and finally `nvim --headless "+checkhealth" +qa` plus a diagnostics smoke-test on 0.12.2. Do not treat #13 as settled in either direction; the bump's risk rating depends entirely on which baseline is real.

## 3. Plugin landscape changes since May

**Most "drops" are transitive deps, not explicit specs — this changes the edit procedure, not the decision.** Of the eight drops, only **boole, git-blame, git-messenger, octo's telescope-as-standalone** are explicit specs; **telescope (octo dep), nvim-notify (noice dep), gv.vim (fugitive dep), cspell (none-ls dep), and gitmoji (blink-cmp source)** are wired as dependency edges. Consequence: **removing the spec block alone is a no-op or an error.** lazy.nvim force-installs any plugin that remains in a `dependencies` list (telescope stays installed until removed from `git.lua:1161`), and removing a dependency while a consumer still references it errors at use-time (gitmoji → blink "provider not found" if `blink-cmp.lua:279` keeps `"gitmoji"`). Each coupled drop is therefore an **atomic multi-edit in one commit**, not a single deletion. The spec's §4 step 3 treats them uniformly — that must be enumerated per-plugin.

Stale / wrong since May, with corrections:

- **octo → fzf-lua rationale is FALSE.** "The direction LazyVim is heading" does not hold: LazyVim's octo extra picks dynamically with **telescope first**, then fzf-lua, then snacks (`lua/lazyvim/plugins/extras/util/octo.lua`), and snacks is the bundled default for fresh installs. fzf-lua *is* a valid octo picker, so the switch is fine **on its own merits**, but the justification must be rewritten to "fzf-lua is a first-class octo picker with low startup cost." **Better still:** the live config already uses `picker = "snacks"` (`git.lua:1170`) and already depends on snacks — **staying on snacks removes telescope with zero net plugin add**, contradicting the spec's "net 83→75 (+fzf-lua)" framing. Recommend keeping snacks unless the user specifically wants fzf-lua's speed.
- **treesitter `master→main` "critical gap" is already done.** Live config is fully on `main` (`treesitter.lua:153`, new `install()` API at `:195`, main-pinned lock). **Drop this item from §3 risks and §4 pre-flight.** Replace with the new fact: **nvim-treesitter was archived (read-only) 2026-04-03** — the pin keeps working but gets no upstream fixes; long-term path is nvim 0.12's builtin treesitter. Flag, no action now.
- **nvim-treesitter-context (NEW concern):** still on `master` (not archived, `pushed_at` 2026-05-06) while core is on `main` — verify `:checkhealth` and the context bar render under 0.12.2 during the bump. Likely fine (works against `vim.treesitter`); flag only.
- **none-ls has NO tags** (maintainers marked tagged releases "not planned," issue #179). "Bump for fixes" is imprecise — there is no version to bump *to*; the action is to **update the lazy-lock commit pin to a main HEAD**, and **record current `0b45795` as the rollback anchor** (see §5 / bug #13).
- **catppuccin v2.0.0 (Apr 2 2026):** the breaking change is the **colorscheme name**, not the plugin name — keep `name="catppuccin"`, change only `vim.cmd.colorscheme("catppuccin")` → `"catppuccin-nvim"` at `ui.lua:60`, in the same commit as the pin bump past `605b460`. Bufferline path (`ui.lua:70`) is already v2-correct.
- **Healthy / unchanged:** `claudecode.nvim` (`provider="none"` still valid and explicitly endorses the tmux workflow; only tag is v0.3.0/Sept-2025 while main advances daily → **pin a commit, don't track main**), `nvim-surround` (bump `^3→^4`, current v4.0.5), `dial.nvim`, `markview`, `toggleterm`, `gopls` (still absent — user-preference add).

## 4. Architecture decisions (α–θ)

| ID | Decision | Status | Required rework |
|----|----------|--------|-----------------|
| **α** | Flatten standalone repo into `dot_config/nvim/` | **risky — keep, fix rationale** | The stated justification is **inert**: `chezmoi.nvim`'s autocmd watches `$HOME/.local/share/chezmoi/*` (`chezmoi.lua:25-32`), but the real source is `~/workspaces/Ivy/webdavis/dotfiles` (verified). Paths don't match; the autocmd also has `watch=false` + commented keys. Strike the autocmd rationale (flatten on "one source of truth" merits instead). **Script removal of `~/.config/nvim/.git`** (live clone of the to-be-archived remote) to avoid dual-VCS state. |
| **β** | Track `lazy-lock.json` | **sound, with caveat** | Add operational note: after `:Lazy update/restore`, run `chezmoi re-add ~/.config/nvim/lazy-lock.json`. **But see the checker gap (§5)** — `checker.enabled=true` automates the very drift β tries to control. |
| **γ** | Delete orphan `lazyvim.json` | **sound (low stakes)** | Confirm-then-delete; LazyVim regenerates if ever needed. Tie to the "not actually LazyVim" cleanup. |
| **δ** | Lazy-load ~14 specs; target <150ms | **flawed lever + stretch target** | **Root cause is `defaults.lazy = false` (`lazy.lua:41`)** — forces *all* ~39 plugin files eager, not 14. Flip `defaults.lazy = true` (with per-spec `event/ft/keys/cmd`) as the primary lever. Treat <150ms as **non-binding** (below the never-achieved ~190ms baseline); §5's real criterion is only "faster startup." Measure with `--startuptime`. |
| **ε** | Audit + fix `custom_api/`, keep module structure | **sound** | No architectural objection. Ship with a headless smoke test (bugs #1/#4/#8/#9 are runtime-only, invisible to luacheck). |
| **ζ** | Bootstrap script (`+Lazy! restore +MasonToolsInstallSync`, `timeout 120`) | **flawed → BLOCKER** | `timeout 120` is unrealistic for a cold machine (clone ~75 repos + build/download ~30 Mason tools incl. `tree-sitter-cli` from source, `codelldb`, `swiftlint`); SIGTERM mid-install + `run_onchange` hash marks it "done." Also `mason-tool-installer` `run_on_start=true`/`start_delay=2000`/`debounce_hours=5` (`lsp.lua:212-226`) races the sync install. **Rework: long/no timeout on cold path; drive Mason only via `MasonToolsInstallSync` with `run_on_start=false`; verify expected binaries post-install and exit non-zero on incompleteness; document network/SSH + cargo prerequisites; add OS guard.** |
| **η** | Track `CLAUDE.md` + `.claude/` | **risky** | Nested `CLAUDE.md` *is* tracked (verified: chezmoi bare ignores are target-root-anchored). But blanket-tracking `.claude/` would **commit `settings.local.json` (a permissions allowlist) into a public repo**. Carve it out: `.config/nvim/.claude/settings.local.json`. Confirm with user before propagating the find/luacheck/git-add allowlist machine-wide. |
| **θ** | Chezmoiignore dev-only files | **flawed** | Bare patterns are **target-root-anchored** (empirically verified) — `README.md`/`stylua.toml`/etc. under `dot_config/nvim/` would **not** be ignored and would land in `$HOME`. Rewrite to **path-anchored** entries (`.config/nvim/.luacheckrc`, `.config/nvim/stylua.toml`, `.config/nvim/.prettierignore`, `.config/nvim/README.md`) and add what the spec missed: `.config/nvim/docs/`, `.config/nvim/.github/` (dead CI for the archived repo → would map to `~/.github/`). |

## 5. Gaps & risks (prioritized)

**Blockers — resolve before implementation:**

1. **Fresh-machine bootstrap reliability (ζ).** The `timeout 120` SIGTERM, the silent per-tool Mason failures (no non-zero nvim exit), the `run_on_start` async race + 5h debounce, and macOS-specific failure surface (`tree-sitter-cli` needs cargo; `kubescape`/`nixfmt` already disabled as unsupported) together threaten §5's core promise: "fresh machine reaches a working editor from `chezmoi apply` alone." Make the script **detectably-incomplete** (verify binaries, exit non-zero → `run_onchange` re-triggers), cold-path timeout, sync-only Mason.
2. **Flatten over live dirty + stashed work (α).** `~/.config/nvim` has **uncommitted `lua/config/autocmds.lua`** and **`stash@{0}`** (WIP overseer keymaps). Hard-sequence: backup to `~/workspaces/backups/` per convention → commit `autocmds.lua` + pop/commit `stash@{0}` → push to `webdavis/neovim-config` (the archive) → copy working tree into source → `rm -rf` nested `.git`. Skipping this = irreversible loss.

**Important:**

3. **Spec premise "stay on LazyVim" is false.** Import commented (`lazy.lua:34`), zero active `lazyvim.plugins` references (verified). `lazyvim.json`/`lazyvim_*` augroups are vestigial starter scaffolding. Reframe the goal as "vanilla lazy.nvim + borrowed LazyVim patterns." This is the root cause behind the fzf-lua false rationale and any "LazyVim will default X" reasoning.
4. **`checker.enabled=true` (`lazy.lua:48-49`) fights β.** The background update checker mutates `lazy-lock.json`, which chezmoi then reports as drift and reverts. Set `checker.enabled=false` (manual `:Lazy update` + `chezmoi re-add`) or document the drift workflow. Decide before flattening.
5. **Bug #4 has a second, distinct crash.** `custom_api/git.lua:247` — the GitHub-API fallback `string.format("...repos/%s/%s...", repo)` has **two `%s`, one arg** → `string.format` raises on the missing value whenever the fallback runs, even after the calling-convention half is fixed. **Promote to its own §3 row** so a row-by-row implementer doesn't miss it; fix both halves together and exercise the no-upstream fallback in the smoke test.
6. **Bug #12 fix method unspecified — riskiest to get wrong.** mason-lspconfig v2.1.0 ignores `servers`. The fix must adopt the v2 API: `vim.lsp.config('lua_ls', {...})` / `vim.lsp.config('clangd', {...})`, leaving mason-lspconfig for `ensure_installed` + `automatic_enable` only. Just moving the table reproduces the silent failure. Concrete assertion: open a `.c` file → `:lua =vim.lsp.get_clients()[1].config.cmd` and confirm `--clang-tidy`/`--header-insertion=iwyu` present.
7. **none-ls bump has no rollback path and shares `lsp.lua` with two other edits.** Record current pin `0b45795` as the rollback anchor (no tags exist). Land the bump as its **own commit**, smoke-test on 0.12.2, *then* do the cspell drop (`lsp.lua:247`) and the bug-#12 fix as separate commits. (Same discipline for catppuccin and treesitter pin bumps.)
8. **No test catches the runtime-only bugs.** #1/#4/#8/#9/#12 pass luacheck and clean startup while broken — exactly how #12 survived a year frozen. Add a headless Lua test (`nvim --headless -l test.lua`): `extract_upstream` on `[origin/main]` retains first word; `default_branch` with string arg + no-upstream fallback; delegate sends a real Escape termcode; clangd cmd-flag assertion. Wire to `just`/bootstrap.
9. **Coupled-drop ordering (§4 step 3).** Enumerate atomic multi-edits: gitmoji = 3 edits/one commit (dep `:114` + provider `:261-263` + `sources.default` `:279`); telescope = switch octo picker + remove dep `:1161` + delete standalone block `:1142-1155`; gv.vim = remove from fugitive deps `:255`. Smoke-test completion after gitmoji, octo picker after telescope.

**Nice-to-have:**

10. **git-blame drop loses features.** `<C-g>By` (copy SHA) and `<C-g>Bo/BO` (commit URL) have **no equivalent** in the named targets — gitlinker only makes file/line permalinks. Also `current_line_blame` is **not** set in gitsigns opts (`git.lua:60`) — must be *added*, not toggled. Flag to user; if retained, remap to `gitsigns.blame_line({full=true})` (already at `git.lua:199`) + custom commit-URL action.
11. **Bootstrap numbering/OS guard.** `80-` doesn't collide (`70-` exists). Add `{{ if eq .chezmoi.os "darwin" }}` guard reasoning and specify the `run_onchange` hash idiom (embed hash of `lazy-lock.json`+`init.lua` in a template comment).

## 6. Recommended next step

**Revise the spec in place — do not rewrite.** Its skeleton (approaches, α–θ, the bug table, drop list, success criteria) is sound; the defects are corrections and additions, not a different design. Concretely:

1. **Fix framing first:** strike "stay on LazyVim" (reframe vanilla lazy.nvim); correct δ to name `defaults.lazy=false` as the lever and demote <150ms to non-binding; correct the octo rationale (and recommend staying on snacks); replace the treesitter "critical gap" with the archived-as-of-2026-04-03 note.
2. **Bug table edits:** mark #7 and #13 refuted (and add the #13 empirical-recheck note), #14 medium, #16 conditional/`ui.lua:60`; **add a new row for the `git.lua:247` format-args crash**; specify the #12 fix as `vim.lsp.config(...)`.
3. **Harden α/ζ/β/η/θ** per §4–5 (flatten sequencing, bootstrap completeness + cold timeout + sync-Mason, `checker.enabled=false`, `.claude/settings.local.json` carve-out, path-anchored ignores).

**Safe implementation sequence (post-revision):**

1. **Backup** `~/.config/nvim` → `~/workspaces/backups/<ts>.nvim-config.backup/`.
2. **Drain VCS state:** commit `autocmds.lua`, pop/commit `stash@{0}`, push to `webdavis/neovim-config` (archive).
3. **Flatten** into `dot_config/nvim/`; `rm -rf` nested `.git`; set `checker.enabled=false`; add path-anchored chezmoiignores + `.claude/settings.local.json` carve-out; delete `lazyvim.json`.
4. **none-ls in isolation:** re-verify bug #13 empirically, record `0b45795` rollback, bump pin (if needed), `:checkhealth` + diagnostics smoke-test. Separate commit.
5. **Per-server LSP fix (#12)** via `vim.lsp.config`, with the `:lua =...config.cmd` assertion. Separate commit.
6. **Custom_api fixes** (#1, #4 both halves, #8, #9) + headless Lua test. Separate commit.
7. **Remaining low-risk bug fixes** (#2, #3, #5, #6, #10, #11, #15, #17). Group sensibly.
8. **Coupled drops** as atomic per-plugin commits (cspell, gitmoji, nvim-notify, gv.vim, git-messenger, git-blame-with-flagged-loss); **telescope last** after octo picker decided; **boole last**, only after the dial-augend spec is written.
9. **Adds:** `claudecode.nvim` (pinned commit, `provider="none"`); fzf-lua only if user opts in over snacks. `nvim-surround` `^4`; catppuccin rename **only** if bumping past v2.0.0.
10. **Lazy-load pass:** flip `defaults.lazy=true` + per-spec triggers; measure `--startuptime`.
11. **Bootstrap script** (hardened ζ); test against a clean `$HOME`.
12. **Verify** per §6: `+checkhealth` clean, smoke tests green, `just l` passes, `chezmoi apply --exclude=templates` reaches a working editor.

Spec file to revise: `/Users/stephen/workspaces/Ivy/webdavis/dotfiles/docs/superpowers/specs/2026-05-24-nvim-overhaul-design.md`. Key live-config evidence verified this pass: `lua/config/lazy.lua:34` (LazyVim commented), `:41` (`defaults.lazy=false`), `:48-49` (`checker.enabled=true`); `chezmoi source-path` = `~/workspaces/Ivy/webdavis/dotfiles`; `~/.config/nvim` remote = `git@github.com:webdavis/neovim-config.git`.
