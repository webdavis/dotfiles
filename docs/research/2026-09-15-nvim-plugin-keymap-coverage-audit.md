# Neovim plugin keymap coverage audit, 2026-09-15

The operator's request: go through every installed plugin, find the useful capabilities that have no key,
and find the keys that sit somewhere they should not.

This document decides nothing. Adding a keymap nobody asked for is the unrequested feature this
repository's rules exist to stop, and a key is muscle memory, so every row below is a proposal with a
recommended key and a reason, and the operator picks. No mapping was added, moved or renamed in the
change that carries this file.

## How the surfaces were established

All 49 specs in `dot_config/nvim/lua/plugins/` were read, plus `dot_config/nvim/lua/config/keymaps.lua`
for the global mappings and `plugins/which-key.lua` for the group structure. Every recommendation names a
command or a Lua function read out of the installed plugin under `~/.local/share/nvim/lazy/`, not out of
memory. Three recommendations were dropped during that reading because the capability turned out to be
mapped already.

The live mapping set was enumerated from a real session rather than from the specs, because lazy.nvim
placeholders, `UIEnter` and `VeryLazy` all install keys the specs do not show side by side:

```bash
DUMP_OUT=/tmp/livemaps.tsv script -q /dev/null nvim \
  -c "lua vim.api.nvim_create_autocmd('User',{pattern='VeryLazy',once=true,callback=function() \
       vim.defer_fn(function() dofile('/tmp/dumpmaps.lua'); vim.cmd('qa!') end, 3000) end})"
```

`dumpmaps.lua` walks `vim.api.nvim_get_keymap` over the eight modes `n v x s o i c t` and writes mode,
left-hand side and description to `DUMP_OUT`. It has to run from inside the `VeryLazy` autocmd: a
`vim.wait` in a `-c` command blocks before `VimEnter`, which is exactly where lazy.nvim parks its
`very_lazy` loader, so an early dump misses roughly a third of the config. The early dump returns 822
mappings and the full one 1274, and the 452 in the gap are where two of the conflicts below live.

Every recommended key was checked against that 1274-row set before it was written down.

## Which plugins correctly have no `keys` table

Sixteen specs declare no `keys`. Ten of them are right to.

| Plugin                           | Why a leader key would be wrong                                                                                                                                                 |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `blink-cmp`                      | A completion engine. Its surface is insert-mode and command-line keys it installs from `keymap = { preset = "default" }`, and the menu only exists while it is open.            |
| `dial`                           | It owns `<C-a>` and `<C-x>`, set in its own `config`. Increment is a Vim operator, reached the way every operator is.                                                           |
| `init` (`vim-rsi`, `vim-repeat`) | Two behavior libraries. `vim-rsi` adds readline keys to insert and command-line mode itself; `vim-repeat` extends `.`. Neither has a command.                                   |
| `lazydev`                        | A Lua language server library. It feeds types to the server on `ft = "lua"` and has nothing to invoke.                                                                          |
| `lsp`                            | No mapping of its own on purpose: Neovim 0.11 ships `grn`, `gra`, `grr`, `gri` and `gO`, which `which-key` labels as the `gr` group, and the `<leader>l` group covers the rest. |
| `mini-move`                      | Owns `<M-h j k l>` in normal and visual mode from its own defaults. Confirmed live.                                                                                             |
| `nvim-lint`                      | Driven by `BufReadPost`, `BufWritePost` and `InsertLeave`. A manual re-lint key would duplicate a write.                                                                        |
| `pns`                            | A library the producers call. `lazy = true` with no trigger of its own is deliberate.                                                                                           |
| `quickscope`                     | Highlights on `f`, `F`, `t` and `T` as the cursor moves. Its one stateful thing, the on/off switch, already has `<leader>uQ`.                                                   |
| `ts-comments`                    | Supplies `commentstring` per language to the built-in `gc`. Nothing to call.                                                                                                    |
| `unimpaired`                     | Ships the whole `[` and `]` family itself, 40-odd pairs, all present in the live set.                                                                                           |

The other six do have something unreachable, and they appear in the gap list below: `chezmoi`
(`:ChezmoiList`), `droast` (`:DroastQuickfix`), `hlslens` (`:HlSearchLensToggle`), `oil`
(`toggle_float`), `ui` (the whole of bufferline), and `textobjects`, whose problem is a conflict rather
than a gap.

`hlslens` is the marginal one. Its toggle command has no key, but `<leader>u` already carries 27 toggles
and none of the letters left mean "search lens" to a hand reaching for it. It is listed here rather than
below because a key that has to be memorized from a table is worse than typing the command twice a year.

## Gaps: a capability with no key

Strongest first. Every key in the last column was checked against the live set and is free.

| #   | Plugin                                           | Unreachable today                               | Exact call                                                   | Key                                                                 | Why it earns one                                                                                                                                                                                                                                                                                |
| --- | ------------------------------------------------ | ----------------------------------------------- | ------------------------------------------------------------ | ------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `bufferline.nvim`                                | Everything. The spec declares no `keys` at all. | `:BufferLinePick`                                            | `<leader>bp`                                                        | Pick puts a letter on every visible tab and jumps to the one you press. `]b` walks the list one step at a time, which is slower once more than three buffers are open.                                                                                                                          |
| 2   | `bufferline.nvim`                                | Pinning                                         | `:BufferLineTogglePin`                                       | `<leader>bP`                                                        | Pinning is what keeps the file under review from scrolling off the tabline during a long session.                                                                                                                                                                                               |
| 3   | `bufferline.nvim`                                | Closing the rest                                | `:BufferLineCloseOthers`                                     | `<leader>bo`                                                        | `<leader>bd` deletes one buffer. After an hour of jumping, the wanted action is the other one.                                                                                                                                                                                                  |
| 4   | `neotest`                                        | Re-run the last position                        | `require("neotest").run.run_last()`                          | `<leader>tl`                                                        | The red-green loop is edit, re-run the same test, edit. `<leader>tt` re-runs whatever is under the cursor, which is the wrong test as soon as the cursor moves to the fix.                                                                                                                      |
| 5   | `neotest`                                        | Run under the debugger                          | `require("neotest").run.run({ strategy = "dap" })`           | `<leader>td`                                                        | `nvim-dap` and `nvim-dap-ui` are both installed and the `<leader>D` group is fully wired, so the debugger is one argument away and has no route in.                                                                                                                                             |
| 6   | `ReviewLedger` (this repo, `config/keymaps.lua`) | The command itself                              | `:ReviewLedger`                                              | `<leader>Xr`                                                        | Its own file builds an awk program, a quickfix list and a `copen` for it, and then nothing calls it. The `<leader>X` group is already "diagnostics／quickfix" and `Xr` is free.                                                                                                                 |
| 7   | `neotest`                                        | Watch mode                                      | `require("neotest").watch.toggle()`                          | `<leader>tw`                                                        | Watch re-runs on save, which turns the loop in row 4 into no keystroke at all.                                                                                                                                                                                                                  |
| 8   | `neotest`                                        | The persistent output panel                     | `require("neotest").output_panel.toggle()`                   | `<leader>tp`                                                        | `<leader>to` opens a float over one result and closes. The panel is where a run with several failures is read.                                                                                                                                                                                  |
| 9   | `chezmoi.nvim`                                   | `:ChezmoiList`                                  | Add `"ChezmoiList"` to the spec's `cmd` table                | none                                                                | Not a missing key, a missing lazy trigger. The plugin registers two commands and the spec names one, so `:ChezmoiList` typed at the command line reaches nothing until something else loads the plugin. Confirmed against the live command list, which has `ChezmoiEdit` and not `ChezmoiList`. |
| 10  | `claudecode.nvim`                                | Model selection                                 | `:ClaudeCodeSelectModel`                                     | `<leader>Cm`                                                        | Switching model mid-session is a decision the operator makes often, by the model-switch event pns already reports on.                                                                                                                                                                           |
| 11  | `claudecode.nvim`                                | Focus the agent pane                            | `:ClaudeCodeFocus`                                           | `<leader>Cf`                                                        | `<leader>Cc` launches or attaches. Once attached, getting back to the pane goes through herdr navigation instead.                                                                                                                                                                               |
| 12  | `gitsigns`                                       | Every hunk in the repository as a list          | `:Gitsigns setqflist all`                                    | `<leader>gq`                                                        | `[g` and `]g` walk the hunks in one buffer. Nothing collects the ones in the other twelve files of a change.                                                                                                                                                                                    |
| 13  | `trouble.nvim`                                   | The call hierarchy                              | `:Trouble lsp_incoming_calls`                                | `<leader>lc`                                                        | Four Trouble modes are mapped and the two call-hierarchy modes are not, and "who calls this" is the question a reader of unfamiliar code asks first.                                                                                                                                            |
| 14  | `trouble.nvim`                                   | The other direction                             | `:Trouble lsp_outgoing_calls`                                | `<leader>lC`                                                        | Pairs with row 13 on the case convention the `<leader>l` group already uses for `lD` against `ld`.                                                                                                                                                                                              |
| 15  | `aerial.nvim`                                    | The floating nav window                         | `:AerialNavToggle`                                           | `<leader>an`                                                        | Eight aerial keys drive the sidebar and none reaches the nav window, which is the two-column browser meant for moving through a file rather than looking at it.                                                                                                                                 |
| 16  | `oil.nvim`                                       | The floating explorer                           | `require("oil").toggle_float()`                              | `<leader>eo`                                                        | `-` and `<leader>ef` both replace the current window. The float leaves the buffer you are working in visible underneath.                                                                                                                                                                        |
| 17  | `claudecode.nvim`                                | Add the file under the cursor                   | `:ClaudeCodeTreeAdd`                                         | `<leader>Ct`                                                        | `<leader>Ca` adds the current file. From an oil listing, the wanted file is the one on the cursor line, and `oil.nvim` is already a declared integration in the spec.                                                                                                                           |
| 18  | `conform.nvim`                                   | Which formatters apply here                     | `:ConformInfo`                                               | `<leader>ci`                                                        | Four keys turn autoformat on and off. None answers what would run, which is the question asked when a write does not reformat.                                                                                                                                                                  |
| 19  | `harpoon`                                        | Remove the current file from the list           | `require("harpoon"):list():remove()`                         | `<leader>hd`                                                        | `<leader>ha` adds and nothing subtracts, so a stale mark can only be cleared by opening the menu and editing it as text.                                                                                                                                                                        |
| 20  | `smart-splits.nvim`                              | Multiplexer-aware resize                        | `require("smart-splits").resize_left` and its three siblings | re-point the existing `<C-Left>`, `<C-Right>`, `<C-Up>`, `<C-Down>` | This config went to real trouble to make `<C-h j k l>` cross the editor and herdr seamlessly, and then resizing stops dead at the editor border, because those four arrows run plain `:resize` from `config/keymaps.lua`. No new key, one changed right-hand side each.                         |
| 21  | `codesnap.nvim`                                  | The text snapshot                               | `:CodeSnapASCII`                                             | `<leader>ca` in visual mode                                         | Two keys produce images. The plain-text version is the one that can be pasted into a pull request body or a commit message.                                                                                                                                                                     |
| 22  | `droast.nvim`                                    | Findings as a list                              | `:DroastQuickfix`                                            | `<leader>Xd`                                                        | Linting on save publishes signs. The quickfix form is how a Dockerfile with eight findings gets worked through.                                                                                                                                                                                 |

`smart-splits` also exports `swap_buf_left` and its three siblings, and no key is recommended for them.
`<M-h j k l>` belongs to `mini.move`, `<leader>w` is a which-key proxy onto `<C-w>` so every letter under
it is already spoken for, and native `<C-w>x` covers the within-editor case. A multiplexer-aware buffer
swap would need a key invented for it, so it is the operator's call whether the capability is wanted at
all.

## Placement: mappings that exist on a key that does not fit

Strongest first. The first one is a defect; the rest are questions of consistency.

### 1. `markdown.lua` installs 80 global mappings, and three of them silently kill built-in motions

`plugins/markdown.lua` calls `vim.keymap.set` 80 times inside the `markdown-plus.nvim` `config` function,
and not one of those calls passes `buffer = true`. The spec loads on `ft = "markdown"`, so opening one
markdown file installs all 80 globally, for the rest of the session, in every buffer.

Most are `<localleader>` keys, which is survivable. Ten are not: normal-mode `gd`, `]]`, `[[`, `o` and
`O`, and insert-mode `<CR>`, `<Tab>`, `<S-Tab>`, `<BS>` and `<C-t>`.

Measured, in a headless session running the real config, comparing a run that opens a markdown file first
against a control run that does not:

- `gd` on a local variable in a Lua buffer. Control moves the cursor from line 4 to the declaration on
  line 2. After a markdown buffer has been open, the cursor does not move at all.
- `]]` from line 1 of the same Lua buffer. Control moves to line 5. After a markdown buffer, it stays on
  line 1.
- `\mb` in a Lua buffer wraps the word under the cursor in `**` and writes `print(**beta**)`.

`markdown-plus.nvim` installs its own defaults buffer-locally, at the same keys, whenever
`keymaps.enabled` is set, and those buffer-local mappings win inside a markdown buffer. Measured: `]]` in
a markdown file runs the plugin's own `Jump to next section` callback and works correctly. So the global
block is redundant where it was meant to apply and harmful everywhere else.

Two things to decide, and they are separable. Whether the global block should exist at all, given that
the plugin already installs the same keys buffer-locally; and, if any of it stays, whether it belongs
behind `buffer = true` in a `FileType` autocmd. Either answer restores `gd` in Lua files, which is the
part worth fixing first.

### 2. `n` and `N` have two owners, and the normal-mode one is not the one the config wrote

`config/keymaps.lua` maps `n` and `N` to `'Nn'[v:searchforward]`, the vim-galore recipe that keeps `n`
going forward whichever direction the search was started in. `plugins/hlslens.lua` then maps the same two
keys, in normal mode only, to a plain counted `n` wrapped in a lens refresh. `hlslens` runs after
`config/keymaps.lua`, so it wins: the live set reports `n` and `N` as `Next Search Result (hlslens)`.

The visual and operator-pending copies in `keymaps.lua` are untouched, so the direction-stable behavior
survives in those two modes and is gone from normal mode. After a `?` search, `n` reverses in normal mode
and does not reverse inside an operator. Whichever behavior the operator wants, it should be the same one
in all three modes, and the two files should not both claim the key.

### 3. `<leader>ar` is Ansible, sitting inside the aerial group

`which-key.lua` labels `<leader>a` as "aerial", and `plugins/ansible.lua` puts
`Ansible: Run Playbook／Role` on `<leader>ar`. The popup for the aerial group therefore lists a playbook
runner among eight sidebar commands. It is filtered to `ft = "yaml.ansible"`, so it only shows in a
playbook, but that is why it will read as a surprise when it does.

Worth noting why it happened: aerial holds eight keys for four actions. `<leader>at` and `<leader>aT` are
declared aliases of `<leader>aa` and `<leader>aA`, and `ao`, `aO`, `ac` and `aC` split open and close by
focus. That leaves very little of the group free, and `ar` was what was left. Thinning the aliases would
free the letters to give Ansible a group of its own.

### 4. `<C-g>dhw` and `<C-g>dhm` diff the working tree from inside the HEAD group

`which-key.lua` labels `<C-g>dh` as "HEAD (latest commit)". Its two lowercase-letter members run
`git diff --color-words` and `git diff --color-moved` through Overseer, and a bare `git diff` compares
the working tree against the index, not against HEAD. The group promises one comparison and these two
keys perform another.

Both also call `git.latest_commit()` and refuse when there is no commit hash, and then never use the
hash. In its present form that lookup only makes the mapping fail in a repository with no commits. Either
the commands should carry `HEAD` and use the hash, or the two keys belong under a diff-options heading
rather than under a diff-target one.

### 5. Two test runners, opposite conventions for nearest against all

`neotest` has `<leader>tt` for the nearest test and `<leader>ta` for all of them. `xcodebuild` has
`<leader>xT` for the nearest test and `<leader>xt` for all of them. So the lowercase key means "nearest"
in one runner and "everything" in the other, and the operator's hands have to know which language the
buffer is in before the shift key decision is made. `neotest` also carries the Swift adapter, so both are
reachable in the same buffer.

### 6. The focus convention flips between plugin families

- `aerial`: `<leader>aa` toggles without focus, `<leader>aA` toggles and focuses. Uppercase focuses.
- `trouble`: `<leader>lX` opens symbols without focus, `<leader>lS` opens symbols and focuses.
- `overseer`: `<leader>oo` opens and focuses, `<leader>oO` opens without focus. Uppercase does not focus.

Aerial and overseer are direct opposites on the same question, in the same kind of sidebar, three keys
apart in the popup.

### 7. Diagnostics scope inverts between snacks and trouble

- `snacks`: `<leader>sd` is buffer diagnostics, `<leader>sD` is all diagnostics.
- `trouble`: `<leader>Xx` is all diagnostics, `<leader>XX` is buffer diagnostics.

Lowercase narrows in one and widens in the other, for the same data.

### 8. `[y` and `]y` describe themselves backwards

`yanky.lua` maps `[y` to `<Plug>(YankyCycleForward)` and `]y` to `<Plug>(YankyCycleBackward)`, with
descriptions to match. Read against `yanky.lua` upstream, `YankyCycleForward` is the same function as
`YankyPreviousEntry`, and `YankyCycleBackward` is `YankyNextEntry`. So the keys are on the right side of
the `[` "prev" and `]` "next" groups and the descriptions contradict them: the popup shows "Cycle
Forward" under "prev". Upstream ships the `PreviousEntry` and `NextEntry` names for exactly this reason,
and swapping to those in the right-hand side would let the descriptions say what happens.

### 9. Case pairs holding unrelated actions

Elsewhere in this config a case pair means a variant of one action: `aa` against `aA`, `oo` against `oO`,
`ld` against `lD`. Three pairs break that:

- `<leader>ts` toggles the neotest summary; `<leader>tS` stops the run.
- `<leader>xg` builds and debugs; `<leader>xG` debugs the nearest test.
- `<leader>uu` toggles Markview globally; `<leader>uU` toggles it for the buffer. This one is a genuine
  variant and is listed only to show the convention is real.

A slipped shift key on either of the first two does something unrelated, and on `<leader>tS` it stops a
running test.

### 10. `harpoon` takes `<C-p>` and `<C-n>` as well as `<leader>hp` and `<leader>hn`

`harpoon.lua` maps the same two functions twice, once on the leader keys and once on `<C-p>` and `<C-n>`.
In normal mode those two are Vim's own synonyms for `k` and `j`, which is a small loss. The question is
whether the control keys are earning their place or were added before the leader pair, since the plugin
now owns four keys for two actions in a group that has room.

### 11. `<M-[>` is the prefix of every terminal control sequence

`overseer.lua` puts `OverseerWatchRun` on `<M-[>`. A terminal sends Alt plus a key as escape followed by
that key, so `<M-[>` arrives as the same two bytes that open every control sequence the terminal emits.
Whether Neovim resolves it depends on timeout and on what Ghostty sends for Option, which is why this is
flagged as worth one test rather than asserted broken. The other four Overseer Alt keys (`<M-'>`,
`<M-;>`, `<M-7>`, `<M-8>`) carry no such ambiguity.

## Conflicts the health report surfaced, and which side actually wins

### targets.vim against nvim-various-textobjs: eight mappings, four objects

`:checkhealth` reports eight conflicting mappings. Two plugins claim the same key here, which is a
different thing from a which-key prefix overlap, and every one of the eight resolves the same way.

`targets.vim` maps exactly four left-hand sides, `a`, `i`, `A` and `I`, in operator-pending and visual
mode, as expression dispatchers that read the next character themselves (read from `plugin/targets.vim`,
which is also where the version, 0.5.0, comes from). `nvim-various-textobjs` installs its default set as
two-character mappings. Vim prefers the longer match, so a two-character mapping shadows the dispatcher
completely for that character. Confirmed in the live set:

| Keys        | Winner                               | What targets.vim loses                                                                              |
| ----------- | ------------------------------------ | --------------------------------------------------------------------------------------------------- |
| `aq` / `iq` | various-textobjs `anyQuote`          | The nearest quote of any kind, plus counts and the `n` and `l` seek variants (`2iq`, `inq`, `ilq`). |
| `a,` / `i,` | various-textobjs `argument`          | The comma separator object, plus the same counts and seek variants.                                 |
| `a_` / `i_` | various-textobjs `lineCharacterwise` | The underscore separator, which is how a snake_case segment was selected.                           |
| `a#` / `i#` | various-textobjs `color`             | The hash separator.                                                                                 |

Three of the four cost very little. `aa` and `ia` are untouched, so the argument object keeps its counts
and seek variants under its usual letter, and `a,` was only ever the alias. `iS` from various-textobjs
selects a subword, which is what `i_` was reached for. A hex-color object is worth more than the hash
separator it replaced.

The fourth is a real loss. `2iq` and `inq` used to reach the second quote ahead; various-textobjs looks
forward a fixed number of lines instead of taking a count, so they no longer do. That is the one to
decide, and it only matters if counted quote-hopping is already in the operator's fingers.

### Snacks scope textobjects are permanently shadowed

`snacks.nvim` declares `ai` and `ii` as scope textobjects and `[i` and `]i` as scope jumps, from its
`scope` module, which this config enables. `textobjects.lua` installs the various-textobjs default set at
`VeryLazy`, which fires after `UIEnter`, and that set includes `ai` and `ii` for indentation.

So `ai` and `ii` belong to various-textobjs for the whole session, not just the startup window the spec's
own comment reasons about. The live set reports `ai` as `outer-inner indentation textobj`. `[i` and `]i`
survive, because nothing else claims them.

Both plugins describe roughly the same region by different rules, treesitter scope against indentation,
so what is lost is a preference and not a capability. It is listed because the spec's own comment reads
as though Snacks keeps these once startup is over, when various-textobjs holds them for the whole
session.

### Treesitter incremental selection loses `an` and `in` at VeryLazy

The early dump has `an` and `in` in operator-pending and visual mode as `Select parent (outer) node` and
`Select child (inner) node`. The full dump has them as `outer number textobj` and `inner number textobj`.
The same `VeryLazy` install takes them.

### Three various-textobjs defaults sit on visual-mode built-ins

Also from the default set, in visual mode:

- `gw` is `visibleInWindow`, over Vim's own `gw`, which formats the selection and keeps the cursor put.
- `!` is `diagnostic`, over the visual-mode filter operator.
- `R` is claimed by `flash.nvim` as treesitter search, over visual-mode `R`.

Upstream documents the first two as deliberate. They are named here because `disabledDefaults` in the
`nvim-various-textobjs` options takes a list of exactly these keys, so declining any of them is one line,
and because the spec currently takes the whole default set without saying which of the trades it
accepted.

### Overlap warnings deliberately not raised

`:checkhealth which-key` also reports prefix overlaps, where a complete mapping is also the start of a
longer one. which-key itself calls those informational and they behave correctly: the shorter mapping
fires after `timeoutlen`. None of the ones present costs a keystroke or hides anything, so none is listed
as a finding. The visual-mode `<leader>y` case was checked specifically, since `<leader>ya`, `<leader>yl`
and the eight pair yanks look like they would overlap it, and they do not: those are normal-mode only,
and visual mode has just the one `<leader>y`.

## Namespace pressure, for context

288 normal-mode leader mappings across 40 first letters. The busiest groups:

```
x 38   g 35   u 27   s 25   o 17   D 14   y 12   l 12   L 11   n 8   f 8   a 8   / 8
```

`<leader>x` is Xcode, the largest group in the config and, on the evidence of this repository's own work,
the least used. `<leader>X` is diagnostics and quickfix with five, and holds the shifted prefix. Nothing
here is broken. It is context for any later decision to move a group, and it is why several
recommendations above land on capital letters rather than on the lowercase ones a hand would find first.
