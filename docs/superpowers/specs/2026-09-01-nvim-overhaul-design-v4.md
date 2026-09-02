# Neovim Config Overhaul Design Spec: v4.3

**Date:** 2026-09-01
**Status:** v4.3, active design, the document the implementation plan is built from. Nothing here is
implemented. The operator's decisions A to H and the four approved custom plugins (recorded in
`~/.claude/pipeline/nvim-overhaul/inventory-2026-09-01.md`) are binding inputs, not open questions.
v4.1 applied the two reviews of v4 (`spec-v4-review-fable-2026-09-01.md`, 11 findings, and
`spec-v4-review-sol-2026-09-01.md`, 16 findings, both under `~/.claude/pipeline/nvim-overhaul/`), and
everything they changed stays applied. v4.2 applies sol's review of v4.1,
`spec-v4-1-review-sol-2026-09-01.md` (15 findings, nine HIGH), in the same directory. The changes: the
import proof keeps its before and after evidence in separate directories with complete commands, its
byte diff excludes the one file the drain edits, and its keymap dump states its preconditions and
projects stable fields only (3.7); the resolver and the launch helper identify Neovim by pane id and by
the socket the helper pins through the split's environment, never by herdr's UI-wide focus, and the
agent lookup is the `herdr-nvim` plugin's own workspace-scoped one (7.2, 7.3); the MCP criteria
separate native instance choice from socket pinning and the decision table is total (7.3); the Codex
registration extends the Codex config template now in the worktree instead of adding a competing
modify template (section 2, 7.3); the Enter key and multiline sends are a recorded PR 11 check (7.4);
plugin #3 sends only to an idle or done agent and queues otherwise (7.7); the agent name is derived
from the pane id (7.2); the MCP counts separate `claude mcp list` from `~/.claude.json` (section 2);
one performance gate is stated once (9.1); stylua and luacheck join the machine package YAML (3.8);
the pns acceptance check names the Discord card (10.9); PRs 4, 10, 17, 22, 26, 29 and 30 are split at
behavior boundaries and every shared-file predecessor is a dependency cell (section 11, then 46 PRs).

v4.3 applies sol's review of v4.2, `spec-v4-2-review-sol-2026-09-01.md` (12 findings, seven HIGH), in
the same directory, one line per finding:

1. The flatten `rsync` also excludes `.github` and `.gitignore`, as the 3.3 table already said (3.3).
1. PR 9 is evaluation-only; the PR that ships the server registers it: PR 10a on the two `nvim-mcp`
   rows, PR 10b on the crate rows, and the 7.5 rule lands with the registration (7.3, 7.5, 10.8, 11).
1. The resolver lists sockets under `${XDG_RUNTIME_DIR:-${TMPDIR}nvim.${USER}}`, the expression
   `:help serverstart()` documents (2, 7.3).
1. PR 30d never writes `lazy = false` onto a spec that has a trigger; the section 9 table now says which
   specs keep their triggers, which already carry `lazy = false`, and which get it written (9, 11).
1. The headless startup runs fire `User VeryLazy` explicitly before `+qa`, the number is labelled
   synthetic, and the TUI run is the acceptance check (9.1, 10.3, 3.7).
1. One fixed benchmark directory, `~/.local/state/nvim-overhaul/bench`, entered with `cd` before
   every measured run (9.1, 3.7).
1. Plugin #3 is best-effort: one waiter at a time, the state re-read immediately before
   `pane send-text`, a send that fails the recheck is dropped with a notice (7.7).
1. Dependency edges: PR 12 joins the `lsp.lua` chain, PR 13 depends on PR 11 (`custom_api/herdr.lua`),
   the registration PR depends on PR 4d (`CLAUDE.md`) and on the Codex-template PR, and the shared-file
   paragraph lists the files per edge (11).
1. PRs 5, 7, 19, 30b and 30c are split at the one-behavior boundary into 5a, 5b, 7a to 7f, 19a, 19b,
   30b1, 30b2 and 30c1 to 30c9; every cross-reference follows; section 11 now has 62 PRs (4, 9, 11,
   appendix A).
1. The import proof computes both warm medians and asserts `after <= before + 10` (3.7).
1. The health normalization replaces only the runtime directory, the state directory, `$TMPDIR` and
   timestamps, never every slash-prefixed token (3.7).
1. The branch counts are refreshed with the compared commit ids recorded (2).

## 0. Status and provenance

This version supersedes v1 (`2026-05-24-nvim-overhaul-design.md`), v2
(`2026-06-02-nvim-overhaul-design-v2.md`) and v3 (`2026-06-03-nvim-overhaul-design-v3.md`, on the
`origin/nvim-overhaul` branch only, together with the agent-integration research doc
`docs/research/2026-06-03-nvim-coding-agent-integration.md`). The reassessment
(`2026-06-02-nvim-overhaul-reassessment.md`) stays the audit trail for the v1 to v2 corrections and is
not restated here. The 78-item inventory of 2026-09-01 is the scope authority: appendix A maps every
item to the section and pull request that closes it, and the five struck items stay struck.

What changed since v3, and why:

1. **Every tmux premise is replaced by herdr.** v2, v3 and the research doc assume the agent lives in a
   tmux pane and that `delegate.lua` drives it with `tmux send-keys`. The tmux to herdr migration
   (`2026-06-18-tmux-to-herdr-migration-design.md`) shipped: the multiplexer is herdr 0.8.2, panes carry
   `HERDR_ENV`, `HERDR_WORKSPACE_ID`, `HERDR_TAB_ID`, `HERDR_PANE_ID`, `HERDR_SOCKET_PATH` and
   `HERDR_BIN_PATH` (the six `HERDR_*` variables in this pane today; there is no `HERDR_SESSION`), pane
   navigation is `smart-splits.nvim` with a custom herdr backend (`lua/smart-splits/mux/herdr.lua`), and
   `nvim-tmux-navigation` is already gone from the live config. `delegate.lua` cannot work at all any
   more (`vim.env.TMUX` is never set), so its retirement is a cleanup, not a choice. vim-slime, which
   the research doc kept for raw-text sends, has no herdr target either; section 7 designs one.
1. **Decision A resolves the v2 versus v3 conflict as both.** `claudecode.nvim` (selection push, provider
   `none`, pinned commit) AND a Neovim Model Context Protocol (MCP) server (live unsaved buffer, pull
   model) ship. v3 removed `claudecode.nvim`; that removal is reversed.
1. **Decision H replaces the v2 and v3 fix-while-flattening order.** The live config is imported into
   chezmoi UNCHANGED first, proven by a byte-level and behavior diff (section 3), and every fix lands
   in a later reviewable pull request. The roadmap's SP6 directives (re-check the branch, back up both
   repositories, inventory the live config, import unchanged, modernize later) are satisfied by
   sections 2, 3 and 11.
1. **Swift, iOS and Vapor come early.** The operator writes Swift starting now, so `xcodebuild.nvim`,
   `sourcekit-lsp` and a Vapor smoke check are in scope and form the first modernization pull request
   after the import. The four Homebrew tools the stack needs (`xcode-build-server`, `xcbeautify`,
   `swiftformat`, `swiftlint`) are already declared in the YAML and installed, so that PR adds no
   package.
1. **Decisions B to G** are folded in: snacks stays the octo picker (telescope drops with zero net add),
   the two git-blame keymaps are rebuilt before git-blame drops, gopls is added, `copy_URL_to_clipboard`
   is renamed, neotest and auto-save are in scope, and this is its own program with its own plan.
1. **All four custom plugins are approved** (the inventory records #4 first and #1 to #3 later the same
   day). #4 is section 7.3: evaluate `nvim-mcp` for herdr workspace awareness first, build a custom
   server only if it falls short. #1 to #3 are section 7.7 with their own PRs. Appendix B now holds
   only the two ideas the operator said not to build.
1. **The acceptance bar is stated once** (section 10): Neovim starts headless with no error output,
   `checkhealth` is clean under a stated definition, and `--startuptime` is measured and below the
   baseline re-measured on the import day (section 2 records today's provisional numbers).

## 1. Scope and non-goals

In scope: everything on the 78-item inventory that is not struck, closed by the pull requests in
section 11. Concretely: the chezmoi import and archive, the bug floor (section 4), the plugin drops,
adds and bumps (section 5), the `custom_api` redesign with headless Lua tests (section 6), agent
integration against herdr including the four custom plugins (section 7), the keymap and which-key
design (section 8), the lazy-load pass (section 9), the bootstrap script, and the verification
(section 10).

Non-goals, stated so nobody widens the program:

- The conform.nvim / nvim-lint migration off none-ls stays out (inventory item 38, deferred). none-ls
  is not force-bumped; if a bump is ever needed it is its own commit with `0b45795` as the rollback
  anchor (item 34).
- SP5 (xonsh) and the SP4 bash-setup work come AFTER this program. Nothing here touches the shell.
- No nvim-dap. `xcodebuild.nvim` can drive a debugger, but nvim-dap is not on the inventory and is
  not installed; it is an open question with a default of no (section 12).
- No removal mechanisms in the dotfiles repo (operator ruling 2026-08-02): the old `~/.config/nvim/.git`
  and any file the flatten leaves behind are removed by hand, by the operator, with `trash`.
- No changes to herdr's config, the pns engine, or moshi. The custom plugins are producers and
  clients of those systems, never edits to them.

## 2. Current state, verified 2026-09-01

Everything below was measured today; the commands are in the verification section so the same
numbers can be reproduced before each pull request.

| Fact                              | Value                                                        |
| --------------------------------- | ------------------------------------------------------------ |
| `origin/nvim-overhaul`            | 3 ahead, 1759 behind `origin/main` (`git rev-list --left-right --count origin/main...origin/nvim-overhaul` printed `1759 3` after a fetch; compared `origin/main` `6af1aecc` against `origin/nvim-overhaul` `22a1fb56`; the count grows with every merge to `main`, so the ids are what a re-run compares against); docs only (v3 + research) |
| Live config repo                  | `~/.config/nvim`, remote `git@github.com:webdavis/neovim-config.git` |
| Live `HEAD`                       | `d45b190`, 7 commits ahead of `origin/main` at `0beb834`, never pushed (`git -C ~/.config/nvim status -sb` prints `## main...origin/main [ahead 7]`) |
| Uncommitted, modified             | `lazy-lock.json` (+1 line: the `herdr-nvim` pin), `lua/config/autocmds.lua` (+`aerial` in the close-sidebars filter) |
| Untracked                         | `CLAUDE.md`, `lua/plugins/herdr-nvim.lua`                     |
| `stash@{0}`                       | "WIP on main: 3e067bd feat(overseer): …"; touches ONLY `lazy-lock.json` (14 pins); its base `3e067bd` is NOT an ancestor of `HEAD` (rewritten history) |
| Neovim                            | `NVIM v0.12.5`, LuaJIT 2.1.1787165859                          |
| Plugin pins in `lazy-lock.json`   | 84 (v2 counted 83; `herdr-nvim` was added since)              |
| Plugin spec files                 | 40 under `lua/plugins/`; 6536 Lua lines total; `custom_api/` 890 lines in 6 modules |
| `defaults.lazy`                   | `false` (`lua/config/lazy.lua:41`); `checker.enabled = true` (`:48`) |
| Startup, first three-run batch    | cold 411.6 ms; warm 181.7 ms and 178.0 ms (runs 2 and 3)      |
| Startup, five-run batch (9.1)     | cold 611.3 ms; runs 2 to 5: 203.6, 259.3, 205.9, 198.0 ms; **median 204.7 ms** |
| Headless stderr                   | empty on all eight runs (no error output)                     |
| `checkhealth`                     | 640 OK, 44 WARNING, 13 ERROR lines (breakdown below)          |
| `stylua --check .` on the live config | clean (exit 0) with Mason's stylua 2.3.1; Homebrew ships 2.5.2 |
| Swift toolchain on dresden        | Xcode 26.6; `sourcekit-lsp` at `/usr/bin` and in the Xcode toolchain; `xcode-build-server`, `xcbeautify`, `swiftlint`, `swiftformat` installed from Homebrew and declared in `.chezmoidata/system_packages_autoinstall.yaml` |
| `gopls`                           | absent; Mason reports "Go: not available"                     |
| `luacheck`                        | not installed (not in Homebrew, not in Mason); Homebrew formula `luacheck` 1.2.0 exists |
| herdr                             | 0.8.2; `HERDR_ENV`, `HERDR_WORKSPACE_ID`, `HERDR_TAB_ID`, `HERDR_PANE_ID`, `HERDR_SOCKET_PATH`, `HERDR_BIN_PATH` set in this pane (values today: workspace `wW`, tab `wW:t9`, pane `wW:p3K`); every `agent`, `pane` and `workspace` subcommand prints JSON by default and none of them takes `--json` (only `herdr api schema --json` does); `pane split` takes `--env <KEY=VALUE>`; agent names must match `[a-z][a-z0-9_-]{0,31}` and be unique among live agents (`herdr --skill`); focus is UI-wide: one `focused = true` pane in the whole session, `false` on every agent in a background workspace (`herdr agent list` today) |
| Neovim RPC socket                 | `stdpath("run")` = `$TMPDIR/nvim.stephen/<random>/` on dresden, one `nvim.<pid>.0` per instance; the portable root is `${XDG_RUNTIME_DIR:-${TMPDIR}nvim.${USER}}` (`:help serverstart()`, `vimfn.txt:8813`: "Example bash command to list all Nvim servers: `ls ${XDG_RUNTIME_DIR:-${TMPDIR}nvim.${USER}}/*/nvim.*.0`"); an instance reads its own path from `v:servername` (verified: `nvim --headless -c 'echo v:servername' +qa` prints it) |
| MCP servers, Claude               | REGISTERED servers are the `mcpServers` keys of `~/.claude.json`: 3 (composio, cua-computer-use, workspace-mcp), and "registered" in this spec always means that file. `claude mcp list` printed 11 lines today (`claude mcp list \| grep -c ' - '`): those 3 plus 7 claude.ai connectors and 1 plugin server (playwright); the connector lines depend on the claude.ai login state, so the same command prints fewer on another day and its count is never what the spec cites. No Neovim server in either |
| MCP servers, Codex                | `~/.codex/config.toml` holds 10 `[mcp_servers.*]` tables (`grep -c '^\[mcp_servers\.' ~/.codex/config.toml`). The target is not chezmoi-managed on `main`, but this worktree carries an uncommitted `private_dot_codex/private_config.toml.tmpl`, a full template of that file (its `[mcp_servers.*]` tables at lines 225 to 325) that another slice is landing; it is the one Codex source this spec extends (7.3) |

The two startup batches disagree by about 25 ms because the second ran while a dozen agent processes
were busy on the machine. Both are provisional, and neither cold value is a 9.1 cold number: whether
`purge` preceded run 1 is not recorded, so both are first-run numbers. The binding baseline is
re-measured with the 9.1 method on the import day, immediately before the flatten, on a machine with
no agent running, and is written into the import PR body; every later PR compares against the number
the previous PR recorded.

The stash is a stale lock snapshot: compared with the live lock it holds older pins for `aerial.nvim`,
`git-blame.nvim` and `overseer.nvim`, a different `lazy.nvim` pin, still lists `nvim-tmux-navigation`,
and lacks `herdr-nvim` and `smart-splits.nvim`. Nothing in it is wanted (section 3.1).

The startup profile's top cost centers, from the first batch's run 1: `require('config.lazy')` 379.6 ms
of the 397.5 ms spent sourcing `init.lua`; `mason-lspconfig` 57.2 ms; `treesj` 17.8 ms;
`telescope.config` 17.6 ms; `octo` 12.6 ms; `auto-save` 10.6 ms; `null-ls` 8.9 ms; `mason-registry`
8.9 ms. All of these load eagerly because of `defaults.lazy = false`.

`checkhealth` errors by section (the 13 ERROR lines):

| Section         | Count | Lines                                                                 | Verdict                              |
| --------------- | ----- | --------------------------------------------------------------------- | ------------------------------------ |
| lazy            | 1     | hererocks `luarocks` not installed                                    | tool absent, not needed (no rock deps) |
| null-ls         | 3     | `nixfmt`, `rubocop`, `eslint` "not executable"                       | CONFIG BUG: sources declared for absent binaries |
| nvim-treesitter | 1     | "is not in runtimepath"                                               | CONFIG BUG candidate; this is item 68's runtimepath bug, investigate |
| snacks          | 8     | image `setup did not run`; `gs`, `tectonic`/`pdflatex`, `mmdc`, kitty graphics; `vim.ui.input`/`vim.ui.select` not set to Snacks; `lazygit` | 5 are absent optional tools; the two `vim.ui.*` lines and `setup did not run` are headless artifacts to re-check in a TUI run |

The 44 warnings are dominated by absent optional providers (Go, luarocks, PHP, Java, Julia, node
`neovim` package, perl, python `neovim`, ruby host), overseer's 14 "no <buildfile> found" lines for the
measurement directory, Mason's version notice, and one `vim.validate{<table>}` deprecation (the hlslens
pin, item 16).

## 3. Migration design

### 3.1 Drain the live repository

Rule, applied per item: **a change the operator made on purpose is committed; a snapshot git made on
its own is dropped after the backup preserves it.** Both branches of the rule leave nothing
unrecoverable because the backup (3.2) is taken and verified before any of this.

| Item                                   | Rule outcome | Action                                                        |
| -------------------------------------- | ------------ | ------------------------------------------------------------- |
| `lua/config/autocmds.lua` (+`aerial`)  | commit       | `fix(autocmds): close the aerial sidebar with the others on quit` |
| `lua/plugins/herdr-nvim.lua` + the `lazy-lock.json` `herdr-nvim` line | commit, one unit | `feat(herdr-nvim): annotate lines back to herdr agents` |
| `CLAUDE.md`                            | commit       | `docs: add the CLAUDE.md conventions file`                    |
| `stash@{0}` (lock only, stale, base not an ancestor) | drop  | `git stash drop stash@{0}` after the backup; its 14 pins are all superseded by or older than the live lock |
| 7 unpushed commits                     | push         | `git push origin main` so the archive holds everything        |

After this the live repo is clean, at 10 commits ahead of where `origin/main` was, and pushed. The
archive's README commit (3.6) is the eleventh and is pushed too.

### 3.2 Backups, taken and verified

Both repositories, before any drain step, in `~/workspaces/backups/` under the repo's naming
convention (timestamp first, `.backup`, extension last):

- `~/workspaces/backups/<YYYY-MM-DDTHH-MM-SS>.neovim-config.backup/`: a full `cp -R` of
  `~/.config/nvim` including `.git`, the stash, and the untracked files. This is the copy the
  zero-behavior-change diff in 3.7 reads.
- `~/workspaces/backups/<YYYY-MM-DDTHH-MM-SS>.dotfiles.backup.bundle`: `git bundle create <path> --all`
  from the dotfiles checkout (every ref including `origin/nvim-overhaul`), plus a
  `<ts>.dotfiles-worktree.backup/` copy only if `git status` is not clean at that moment. A bundle,
  not a copy: the checkout carries three Rust `target/` trees.

Each backup is verified before the first drain step, and the verification output goes in the import
PR body:

```bash
diff -r ~/.config/nvim "$B"                       # empty: the copy is complete, .git included
git -C "$B" stash list | grep -c .                # 1: the stash travelled
git -C "$B" status --porcelain | wc -l            # 4: the two modified and two untracked files
git bundle verify "$BUNDLE"                       # "is okay", and lists every ref
```

### 3.3 Flatten into `dot_config/nvim/`

The standalone repo's working tree is copied file for file into `dot_config/nvim/` in the dotfiles
source with `rsync -a --exclude=.git --exclude=.claude --exclude=.DS_Store --exclude=.github
--exclude=.gitignore` and NOT with `cp -R`, so the nested repository's `.git` never enters the dotfiles
source and the two files the table below drops are never copied. The copy is followed by an assertion
that `dot_config/nvim/.git` does not exist and that `git -C <dotfiles> status --porcelain` lists no
path containing `/.git/`. The source-name translations (verified today with a scratch chezmoi source:
a literal dot-prefixed source entry is silently ignored by chezmoi, so every deployed dotfile needs the
`dot_` prefix):

| Standalone path              | Source path                                | Reaches `$HOME`? | Why                                                  |
| ---------------------------- | ------------------------------------------ | ---------------- | ---------------------------------------------------- |
| `init.lua`, `lua/**`, `lazy-lock.json`, `CLAUDE.md`, `stylua.toml` | same, unchanged | yes | the config, its lock, and the formatter config `stylua` reads when a config file is saved |
| `.luacheckrc`                | `dot_luacheckrc`                           | yes              | luacheck reads it from the file's root when run in place |
| `.prettierignore`            | `dot_prettierignore`                       | yes              | keeps `prettierd` off `lazy-lock.json` on save        |
| `lua/overseer/template/user/run_script.lua` | `literal_run_script.lua`     | yes              | chezmoi reads a bare `run_` prefix as an executable script to run rather than a file to copy (chezmoi's own source-state-attributes reference); `literal_` is the documented escape and stops attribute parsing for that path component; the deployed name is unchanged |
| `README.md`, `docs/`         | same                                       | NO (ignored)     | repo metadata                                         |
| `.github/workflows/lint.yml` | dropped, not copied                        | no               | CI for the archived repo; the dotfiles workflow lints instead (3.8) |
| `.gitignore`                 | dropped, not copied                        | no               | its three rules (`.DS_Store`, `private/`, `*.sw*`); the first and third are already global rules in the dotfiles `.gitignore`, so only `dot_config/nvim/private/` is a new line there |
| `lazyvim.json`               | copied unchanged in the import, deleted in PR 4b | yes, until PR 4b | decision gamma; the import carries zero change     |
| `.claude/settings.local.json` | never added                               | no               | 3.5                                                   |
| `tests/` (new, section 6)    | `tests/`                                   | NO (ignored)     | test code, run from the source tree (6.3)             |

v2's theta wanted `stylua.toml`, `.luacheckrc` and `.prettierignore` ignored. That is reversed with a
reason: the editor formats its own files on save, and `stylua` and `prettierd` resolve their config by
walking up from the file being formatted. An undeployed `stylua.toml` means the formatter's defaults
(4-space indent) rewrite every config file the operator saves.

After the copy, sweep the WHOLE flattened tree for every other chezmoi keyword prefix (`run_ exact_
modify_ symlink_ private_ executable_ create_ remove_ encrypted_ external_ empty_ readonly_ once_
onchange_ before_ after_`) and the `.tmpl` suffix, at every path component, not just the top level:
`run_script.lua` above is the only hit and no `.tmpl` file exists anywhere in the tree (verified
2026-09-02). Verify the rename against a SCRATCH chezmoi deployment, never the live tree: an unmanaged
file already sitting at the live target path could hide a broken rename inside what looks like an empty
byte diff.

### 3.4 Path-anchored chezmoiignores

Bare patterns in `.chezmoiignore` are target-root anchored (verified in the reassessment; the existing
bare `docs/` and `README.md` lines do not reach `.config/nvim/`). The import adds:

```
# Neovim config: repo metadata and test code never reach $HOME.
.config/nvim/README.md
.config/nvim/docs/
.config/nvim/tests/
```

No `.github/` or `.gitignore` entries are needed because those files are not copied (3.3). The
OS-conditional Linux block does not drop `.config/nvim`: the editor deploys on both operating systems.

### 3.5 What is tracked and what is carved out

- `lazy-lock.json` is tracked (beta). `checker.enabled` flips to `false` in PR 4a, not in the import.
  The operational rule after any `:Lazy update` or `:Lazy restore`: run
  `chezmoi re-add ~/.config/nvim/lazy-lock.json`, review the diff, commit. Whether lazy's background
  checker itself writes the lock is not verified; what is verified is that `:Lazy sync` and
  `:Lazy update` do, and the checker's notifications are what invite those.
- `CLAUDE.md` is tracked; a nested `CLAUDE.md` is not caught by the root-anchored ignore (verified).
- `.claude/` (eta): today it holds only `settings.local.json`, which the operator's global git ignore
  (`~/.config/git/ignore:15`, pattern `**/.claude/settings.local.json`) already keeps out of every
  repo, so it was never tracked in the standalone repo either. The carve-out is therefore: the file is
  never added to the source, so chezmoi never manages it and the live copy stays where it is. A future
  `dot_config/nvim/dot_claude/settings.json` (shared, non-local) can be tracked without any change to
  this rule.
- `lazyvim.json` is deleted in PR 4b together with the `lazyvim_` augroup prefix in `autocmds.lua`
  (renamed to `nvim_config_`; the `lazyvim_last_loc` buffer variable goes with it). This closes gamma
  and the "not a LazyVim distribution" framing.

### 3.6 Archive the standalone repository, in order, with guards

The order matters because the last step is destructive and the steps before it are what make it safe:

1. The three drain commits (3.1) and the stash drop.
1. One more commit that prepends a "moved to `webdavis/dotfiles` under `dot_config/nvim/`" line to
   `README.md`.
1. `git push origin main`, then `git fetch origin` and `git rev-list origin/main..HEAD | wc -l` must
   print 0.
1. Rename `webdavis/neovim-config` to `webdavis/neovim-config-archive` and archive it through
   `gh-axi`; confirm with `gh-axi` that the repository reports `archived: true`.
1. The dotfiles import PR (section 11, PR 2) merges and the operator runs a full `chezmoi apply`,
   which writes byte-identical files over the same tree (3.7).
1. Only then the operator removes `~/.config/nvim/.git` with `trash` (destructive gate: the operator
   runs it), and only when all three guards print nothing:

   ```bash
   git -C ~/.config/nvim status --porcelain
   git -C ~/.config/nvim rev-list origin/main..HEAD
   git -C ~/.config/nvim stash list
   ```

The deployed editor never changes through any of this; `README.md` and `docs/` stay on disk as
unmanaged files.

### 3.7 The zero-behavior-change proof for the import pull request

The import pull request must show, in its body, that the deployed editor is unchanged. Five checks,
all scripted so the reviewer reruns them. `B` is the backup directory (3.2), `BENCH` is the fixed
benchmark directory of 9.1, and `S` is one scratch directory with two phase subdirectories:
`$S/before`, written after the drain and the README commit and immediately before the flatten, and
`$S/after`, written after the apply. No path is written twice, so the before evidence survives the
after run, and every `nvim` run starts from `cd "$BENCH"` so overseer's "no buildfile" lines and every
other cwd-relative line are the same in both phases. The startup runs fire `User VeryLazy` by hand
before quitting, for the reason 9.1 gives (the number is synthetic, and the same synthetic number in
both phases):

```bash
P="$S/before"; mkdir -p "$P"; cd "$BENCH"      # second phase: P="$S/after"
NVIM_CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}/nvim"
export DOTFILES
for i in 1 2 3 4 5; do
  : >"$P/st-$i.log"    # --startuptime APPENDS to an existing file, `nvim --help`
  nvim --headless --startuptime "$P/st-$i.log" -c 'doautocmd User VeryLazy' +qa 2>"$P/err-$i.log"
done
nvim --headless "+checkhealth" "+w! $P/health.txt" +qa
RUN="${XDG_RUNTIME_DIR:-${TMPDIR}nvim.${USER}}"
sed -E "s#${RUN}/[^/ ]+#<run>#g; s#${TMPDIR}#<tmpdir>/#g; s#${HOME}/.local/state/nvim/#<state>/#g; \
  s#${NVIM_CONFIG}#<config>#g; s/Log size: [0-9]+ KB/Log size: <n> KB/g; \
  s/[0-9]{4}-[0-9]{2}-[0-9]{2}[^ ]*//g" "$P/health.txt" \
  | awk '
      /^markview:/ { print; print "<markview: symbol/parser samples omitted, nondeterministic between runs>"; skip=1; next }
      skip && /^==============================================================================$/ { skip=0 }
      skip { next }
      { print }
    ' >"$P/health.norm"
nvim --headless -u "$NVIM_CONFIG/init.lua" -l "$DOTFILES/dot_config/nvim/tests/dump_state.lua" "$P/state.json"
```

`-u` is load-bearing on the last line: `-l` alone skips ALL source-state initialization
(`:help startup`, item 9, "`-es`/`-Es`/`-l`... skipped"), verified 2026-09-02 by probing `_G.arg`,
`loadplugins` and a global the config sets after startup, each absent under a bare `-l` and present once
`-u <config>/init.lua` is added. Without it `dump_state.lua` bootstraps no plugins at all and silently
writes an empty-of-real-content dump on both sides of every diff, which compares equal for the worst
possible reason. `DOTFILES` must be exported (not merely set) so the child nvim process invoked by `-l`
can read it via `os.getenv`; an unexported shell variable never reaches a forked process's environment.
`--startuptime` truncation and the `NVIM_CONFIG`/log-size normalization are further fixes recorded next
to the phase block above; `markview`'s health section is blanked because its own `:checkhealth` output
demo-samples random glyphs and reorders its parser list on every run regardless of config content
(verified by diffing two runs of the identical, unchanged config), which can never compare equal and
carries no pass/fail signal nvim-treesitter's own section does not already carry.

The normalization replaces only the roots that differ between two runs on the same machine: the
per-instance runtime directory (`stdpath("run")`, a random name under `$RUN`), `$TMPDIR` itself, the
state directory (`~/.local/state/nvim/`, anchored on a trailing slash so it does not also match the
PREFIX of a sibling like `~/.local/state/nvim-overhaul`, BENCH's own parent), the nvim config root
(`$NVIM_CONFIG`, needed because a pre-merge preview compares the live root against a scratch one at a
different absolute path, which the after-phase does not hit since both its sides share one root), log
size (a monotonically growing counter, not a signal), and timestamps. Every other path stays, so a
health line that starts naming a different binary, plugin directory or URL compares unequal; the old
`s#/[^ ]+##g` erased those too.

Even with every normalization above, `checkhealth` carries RESIDUAL, config-independent
non-determinism from third-party plugins this program does not own: a same-config control run (the
identical, unchanged live config, `:checkhealth` twice in a row) was measured to differ on a system-info
section reordering two lines, an overseer health line reporting a real elapsed-ms timing, and a
filetype-mismatch warning listing its set in hash-iteration order. The health check (check 4 below) is
therefore ADVISORY, reviewed by eye against this known noise profile, never a hard gate; gating on it
would fail every future PR, including a perfect one.

`sort.nvim` and `nvim-treesitter-textobjects` both map `[s`, `]s` and `as`, and which of the two owns
them is settled by plugin load order at startup rather than by anything this program controls. Two of
twelve dumps of the IDENTICAL unchanged config named the other winner (measured 2026-09-02; an explicit
one-second event-loop drain before the capture did not change the rate, so the race is already settled
by the time the dump can observe it). Those eleven rows are printed rather than gated, the same way the
health diff is, and every other row in `state.json` stays a hard gate. Gating them would fail roughly
one run in six on a correct change, and the documented answer to a red gate is to look again, which is
exactly how a real regression gets re-rolled away. PR 17-series owns the conflict itself and checks
those three maps by hand.

The comparison, run once after the second phase under `set -euo pipefail` (a comparison that can pass on
missing or malformed input is not a proof): the byte, lock, state and stderr-log checks are hard gates,
the health diff is advisory (printed, not gated), and the last line prints the two warm medians without
gating on them (9.1's baseline precondition, no agent running, cannot hold while this very program runs
agents continuously, and section 2 already records agent load moving the result by more than twice the
gate's 10ms tolerance):

```bash
set -euo pipefail
diff -r --exclude=.git --exclude=.claude --exclude=.DS_Store --exclude=README.md "$B" ~/.config/nvim
diff "$B/lazy-lock.json" ~/.config/nvim/lazy-lock.json
diff "$S/before/health.norm" "$S/after/health.norm" || true   # advisory, see above
RACING='"lhs":"\[s"|"lhs":"\]s"|"lhs":"as"'                   # advisory rows, see above
diff <(grep -Ev "$RACING" "$S/before/state.json") <(grep -Ev "$RACING" "$S/after/state.json")
diff <(grep -E "$RACING" "$S/before/state.json") <(grep -E "$RACING" "$S/after/state.json") || true
err_report="$(wc -c "$S"/*/err-*.log | awk '$1 != 0 && $2 != "total"')"
[[ -z "$err_report" ]] || { echo "$err_report"; echo "FATAL: a startup run wrote to stderr" >&2; exit 1; }
median() {
  local n
  n="$(grep -h "NVIM STARTED" "$1"/st-{2,3,4,5}.log | wc -l | tr -d ' ')" || true
  [[ "$n" == "4" ]] || { echo "FATAL: expected exactly 4 warm samples in $1, found $n" >&2; exit 1; }
  grep -h "NVIM STARTED" "$1"/st-{2,3,4,5}.log | sort -n | awk '{a[NR]=$1} END {print (a[2]+a[3])/2}'
}
before_median="$(median "$S/before")"; after_median="$(median "$S/after")"
printf 'before %s after %s (advisory)\n' "$before_median" "$after_median"
```

The median function used to return 0 from zero samples and half a real value from two, and neither a
missing log nor a nonempty diff forced the whole comparison to fail; this version requires exactly four
warm samples per phase (a possibility opened by a real bug: `--startuptime` appends rather than
overwrites, so a re-run against the same `P` without truncating first silently doubles the sample count
and corrupts the median without erroring) and treats a nonempty stderr log as fatal.

1. **Bytes.** The first diff. `README.md` is excluded because the README commit (3.6 step 2) is the
   ONE working-tree change between the backup and the flatten: the three drain commits and the stash
   drop change no tracked content, and `rsync` copies the tree as it stands after that commit. The
   exclusion is proven to hide exactly that file by running the same diff WITHOUT `--exclude=README.md`
   before the flatten and showing that its only output names `README.md`. Chezmoi does not delete the
   four files it does not manage (`README.md`, `docs/`, `.github/`, `.gitignore`), and `CLAUDE.md` is
   byte-identical because the import PR excludes `dot_config/nvim/**` from mdformat AND taplo (3.8),
   the second because `stylua.toml` is itself a TOML file taplo would otherwise reformat. The stylua
   formatter that PR 1 lands is run as `stylua --check` against the backup copy with Homebrew's
   stylua BEFORE the flatten; the live config is clean under Mason's 2.3.1 today, and if Homebrew's
   2.5.2 wants a rewrite, the import PR adds the same `dot_config/nvim/**` exclusion for stylua and
   PR 4d lifts every exclusion with the reformat as its own commit.
2. **Lock.** The second diff.
3. **Startup.** The five runs per phase are the 9.1 method (synthetic, with `User VeryLazy` fired by
   hand). The stderr check above is fatal on any nonempty log. The `median` function is the 9.1 median
   (runs 2 to 5), computed for both phases and printed but NOT gated here (see the advisory-timing
   paragraph above); a future PR's own gate, run when the machine is quiescent, is where `after <= before
   + 10` is actually asserted.
4. **Health.** The third diff, over the normalized files (the volatile roots, config root, log size and
   dates replaced, the rest intact), ADVISORY per above: printed for review, never gated.
5. **Keymaps and plugins.** The fourth diff, a HARD gate on every row but the three above.
   `dump_state.lua` is the first file in `tests/`
   (section 6) and every later PR shows its diff. Its preconditions and its projection are fixed here,
   so a regression cannot slip through a timing gap and a run cannot fail on encoding:
   - It runs WITHOUT `--clean`, invoked as `nvim --headless -u <config>/init.lua -l tests/dump_state.lua
     <out.json>` from `cd "$BENCH"`, and writes to `argv[1]`. See the `-u` note above the phase block.
   - It fires `doautocmd User VeryLazy` FIRST, inside its own process, then asserts a known
     VeryLazy-triggered plugin (`which-key.nvim`, via `require("lazy.core.config").plugins["which-key.nvim"]._.loaded`)
     shows loaded, erroring otherwise. The dump runs in a separate process from the phase block's own
     startup runs, so without firing the event itself it silently misses every VeryLazy-triggered plugin
     (which-key, noice, textobjects, unimpaired, claudecode after PR 30c9) on BOTH sides of a diff, which
     is a comparison that passes for the wrong reason.
   - Global pass: `nvim_get_keymap(mode)` for each of `n`, `v`, `x`, `s`, `o`, `i`, `c`, `t`.
   - Buffer-local pass: `:edit $DOTFILES/justfile` (a tracked file in a git repository; `DOTFILES` must
     be exported by the caller, see above), then
     `vim.wait(5000, function() return vim.fn.maparg("]g", "n", false, true).buffer == 1 end)`: `]g`
     is the first map gitsigns `on_attach` sets (`git.lua:84-87` today) and `maparg()`'s dict reports
     `buffer = 1` for a buffer-local map (`:help maparg()`). The dump exits 1 if the wait times out;
     `vim.b.gitsigns_head` is NOT the signal, because gitsigns sets it before `on_attach` runs. Then
     `nvim_buf_get_keymap(0, mode)` for the same modes. This proves only the gitsigns generic-buffer
     surface, not filetype-local maps (markdown, octo, etc.); the claim is scoped to that surface.
   - Octo's dynamic `<localleader>` groups (8.1) are EXCLUDED. They are which-key metadata registered
     with `wk.add(..., { buffer = 0 })` on the `octo` FileType, not buffer keymaps, so no keymap query
     sees them; PR 24, the one PR that changes octo, checks them by hand with `\` in an octo buffer
     and records the popup.
   - Which-key pass: `dofile("<config>/lua/plugins/which-key.lua")` returns the spec table; its
     `opts.spec` is a list of blocks that each nest their actual group rows ONE LEVEL DEEPER as their
     own array part (`{ mode = {...}, { "<C-g>", group = "git-1" }, ... }`); a naive top-level read of
     `opts.spec` finds no `.group` field on either side and compares equal on zero groups, which is a
     comparison that passes for the wrong reason. The dump walks every nesting depth and refuses to
     write a capture with zero groups.
   - Plugin state pass: for every `require("lazy").plugins()` entry, emit `name`, `lazy` (its `.lazy`
     field) and `loaded` (whether `._.loaded` is non-nil), not a bare sorted name list; a plugin's name
     does not change when it flips between eager and lazy, so a name-only list compares equal across
     exactly the regression this pass exists to catch.
   - Projection: each keymap row keeps ONLY `mode`, `lhs`, `buffer`, `desc`, `noremap`, `silent`,
     `expr`, `nowait` and `rhs`. This is a keymap-metadata dump, not a full behavior proof: a Lua
     `callback` is fingerprinted as `<callback:source:line>` via `debug.getinfo` (a function's own
     address differs every run and cannot be JSON-encoded; two callbacks share a fingerprint only if
     defined on the same line of the same file, which does not happen in practice, and a callback's
     runtime behavior is never exercised), with the config-root prefix of `source` normalized to
     `<config>` (needed for the same pre-merge-preview reason as the health normalization above). A
     classic Vimscript `<SNR>NNN_name` reference (a compat shim some plugins use, e.g. unimpaired's
     `<Plug>` mappings) has its number stripped to `<SNR>_name`: the number is a per-process
     script-sourcing-order counter, confirmed to vary between separate nvim invocations of the
     identical, unchanged config (`<SNR>126_` one run, `<SNR>124_` the next), not a property of the map
     itself. Rows are written with an EXPLICIT, hand-ordered key sequence rather than
     `vim.json.encode` on a whole Lua table: a table's hash-part iteration order is not guaranteed
     stable across separate process runs (measured: the identical plugin row encoded with two different
     key orders on two runs of the identical config), which would make every line compare unequal for a
     reason that has nothing to do with the config. Rows are sorted by `mode` then `lhs` then `buffer`
     and written one JSON object per line, so a diff names the changed map.

### 3.8 Lint in the dotfiles repo

The standalone CI ran `stylua --check` and `luacheck`. PR 1, before the import, gives treefmt a
`stylua` formatter (rewrites in place, like shfmt) and a `luacheck` check-only formatter, both with
`includes = ["dot_config/nvim/**/*.lua"]` (a glob that matches nothing until PR 2 lands, so PR 1 is
inert on the tree it ships in), both Homebrew formulae (`stylua` 2.5.2, `luacheck` 1.2.0, verified
with `brew info`) added in three places by hand: `.chezmoidata/system_packages_autoinstall.yaml` under
`packages.macos.homebrew.formulae` in alphabetical order (the weekly bundle's guarded
`brew bundle cleanup` removes any formula that file does not declare, so without this line the lint
prerequisites disappear from dresden on the next Monday), `Brewfile.dev`, and the toolchain step of
`.github/workflows/lint.yml` (those two are hand-synced, per CLAUDE.md). luacheck finds `.luacheckrc`
by that exact name and the
source file is `dot_luacheckrc`, so the formatter runs
`luacheck --config dot_config/nvim/dot_luacheckrc` (the `--config <path>` option is documented in
luacheck's CLI reference).

`mdformat` excludes nothing under `dot_config/` today, so it would rewrap the deployed `CLAUDE.md` and
break check 1 of 3.7. PR 2 therefore adds `dot_config/nvim/**` to the mdformat `excludes` list, and
PR 4d removes that line and commits the rewrap of `CLAUDE.md` and `docs/todo.md` as one
formatting-only commit, called out in its body.

`taplo` excludes nothing under `dot_config/` either, and `dot_config/nvim/stylua.toml` is a TOML file
it would format, breaking check 1 of 3.7 the same way. PR 2 adds `dot_config/nvim/**` to the taplo
`excludes` list alongside `dot_aerospace.toml`, and PR 4d lifts it with the other exclusions.

### 3.9 Bootstrap script

`.chezmoiscripts/run_onchange_after_80-bootstrap-nvim.sh.tmpl` (80 is free; 72 is the highest today).
It lands in PR 31, after the Mason race is removed (PR 5b) and the lock is final.

- **Trigger.** The rendered script embeds the sha256 of `lazy-lock.json` and of `lua/plugins/lsp.lua`
  (the two tool lists live there), the same `include | sha256sum` idiom `run_onchange_after_58` uses,
  so a pin or tool-list change re-fires it. A retry marker's modification time is embedded as well,
  exactly as the pns build script does, so a deferred run (no `nvim` on PATH yet) re-fires on the next
  apply instead of consuming the trigger.
- **Guard.** `command -v nvim` or defer with the marker. No darwin guard: the config deploys on both
  operating systems, and the only macOS-only pieces (the Swift stack) are gated inside Lua with
  `vim.fn.has("mac")`. This replaces item 50's "darwin guard" by decision.
- **No timeout.** A cold machine clones 80-odd repositories and builds `tree-sitter-cli` with cargo;
  the v1 `timeout 120` was the reason a half-installed editor read as "done". A network failure makes
  the commands below exit non-zero, chezmoi does not record the run, and the next apply retries.
- **Steps.** `nvim --headless "+Lazy! restore" +qa`, then `nvim --headless "+MasonToolsInstallSync" +qa`
  (valid only once `run_on_start = false`, PR 5b, so the sync run and the autostart run cannot race),
  then the test runner from the SOURCE tree against the deployed config (6.3):
  `nvim --headless --clean -l {{ .chezmoi.sourceDir }}/dot_config/nvim/tests/run.lua --config
  "$HOME/.config/nvim"`. `tests/` is chezmoiignored and never exists under `$HOME`; the runner is
  addressed through the source directory chezmoi already knows.
- **Verification, then non-zero exit.** Every pin in `lazy-lock.json` has a directory under
  `~/.local/share/nvim/lazy/`; every name in the two `ensure_installed` lists has a Mason package
  directory; the Lua tests pass. The missing names are printed with their tool, and the script exits 1.
  A quiet apply prints nothing (operator ruling 2026-08-05).
- **Prerequisites documented in the script header:** network access, `git`, `cargo` (from
  `run_once_before_20-install-rustup`, needed by `tree-sitter-cli`), `go` (needed by Mason's `gopls`,
  section 5.3), and Xcode for the Swift stack.

## 4. The bug list as a floor

Every open bug from the inventory, by number, with its fix and the pull request that lands it. Items
7 and 13 stay struck. Line numbers are today's (`d45b190`).

| #   | Sev      | Bug                                                                   | Fix                                                                                          | PR    |
| --- | -------- | --------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- | ----- |
| 1   | high     | `github.username()` is nil; `<C-g>i` errors (`git.lua:267`)          | read `github.account().username`; the redesigned function returns `(account, err)`          | PR 7d |
| 2   | low      | `<C-g>bc` mapped twice (`git.lua:464-475` dead, `:538-543` wins)      | delete the dead definition                                                                   | PR 18 |
| 3   | medium   | `toggle_runner("OverseerWatchRun")` is an invalid action (`overseer.lua:412-414`) | bind `<M-[>` to `vim.cmd("OverseerWatchRun")`                                     | PR 19a |
| 4   | high     | `default_branch` reads `opts.repo` from a string (`custom_api/git.lua:231`; caller `git.lua:997`) | the GitHub fallback moves into `github` (item 56); `git.default_branch()` takes no repo and returns `(name, err)` | PR 7c |
| 4b  | high     | `string.format` with two `%s` and one arg (`custom_api/git.lua:247`)  | same move; `github.default_branch({owner, name})` supplies both                              | PR 7c |
| 5   | medium   | `delegate.setup()` called twice                                       | moot: `delegate.lua` is deleted                                                              | PR 8  |
| 6   | low      | duplicate, ungrouped `checktime` autocmd (`options.lua:116-118`)      | remove the `options.lua` copy; the auto-reload design (5.4) owns `checktime`                 | PR 20 |
| 8   | medium   | `extract_upstream` returns `i + 1`, dropping the first commit word (`custom_api/git.lua:101-112`) | return `i`; pinned by a unit test                                                | PR 7a |
| 9   | medium   | literal `"\\<Esc>"` (`delegate.lua:100`)                              | moot: `delegate.lua` is deleted                                                              | PR 8  |
| 10  | low      | `nvim_win_get_width(0)` at spec load (`harpoon.lua:6`)                | `opts = function() … end`                                                                    | PR 22a |
| 11  | high     | hardcoded `mkdp_open_ip = "dresden.home.webdavis.io"` (`markdown.lua:314`) | `vim.fn.hostname()` plus the MagicDNS suffix read from `vim.env.NVIM_MKDP_HOST` when set; falls back to `127.0.0.1` off dresden | PR 21 |
| 12  | critical | mason-lspconfig v2 never reads the `servers` block (`lsp.lua:50-148`) | move every per-server table to `vim.lsp.config("<name>", {…})`; mason-lspconfig keeps `ensure_installed` and `automatic_enable`; assert clangd's `cmd` carries `--clang-tidy` and `--header-insertion=iwyu` | PR 5a |
| 14  | medium   | Overseer: `run_template` alias, dead `bundles` and `log` config       | rename to `run_task`; delete the dead tables (`overseer.lua:192-200`, the `log` block)        | PR 19b |
| 15  | low      | hlslens pin `4254054` one commit behind the `vim.validate` fix        | bump to `be2d7b2`; closes the one `vim.deprecated` warning                                   | PR 26b |
| 16  | low      | catppuccin colorscheme rename past `605b460`                          | bump the pin and change `vim.cmd.colorscheme("catppuccin")` at `ui.lua:60` to `"catppuccin-nvim"` in the same commit; `name = "catppuccin"` at `ui.lua:7` stays | PR 26c |
| 17  | low      | noice `inc_rename = true` for an uninstalled plugin (`noice.lua:36`)  | set `false`                                                                                  | PR 22b |
| 19  | arch     | `helpers.wrap` name reflection always "anonymous"; soft-error loop mishandles the 4-tuple | delete `helpers.wrap`; section 6                                                     | PR 6  |
| 20  | low      | error text names the wrong parameter (`latest_commit` "project", `url` "user") | align with the field names during the `(value, err)` conversion                         | PR 6  |
| 21  | code     | TODO at `git.lua:1132`: git-blame only on attach                      | moot: git-blame drops; the rebuilt keymaps live in gitsigns `on_attach`                      | PR 23 |
| 22  | code     | HACK at `noice.lua:64`                                                | informational, no action                                                                     | none  |
| 68  | verify   | runtimepath bug never re-verified                                     | the `nvim-treesitter` health line "is not in runtimepath" is the candidate; investigate and fix or record as a health-check artifact | PR 29b |

Two more config defects found by today's `checkhealth` join the floor: none-ls declares `nixfmt`,
`rubocop` and `eslint` sources whose binaries are absent (three ERROR lines). Fix: wrap each in
`.with({ condition = function(utils) return utils.executable("<bin>") end })` in PR 29a, so the health
run is clean on a machine without them and the sources still work where they exist.

## 5. Plugin plan

### 5.1 Drops

Removing a spec block alone is a no-op or an error when the plugin is a `dependencies` edge (lazy
force-installs anything still named there). Each drop is one atomic commit with every edit it needs.

| Plugin              | Edits (one commit)                                                                                     | Replacement                       | PR    |
| ------------------- | ------------------------------------------------------------------------------------------------------ | --------------------------------- | ----- |
| `cspell.nvim`       | remove the dep at `lsp.lua:247`                                                                        | none (author-deprecated)          | PR 17a |
| `gitmoji.nvim`      | dep `blink-cmp.lua:114`, provider `:261-263`, `sources.default` `:279`                                 | none                              | PR 17b |
| `nvim-notify`       | remove the dep at `noice.lua:11`; noice falls back to `snacks.notifier` (already enabled)              | `snacks.notifier`                 | PR 17c |
| `gv.vim`            | remove from fugitive deps `git.lua:255`                                                                | `Snacks.picker.git_log` (`<leader>gl` exists) | PR 17d |
| `git-messenger.vim` | remove the spec and its `<leader>gm` keymap                                                            | `gitsigns.blame_line({full=true})` (`<C-g>` blame already at `git.lua:199`); `<leader>gm` is re-pointed at it | PR 17e |
| `git-blame.nvim`    | AFTER the remap (5.2): remove the spec `git.lua:1131-1140`; add `current_line_blame = true` to gitsigns opts (`git.lua:60`) | gitsigns + `custom_api`   | PR 23 |
| `telescope.nvim`    | remove the standalone block `git.lua:1142-1155` and the octo dep `:1161`; octo already has `picker = "snacks"` (`:1170`); the `telescope = {…}` key in `chezmoi.lua` and `TelescopePrompt` in `autosave.lua` are inert and are removed in the same commit | snacks (decision B) | PR 24 |
| `boole.nvim`        | write the dial spec first: `augend.constant.new` entries for every `additions` pair and `allow_caps_additions` pair in `boole.lua`, bound to `<C-a>`/`<C-x>` in normal and visual mode; then delete `boole.lua` | `dial.nvim` (already installed as a bare spec) | PR 25 |

### 5.2 The git-blame remap (decision C)

`git-blame.nvim` carried four keymaps. `<C-g>Bt` (toggle) maps onto `gitsigns.toggle_current_line_blame`.
The other three have no gitsigns equivalent and are rebuilt on `custom_api`, before the drop, keeping
their `lhs` and their which-key group `<C-g>B` "blame":

| Keymap    | Behavior                                  | Implementation                                                                          |
| --------- | ----------------------------------------- | --------------------------------------------------------------------------------------- |
| `<C-g>By` | copy the current line's commit SHA        | `git.blame_sha({ file, line })`: runs `git blame -L <n>,<n> --porcelain -- <file>` through the injected runner; the first token of the first line is the SHA; a `0000000…` SHA is the "not committed yet" soft error |
| `<C-g>Bo` | open the commit on GitHub                 | `github.commit_url(sha)` builds `https://github.com/<owner>/<name>/commit/<sha>` from `github.repo()`; `vim.ui.open` |
| `<C-g>BO` | copy the commit URL                       | same URL through `util.copy_to_system_clipboard`                                        |

The porcelain parse is a pure helper (`parse_blame_porcelain`) with a unit test; the three keymaps
move into gitsigns `on_attach` so they exist only in git buffers, which closes the TODO at
`git.lua:1132`. Because they become buffer-local, the dump's global pass reads them as removed and its
buffer-local pass (3.7, check 5b) reads them as present; PR 23's body shows both halves.

### 5.3 Adds

| Plugin or tool                  | Spec and pin                                                                                        | Notes                                                                                           | PR    |
| ------------------------------- | --------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- | ----- |
| `sourcekit-lsp`                 | `vim.lsp.config("sourcekit", { cmd = { "sourcekit-lsp" } })` and `vim.lsp.enable("sourcekit")`, darwin only | not a Mason package (verified: the registry has no `sourcekit-lsp`); the Xcode toolchain binary is used. Root markers per nvim-lspconfig: `buildServer.json`, `.bsp`, `*.xcodeproj`, `*.xcworkspace`, `compile_commands.json`, `Package.swift`, `.git` | PR 3 |
| `xcode-build-server`, `xcbeautify`, `swiftformat`, `swiftlint` | already in `.chezmoidata/system_packages_autoinstall.yaml` (lines 169, 170, 187, 188) and installed | no YAML edit and no Mason entry: none-ls already declares `formatting.swiftformat` and `formatting.swiftlint` (`lsp.lua:287-288`) against the PATH binaries; `xcode-build-server` generates `buildServer.json` for an `.xcodeproj` or `.xcworkspace` (not needed for a SwiftPM package); `xcodebuild.nvim` requires `xcbeautify` | PR 3 (nothing to add) |
| `wojciech-kulik/xcodebuild.nvim` | `commit = "633eb71"` (main HEAD today, `git ls-remote`); deps `MunifTanjim/nui.nvim` (present via noice), `folke/snacks.nvim` (picker), `stevearc/oil.nvim` (present); `ft = "swift"` plus `cmd`; `cond = vim.fn.has("mac") == 1` | supports Swift Packages (build and test) as well as Xcode projects, and has its own Test Explorer; no nvim-dap | PR 3 |
| `gopls` and `go`                | `"gopls"` in mason-lspconfig `ensure_installed`; `go` Homebrew formula in the YAML                   | Mason installs `gopls` with `go install`, and the health check shows Go absent today, so the formula is part of decision D | PR 27 |
| `nvim-neotest/neotest` + `nvim-neotest/nvim-nio` | `commit = "27bf921"` (neotest main HEAD today); `keys` under `<leader>t`; adapters per the open question in section 12 |                                                                          | PR 28 |
| `okuuva/auto-save.nvim`         | already installed (`version = "^1.0.0"`, disabled by default with the `<leader>uv` toggle)         | item 42: the save condition gains the two `claudecode.nvim` exclusions its README documents (buffer name matching `(proposed)` or `(NEW FILE - proposed)`, buftype `acwrite`); formatting on autosave is suppressed: the `AutoSaveWritePre` user event sets `vim.b.autosave_write`, lsp-format's `BufWritePre` handler returns early when it is set, `AutoSaveWritePost` clears it. Explicit `:w` keeps formatting | PR 12 |
| `coder/claudecode.nvim`         | `commit = "2390c6e"` (main HEAD today; the last tag v0.3.0 is from 2025-09); `opts = { terminal = { provider = "none" } }`; `dependencies = { "folke/snacks.nvim" }` (required per its README) | section 7.2                                                                | PR 12 |
| Neovim MCP server               | per section 7.3                                                                                     |                                                                                                 | PR 9, PR 10a, PR 10b |
| buffer auto-reload              | section 5.4, no plugin                                                                              |                                                                                                 | PR 20 |
| `jpalardy/vim-slime`            | `commit = "305b4d8"` (main HEAD today), `g:slime_no_mappings = 1`, `g:slime_target = "herdr"`       | section 7.4 supplies the herdr target                                                           | PR 11 |
| the three custom plugins        | section 7.7                                                                                         |                                                                                                 | PR 14, 15, 16 |

The pins above are the `git ls-remote <repo> HEAD` answers of 2026-09-01; the implementer re-reads
them at PR time and records the SHA it pins in the PR body.

### 5.4 Buffer auto-reload for agent writes (item 40)

Today `autoread` is on and `checktime` fires on `FocusGained`, `BufEnter`, `CursorHold` (options.lua)
and again on `FocusGained`, `TermClose`, `TermLeave` (autocmds.lua): the duplicate is bug #6. Neither
fires while Neovim sits idle in one herdr pane and an agent writes the file from another, because
`CursorHold` fires once per cursor position and focus never changes.

Design: one `nvim_config_auto_reload` augroup in `autocmds.lua`. On `BufReadPost` and `BufWritePost`
for a normal file buffer, start a `vim.uv.new_fs_event()` on the file's real path (`vim.uv.fs_realpath`)
if the buffer has none; the callback schedules `checktime <bufnr>`. `BufDelete`/`BufUnload` stop and
close the handle. The `options.lua` autocmd is deleted; the grouped one in `autocmds.lua` stays as the
focus-change path. Roughly 30 lines, no plugin, and it is the same mechanism the research doc cites.

One macOS detail: the watch is a kqueue on the file's inode, and a writer that replaces the file by
rename (most formatters, and any agent that writes a temp file and renames it into place) leaves the
watch on the old inode, where nothing ever fires again. The callback therefore re-arms after every
event: stop and close the handle, then start a new one on the current real path. That also covers the
case where the rename changed the realpath.

### 5.5 Bumps and pins

- `nvim-surround`: `version = "^3.0.0"` to `"^4.0.0"` (`textobjects.lua:4`). Breaking change 1: v4
  changed the `setup` surface; the config passes an empty table, so the bump is expected to be
  inert, verified by the keymap dump and a manual `ys`/`cs`/`ds` check. PR 26a.
- `catppuccin`: bump past `605b460` and rename the colorscheme call (bug #16). Breaking change 2. Own
  commit, PR 26c; the bufferline path at `ui.lua:69-70` is already v2-correct.
- `nvim-hlslens`: `4254054` to `be2d7b2` (bug #15). PR 26b.
- `none-ls`: no forced bump (item 34).
- `nvim-treesitter`: already on `main` (item 36 struck). The plugin was archived 2026-04-03; the pin
  keeps working but receives no fixes, and nvim 0.12's builtin treesitter is the long-term path. Flag
  only. `nvim-treesitter-context` is still on `master`; during PR 29b confirm the context bar renders on
  0.12.5 and `:checkhealth` stays clean, then leave it.
- Everything else keeps its pin. `dial`, `markview` and `toggleterm` stay (item 35).

## 6. The `custom_api` redesign and its tests

v3 section 1 stands as the design; this section restates only what the plan needs and adds the test
harness. `delegate.lua` leaves in PR 8; `init.lua` drops its `M.delegate` line.

### 6.1 Errors (item 54)

Two failure modes, kept apart: an operational failure (not a git repository, `gh` not logged in) is a
result, `nil, message`; a bug is `error()` caught at the boundary. `git`, `github` and `util` never call
`vim.notify`. One boundary helper, `custom_api.try(fn, { label = "git.default_branch" })`, wraps the
keymap-layer calls with `xpcall(fn, debug.traceback)` and presents `[label] message` plus the traceback
through `vim.notify`. The label is explicit data. `debug.getinfo(fn, …)` and `helpers.wrap` are
deleted, and `helpers.lua` with them once `pack` has no caller.

### 6.2 Seams (items 55 to 59)

- The shell runner is injected: `git` and `github` read `M.runner` (default `util.run_shell_command`),
  and a test sets `git.runner = fake` where `fake` returns `(exit_code, output)` by command string.
- The GitHub-API fallback moves into `github.default_branch({ owner, name })`; `git.default_branch()`
  checks `refs/remotes/origin/main` and `master` and returns `nil, "no default branch"` otherwise. The
  keymap at `git.lua:997` calls `git.default_branch()` and falls through to `github.default_branch`
  with `github.repo()`'s owner and name. That closes #4 and #4b.
- `map` and `overseer_runner` move out of `util.lua` into `custom_api/keymap.lua` and
  `custom_api/overseer.lua`; `util.lua` keeps the string helpers and `run_shell_command`. The
  redundant closure in `map` (`util.lua:141-147`) is dropped: `vim.keymap.set` handles a string or a
  function `rhs` natively.
- `latest_commit` returns `({ hash, summary, body }, err)`.
- No module does anything on `require` beyond building its table.
- `copy_URL_to_clipboard` becomes `copy_url_to_clipboard` (decision E); the one caller is
  `git.lua:33`.

### 6.3 Headless Lua tests, run from the source tree

Layout, inside the config source so it travels with it in git, but chezmoiignored (3.4) so it never
reaches `$HOME`:

```
dot_config/nvim/tests/
  run.lua              # runner: loads tests/<name>_spec.lua, runs each case, exits 1 on failure
  dump_state.lua       # the keymap + plugin dump used by section 3.7 (writes JSON to argv[1])
  util_spec.lua        # trim, sanitize_input, normalize
  git_spec.lua         # convert_remote_protocol, normalize_branch, is_current_branch,
                       # extract_upstream, parse_branch_line, parse_blame_porcelain,
                       # default_branch with a fake runner, latest_commit (table, err)
  github_spec.lua      # account().username resolves with a fake runner; default_branch fallback
  try_spec.lua         # the label is reported verbatim, never "anonymous"; traceback present
  task_events_spec.lua # 7.7 #1: tier and detail formatting
  review_ledger_spec.lua # 7.7 #2: the findings-table parser
  agent_context_spec.lua # 7.7 #3: the at-mention composer and the may_send state gate
```

The runner takes the CONFIG ROOT as an argument, `--config <dir>`, defaulting to its own grandparent
(`tests/../`), and prepends `<config>/lua/?.lua;<config>/lua/?/init.lua` to `package.path`. That one
argument is why the tests can stay out of `$HOME`: the same runner tests the source tree from `just`
and the deployed tree from the bootstrap (3.9), and the Lua under test is whichever root it was
pointed at. Deploying `tests/` instead was rejected because it would put test code in the editor's
runtime directory and add a fourth chezmoiignore exception for nothing the editor uses.

`run.lua` is about 40 lines: it parses `--config`, iterates the spec files (or the one named in
`argv`), calls each `name = function()` case inside `pcall`, prints `ok`/`FAIL <name>: <err>`, and
ends with `os.exit(failures == 0 and 0 or 1)`. Assertions are plain `assert` and `vim.deep_equal`. No
plenary, no busted: the runner is invoked as `nvim --headless --clean -l tests/run.lua [spec]` and
`--clean` keeps the whole plugin tree out, so a run costs about 30 ms. `dump_state.lua` is the one
file that needs the full config loaded, so it runs WITHOUT `--clean` (3.7).

Wiring:

- `just test-nvim` runs the runner against the source tree.
- `test/unit/nvim-custom-api.bats`: one `@test` per spec file, each spawning the runner with that
  spec's name. Each test is well under the 200 ms warning threshold; the process boundary is the
  behavior (a headless Neovim running our Lua), which is the case the bats ruling allows a spawn for.
  `just test-unit` picks it up automatically.
- The bootstrap (3.9) runs the same runner with `--config "$HOME/.config/nvim"` and fails on a red
  test.

The clangd `cmd` assertion (bug #12) is not a unit test: it needs the full config loaded. It is a
line in the bootstrap's verification and an acceptance item in section 10:

```bash
nvim --headless -c 'lua assert(vim.tbl_contains(vim.lsp.config.clangd.cmd, "--clang-tidy"))' +qa
```

## 7. Agent integration against herdr

Every herdr command in this section was checked against `herdr --help`, `herdr agent --help`,
`herdr pane --help`, `herdr workspace --help` and the per-subcommand help on 0.8.2 today. The facts
that shape the designs: no `agent`, `pane` or `workspace` subcommand takes `--json` (the one flag of
that name is on `herdr api schema`), every subcommand's output is JSON already;
`herdr agent list` reports each agent's `agent`, `agent_status`, `focused`, `cwd`, `pane_id` and
`workspace_id`; `herdr pane list --workspace <id>` filters by workspace and each pane carries
`focused`; `herdr pane get <pane_id>` and `herdr pane current` report the same fields plus scroll
state; `herdr pane split [--current] [--direction right|down] [--cwd <path>] [--focus]` creates a
pane; `herdr agent start <NAME> --kind claude --pane <ID> -- <agent args>` launches a supported agent
in a pane that is at a shell prompt; `herdr agent prompt <TARGET> <TEXT>` submits text to a running
agent and refuses (`agent_blocked`) while the agent is waiting on an approval; `herdr pane run
<PANE_ID> <COMMAND>...` sends text plus Enter; `herdr pane send-text <PANE_ID> <TEXT>` sends literal
text with no Enter; `herdr pane send-keys <PANE_ID> <KEY>...` sends key names (`esc` is canonical).
The workspace of the current pane is `HERDR_WORKSPACE_ID` (there is no `HERDR_SESSION`).

### 7.1 `delegate.lua` is deleted (item 61)

`custom_api/delegate.lua` goes, with its `require(...).setup()` at `keymaps.lua:6` and the
`M.delegate` line in `custom_api/init.lua`. The which-key `<leader>d` group "delegate" keeps two
keymaps that were never delegates (`<leader>dx` chmod, `<leader>da` code action); the group is renamed
"do" (section 8.3). Bugs #5 and #9 close as moot.

### 7.2 `claudecode.nvim` with provider `none` and a herdr pane launch

The plugin runs the WebSocket server and writes `~/.claude/ide/<port>.lock`; with
`terminal.provider = "none"` it opens no window. The CLI in the herdr agent pane connects with
`claude --ide`, or `/ide` inside a running session. Keymaps (section 8.3): `:ClaudeCodeSend` in visual
mode, `:ClaudeCodeAdd %` for the current file, `:ClaudeCodeDiffAccept` and `:ClaudeCodeDiffDeny`.

The research doc's eight open questions (inventory item 78), each with its disposition, carried as
the plugin spec's header comment in PR 12:

1. Reverse-engineered protocol: accepted; the pin is a commit, not a tag, and a protocol break shows
   up as a failed section 10.8 check at bump time, never silently.
1. No auto-launch under `none`: by design; the launch helper below is the convenience.
1. The send queue clears on the connection timeout: documented; a send with no client connected is
   lost after the timeout, so connect first.
1. Connection ordering: Neovim first (it writes the lock file), then `claude --ide` or `/ide`.
1. The snacks dependency under `none`: declared in the spec because the README requires it; whether
   `none` exercises it is checked once in PR 12 by loading with snacks present, which it always is.
1. Diff auto-accept versus auto-save: the auto-save exclusions in 5.3 keep auto-save off the
   `(proposed)` buffers, so the diff is accepted or denied only by the keymaps.
1. The "beta" label: accepted as the cost of decision A.
1. Local-install PATH: `claude` is on PATH in every herdr pane through the bashrc; nothing to do.

The launch helper is one keymap, `<leader>Cc`, implemented in Lua over `vim.system`. The agent lookup
is not written by this program: `herdr-nvim` (installed, `lua/herdr-nvim/agents.lua`) already lists
the agents of `HERDR_WORKSPACE_ID` (`agents.list()`), narrows to the one that shares `HERDR_TAB_ID` or
to a lone agent in the workspace (`agents.resolve()`), and shows a picker when that is ambiguous
(`ui.pick_agent`). Focus is never consulted, here or in 7.3 or 7.7: herdr focus is UI-wide, one
focused pane in the whole session, so every agent of a background workspace reports `focused = false`
and the field cannot say which agent an editor pane means. The helper:

1. `agents.list()` filtered to `kind == "claude"`, then `agents.resolve()`, else the picker.
1. If one exists, `herdr agent prompt <pane_id> /ide`. It submits the slash command and Enter, and
   refuses while that session is blocked on an approval, which is the right behavior: typing into a
   blocked prompt with `pane run` would answer the approval instead.
1. Otherwise `herdr pane split --current --direction right --cwd <vim.fn.getcwd()> --focus --env
   NVIM_MCP_SOCKET=<vim.v.servername>`. The reply's new pane is `.result.pane` and its id is
   `.result.pane.pane_id` (`herdr --skill`: "pane split returns the new pane as .result.pane" and
   "read the new pane ID from .result.pane.pane_id"). The `--env` pins the socket: the MCP server the
   CLI starts in that pane inherits it and the resolver (7.3) connects to it with no discovery. Then
   `herdr agent start <name> --kind claude --pane <pane_id> -- --ide`, where `<name>` is `claude-` plus
   the pane id lowercased with `:` replaced by `-` (`wW:p3K` becomes `claude-ww-p3k`). Names must match
   `[a-z][a-z0-9_-]{0,31}` and be unique among LIVE agents, and a name is cleared when its agent exits
   (`herdr --skill`); pane ids are never reused, so the derived name is free unless a case-folded twin
   is live, in which case `agent start` rejects it and the helper retries once with a `-2` suffix. A
   fixed name would fail in the second workspace that launches.

This is a convenience, not a dependency; typing `claude --ide` in the pane is the documented path.

### 7.3 The MCP server: evaluate `nvim-mcp`, custom only if it falls short (custom plugin #4)

What the operator wants: keep a buffer open, prompt the agent in its pane, the agent reads the buffer
in its current unsaved state and edits it in place, without `:w`. Only Neovim's remote-procedure-call
API can do that, so an MCP server over the RPC socket is the floor. The question #4 asks is whether an
existing server can tell WHICH Neovim instance and buffer is current when eight herdr workspaces may
each hold one or more Neovim panes.

One candidate is left. Both were checked today:

| Candidate                 | State (2026-09-01)                                  | Discovery and instance choice                                                                     |
| ------------------------- | --------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `linw1995/nvim-mcp`       | Rust, HEAD `0b5ace3`, 66 stars, `cargo install`     | `--connect auto` connects to "current project" instances; its Neovim-side plugin registers one socket per git root; 26 tools incl. `cursor_position`, `read`, `list_buffers`, LSP; no tool answers "which instance is focused" |
| `paulburgess1357/nvim-mcp` | Python, HEAD `4e581a1`, 61 stars; README "Requirements: Linux" | DISQUALIFIED: Linux-only per its own requirements section; dresden is macOS                 |

Evaluation criteria, run in PR 9 against the live setup and recorded in the PR body:

1. **Discovery without `--listen`.** Finds the socket nvim 0.12 creates in `stdpath("run")` by default,
   or registers its own through its Neovim-side plugin.
2. **Workspace match.** With Neovim open in two herdr workspaces (two directories), a Claude session
   started in one workspace's pane reaches that workspace's Neovim without a prompt. herdr anchors each
   workspace to a directory, so a cwd or git-root match is the expected mechanism; a worktree
   workspace (`~/.herdr/worktrees/<repo>/<branch>`) counts as its own root and must match itself.
3. **Native choice within a workspace.** With two Neovim panes in one workspace, the server started
   from an agent pane reaches the Neovim that pane means (the one in its tab, or the lone one) with
   no wrapper. Focus is not an acceptable mechanism (7.2).
4. **Explicit socket.** `--connect <socket path>` connects to exactly that instance, so a wrapper can
   choose; upstream documents explicit sockets in its `docs/usage.md`.
5. **Current buffer.** A tool returns the current buffer's path and cursor, and reads and edits its
   unsaved content, with undo.
6. **Install.** No node runtime; `cargo install` from the YAML-declared toolchain, wired by a
   `run_onchange` script the way the pns and herdr plugins are built.

PR 9 is the evaluation and nothing else: `nvim-mcp` is installed by hand (`cargo install`) for the
day, the six criteria are run and recorded in `docs/research/2026-09-nvim-mcp-evaluation.md`, the row
taken is named, and that record is PR 9's one commit. It installs nothing through chezmoi, registers
nothing, and edits no CLAUDE.md, so a candidate that fails never reaches `~/.claude.json` or the Codex
template. The install script, the registrations and the 7.5 rule land in the PR that SHIPS a server:
PR 10a on the two `nvim-mcp` rows, PR 10b on the crate rows.

Registration in both harnesses is a requirement of whatever ships, not a criterion: it is ours to
write, so it cannot fail an evaluation. Claude: a stable entry in `modify_private_dot_claude.json`,
beside the composio and workspace-mcp entries it already manages, applied by the operator. Codex: one
`[mcp_servers.nvim]` table in `private_dot_codex/private_config.toml.tmpl`, the full template of
`~/.codex/config.toml` that is in this worktree today (section 2), beside its ten existing tables.
That template is untracked here and has no pull request yet; the shipping PR depends on the PR that
first lands `private_dot_codex/private_config.toml.tmpl` on `main`, and its number is written into
PR 9's evaluation record and the shipping PR's body the day it merges. The shipping PR never adds a
`modify_config.toml`: two chezmoi sources for one target is a conflict, not a merge. No project
`.mcp.json`. Both registrations land together because the 7.5 rule binds both harnesses.

The decision table. Criteria 5 and 6 are evaluated FIRST, because nothing ships without them; then 4;
then 1 to 3. "Undecided" is a criterion the working day ended without a recorded pass, and it is read
as a fail everywhere except the first row, where it stops the ship instead:

| Outcome                                              | Ship                                                                                  |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------- |
| 5 or 6 undecided                                     | nothing; PR 9 is the evaluation record and the budget extends by one day, once        |
| 5 or 6 fails                                         | the custom crate: PR 10a is its design spec, PR 10b its build; PR 10b registers it    |
| 5, 6 pass; 4 fails or undecided                      | the custom crate (a wrapper cannot pin what the server cannot accept): PR 10a the spec, PR 10b the build and the registration |
| 5, 6, 4 pass; 1, 2, 3 all pass                       | `nvim-mcp` as-is: PR 10a installs it, registers it directly and adds the 7.5 rule; its body says PR 10b is skipped |
| 5, 6, 4 pass; any of 1, 2, 3 fails or undecided      | `nvim-mcp` plus the resolver: PR 10a installs it, adds the resolver with its bats test, registers the resolver as the server command and adds the 7.5 rule; 10b skipped |

Every combination of the six outcomes lands in exactly one row, and no row depends on itself: the
as-is row needs no pinning, and the resolver row is reachable only after criterion 4 has passed. On
every row exactly one PR registers a server (10a or 10b), and it is the PR whose server the operator
keeps.

The resolver, `~/.local/libexec/nvim-mcp-connect.sh` (it is the command the MCP registration runs, so
`libexec` per the placement rules), identifies Neovim by pane id and by the environment of the pane the
agent runs in, never by focus. In order:

1. `NVIM_MCP_SOCKET` is set (the launch helper's `--env`, 7.2): exec the server with
   `--connect "$NVIM_MCP_SOCKET"` and stop.
1. Otherwise list `${XDG_RUNTIME_DIR:-${TMPDIR}nvim.${USER}}/*/nvim.*.0`, the runtime root Neovim
   documents (`:help serverstart()`, `vimfn.txt:8813`: "Example bash command to list all Nvim
   servers: `ls ${XDG_RUNTIME_DIR:-${TMPDIR}nvim.${USER}}/*/nvim.*.0`"); `$TMPDIR` alone is the
   macOS case and misses every Linux socket. For each socket ask the instance for its own three ids
   under a short deadline, and SKIP any socket that refuses or times out (a crashed instance leaves
   its path behind):

   ```bash
   nvim --server "$sock" --remote-expr \
     'join([getenv("HERDR_WORKSPACE_ID"), getenv("HERDR_TAB_ID"), getenv("HERDR_PANE_ID")], " ")'
   ```

   Keep the sockets whose workspace equals the caller's `$HERDR_WORKSPACE_ID`, which the agent's pane
   exports like every pane.
1. Narrow the way `herdr-nvim`'s `agents.resolve` does, mirrored from the editor's side: one candidate
   in the caller's `$HERDR_TAB_ID` wins; else a lone candidate in the workspace wins; else REFUSE with
   exit 1, the candidate pane ids on stderr, and the instruction to launch the agent from Neovim
   (`<leader>Cc`) or export `NVIM_MCP_SOCKET`. Two unpinned Neovim panes in one workspace are
   ambiguous, and a guess edits the wrong buffer.

At most 80 lines of bash, with a bats test on the selection function fed fixture strings.

The custom server, if built: a Rust crate under `~/.local/share/nvim-workspace-mcp` (proposed name,
function-named, no handle; confirm before it is created), built by a `run_onchange_after_59`-style
script, exposing exactly five tools (`current_buffer`, `list_buffers`, `read_buffer`, `edit_buffer`,
`diagnostics`; `list_buffers` is required by the 7.5 rule) and doing the same pane-id resolution
internally, reading the same three variables and the same `NVIM_MCP_SOCKET` pin.

The budget: the PR 9 evaluation is one working day, extended once by the first row of the table and
never otherwise; the resolver is at most 80 lines of bash plus its bats file; the custom crate gets
its own design spec (PR 10a) before code and is capped at the five tools and about 600 lines of Rust
(PR 10b). When the crate row is reached, the crate is still built inside this program, because no
other candidate exists (the second one is Linux-only). Today's reading is that `linw1995/nvim-mcp`
meets 1, 2, 4, 5 and 6 and the resolver closes 3, so the expected shipping PR is PR 10a on the
resolver row; the crate is the last rung.

### 7.4 vim-slime with a herdr target (item 39)

vim-slime dispatches `slime#targets#<name>#config` and `#send` through autoload (verified in its
`autoload/slime.vim`), so a target file on the runtimepath is a first-class backend with no fork. The
config adds `dot_config/nvim/autoload/slime/targets/herdr.vim`: `config` prompts for a pane id with the
workspace's agent pane as the default (the `herdr-nvim` lookup of 7.2), `send` calls
`herdr pane send-text <pane> <text>` (literal text, no Enter) and then `herdr pane send-keys <pane>
<enter>`, and `ValidEnv` checks `$HERDR_ENV`. Neither the Enter key name nor multiline integrity is
documented on 0.8.2: `herdr pane send-keys --help` names only `esc`, and `herdr --skill` shows `esc`
and `ctrl+c`. PR 11 therefore records this check on a scratch pane in its body before binding
anything (every flag is one the 0.8.2 help lists):

```bash
p=$(herdr pane split --current --direction down --cwd "$PWD" --no-focus | jq -r .result.pane.pane_id)
herdr pane send-text "$p" $'printf "%s|" a b\nprintf "%s|" c d\n'   # two lines in one send
herdr pane send-keys "$p" enter                                    # the spelling under test
sleep 1; herdr pane read "$p" --lines 6                            # expect a|b| then c|d|
herdr pane close "$p"
```

If `enter` is rejected, `return` and `cr` are tried in that order and the accepted spelling is what
the target binds. If the two-line send arrives merged or truncated, `send` splits on newlines and
sends one line per `send-text` call. This is the same precedent as the smart-splits herdr mux backend.
It covers the three losses the research review named: free-text prompts, non-Claude agents, and
unsaved scratch buffers, all as raw text to any pane.

### 7.5 The rule for open buffers, both harnesses (item 63)

Added to `dot_config/nvim/CLAUDE.md` and to the shared partial
`.chezmoitemplates/global-agent-rules.md`, which renders into both `~/.claude/CLAUDE.md` and
`~/.codex/AGENTS.md`:

> A file that is open in a Neovim buffer is edited through the Neovim MCP tools, never with `Write`
> or `Edit`. Check with the MCP `list_buffers` tool before writing a file under the current project;
> a disk write collides with the unsaved buffer and the operator loses one side.

Because the rule reaches Codex, the server is registered for Codex too (7.3, criterion 6), and the
custom fallback carries `list_buffers`. A rule that names a tool one harness cannot see would be a
rule that harness silently breaks, which is also why the rule lands in the same PR as the registration
(PR 10a or PR 10b, by the 7.3 row) and never before a server is registered.

### 7.6 The ACP hedge (item 64)

The Agent Client Protocol would move the agent out of its herdr pane into a Neovim chat buffer, the
inverse of this setup. Deferred, recorded here so it is not re-litigated; revisit only if cross-agent
portability becomes a goal.

### 7.7 Custom plugins #1 to #3, approved

Each is a module under `lua/custom_api/`, function-named with no handle so it can be lifted into its
own repository later, with a pure core under unit test (6.3) and a thin editor edge. The module names
are proposals; the rename rule applies (confirm before creating).

**#1, the editor-side pns producer: `custom_api/task_events.lua` (PR 14).** Overseer, `xcodebuild.nvim`
and neotest runs fire the same event the shell notifier fires. `report({ tool, task, state,
seconds })` runs `vim.system` on `~/.local/libexec/pns/pns` with the producer flags `pns --help`
lists today: `--agent nvim --state done|failed --project <cwd basename> --detail "<tool>: <task>
(<duration>)" --pane "$HERDR_PANE_ID"`, plus `--long-running` at 300 s or more. The tiers mirror the
shell notifier exactly: nothing under 30 s, the presence gate from 30 s, the lights from 300 s, so a
build reaches the banner, Discord and the phone through the engine's own delivery plan and the editor
never raises a banner of its own. Edges: an overseer component (`on_complete`) registered from the
overseer spec, and the completion callbacks `xcodebuild.nvim` and neotest expose, whose exact names
are verified at plan time from the pinned commits. Pure and tested: `tier(seconds)` and
`detail(tool, task, seconds)`.

**#2, the review-ledger quickfix: `custom_api/review_ledger.lua` (PR 15).** The pipeline's findings
registers (`~/.claude/pipeline/slices/findings-*.md`) hold one table whose rows start with `| F<n>`
and whose columns are `id, step, severity, summary, disposition, evidence` (verified against
`findings-pns-loop-rule.md` today). `parse(lines, path)` returns quickfix items: `filename` and
`lnum` from a `path:line` token in the summary when the row has one, else the ledger file and the
row's own line number; `text` is `F<n> <severity> <disposition>: <summary>`; rows whose disposition is
`FIXED` are skipped unless the command is banged. `:ReviewLedger[!] [file]` (default: the newest
findings file) sets the quickfix list, so a fix round is `:cnext` through the findings. Pure and
tested: `parse`.

**#3, the agent-context sender: `custom_api/agent_context.lua` (PR 16).** `compose()` builds one
string from the cursor: the `@<path relative to cwd>:<line>` at-mention, the first diagnostic on the
line (`vim.diagnostic.get`), the enclosing function's name through `vim.treesitter` (walk up from the
cursor node to the first node whose type ends in `function_definition`, `function_declaration` or
`method_definition`; empty when no parser is attached), and the line's blame SHA and summary through
`git.blame_sha` (5.2) and `git.latest_commit`. `send(text)` resolves the agent the way 7.2 does
(`herdr-nvim`'s `agents.list()` and `agents.resolve()`, the picker when ambiguous), then reads its
state with `herdr agent get <pane_id>`: `.result.agent.agent_status` (verified live today), one of
`idle`, `working`, `blocked`, `done`, `unknown` (the values `herdr agent wait --until` lists). On
`idle` or `done` it runs `herdr pane send-text <pane> <text>`: no Enter, so the operator reads the
composed context in the prompt and submits it. On any other state nothing is typed, because text into a
`working` agent lands mid-turn and text into a `blocked` one answers its approval dialog. The text is
held in a one-slot queue (a newer send replaces it, with a notice) and
`vim.system({ "herdr", "agent", "wait", <pane>, "--until", "idle", "--until", "done", "--timeout",
"600000" })` runs detached; its `on_exit` sends the held text on exit 0 and drops it with a notice
otherwise. The gate is BEST-EFFORT, and the module header says so: `agent get` or `wait` and the
`send-text` that follows are two herdr calls, and the agent can start a turn or block between them.
Three rules narrow the window without pretending to close it. One waiter: a second `<leader>Cx` while
a `wait` is running replaces the queued text and starts no second `wait`, so at most one `on_exit` can
ever send. Recheck: immediately before every `send-text`, on the direct path and on the waiter's exit
0 alike, the state is read again with `herdr agent get`, and the send happens only if `may_send` still
holds. Drop, never retry: a failed recheck drops the text with a notice naming the state seen, and the
operator presses the key again when the agent settles. A send that lands in a turn that began after
the recheck is the accepted residual. `herdr agent prompt` is not used: it appends Enter, and this
plugin's contract is that the operator submits. That is one step stricter than `herdr-nvim`'s own
`dispatch.send`, which warns on `working` and sends anyway. It is a composer over the same transport the
slime target uses, not a third transport; where `:ClaudeCodeSend` (7.2) carries a selection, this
carries context about a position. Keymap `<leader>Cx` "send context" (8.3). Pure and tested:
`compose_text(parts)` and `may_send(status)`.

## 8. Keymap and which-key design

### 8.1 The observed pattern

- `folke/which-key.nvim`, `event = "VeryLazy"`, `preset = "helix"`, no icon declarations (which-key's
  own inference applies), `opts_extend = { "spec" }`.
- Every group is declared centrally in `lua/plugins/which-key.lua` inside one `opts.spec` entry with
  `mode = { "n", "v" }`, as `{ "<prefix>", group = "<name>" }` rows sorted by prefix. Group names are
  mostly lowercase nouns, with exceptions that stay as they are (`GitHub (Octo)`, `HEAD (latest
  commit)`, `LSP`); a slash is written with the full-width `／` character so it renders in the popup;
  two groups use `expand` (buffer, windows) and windows uses `proxy = "<c-w>"`. Two prefixes live
  outside that table: octo adds eight `<localleader>` groups per buffer through `wk.add({...},
  { buffer = 0 })` on the `octo` FileType (`git.lua:1200-1215`), and `herdr-nvim` sets
  `prefix = "<leader>A"` (`herdr-nvim.lua:6`) with no group row at all, so `<leader>` shows `A` with
  no name today.
- Two git namespaces: `<C-g>` is the fugitive command tree (group "git-1", 16 subgroups such as
  `<C-g>b` branch, `<C-g>B` blame, `<C-g>l` log); `<leader>g` is gitsigns and snacks pickers ("git-2");
  `<leader>G` is gh; `<leader>gh` is octo.
- Lowercase `<leader>` letters are editor features (find, search, LSP, overseer, yank, toggle);
  uppercase letters are external systems or rarer surfaces (`G` gh, `R` rest, `U` urlview, `L` lazy,
  `D` debug, `A` herdr-nvim). `<leader>C`, `<leader>t`, `<leader>X` and `<leader>T` are free.
- Keymaps are defined with the global `map({ mode, lhs, rhs, desc })` helper outside plugin `keys`
  tables, except snacks, noice and which-key which use `keys`. Descriptions in the plugin specs mostly
  follow `Tool: action` ("Snacks (Git): branches", "Overseer: open task in floating window", "Git
  Blame: toggle"); `keymaps.lua` uses plain phrases ("Save file", "Write all files", "Next Search
  Result", "Keywordprg"). Existing descriptions are not rewritten by this program.
- Local-leader `\` is used for buffer-local octo groups added on the `octo` FileType.

### 8.2 The rule for new keymaps

1. Every NEW keymap has a `desc` in the `Tool: action` form; new group names are lowercase nouns.
2. Every new prefix gets a `group` row in `which-key.lua` in the same commit, in prefix order. PR 4c
   adds the missing `{ "<leader>A", group = "herdr" }` row for the prefix that exists today.
3. Namespace by the pattern above: an editor feature takes a free lowercase `<leader>` letter; an
   external system takes an uppercase one. No new `<C-…>` or `<M-…>` chords: herdr owns ctrl-h/j/k/l,
   harpoon owns `<C-p>`/`<C-n>`, overseer owns `<M-7>`, `<M-8>`, `<M-;>`, `<M-[>`.
4. The PR 2 baseline dump (3.7, check 5) is the reference. An `lhs` present in that baseline is never
   rebound by a later PR except where this spec names it (5.2, the 8.3 rows); "newer" means "not in
   the baseline", and the newer keymap is the one that moves. The PR body shows the dump diff and the
   `:verbose map <lhs>` line for each new `lhs`.
5. Plugin-local keymaps go in that plugin's spec (`keys` when they can lazy-load it, `config` when the
   `rhs` needs the loaded module); global ones in `config/keymaps.lua`.

The one stated improvement to the existing pattern: stale group names are corrected where the group's
contents changed, and only there. "lazyvim" becomes "lazy" (the config is not LazyVim), "delegate"
becomes "do" (its two survivors are actions), and the new groups follow the naming style.

### 8.3 Conflicts to resolve and the proposed new keymaps

| Item                        | Resolution                                                                                         |
| --------------------------- | -------------------------------------------------------------------------------------------------- |
| dial vs boole `<C-a>`/`<C-x>` | gone: dial binds nothing today; PR 25 gives dial the keys after boole is deleted, in the same commit |
| git-blame `<C-g>B*`         | same `lhs`, rebuilt (5.2), group row unchanged                                                    |
| `<leader>gm` (git-messenger) | re-pointed at `gitsigns.blame_line({ full = true })`                                              |
| `<leader>d` group           | renamed "do"; `<leader>dt`, `<leader>dp`, `<leader>ds` (delegate) removed                          |
| `<leader>L` group           | renamed "lazy"; descriptions "LazyVim: …" become "Lazy: …"                                         |
| `<leader>A` group           | row added, "herdr" (PR 4c)                                                                         |
| Swift                       | `<leader>X` group "xcode": `Xb` build, `Xr` run, `Xt` test (all), `XT` test (current), `Xs` select scheme, `Xd` select device, `Xl` toggle logs, `Xp` project manager; `cond` on darwin |
| Tests                       | `<leader>t` group "test": `tt` nearest, `tf` file, `ta` all, `ts` summary, `to` output, `tS` stop |
| Agent                       | `<leader>C` group "claude": `Cs` send selection (visual), `Ca` add current file, `Cy` accept diff, `Cn` deny diff, `Cc` launch or attach `claude --ide` (7.2), `Cx` send context (7.7 #3), `Cp` slime: pipe selection or paragraph to the target pane, `CP` slime: set target pane |
| Review ledger               | `:ReviewLedger` command only, no keymap; it is a quickfix producer and `<leader>x` already holds the quickfix maps |

The implementer finalizes the letters under the rule; the table is the proposal the plan starts from.

### 8.4 How groups stay discoverable

Groups keep living in the one `opts.spec` table so `<leader>` alone shows every namespace and
`<leader>b?` still lists buffer-local maps. A group whose plugin is darwin-only (`<leader>X`) is
declared unconditionally with its name so the popup is the same on both operating systems; the keymaps
under it carry the `cond`.

## 9. Startup

The lever is `defaults.lazy = false` (`lazy.lua:41`), which forces eager every spec that has no trigger
of its own. PR 30a to 30c9 add a trigger to every spec that needs one, in this order of cost (from
today's profile), and PR 30d flips the default. The rule 30d follows, from lazy.nvim's own resolution
(`lazy/core/plugin.lua:235-241` at the installed pin: an explicit `lazy` wins, else
`plugin.lazy = dep or defaults.lazy or event or keys or ft or cmd`): **`lazy = false` is written only
onto a spec that has no `event`, `keys`, `ft` or `cmd` and must be present at startup.** Writing it
onto a triggered spec would override the trigger and make the plugin eager again, which is the
opposite of the pass. The table says which is which; every spec name below was checked against the
live spec files on 2026-09-01:

| Spec                          | Trigger                                                                          |
| ----------------------------- | -------------------------------------------------------------------------------- |
| `mason-lspconfig`, `nvim-lspconfig`, `none-ls`, `lsp-format` | `event = { "BufReadPre", "BufNewFile" }`; Mason itself `cmd = "Mason"` plus the same event through its dependents (PR 30a) |
| `vim-fugitive` and its deps   | `cmd = { "Git", "G", … }` plus the `<C-g>` keys (PR 30b1)                        |
| `octo`                        | `cmd = "Octo"` plus its `<leader>gh` keys (PR 30b2)                              |
| `treesj`                      | `keys` (its five `<leader>j` maps) (PR 30c1)                                     |
| `auto-save`                   | `event = { "InsertLeave", "TextChanged" }` and the `<leader>uv` toggle key (PR 30c2) |
| `overseer`                    | `cmd` plus its `<leader>o` and `<M-…>` keys (PR 30c3)                            |
| `harpoon`                     | `keys` (PR 30c4)                                                                 |
| `urlview`                     | `cmd` and `keys` (PR 30c5)                                                       |
| `sort`                        | `cmd` and `keys` (PR 30c6)                                                       |
| `live-rename`                 | `keys` (PR 30c7)                                                                 |
| `aerial`                      | `cmd` and `keys` (PR 30c8)                                                       |
| `claudecode.nvim`             | `event = "VeryLazy"` (the lock file must exist before the CLI connects) (PR 30c9) |
| already triggered today, untouched | `which-key` (`event = "VeryLazy"`), `noice` (`event = "VeryLazy"`), `blink-cmp` (`event = { "InsertEnter", "CmdlineEnter" }`), `textobjects` and `unimpaired` (`VeryLazy`), `kulala`, `ansible`, `docker`, `markdown-plus`, `markdown-preview`, `lazydev` (`ft`, `cmd` or `event`), `grug-far`, `codesnap`, `flash`, `trouble`, `todo-comments`, `yanky`, `chezmoi` (`keys` or `cmd`), and `xcodebuild.nvim` (`ft = "swift"` plus `cmd` from PR 3); lazy.nvim already marks each lazy, so no trigger PR touches them and 30d writes nothing onto them |
| already `lazy = false` today, untouched | `snacks` (priority 1000), `smart-splits` (keys are set in `config`), `oil`, `markview`, `witch-line`, `helpview`, `nvim-treesitter`, `blink.nvim` (`chartoggle.lua`) |
| gets `lazy = false` in PR 30d | the specs with no trigger key at all that must exist at startup: `catppuccin` (colorscheme, `priority = 1000`, no `lazy` key today), `bufferline` and `deadcolumn` (`ui.lua`), `herdr-nvim`, `hlslens`, `mini.move`, `quick-scope`, `ts-comments`, and the `init.lua` trio (`vim-rsi`, `vim-repeat`, `dial`); PR 30d's body lists the final set from `:Lazy` before the flip, and a name outside this row needs a stated reason |

### 9.1 Measurement method

Used for the baseline and for every later pull request, always from one fixed benchmark directory,
`BENCH=~/.local/state/nvim-overhaul/bench`: created once with `mkdir -p`, kept EMPTY (no buildfile,
no `.git`, no file of any kind, so overseer, gitsigns and every root-sensitive plugin see the same
nothing on every run), never a scratch directory, and entered with `cd "$BENCH"` before every measured
run. `S` is a scratch directory that is kept (every log, every stderr file and the computed median go
in the PR body's evidence, not just the number):

```bash
mkdir -p "$BENCH" && cd "$BENCH"                     # the one fixed directory, empty
sudo purge                                           # drops the file cache: run 1 is then COLD
for i in 1 2 3 4 5; do
  nvim --headless --startuptime "$S/st-$i.log" -c 'doautocmd User VeryLazy' +qa 2>"$S/err-$i.log"
done
grep -h "NVIM STARTED" "$S"/st-{1,2,3,4,5}.log       # run 1 is the cold number, recorded, not gated
grep -h "NVIM STARTED" "$S"/st-{2,3,4,5}.log | sort -n | awk '{a[NR]=$1} END {print (a[2]+a[3])/2}'
wc -c "$S"/err-*.log                                 # every stderr file is 0 bytes
```

The `doautocmd User VeryLazy` is load-bearing and the number it produces is SYNTHETIC. `+qa` runs
before `VimEnter`, and lazy.nvim fires `User VeryLazy` from a `vim.schedule` after `UIEnter`
(`lazy/core/util.lua:174-175` at the installed pin), which a headless `+qa` never reaches, so without
it every `event = "VeryLazy"` spec (which-key, noice, textobjects, unimpaired, claudecode after
PR 30c9) is absent from the measurement and a regression in that set is invisible to the gate.
Verified 2026-09-01 on the live config: `nvim --headless +qa` leaves which-key unloaded and the same
command with `-c 'doautocmd User VeryLazy'` loads it. Firing the event by hand puts that work inside
the `--startuptime` window; it is not the TUI's own ordering, so every recorded number is labelled
"synthetic (VeryLazy fired by hand)". The interactive number is the TUI run (`nvim --startuptime`
inside a herdr pane, the editor quit by hand once the statusline paints), which is the acceptance
check (10.3) and is recorded beside the synthetic one; it is not the gate because it measures the
terminal too.

Definitions: **cold** is run 1 immediately after `/usr/sbin/purge` (present on dresden), which drops
the unified buffer cache so the Lua files, the plugin trees and the `nvim` binary itself are read
from disk; it is recorded for the human-facing number and never gated, because it measures the disk.
**Warm** is the median of runs 2 to 5, which follow within seconds; that median is the gate. No agent
may be running during a gated measurement (section 2 shows what a busy machine does to the number).
The second batch today, with agents running, gave cold 611.3 ms and warm 204.7 ms; the first batch,
without them, gave cold 411.6 ms and warm about 180 ms; both were plain `+qa` runs without the
synthetic event, so the import-day baseline is the first number measured with this method. The
binding pre-change baseline is the one measured on the import day (section 2).

**The one performance gate**, stated here and only referenced from 3.7, 10 and 11: a PR passes when
its warm median is not slower than the warm median the previously merged PR recorded, within a 10 ms
tolerance (`after <= before + 10`), both measured with this method on the same machine with no agent
running. PR 30d (the flip) and PR 31 (the final acceptance) must ALSO beat the import-day baseline by
more than that tolerance (`after < baseline - 10`). The tolerance is 10 ms because it is about 5
percent of today's warm median and wider than the spread of the quiet batch's warm runs (181.7 and
178.0 ms), so noise alone cannot fail a behavior-neutral PR, while the smallest eager plugin cost in
the profile (8.9 ms for `null-ls`) is close to it and the next ones (10.6, 12.6, 17.6 ms and up)
cannot hide inside it. The TUI run above is recorded beside it and is the acceptance check for the
interactive editor, not the gate, because it measures the terminal too. The v1 target
of under 150 ms is non-binding; the profile above suggests the flip alone removes most of the 380 ms
`config.lazy` block, and the final number is whatever the measurement says.

## 10. Verification and acceptance, itemized

The acceptance bar is "verify Neovim works and does not start with any errors". Itemized:

1. `nvim --headless +qa` writes nothing to stderr, five runs.
2. `nvim --headless "+checkhealth" "+w! <file>" +qa` is clean: zero ERROR lines except those that
   name an absent optional external tool the config does not require (`luarocks`/hererocks, `gs`,
   `tectonic`, `pdflatex`, `mmdc`, `lazygit`, the kitty graphics protocol); each such exception is
   listed in the PR body with its line. The three none-ls executable errors and the treesitter
   runtimepath error are NOT exceptions; PR 29a and PR 29b remove them. The two Snacks `vim.ui.*`
   lines are re-checked in a TUI `:checkhealth snacks`; if they persist there they are config bugs
   and are fixed.
3. The warm headless startup median (synthetic, `User VeryLazy` fired by hand) passes the 9.1 gate:
   within 10 ms of the previous PR's number for every PR, and below the import-day baseline by more
   than 10 ms for PR 30d and PR 31. The TUI run of 9.1, inside a herdr pane, is the acceptance check
   that the interactive editor starts without an error and with every `VeryLazy` plugin loaded; its
   number is recorded, not gated.
4. `just test-unit` green, including `nvim-custom-api.bats`; `just lint-check` green (stylua and
   luacheck included from PR 1).
5. The keymap and plugin dump diff (3.7) matches the PR's stated intent exactly: the import PR shows
   an empty diff, a drop PR shows only the dropped plugin's maps, a remap PR shows the same `lhs` set
   with the blame maps moving from the global pass to the buffer-local pass.
6. Each high or critical bug is exercised live once fixed: `<C-g>i` runs to the prompt (#1); the
   `.c` file assertion for clangd's `cmd` (#12); a branch with an upstream lists its full commit
   message (#8); the no-upstream fallback returns the GitHub default branch (#4, #4b); markdown preview
   opens off dresden (#11).
7. Swift (PR 3): in an Xcode project with `buildServer.json`, `sourcekit-lsp` attaches and hover
   resolves a UIKit symbol; `:XcodebuildBuild` succeeds through `xcbeautify`; `swiftformat` and
   `swiftlint` run on save. Vapor smoke check: a fresh `swift package init --type executable` with a
   `Vapor` dependency (the Vapor toolbox is not required), `swift build` once so the package graph
   resolves, then `sourcekit-lsp` attaches with `Package.swift` as root, hover on `Application`
   resolves, and `swift test` is run DIRECTLY from a shell in the package directory and exits 0.
   neotest (PR 28) is not a dependency of this check and overseer is not used for it.
8. Agent loop (item 73): open a buffer, make an unsaved edit, prompt Claude in the herdr pane, confirm
   it reads the in-memory text through the MCP tool and writes back without a prior `:w`, in the PR
   that registers the server (PR 10a on the `nvim-mcp` rows, PR 10b on the crate rows; PR 9 runs it
   only as criterion 5's hand check); then `:ClaudeCodeSend` on a visual selection lands as an
   at-mention in the same session (PR 12). The same loop is run once from Codex for the MCP half, in
   that same registering PR.
9. Custom plugins (7.7): a 35 s overseer task produces one Discord card whose detail reads
   `overseer: <task> (35s)`, and nothing under 30 s produces any card (PR 14). The banner is NOT part
   of the assertion: the engine suppresses it when the operator is watching the pane the event names
   at delivery time (`dot_local/share/pns/src/engine.rs`, the timing contract), and a manual check
   watches that pane by construction. A banner run for the human record, if wanted, switches to
   another workspace before the task ends and says so. `:ReviewLedger` on a real findings file fills
   the quickfix list and `:cnext` lands on the ledger row (PR 15); `<leader>Cx` on a line with a
   diagnostic, with the agent `idle`, puts the at-mention, the diagnostic and the blame line into the
   agent pane's prompt unsent, and with the agent `working` sends nothing until it settles (PR 16).
10. A clean-`$HOME` full `chezmoi apply` (item 72; `--exclude=templates` is retired) reaches a working
    editor: the bootstrap exits 0, and items 1 to 4 pass in that home. Run once, at PR 31, in a
    throwaway user on dresden.
11. Every apply is quiet on no-op: the bootstrap prints nothing when nothing changed.

## 11. The pull request sequence

One pull request per behavior, small (operator rule 2026-08-10): a PR changes one plugin, one bug, one
option or one mechanism, so 62 PRs (v4.3 split 5, 7, 19, 30b and 30c; the letter and digit suffixes
keep the old numbers readable). Each has the review pipeline the memory describes and its own
keymap dump diff. The **Depends on** cell is complete: it names every PR whose merge the row needs for
its behavior AND every earlier PR that edits a file the row edits (the shared-file rule below), so a
PR opens for review only when every PR in its cell is merged. `lazy-lock.json` is the one shared file
left out of those cells: every lock edit adds, removes or moves ONE key, a textual conflict there is
resolved by keeping both sides, and the re-gate rule below re-proves the result.

| PR    | Behavior                                                                                                 | Depends on            | Closes (inventory)                          |
| ----- | -------------------------------------------------------------------------------------------------------- | --------------------- | ------------------------------------------- |
| PR 1  | Lint infrastructure: stylua and luacheck in treefmt (scoped to `dot_config/nvim/**`), the machine YAML, `Brewfile.dev`, the CI toolchain step; inert until the import | none | 71 (lint half) |
| PR 2  | Import unchanged: backups and their verification, drain, push, README commit and push, archive, flatten (no `.git`), ignores, `.gitignore` rules, the mdformat exclusion, the dump script, the zero-change proof | PR 1 | 45 (part), 46 (track), 51, 52, 65, 74 |
| PR 3  | Swift stack: sourcekit config, `xcodebuild.nvim`, `<leader>X` group, the Vapor smoke check; no package edits | PR 2 | 43 |
| PR 4a | `checker.enabled = false` (`lazy.lua`)                                                                    | PR 2                  | 46 (checker)                                |
| PR 4b | Remove the LazyVim scaffolding: delete `lazyvim.json`, rename the `lazyvim_` augroups (`autocmds.lua`), rename `<leader>L` "lazy" and its descriptions (`which-key.lua`) | PR 3 | 47, 53 |
| PR 4c | The `<leader>A` "herdr" group row (`which-key.lua`)                                                      | PR 4b                 | none (8.2 rule 2)                           |
| PR 4d | Lift the formatter exclusions (`treefmt.toml`) and commit the rewrap of `CLAUDE.md` and `docs/todo.md`  | PR 2                  | none (12.7 default)                         |
| PR 5a | LSP: bug #12 via `vim.lsp.config`, the clangd `cmd` assertion (`lsp.lua`)                               | PR 3                  | 13                                          |
| PR 5b | `run_on_start = false` for mason-tool-installer, the bootstrap's prerequisite (`lsp.lua`)               | PR 5a                 | 50 (prep)                                   |
| PR 6  | `custom_api` errors: `try`, `(value, err)`, delete `helpers.wrap`; the test runner and bats wiring; error-text fixes | PR 2 | 19, 20, 54, 58, 59, 66 (harness) |
| PR 7a | `custom_api` pure-helper tests and bug #8, `extract_upstream` returns `i` (`custom_api/git.lua`, `tests/git_spec.lua`) | PR 6 | 9, 67 |
| PR 7b | Inject the shell runner into `git` and `github`, with the fake-runner tests (`custom_api/git.lua`, `custom_api/github.lua`, both specs) | PR 7a | 55 |
| PR 7c | The GitHub default-branch fallback into `github`, `git.default_branch()` as `(name, err)`, the caller at `git.lua:997`: bugs #4 and #4b (`custom_api/git.lua`, `custom_api/github.lua`, `git.lua`) | PR 7b | 4, 5, 56 |
| PR 7d | Bug #1: `github.account().username` at `git.lua:267` (`custom_api/github.lua`, `git.lua`)                | PR 7c                 | 1                                           |
| PR 7e | Split `map` and `overseer_runner` out of `util.lua` into `keymap.lua` and `overseer.lua`, drop the redundant closure (`custom_api/util.lua`, `custom_api/init.lua`) | PR 7d | 57 |
| PR 7f | Rename `copy_URL_to_clipboard` to `copy_url_to_clipboard` and its one caller (`custom_api/util.lua`, `git.lua:33`) | PR 7e | 60 |
| PR 8  | Delete `delegate.lua`; `<leader>d` regroup (`keymaps.lua`, `which-key.lua`)                             | PR 7f, PR 4c          | 6, 10, 61                                   |
| PR 9  | MCP server evaluation ONLY: `nvim-mcp` installed by hand, the six criteria in table order, the row taken; one commit, the evaluation record; nothing installed by chezmoi, nothing registered, no CLAUDE.md edit | PR 2 | 62 (evaluation) |
| PR 10a | By PR 9's row. `nvim-mcp` rows: the `run_onchange` install script, `lua/plugins/nvim-mcp.lua`, the registrations in `modify_private_dot_claude.json` and the Codex config template, the 7.5 rule in both CLAUDE.md files, and on the resolver row also the resolver with its bats test as the registered command. Crate rows: the custom crate's design spec only | PR 9, PR 4d (`dot_config/nvim/CLAUDE.md`), the PR that lands `private_dot_codex/private_config.toml.tmpl` on `main` (number recorded in PR 9's record) | 62 (ship), 63, 73 (MCP half), custom #4 (resolver) on the `nvim-mcp` rows |
| PR 10b | The custom crate's build with its registrations and the 7.5 rule, only on a crate row of PR 9's table; skipped with a stated reason otherwise | PR 10a, PR 4d (`dot_config/nvim/CLAUDE.md`), the Codex-template PR | custom #4 (build), 63, 73 (MCP half) on the crate rows |
| PR 12 | `claudecode.nvim` provider none, the `<leader>C` group row and keymaps, the item-78 header comment, auto-save compatibility (`which-key.lua`, `autosave.lua`, `lsp.lua` for lsp-format's early return) | PR 8, PR 29a (`lsp.lua`) | 31, 42, 73 (send half), 77, 78 |
| PR 11 | vim-slime with the herdr target, the Enter and multiline check recorded, `<leader>Cp` and `CP` under the PR 12 group; creates `custom_api/herdr.lua` | PR 12 | 39 |
| PR 13 | The launch helper `<leader>Cc` over `herdr-nvim`'s lookup, `agent prompt`, `pane split --env`, `agent start`; extends `custom_api/herdr.lua` | PR 12, PR 11 (`custom_api/herdr.lua`) | 77 (launch) |
| PR 23 | git-blame: rebuild the three keymaps on `custom_api` and gitsigns `on_attach`, then drop the plugin (`git.lua`, `custom_api/git.lua`, `custom_api/github.lua`) | PR 7f, PR 18 | 21, 28 |
| PR 16 | Custom #3: the agent-context sender, `agent_context`, the best-effort state gate and one-slot queue, `<leader>Cx`; shares the `<leader>C` keymap file with PR 13 | PR 12, PR 13, PR 23 | custom #3 |
| PR 14 | Custom #1: the editor-side pns producer, `task_events`, with its overseer and `xcodebuild.nvim` edges (`overseer.lua`, the xcodebuild spec); the neotest edge is PR 28's | PR 7f, PR 3, PR 19b | custom #1 |
| PR 15 | Custom #2: the review-ledger quickfix, `review_ledger`                                                  | PR 7f                 | custom #2                                   |
| PR 17a | Drop cspell (`lsp.lua`)                                                                                 | PR 5b                 | 23                                          |
| PR 17b | Drop gitmoji (`blink-cmp.lua`)                                                                          | PR 2                  | 24                                          |
| PR 17c | Drop nvim-notify (`noice.lua`)                                                                          | PR 2                  | 25                                          |
| PR 17d | Drop gv.vim (`git.lua`)                                                                                 | PR 7f                 | 26                                          |
| PR 17e | Drop git-messenger and re-point `<leader>gm` (`git.lua`)                                                | PR 17d                | 27                                          |
| PR 18 | git.lua: bug #2, the dead `<C-g>bc`                                                                     | PR 17e                | 2                                           |
| PR 19a | overseer: bug #3, `<M-[>` to `OverseerWatchRun` (`overseer.lua`)                                        | PR 2                  | 3                                           |
| PR 19b | overseer: bug #14, `run_task` and the dead `bundles` and `log` tables (`overseer.lua`)                  | PR 19a                | 15                                          |
| PR 20 | autocmds: bug #6 and the auto-reload watch with its re-arm (`autocmds.lua`, `options.lua`)             | PR 4b                 | 7, 40                                       |
| PR 21 | markdown: bug #11, the preview host                                                                      | PR 2                  | 12                                          |
| PR 22a | harpoon: bug #10, the width read at spec load                                                           | PR 2                  | 11                                          |
| PR 22b | noice: bug #17, `inc_rename` (`noice.lua`)                                                              | PR 17c                | 18                                          |
| PR 24 | Drop telescope (octo on snacks); the octo `<localleader>` groups checked by hand (`git.lua`, `chezmoi.lua`, `autosave.lua`) | PR 23, PR 12 | 29, 32 |
| PR 25 | dial spec with boole's augends, then drop boole                                                          | PR 2                  | 30                                          |
| PR 26a | Bump nvim-surround to ^4 (`textobjects.lua`)                                                            | PR 2                  | 33, 34 (none-ls no-op recorded here)        |
| PR 26b | Bump hlslens +1 (bug #15)                                                                                | PR 2                  | 16                                          |
| PR 26c | Bump catppuccin with the colorscheme rename (bug #16, `ui.lua`)                                         | PR 2                  | 17                                          |
| PR 27 | gopls in Mason and `go` in the YAML (`lsp.lua`)                                                          | PR 17a                | 37                                          |
| PR 28 | neotest with its adapters, `<leader>t`, and the neotest edge of the pns producer (`which-key.lua`)      | PR 3, PR 12, PR 14    | 44                                          |
| PR 29a | Health floor: none-ls executable gating (`lsp.lua`)                                                     | PR 27                 | 68 (none-ls half), 71 (health half)         |
| PR 29b | Health floor: the treesitter runtimepath investigation and the treesitter-context check                | PR 2                  | 36 (note), 68, 71 (health half)             |
| PR 30a | Startup: triggers for the LSP group (`lsp.lua`)                                                         | PR 29a, PR 12 (`lsp.lua`) | 48 (part)                               |
| PR 30b1 | Startup: triggers for fugitive and its deps (`git.lua`)                                                | PR 24                 | 48 (part)                                   |
| PR 30b2 | Startup: triggers for octo (`git.lua`)                                                                 | PR 30b1               | 48 (part)                                   |
| PR 30c1 | Startup: `keys` for treesj (`treesj.lua`)                                                              | PR 1 to 29b           | 48 (part)                                   |
| PR 30c2 | Startup: `event` and the toggle key for auto-save (`autosave.lua`)                                     | PR 1 to 29b; PR 24 (`autosave.lua`) | 48 (part)                     |
| PR 30c3 | Startup: `cmd` and `keys` for overseer (`overseer.lua`)                                                | PR 1 to 29b; PR 14 (`overseer.lua`) | 48 (part)                     |
| PR 30c4 | Startup: `keys` for harpoon (`harpoon.lua`)                                                            | PR 1 to 29b; PR 22a (`harpoon.lua`) | 48 (part)                     |
| PR 30c5 | Startup: `cmd` and `keys` for urlview (`urlview.lua`)                                                  | PR 1 to 29b           | 48 (part)                                   |
| PR 30c6 | Startup: `cmd` and `keys` for sort (`sort.lua`)                                                        | PR 1 to 29b           | 48 (part)                                   |
| PR 30c7 | Startup: `keys` for live-rename (`live-rename.lua`)                                                    | PR 1 to 29b           | 48 (part)                                   |
| PR 30c8 | Startup: `cmd` and `keys` for aerial (`aerial.lua`)                                                    | PR 1 to 29b           | 48 (part)                                   |
| PR 30c9 | Startup: `event = "VeryLazy"` for claudecode.nvim (`claudecode.lua`)                                   | PR 1 to 29b; PR 16 (`claudecode.lua`) | 48 (part)                   |
| PR 30d | Startup: `defaults.lazy = true`, with `lazy = false` written only onto the untriggered must-load set of the section 9 table; the strict gate | PR 30a, 30b1, 30b2, 30c1 to 30c9 | 48 |
| PR 31 | Bootstrap script, the clean-home apply, the final acceptance record                                      | PR 30d                | 50, 66 (bootstrap), 69, 70, 72              |

PR 30a to 30c9 move the number on their own: lazy.nvim marks a spec lazy whenever it has `event`,
`keys`, `ft` or `cmd`, even with `defaults.lazy = false` (`lazy/core/plugin.lua:235-241` in the
installed pin: `plugin.lazy = plugin._.dep or defaults.lazy or plugin.event or plugin.keys or
plugin.ft or plugin.cmd`), so each trigger PR is measured by the 9.1 gate and 30d only flips the
default for the specs that have none. The `PR 1 to 29b` cell on the 30c rows is the lane rule made
explicit: no trigger lands before the functional work, so the trigger PRs are measured against a
finished config; the second entry in that cell is the shared-file predecessor.

Lanes after PR 2 and PR 3, which are strictly first and second; a lane is the suggested order, and the
**Depends on** cell is what actually holds a PR back:

| Lane          | Order                                                                  |
| ------------- | ---------------------------------------------------------------------- |
| custom_api and agent | PR 6, 7a, 7b, 7c, 7d, 7e, 7f, 8, 9, 10a, 10b, 12, 11, 13, 23, 16, 14, 15 |
| LSP and tools | PR 5a, 5b, 17a, 27, 29a                                                |
| drops and git | PR 17b, 17c, 17d, 17e, 18, 22b, 24, 25                                 |
| standalone    | PR 4a, 4b, 4c, 4d, 19a, 19b, 20, 21, 22a, 26a, 26b, 26c, 28, 29b       |
| last          | PR 30a, 30b1, 30b2, 30c1 to 30c9, 30d, 31                              |

At most two lanes open at once (the memory's parallel-slices rule). Nothing in PR 3 to PR 29b changes
`lazy.lua`'s `defaults` or adds a trigger, so PR 30a to 30d are the only PRs whose startup number is
expected to move by more than noise; every other PR is held to the 9.1 tolerance.

**Shared files and the merge-main-then-re-gate rule.** The lanes are not disjoint on disk. The files
more than one PR edits, each with its PRs in merge order, which is the order the **Depends on** cells
encode:

- `lua/plugins/lsp.lua`: PR 3, 5a, 5b, 17a, 27, 29a, 12, 30a (PR 12 edits lsp-format's `BufWritePre`
  handler for the auto-save flag, so it sits in this chain after PR 29a and before PR 30a).
- `lua/plugins/which-key.lua`: PR 3, 4b, 4c, 8, 12, 28.
- `lua/plugins/git.lua`: PR 7c, 7d, 7f, 17d, 17e, 18, 23, 24, 30b1, 30b2.
- `lua/custom_api/git.lua`: PR 6, 7a, 7b, 7c, 23.
- `lua/custom_api/github.lua`: PR 6, 7b, 7c, 7d, 23.
- `lua/custom_api/util.lua`: PR 6, 7e, 7f. `lua/custom_api/init.lua`: PR 6, 7e, 8.
- `tests/git_spec.lua`: PR 6, 7a, 7b, 7c, 23. `tests/github_spec.lua`: PR 7b, 7c, 7d, 23.
- `lua/custom_api/herdr.lua`: PR 11 (creates it), 13 (extends it), 16 (reads it).
- `lua/plugins/claudecode.lua`, the `<leader>C` keymap file: PR 12, 11, 13, 16, 30c9.
- `lua/plugins/noice.lua`: PR 17c, 22b. `lua/config/autocmds.lua`: PR 4b, 20.
- `lua/plugins/overseer.lua`: PR 19a, 19b, 14, 30c3. `lua/plugins/autosave.lua`: PR 12, 24, 30c2.
- `lua/plugins/harpoon.lua`: PR 22a, 30c4. The xcodebuild spec: PR 3, 14. `lua/config/lazy.lua`:
  PR 4a, 30d.
- `dot_config/nvim/CLAUDE.md`: PR 2, 4d, then the registering PR (10a or 10b).
- `private_dot_codex/private_config.toml.tmpl`: the PR that lands it, then the registering PR.

Two rules keep that from being a pipeline gap. First, two PRs that touch the same file are serialized:
the later one waits for the earlier one to merge before it opens for review, and that edge is in its
**Depends on** cell (which is why PR 20 depends on PR 4b, PR 22b on PR 17c, PR 24 on PR 23, PR 16 on
PR 13, PR 13 on PR 11, PR 12 on PR 29a, and PR 10a on PR 4d). Second, every PR merges `main` into its
branch immediately before each review round
and re-runs its whole gate after the merge: the dump diff (3.7 check 5), the startup median against
the 9.1 gate, `just test-unit` and `just lint-check`. A review verdict on a branch that has not been
re-gated since its last merge of `main` is not a verdict.

## 12. Open questions, each with a proposed default

1. **neotest adapters.** Which adapters ship in PR 28. Default: Swift through the operator's own
   `webdavis/neotest-swift` (a SwiftPM adapter, pushed 2024-03-26) for Vapor packages, with
   `xcodebuild.nvim`'s own Test Explorer for Xcode projects; Rust through a `cargo test` adapter; no
   Python, Go or Lua adapters until a project needs one. Adapter repository names are verified at plan
   time, not here.
2. **nvim-dap for xcodebuild.nvim.** Default: no. It is not on the inventory; `codelldb` is already in
   Mason if this changes later.
3. **The MCP server choice.** Default: `linw1995/nvim-mcp` plus the resolver script when any of
   criteria 1 to 3 fail; the custom crate only on a criterion 4, 5 or 6 failure. The evaluation in
   PR 9 decides, by the 7.3 table, inside its one-day budget; PR 10a or PR 10b ships and registers.
4. **The custom server's name**, if built. Default `nvim-workspace-mcp`; confirm before creating it
   (rename rule). The same applies to the three module names in 7.7.
5. **Agent keymap prefix.** Default `<leader>C` "claude"; the implementer may pick another free letter
   under the section 8.2 rule, stating why.
6. **`go` via Homebrew for Mason's gopls.** Default: yes, it is what decision D requires to work; the
   alternative (`gopls` from Homebrew, outside Mason) contradicts D.
7. **The formatter exclusions.** Default: PR 2 excludes `dot_config/nvim/**` from mdformat (and from
   stylua only if Homebrew's stylua disagrees with the clean Mason check), and PR 4d lifts them with the
   rewrap as its own commit. The alternative, a permanent exclusion, would leave the editor's own
   markdown the only unformatted markdown in the repo.

## 13. Appendix A: inventory item to section and pull request

| Item | Subject                                       | Section     | Closed by |
| ---- | --------------------------------------------- | ----------- | --------- |
| 1    | #1 github.username nil                        | 4, 6.2      | PR 7d     |
| 2    | #2 `<C-g>bc` twice                            | 4           | PR 18     |
| 3    | #3 toggle_runner invalid action               | 4           | PR 19a    |
| 4    | #4 default_branch string vs table             | 4, 6.2      | PR 7c     |
| 5    | #4b string.format arity                       | 4, 6.2      | PR 7c     |
| 6    | #5 delegate.setup twice                       | 4, 7.1      | PR 8 (moot) |
| 7    | #6 duplicate checktime autocmd                | 4, 5.4      | PR 20     |
| 8    | #7 dial vs boole                              | 8.3         | STRUCK    |
| 9    | #8 parse_branch_line off-by-one               | 4, 6.3      | PR 7a     |
| 10   | #9 literal Esc in delegate                    | 4, 7.1      | PR 8 (moot) |
| 11   | #10 harpoon width at spec load                | 4           | PR 22a    |
| 12   | #11 hardcoded mkdp host                       | 4           | PR 21     |
| 13   | #12 mason-lspconfig servers dead              | 4           | PR 5a     |
| 14   | #13 none-ls 0.12 crashes                      | 1           | STRUCK    |
| 15   | #14 Overseer cleanup                          | 4           | PR 19b    |
| 16   | #15 hlslens pin                               | 4, 5.5      | PR 26b    |
| 17   | #16 catppuccin rename                         | 4, 5.5      | PR 26c    |
| 18   | #17 noice inc_rename                          | 4           | PR 22b    |
| 19   | helpers.wrap reflection                       | 4, 6.1      | PR 6      |
| 20   | error text vs param names                     | 4, 6.2      | PR 6      |
| 21   | TODO git-blame on_attach                      | 4, 5.2      | PR 23     |
| 22   | HACK noice pre-enable                         | 4           | none (informational) |
| 23   | drop cspell                                   | 5.1         | PR 17a    |
| 24   | drop gitmoji                                  | 5.1         | PR 17b    |
| 25   | drop nvim-notify                              | 5.1         | PR 17c    |
| 26   | drop gv.vim                                   | 5.1         | PR 17d    |
| 27   | drop git-messenger                            | 5.1         | PR 17e    |
| 28   | drop git-blame with remap (C)                 | 5.1, 5.2    | PR 23     |
| 29   | drop telescope                                | 5.1         | PR 24     |
| 30   | drop boole after dial augends                 | 5.1         | PR 25     |
| 31   | claudecode.nvim (A)                           | 5.3, 7.2    | PR 12     |
| 32   | fzf-lua only if chosen (B: snacks)            | 5.1         | PR 24 (not added) |
| 33   | nvim-surround ^4                              | 5.5         | PR 26a    |
| 34   | none-ls no forced bump                        | 1, 5.5      | PR 26a (recorded as no-op) |
| 35   | keep dial, markview, toggleterm               | 5.5         | resolved  |
| 36   | treesitter master to main                     | 5.5         | STRUCK; archival note and context check in PR 29b |
| 37   | gopls (D)                                     | 5.3         | PR 27     |
| 38   | conform / nvim-lint                           | 1           | deferred (non-goal) |
| 39   | vim-slime                                     | 7.4         | PR 11     |
| 40   | buffer auto-reload                            | 5.4         | PR 20     |
| 41   | claude-tmux.nvim                              | 0           | moot: tmux is gone; not added |
| 42   | auto-save formatting compat (F)               | 5.3         | PR 12     |
| 43   | xcodebuild.nvim (F, early)                    | 5.3, 10.7   | PR 3      |
| 44   | neotest (F)                                   | 5.3, 12.1   | PR 28     |
| 45   | alpha flatten, archive, rm nested .git        | 3.3, 3.6    | PR 2 (trash by the operator, after the guards) |
| 46   | beta lock tracked, checker off                | 3.5         | PR 2 (track), PR 4a (checker) |
| 47   | gamma lazyvim.json and augroups               | 3.5         | PR 4b     |
| 48   | delta lazy flip and triggers                  | 9           | PR 30a, 30b1, 30b2, 30c1 to 30c9, 30d |
| 49   | epsilon custom_api audit                      | 6           | superseded by 54 to 59 |
| 50   | zeta bootstrap                                | 3.9         | PR 31 (prep in PR 5b) |
| 51   | eta CLAUDE.md and .claude carve-out           | 3.5         | PR 2      |
| 52   | theta path-anchored ignores                   | 3.4         | PR 2      |
| 53   | stay on LazyVim                               | 3.5         | STRUCK; PR 4b removes the last scaffolding |
| 54   | boundary try()                                | 6.1         | PR 6      |
| 55   | inject the shell runner                       | 6.2         | PR 7b     |
| 56   | GitHub fallback into github                   | 6.2         | PR 7c     |
| 57   | separate map/overseer_runner                  | 6.2         | PR 7e     |
| 58   | latest_commit (table, err)                    | 6.2         | PR 6      |
| 59   | no side effects on require                    | 6.2         | PR 6      |
| 60   | copy_URL_to_clipboard rename (E)              | 6.2         | PR 7f     |
| 61   | delete delegate.lua                           | 7.1         | PR 8      |
| 62   | adopt nvim-mcp                                | 7.3         | PR 9 (evaluation), then PR 10a or 10b ships and registers, as the table says |
| 63   | CLAUDE.md edit rule, both harnesses           | 7.5         | the registering PR: 10a on the `nvim-mcp` rows, 10b on the crate rows |
| 64   | ACP hedge                                     | 7.6         | deferred, recorded here; no PR |
| 65   | flatten data-loss sequence                    | 3.1, 3.2, 3.6 | PR 2    |
| 66   | headless Lua test wired to just and bootstrap | 6.3         | PR 6 (harness), PR 31 (bootstrap) |
| 67   | pure-helper tests                             | 6.3         | PR 7a     |
| 68   | runtimepath bug                               | 2, 4        | PR 29b    |
| 69   | sequence                                      | 11          | PR 1 to PR 31, 62 rows |
| 70   | success criteria                              | 10          | PR 31     |
| 71   | checkhealth, startuptime, bugs live, lint     | 10          | every PR; lint from PR 1, health floor PR 29a and 29b |
| 72   | clean-home apply (full apply, not --exclude)  | 10.10       | PR 31     |
| 73   | manual agent-loop check                       | 10.8        | PR 10a or 10b (MCP half, the registering PR), PR 12 (send half) |
| 74   | SP6 directives                                | 0, 2, 3     | PR 2      |
| 75   | re-evaluate critically, bug list as floor     | 0, 4        | this spec |
| 76   | own effort (G)                                | 0           | this spec and its plan; no PR closes it |
| 77   | research workflow (claudecode provider none)  | 7.2, 5.4    | PR 12 (provider none, send, add), PR 13 (launch), PR 20 (auto-reload); the claude-tmux part is item 41, moot |
| 78   | research open questions                       | 7.2         | PR 12: the eight dispositions in the spec header comment |
| A    | agent channel: both                           | 7           | PR 9 then 10a or 10b (MCP), PR 12 (push) |
| B    | octo picker: snacks                           | 5.1         | PR 24     |
| C    | git-blame: remap                              | 5.2         | PR 23     |
| D    | gopls: add                                    | 5.3         | PR 27     |
| E    | rename                                        | 6.2         | PR 7f     |
| F    | neotest, auto-save, xcodebuild in scope       | 5.3         | PR 3, PR 12, PR 28 |
| G    | own program                                   | 0           | this spec |
| H    | import unchanged first                        | 3.7         | PR 2      |
| custom #1 | editor-side pns producer                 | 7.7         | PR 14     |
| custom #2 | review-ledger quickfix                   | 7.7         | PR 15     |
| custom #3 | agent-context sender                     | 7.7         | PR 16     |
| custom #4 | Neovim MCP server, evaluate then build   | 7.3         | PR 9 (evaluate), PR 10a (resolver or crate spec), PR 10b (crate build) |

Struck and staying struck: items 8, 14, 36 (as originally worded), 45's chezmoi.nvim autocmd
rationale, 48's "14 eager specs" premise, and 53.

## Appendix B: not to build

The operator's list of ideas that are NOT built, kept so nobody proposes them again: chezmoi
redirection (`chezmoi.nvim` exists) and GitHub URL helpers (`Snacks.gitbrowse` and the existing
`custom_api` cover them). Every proposed custom plugin is approved and appears in section 7.
