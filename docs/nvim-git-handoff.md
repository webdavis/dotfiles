# Neovim git setup: hand-off for the replacement project

Paths are relative to `dot_config/nvim/`. Short names:
`G` = `lua/plugins/git.lua`, `CG` = `lua/custom_api/git.lua`, `CH` = `lua/custom_api/github.lua`,
`SN` = `lua/plugins/snacks.lua`. Commit hashes are from `git log` on these files.

## 1. Feature inventory

Who provides each feature: **GS** gitsigns.nvim, **FG** vim-fugitive (+ vim-rhubarb for `GBrowse`),
**GL** gitlinker.nvim, **OC** octo.nvim, **AT** atlas.nvim, **SP** snacks.nvim picker,
**CU** custom Lua in `G`/`CG`/`CH`, **OV** overseer task (a background job with an output buffer).
"buf" means the key exists only in buffers where gitsigns attached.

### Signs, hunks, staging

| Key / command | Does | By |
|---|---|---|
| sign column, number highlight, staged signs | show changed lines (`G:112-131`) | GS |
| `]g` / `[g` (buf) | next / previous hunk, or `]c`/`[c` in diff mode (`G:137-165`) | GS |
| `ih` (o, x; buf) | select hunk text object (`G:416`) | GS |
| `<leader>ga` n/v (buf) | stage hunk or selected lines (`G:169-185`) | GS |
| `<leader>gu` (buf) | "unstage staged hunk", calls `stage_hunk`, which toggles (`G:229-235`) | GS |
| `<leader>gA` / `<leader>gU` (buf) | stage buffer / unstage buffer (`G:187,221`) | GS |
| `<leader>gr` n/v, `<leader>gR` (buf) | reset hunk or lines / reset buffer (`G:195-219`) | GS |
| `<leader>gp` / `<leader>gi` (buf) | preview hunk in float / inline (`G:237-251`) | GS |
| `<leader>gq` / `<leader>gQ` (buf) | hunks to quickfix, buffer / all files (`G:273-289`) | GS |
| `<C-g>a` | `:Gwrite`, stage whole file (`G:1005`) | FG |
| `<leader>uG` | toggle signs (`SN:359-371`) | SP toggle + GS |
| `<leader>gd` | picker of diff hunks (`SN:248`) | SP |
| null-ls `code_actions.gitsigns` | hunk actions in the LSP code-action menu (`lua/plugins/lsp.lua:338`) | GS |

### Status

| Key | Does | By |
|---|---|---|
| `<C-g>ss` | `:Git` status window (`G:1001`) | FG |
| `<C-g>sn` | `:Git status -sb` (`G:1002`) | FG |
| `<leader>gs` | status picker (`SN:252`) | SP |
| lualine `branch`, `diff` | branch and change counts in statusline (`lua/plugins/ui.lua:119`) | lualine |
| oil columns | git status in the oil file browser (`lua/plugins/oil.lua:53-60`) | oil-git.nvim |

### Diff

| Key | Does | By |
|---|---|---|
| `<C-g>dhd` (buf) | vertical diff against `~1` (`G:292-301`) | GS |
| `<C-g>did` (buf) | vertical diff against index (`G:304-312`) | GS |
| `<C-g>dis` (buf) | show index version of file (`G:314`) | GS |
| `<C-g>dw` / `<C-g>dl` (buf) | toggle word diff / line highlight (`G:322-336`) | GS |
| `<C-g>dhf` / `<C-g>dhF` | `git diff -p --stat --function-context`, vertical / horizontal (`G:1429-1441`) | FG |
| `<C-g>dhw` / `<C-g>dhm` | `git diff --color-words` / `--color-moved` (`G:1443-1477`) | OV |
| `<C-g>dic` / `<C-g>diC` | `git diff --cached -U0`, all / current file (`G:1480-1492`) | FG |
| `<C-g>dif` / `<C-g>diF` | cached diff with function context, vertical / horizontal (`G:1494-1506`) | FG |
| `<leader>g<` | picker of files changed in last commit (`SN:456-482`) | SP + CU |
| `<leader>gu` (global) | picker of files changed vs upstream (`SN:466-488`) | SP + CU |

### Blame

| Key | Does | By |
|---|---|---|
| current-line blame virtual text, on by default (`G:131`) | | GS |
| `<C-g>Bt` (buf) | toggle current-line blame (`G:359`) | GS |
| `<leader>gB` (buf), `<leader>gm` (global) | full blame float for the line (`G:253-261`, `G:430-437`) | GS |
| `<leader>gM` (global) | blame split; `r`/`R` walk commits; focuses an open blame instead of opening a second (`G:439-536`) | GS + CU |
| `<leader>gc` (buf) | show commit of the line (`G:263`) | GS |
| `<C-g>By` (buf) | copy line's commit SHA (`G:367-381`, `CG:384-418`) | CU |
| `<C-g>Bo` / `<C-g>BO` (buf) | open / copy GitHub commit URL for the line (`G:383-412`, `CH:115-126`) | CU |
| `q` closes `gitsigns-blame` windows (`lua/config/autocmds.lua:93`) | | config |

### Commit

| Key | Does | By |
|---|---|---|
| `<C-g>cc` | `Git commit --verbose` (`G:1211`) | FG |
| `<C-g>cf` | commit current file only (`G:1212`) | FG |
| `<C-g>ca` | amend, edit message (`G:1213`) | FG |
| `<C-g>cn` | amend, no edit, in background (`G:1214`) | FG |
| `<C-g>cA` | amend author and email via two prompts (`G:1237-1294`) | CU + FG |
| `<C-g>cp` | paste last commit message into buffer (`G:1217-1235`) | CU |
| `<C-g>!` | stage file, amend no-edit, then `push --force` (`G:1008-1035`) | CU + FG |

### Log and history

| Key | Does | By |
|---|---|---|
| `<C-g>lo` / `<C-g>lO` / `<C-g>lc` | `log --oneline`, all / current file / current file again (`G:1298-1300`) | FG |
| `<C-g>ll` / `<C-g>lL` | `git log`, all / current file (`G:1302-1303`) | FG |
| `<C-g>lso` / `<C-g>lsl` | log of `origin/main..HEAD`, oneline / full (`G:1304-1305`) | FG |
| `<C-g>lr` / `<C-g>lsr` | coloured pretty log, all / `origin/main..HEAD` (`G:1308-1388`) | OV |
| `<C-g>lw` / `<C-g>lW` | my commits since Monday, Fugitive / coloured in Overseer (`G:1319-1377`) | CU |
| `<C-g>lp` | prompt for N, `log --graph` of N commits (`G:1390-1426`) | CU + FG |
| `<C-g>ww` / `<C-g>wb` / `<C-g>wc` | `git whatchanged`, all / file / `--since=` typed by user (`G:1552-1570`) | FG |
| `<leader>gl` / `<leader>gf` / `<leader>gL` | log picker: repo / file / line (`SN:249-251`) | SP |

### Branches and checkout

| Key | Does | By |
|---|---|---|
| `<C-g>bb` / `bV` / `bR` / `bA` | `git branch` in four verbosity levels (`G:1190-1193`) | FG |
| `<C-g>bc` | notification: branch, upstream, last commit; copies hash (`G:650-702`, `G:1195`) | CU |
| `<C-g>bv` | notification listing all local branches (`G:715-797`, `G:1202`) | CU |
| `<C-g>Cb` | prompt, `checkout -b <name>` (`G:1121-1183`) | CU + FG |
| `<C-g>C-` / `<C-g>Cm` | checkout previous / `main` (`G:1185-1186`) | FG |
| `<leader>gb` | branch picker (`SN:247`) | SP |

### Stash

| Key | Does | By |
|---|---|---|
| `<C-g>Sd` / `Se` / `Sw` / `SW` / `Ss` | stash push variants (`G:1038-1071`) | FG |
| `<C-g>Sp` / `<C-g>SP` | pop latest / pop by typed index (`G:1074-1091`) | FG |
| `<C-g>Sa` / `<C-g>SA` | apply latest / apply by typed index (`G:1094-1111`) | FG |
| `<leader>gS` | stash picker (`SN:253`) | SP |

### Remotes: fetch, pull, push

| Key | Does | By |
|---|---|---|
| `<C-g>Ff` / `Fp` / `Fr` | fetch / pull / pull --rebase, in background with `:Git!` (`G:1509-1511`) | FG |
| `<C-g>pp` / `<C-g>pf` | push / push --force-with-lease, background (`G:1515-1516`) | FG |
| `<C-g>pu` | confirm prompt, `push -u origin HEAD` (`G:1523-1549`) | CU + FG |
| `<C-g>rh` / `rH` / `rs` / `rS` | copy origin/upstream remote URL as HTTPS/SSH (`G:48-90`, `G:1114-1117`, `CG:253-323`) | CU |
| `<C-g>i` | `git init` if needed, then `gh repo create --public <name>` (`G:933-998`) | CU + OV |

### Browse and links

| Key | Does | By |
|---|---|---|
| `<C-g>of` / `<C-g>ol` | `:GBrowse` file / line (`G:1573-1574`) | FG + rhubarb |
| `<C-g>oL` (n, v) | `:GitLink!` open line/range in browser (`G:1595`) | GL |
| `<C-g>y` (n, v) | `:GitLink` copy permalink (`G:1594`) | GL |
| `<leader>go` (n, v) | open file on GitHub (`SN:246`) | SP (`Snacks.gitbrowse`) |
| `<C-g>oo` `ob` `oi` `op` `oa` `oP` `ow` `oS` `oI` `os` | open repo home, branch, issues, pulls, actions, projects, wiki, security, pulse, settings (`G:831-929`, `G:1576-1585`) | CU |

### GitHub issues, PRs, gists, runs

| Key / command | Does | By |
|---|---|---|
| `<leader>ghg` `ghi` `ghI` `ghm` `ghn` `ghp` `ghP` `ghr` `ghw` | Octo: gists, issues, new issue, merge PR, notifications, PRs, new PR, repos, workflow runs (`G:1613-1623`) | OC |
| picker keys `<C-b>` `<C-y>` `<C-e>` `<C-o>` `<C-r>` | open, copy URL, copy SHA, checkout PR, merge PR (`G:1634-1640`) | OC |
| `<localleader>` a c i g l p r v groups in `octo` buffers (`G:1650-1665`) | which-key labels only | OC + which-key |
| `<leader>ghfi` `ghfI` `ghfp` `ghfP` | issue / PR pickers, open or all (`SN:256-259`) | SP (`gh` source) |
| `<leader>gtt` `gtp` `gtP` `gti` `gtI` `gtr` `gtn` `gts` | Atlas: commands, PRs, new PR, issues, new issue, review, notes, search (`lua/plugins/atlas.lua:20-29`) | AT |
| issue/PR completion in `octo`, `gitcommit`, `markdown` (`lua/plugins/blink-cmp.lua:231-235`) | | blink-cmp-git |

### Files and grep

| Key | Does | By |
|---|---|---|
| `<C-g><C-f>`, `<leader>g,` | git files picker (`SN:216-217`) | SP |
| `<C-g>.`, `<leader>g.`, `<leader>/g` | git grep picker (`SN:218-219,237`) | SP |
| `:G`, `:Git`, `:Gedit`, `:Gread`/`:Gr`, `:Gdiffsplit`, `:GBrowse`, etc. | all Fugitive commands (`G:550`, `G:609-634`) | FG |

## 2. Issues

### Crashes and wrong results (verified by reading the code)

| Where | Issue |
|---|---|
| `G:852`, `G:1129-1132`, `G:1393`, `G:1526` | `git.current_branch()` returns `nil, message` outside a repo (`CG:163-164`), and these read `.name` off it unguarded, so the key raises a Lua error. `G:687-690` is the one caller that checks. |
| `CG:274` | The "set the remote" hint prints `git@github.com/<acct>/<repo>.git`; SSH syntax needs `git@github.com:`. |
| `G:69-71`, `G:74-82` | The error message from `git.url` and the warning from `git.copy_url_to_clipboard` are both dropped; the user sees only "Nothing copied" or nothing at all. |
| `G:697`, `G:1225`, `G:1246`, `G:1451`, `G:1469` | `git.latest_commit(...) or {}` drops its error message, so a failure exits silently. |
| `G:1099` | `<C-g>SA` (apply) prompts "Stash index to pop". |
| `G:1443-1477`, `G:1429-1441` | Keys sit in the "HEAD (latest commit)" group (`lua/plugins/which-key.lua:17`) but run `git diff` with no revision, which compares worktree to index, not HEAD. |
| `G:229-235` | `<leader>gu` "Unstage Staged Hunk" calls `stage_hunk`, the same function as `<leader>ga`. It relies on gitsigns making `stage_hunk` a toggle, after `undo_stage_hunk` was deprecated (commit `9f1a1b6fb`). |
| `G:1028` | `<C-g>!` pushes with `--force`, while `<C-g>pf` uses `--force-with-lease` (`G:1516`). |
| `G:984-992` | `<C-g>i` runs `gh repo create --public <name>` with no `--source`/`--remote`/`--push`, so the new GitHub repo is not linked to the local one. (Read from args; not run.) |
| `G:1336-1343` | `<C-g>lw` splices `user.name` into an ex command line inside single quotes. Commit `a596da926` removed this pattern elsewhere. How Fugitive splits it was not verified. |
| `CH:14` | `_ = opts or {}` writes the global `_`. |
| `CG:36-38`, `G:936` | `initialized` ignores its `opts`, so `{ quiet = true }` does nothing. |
| `CG:47-61` | `top_level` reads `opts.full_path` with no `opts or {}`; it raises when called with no argument. It has no callers (grep). |
| `SN:485` vs `G:231` | `<leader>gu` is a global snacks picker and a buffer-local gitsigns key. In every git file buffer the gitsigns one wins and the picker is unreachable. `tests/plugins_keymap_collisions_spec.lua:5-10` scans only `keys` lists and `keymaps.lua`, so it does not see `map()` calls in `on_attach` or snacks `config`. |

### Freezes and blocking calls

- `util.run_shell_command` waits for the process (`lua/custom_api/util.lua:77`, `:wait()`; `:86`, `vim.fn.system`). Every `CG`/`CH` call blocks the editor.
- `<C-g>o*` runs, per press: `git rev-parse`, `gh repo view` (network), maybe `git show-ref` x2 and `gh api` (network), then a `python3` + `osascript` pipeline only to print the browser name (`G:808-829`, `G:914`).
- `<C-g>r*` copy-URL runs `git rev-parse`, up to four calls in `github.account` (two of them `gh api`, `CH:16-33`), `gh repo view`, and `git config` (`G:52-67`). The username and repo name are only needed for the error hint at `CG:274`.
- `<C-g>dhw`/`<C-g>dhm` call `gh repo view` and two git commands as a guard, then run an unrelated `git diff` (`G:1446-1455`, `G:1464-1473`).
- `<C-g>Cb` and `<C-g>cA` call `FugitiveExecute` without a callback, which waits (`G:1168`, `G:1273`).
- History: `:Git` freezes the editor until git exits; `62f872339` moved fetch/pull/push to `:Git!`, and `1873893ab` reworked `<C-g>!` for the same reason.

### Race conditions

- `<C-g>!`: a buffer switch during the amend's pre-commit hook sent the push to another repo. Fixed by pinning the buffer with `nvim_buf_call` (`G:1011-1031`, commit `3dad2dc6a`). Any async chain in the new project needs the repo captured at key press.
- Repo is resolved two ways. Blame uses the buffer's directory (`CG:411`, `CH:120`); everything else uses Neovim's cwd (`CH:45-50` documents the mismatch). In a buffer from another repo, `<C-g>o*`, `<C-g>r*`, `<C-g>bc`, `<C-g>cp` answer for the cwd repo.

### Code that depends on plugin internals or deprecated APIs

| Where | Depends on |
|---|---|
| `G:439-512` | gitsigns blame buffer name format `gitsigns-blame://<gitdir>//<rev>:<path>`, `b:gitsigns_status_dict`, and a bug in the pinned gitsigns (opens split before naming it, E95). Comment cites `actions/blame.lua` at commit `130beacf`. |
| `G:478-483` | Fugitive URL format `fugitive://...` and `FugitiveParse`. |
| `G:99-104` | lazy.nvim internals (`lazy/core/handler/keys.lua`, `_add`/`_set`) so rhs-less key rows keep a which-key label. |
| `G:546-636` | lazy.nvim placeholder command behaviour vs Fugitive's `-bar`/`-range=-1` declarations; `:Gr` re-declared by hand to mirror Fugitive (`G:561-589`) and calls `fugitive#ReadComplete`. Loading on `CmdlineEnter` (`G:606`) means Fugitive loads on the first `:` press. |
| `G:1016`, `G:1168`, `G:1273` | `FugitiveExecute`, `FugitiveDidChange` (Fugitive's public but Vimscript-only API). |
| `G:1553-1568` | `git whatchanged --i-still-use-this`; git deprecated `whatchanged` and requires that flag to run it. |
| `SN:366` | reads `package.loaded["gitsigns.config"].config.signcolumn` to avoid loading gitsigns. |
| `SN:432-445` | `require("snacks.picker.source.proc").proc`, a snacks internal module. |
| `G:144`, `G:159`, `G:296` | `---@diagnostic disable` for gitsigns type mismatches. |

### Repeated breakage in history

| Area | Fix commits |
|---|---|
| `<leader>gM` blame-window lookup | `d7abd0b05`, `c795f2366`, `3198bc65a`, `b1564ae72`, `54e403414`, `b792197a4` (6 fixes, 2026-09-04/05) |
| `blame_sha` correctness | `095bad12f` (buffer text), `21a9669c2` (exact bytes), `2475a033e`, `e1ef388a4` (mac/binary line ends), `6f7febfeb` (symlinks), `9c74e1104` (zero SHA), `0316787ec`, `f4db0b7f2`, `c4c54de74` (which repo) |
| Fugitive lazy-loading | `b8686c426`, `9d73bda47`, `ec6158005`, `809b4b539`, `23a738387` (completion recursion segfault), `5d7853e86` (`:0Gr`) |
| gitsigns lazy-loading | `d8fdbe3da`, `2061b9d84` |
| `gh` failure handling | `7227b34a8` (10 unguarded dereferences), `e7b480ddb`, `0fbb4f1f7`, `023b315d2`, `2dbac8d68` |
| Shell injection / quoting | `a5ebaa895`, `a596da926`, `374a3194e` |
| Editor freezes / ordering | `62f872339`, `1873893ab`, `3dad2dc6a` |
| Removed plugins | `59851e2cb` gv.vim, `4bfcd48ae` git-messenger, `4d4648bf3` git-blame.nvim, `02a54acc8` telescope |

## 3. Bloat

### Same job, several providers

| Job | Providers |
|---|---|
| Open file/line on GitHub | `<C-g>of`/`ol` rhubarb, `<C-g>oL` gitlinker, `<leader>go` snacks (`G:1573-1574`, `G:1595`, `SN:246`) |
| GitHub client | octo `<leader>gh*`, atlas `<leader>gt*`, snacks `<leader>ghf*` (`G:1599-1667`, `atlas.lua:1-4` says it is a side-by-side trial, `SN:256-259`) |
| Blame float | `<leader>gB` and `<leader>gm` call the same function (`G:253-261`, `G:430-437`; `<leader>gm` kept for git-messenger muscle memory, `4bfcd48ae`) |
| Line commit SHA | custom `git blame` subprocess (`CG:325-418`) while gitsigns already blames the line for virtual text (`G:131`). Gitsigns exposes `b:gitsigns_blame_line_dict` per its docs; not checked at the pinned commit. |
| Status | `<C-g>ss` Fugitive, `<leader>gs` snacks, oil-git, lualine |
| Log | 14 `<C-g>l*`/`<C-g>w*` keys plus 3 snacks log pickers |
| Stage file | `<C-g>a` `:Gwrite` and `<leader>gA` `stage_buffer` |
| Stash | 9 `<C-g>S*` keys and `<leader>gS` picker |
| Branches | 6 `<C-g>b*` keys and `<leader>gb` picker |
| Diff against index | `<C-g>did` gitsigns, `<C-g>dic`/`dif`/`diF` Fugitive, `<C-g>dhw`/`dhm` Overseer |
| Default branch | `git.default_branch` (`CG:239-251`) + `github.default_branch` (`CH:79-106`) for `<C-g>ob`, while `<C-g>lso`/`lsl`/`lsr`/`Cm` hardcode `main` (`G:1186`, `G:1304-1305`, `G:1383`) |

### Exact or near duplicates in code

- `<C-g>lO` and `<C-g>lc` are the same command (`G:1299-1300`).
- `<C-g>SP` and `<C-g>SA` bodies differ by one word (`G:1076-1111`).
- `<C-g>lw` and `<C-g>lW` share their arg list (`G:1319-1377`); `<C-g>lr` and `<C-g>lsr` share their format string (`G:1312`, `G:1383`).
- Two branch-formatting systems: `format_section`/`build_sections`/`sections_to_message` (`G:638-684`) and `format_branch_field`/`format_branch`/`group_branches`/`build_notification` (`G:715-785`).
- The `yes_values` table is copied (`G:962`, `G:1533`).
- `account_or_notify`/`repo_name_or_notify` exist (`G:22-46`) but `<C-g>i` and `build_remote_repo_info` repeat the `try` call inline (`G:838-843`, `G:942-947`).
- `which-key` descriptions repeat: `lo` and `lso` both "Fugitive: oneline", `lr` and `lsr` both "Overseer: pretty (relative time)", `dhd` and `did` both "Gitsigns: side-by-side" (`G:1298-1388`, `G:299,310`).

### Dead or unused

- `git.top_level` has no callers (`CG:47-61`, `CG:430`).
- `opts` params ignored in `initialized` and `account` (`CG:37`, `CH:14`).
- `return 1` from a keymap callback (`G:1329`, `G:1359`).
- Octo `use_emojis` is commented as fzf-lua only, while `picker = "snacks"` (`G:1630-1633`).
- Catppuccin `neogit = true` (`lua/plugins/ui.lua:56`); neogit is not in `lazy-lock.json`.
- Autosave exclusion for filetype `lazygit` (`lua/plugins/autosave.lua:9`); lazygit is not configured anywhere in `lua/`.
- The octo spec is wrapped in an extra table (`G:1598-1599`, `G:1667-1668`).

### Oversized pieces

- `G` is 1,669 lines; the Fugitive `keys` function alone is `G:637-1587` (950 lines), holding formatting helpers, a browser detector and 70+ key rows.
- Comment blocks longer than the code they describe (`G:439-522`, `G:546-636`).
- `tests/plugins_git_spec.lua` is 789 lines, mostly fakes of gitsigns and Fugitive (`tests/plugins_git_spec.lua:51-110`, `:429-525`).

### Glue that only adapts one plugin to another

- `<leader>gM` window search (`G:439-536`): works around a gitsigns E95 bug and parses Fugitive URLs.
- `:Gr` shim and the 24-name `CmdUndefined` list (`G:561-635`): keeps Fugitive lazy-loadable under lazy.nvim.
- rhs-less gitsigns `keys` rows (`G:105-108`): keep which-key labels before gitsigns loads.
- snacks signs toggle reading gitsigns internals (`SN:359-371`) and its own 160-line test (`tests/plugins_snacks_gitsigns_toggle_spec.lua`).
- `get_default_browser` (`G:808-829`) exists only to print a browser name.

## 4. Dependencies to keep or replace

### External tools

| Tool | Used for | Where |
|---|---|---|
| `git` | everything; `blame --contents -`, `branch -vv` parsing, `show-ref`, `config --get` | `CG` |
| `git whatchanged` with `--i-still-use-this` | `<C-g>w*` | `G:1553-1568` |
| `gh` (logged in) | `repo view`, `api user`, `api repos/...`, `repo create`; also octo, atlas, snacks `gh_*`, blink-cmp-git | `CH`, `G:973,990` |
| git config `user.name`, `github.username` | account lookup, falls back to `gh api user` | `CH:16-33` |
| macOS `open`, `python3`, `osascript`, LaunchServices plist | `<C-g>o*` open and browser name | `G:810-822`, `G:922` |
| system clipboard | `util.copy_to_system_clipboard` | `G:376`, `CG:312` |

Minimum git or `gh` versions are not stated anywhere in the repo (not verified).

### Plugins (pinned in `lazy-lock.json`)

gitsigns.nvim `fd780920`, vim-fugitive `3b753cf8`, vim-rhubarb `5496d7c9`, gitlinker.nvim `a1b74070`,
octo.nvim `af241160`, atlas.nvim `5aa47cd3` (also pinned by `commit` in `atlas.lua:11`), oil-git.nvim `95b0d83f`,
blink-cmp-git `d3f2a906`.

### Other config the git code talks to

| Piece | How | Where |
|---|---|---|
| `_G.map` (`custom_api.keymap`) | defines every non-`keys` mapping | `init.lua:24` |
| `custom_api.try` | turns `nil, message` and errors into notifications | `lua/custom_api/try.lua` |
| `custom_api.util` | shell runner, trim, clipboard, `file_dir`, `sanitize_input` | `lua/custom_api/util.lua` |
| `custom_api.overseer` + overseer.nvim | runs coloured log/diff and `<C-g>i` as tasks | `lua/custom_api/overseer.lua` |
| snacks.nvim | octo/atlas picker, `vim.notify` UI, git pickers, signs toggle | `G:1630`, `SN` |
| which-key | `<C-g>` group labels; octo `<localleader>` groups | `lua/plugins/which-key.lua:11-27`, `G:1653-1663` |
| lualine | `branch`, `diff` components and `fugitive` extension | `lua/plugins/ui.lua:119,182` |
| catppuccin | `gitsigns`, `octo` integrations | `lua/plugins/ui.lua:44,61` |
| none-ls | `code_actions.gitsigns` | `lua/plugins/lsp.lua:338` |
| autosave | must keep writing `acwrite` buffers of octo (pushes to GitHub) and gitsigns (stages a hunk) | `lua/custom_api/autosave.lua:16-18`, `tests/autosave_spec.lua:7,70-75` |
| close-with-`q` autocmd | `gitsigns-blame` filetype | `lua/config/autocmds.lua:93` |
| tests | `tests/git_spec.lua`, `tests/github_spec.lua`, `tests/plugins_git_spec.lua`, `tests/plugins_snacks_gitsigns_toggle_spec.lua`, `tests/dump_state.lua:134-144` (waits for gitsigns `]g`) | |

No git code in `lua/plugins/herdr-nvim.lua` (grep found none).

## 5. Owner decisions

- **vim-rhubarb goes.** The owner finds it clunky, and the new project should do its jobs better. It provides two
  things today: `:GBrowse` for GitHub remotes (`<C-g>of`/`<C-g>ol` open the file or line; `:GBrowse!` copies the
  URL instead), and completion of GitHub issues and `@users` in commit messages through its `omnifunc`, which
  blink.cmp's `omni` source shows (`lua/plugins/blink-cmp.lua`).
