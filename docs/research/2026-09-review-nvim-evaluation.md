# review.nvim evaluated against the annotation flow

**Date:** 2026-09-05 · **Plan task:** 40b (PR 24b) · **Spec:** 5.6 · **Machine:** dresden **Subject:**
`vuki656/review.nvim` at `a777ea0fa8d8a3a44c9f63eb8c5d72f0eae29a98` (2026-09-04, no tags, no releases)
**Incumbent:** the annotation flow, `dot_config/nvim/lua/custom_api/annotate.lua` over
`ChmaraX/herdr-nvim` at `41c30f528e645cecb1e387c46de9dc416ecf978e`, as pinned in `lazy-lock.json` and
configured in `dot_config/nvim/lua/plugins/herdr-nvim.lua`

This pull request adds this document and nothing else. No plugin was declared through chezmoi, the lock
was not touched, and no `chezmoi apply` was run.

______________________________________________________________________

## 1. Verdict

# DO NOT ADOPT

Keep the annotation flow. The plan's default stands, and it is not a tie broken by taste: review.nvim's
comment is anchored by a plain integer that nothing ever updates, so on the one question this evaluation
was built to answer it is measurably worse than what is installed.

**Deciding rationale.** A herdr-nvim comment is an extmark (`comments.lua:9`), so an edit above it moves
it. A review.nvim quick comment is a number in a table (`quick_comments/state.lua:54`), so an edit above
it does not. Measured side by side in one process, on the same buffer, with three lines inserted above a
comment on line 5: the herdr-nvim comment moved to line 8 and still pointed at the text it was written
about, and the review.nvim comment stayed on line 5 and now pointed at different text (3.5, 4.5). That is
not a cosmetic difference, because both stores exist to hand an address to an agent. The prompt
review.nvim would send after that edit says `**Line 5**`, and line 5 is no longer the line the operator
was complaining about. An agent acting on it edits the wrong line. Adopting review.nvim would mean
replacing a store that re-anchors with one that cannot.

**Two capabilities it genuinely has that the current flow lacks, weighed rather than waved off.** First,
persistence: quick comments are written to `.git/review-comments.json` and reloaded on the next start
(4.6), where herdr-nvim's store is an in-memory Lua table with no file writer anywhere in the plugin.
Second, a comment on a DELETED diff line: review.nvim tags a deleted line `side = "old"` and keeps its
old-side line number (`core/diff.lua:290,297`), which an extmark structurally cannot do, since a deleted
line has no line in the working tree to anchor to.

Neither overturns the default. Persistence buys little here because the flow's own design is
send-then-clear: `clear_after_send` defaults to true (`init.lua:8`) and deletes every comment once it is
delivered, so a comment's intended life is one review pass, measured in minutes, not across restarts. And
persistence is the capability least in need of a new plugin, because a persistent local note store landed
on this machine yesterday: atlas.nvim was adopted 2026-09-05 with `delete_notes = false` and
`<leader>gtn` (`dot_config/nvim/lua/plugins/atlas.lua:26,50`). The atlas evaluation asked this document
to re-read it on exactly this point, and the answer it was looking for is that the gap is already closed.
Adopting review.nvim for persistence would make three annotation stores on one machine, which is a
sharper version of the cost the plan named when it set the default.

The deleted-line comment is the more interesting of the two, and it is unmatched as far as this
evaluation measured: whether an atlas local note can attach to an old-side line was not tested. It still
does not carry the decision, because it is a question about reviewing a diff rather than about the job
the annotation flow does. That job, as `annotate.lua:1-8` states it, is writing down what the operator
would otherwise retype at an agent about a line of live code: its path and number, its diagnostic, its
enclosing function, its blame. A deleted line has no live code to point at, and the diff is something the
agent can read for itself.

**Everything else is a tie or a loss.** Commenting a line or a selection, listing, pasting into the
agent's input and sending are all covered, and the send path is not merely equivalent but identical:
review.nvim shells `herdr pane send-text` (`export/markdown.lua:452`) and so does herdr-nvim
(`dispatch.lua:15`). The submit variant is a loss: `<leader>AS` auto-submits through `herdr agent prompt`
(`dispatch.lua:9`), and review.nvim has no auto-submit on the herdr path at all, since its `auto_enter`
option is wired only into the tmux branch (`export/markdown.lua:322-324`). Its comments also carry none
of the enrichment the annotator composes: zero references to `vim.diagnostic` and zero to blame anywhere
in its Lua (4.1).

**Reconsider when any of these becomes true.** review.nvim re-anchors its quick comments on buffer
changes, by extmark or an equivalent, which would remove the one disqualifying finding and reopen the
comparison on its merits. Or the annotation flow acquires a need to comment on deleted lines, in which
case this is the plugin that already does it and section 5 is the shape of the work. Or herdr-nvim's
in-memory store starts losing work the operator cares about, which would be an argument for persisting
the EXISTING store, not for importing a second one.

______________________________________________________________________

## 2. How it was tested

review.nvim was cloned into a `mktemp -d` at `/tmp/rv.QTHw` and loaded only in headless Neovim (v0.12.5)
with all four roots redirected to that directory (`XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_STATE_HOME`,
`XDG_CACHE_HOME`). It was never declared through chezmoi, never written into `~/.config/nvim` or
`~/.local/share/nvim`, and never loaded into the operator's running editor. herdr-nvim was cloned
separately and checked out at the commit `lazy-lock.json` pins, rather than read out of the live data
directory, so both sides of every comparison come from a clean checkout at a known commit.

Probes run under `nvim --headless --clean -u <probe>.lua`, with the plugins appended to the runtimepath.
The fixture is a throwaway git repository holding one ten-line file, so the persistence path
(`.git/review-comments.json`) resolves and nothing outside the scratch tree is touched.

**`-c 'qa!'` cannot be used with a `VimEnter` probe on this version, and that is worth recording.** The
standing rule pairs the two, but measured here they are mutually exclusive: with `-c 'qa!'` the init file
is sourced and `VimEnter` never fires, because the command runs first and quits.

```
A: --headless --clean -u t.lua -c qa!   -> init sourced/
D: --headless --clean -u t.lua          -> init sourced/VimEnter fired/
```

So every probe below registers its work on `VimEnter` and calls `os.exit` itself, with no `-c`, which is
the same shape the atlas evaluation used. Each run is wrapped in `timeout 60` so a probe that never
reaches `VimEnter` fails instead of hanging. Probes write their results to a file, because `os.exit` from
inside the autocmd discards buffered stdout.

Both probe guards were mutation-verified. Pointed at a runtimepath with no review.nvim on it, and run
from outside the scratch repository, the probe refuses rather than reporting:

```
=== mutation check: guard with review.nvim removed from the runtimepath ===
PROBE FAIL: review.nvim not on runtimepath: module 'review' not found:
	no field package.preload['review']
	no file './review.lua'
exit=1
=== mutation check: guard run outside the scratch repo ===
PROBE FAIL: not in the scratch repo, cwd=/Users/stephen/.herdr/worktrees/dotfiles/nvim-eval-review
exit=1
```

Two behaviours could not be exercised headlessly and are judged from source with a citation instead: the
full review user interface, which is a tab of floating windows driven by buffer-local keys, and the
delivery itself, which would have typed into a live agent pane. Adding a comment through `qc.add()` opens
an interactive input popup, so the probes call the state API that popup's submit handler calls, which is
the same store by the same path minus the prompt.

`:checkhealth review` on the throwaway root:

```
review:                                                                     ✅
review.nvim ~
- ✅ OK Neovim 0.12.5 (>= 0.10 required)
- ✅ OK `git` found: git version 2.55.0
- ✅ OK Inside a git repository: /private/tmp/rv.QTHw/repo
- ✅ OK `tmux` not found in PATH, herdr is used for sending
- ✅ OK Running inside a herdr session (pane wW:p3K)
- ✅ OK `require('review').setup()` has been called
- ✅ OK Log level: WARN
- Log file (not created yet): /var/folders/l5/8czjbbwx4pl1063q75rxypfr0000gn/T/review.nvim/review.log
```

The herdr line reads OK because `HERDR_PANE_ID` is inherited from the shell the probe ran in. No herdr
pane, tab or workspace was created at any point, and nothing was sent to any agent.

______________________________________________________________________

## 3. Outcome table

- **Comment a line or a selection (`<leader>Ac`): tie on the action, loss on the content.** Both comment
  a line and both comment a range. review.nvim adds nothing here that `<leader>Ac` does not already do,
  and it carries less: no diagnostic, no enclosing function, no blame (4.1).
- **List (`<leader>Al`): tie.** Both open an interactive list with jump, edit and delete on single keys.
  review.nvim's is a persistent side panel, herdr-nvim's is a transient float. Neither can do something
  the other cannot (4.2).
- **Paste into the agent's input (`<leader>As`): tie, and the same command underneath.** Both shell
  `herdr pane send-text` with no Enter, both pick the agent with `vim.ui.select` (4.3).
- **Send and submit (`<leader>AS`): fail, no equivalent.** herdr-nvim auto-submits through
  `herdr agent prompt`. review.nvim's auto-submit is tmux-only; inside herdr it always leaves the prompt
  unsent (4.4).
- **Survive an edit above it: fail, and this is the deciding row.** The herdr-nvim comment moved 5 to 8
  and kept pointing at its line. The review.nvim comment stayed on 5 and now points at different text
  (4.5).

______________________________________________________________________

## 4. Row detail

### 4.1 Comment a line or a selection: tie on the action, loss on the content

Both sides do the action. `<leader>Ac` is bound in normal and visual mode (`init.lua:22-23`), calling
`comment_line` or `comment_selection`. review.nvim offers `:Review qc` for the current line and
`:'<,'>Review qc` for a range, plus `qc.add(42, 50)` and `qc.add_visual()` in its Lua API. review.nvim
adds comment types (note, fix, question) and eight canned templates on `<C-t>`, which herdr-nvim has no
equivalent of; both are conveniences on the text, not new capability.

The content is where the incumbent is ahead, and the gap is the whole reason `annotate.lua` exists. It
composes four parts into the stored text: a `@path:line` mention, the language server diagnostic on that
line, the enclosing function from treesitter, and the blame commit (`annotate.lua:260-265`). review.nvim
composes none of them. Its Lua contains no reference to diagnostics or blame at all:

```
=== enrichment sources in review.nvim's comment paths ===
  vim.diagnostic   0 hits anywhere in lua/
  treesitter       lua/review/ui/diff_view.lua:57:---Namespace for treesitter syntax highlights
  blame            0 hits anywhere in lua/
```

The one treesitter use is syntax highlighting inside its own diff buffers, not an enclosing-function
lookup. A quick comment stores exactly the text typed, plus the raw line content at creation time.

### 4.2 List: tie

`<leader>Al` opens herdr-nvim's `comment_list`, where hover auto-jumps to each comment, `<CR>` edits and
`d` deletes (`init.lua:69-80`). review.nvim's `:Review qp` toggles a side panel where `<CR>` jumps, `e`
edits, `d` deletes, `L` previews the full text, `c` copies and `s` sends.

review.nvim's panel is persistent and dockable where herdr-nvim's is a transient float, and it has a
preview key the float has no need for, since the float shows the text already. Neither reaches a
capability the other lacks. Note that review.nvim creates no global key for this by default, or for
anything else:

```
=== user commands registered by review.nvim ===
  :Review

=== global keymaps created by review.nvim with the default config ===
  (none: every keymaps entry defaults to nil, README Configuration)
```

### 4.3 Paste into the agent's input: tie, and the same command underneath

This row is not a near-match, it is the same transport. herdr-nvim's non-submit branch runs
`herdr pane send-text <pane_id> <text>` (`dispatch.lua:15`). review.nvim's herdr branch runs
`{ "herdr", "pane", "send-text", agent.pane_id, content }` (`export/markdown.lua:452`). Both discover
agents with `herdr agent list`, both present them through `vim.ui.select` when the target is ambiguous,
and both deliberately send no Enter so the operator reads the prompt before submitting it.

The one behavioural difference is which agent gets picked without asking. herdr-nvim resolves silently
when the target is unambiguous, preferring a single agent in the current tab via `HERDR_TAB_ID`, and only
then falls back to the picker (`agents.lua:38-49`). review.nvim asks every time on the interactive path:
its skip-the-picker branch is gated on a `silent` flag, which auto-sends with exactly one agent and
refuses outright with more (`export/markdown.lua:495-503`), so `:Review qs` opens the picker even when
one agent is the only possible target. herdr-nvim also warns when the chosen agent is mid-task and sends
anyway (`init.lua:115-117`), which review.nvim has no notion of.

What each one actually sends differs, and is shown under 4.5, because the difference is created by the
anchoring rather than by the transport.

### 4.4 Send and submit: fail, no equivalent

`<leader>AS` sends and presses Enter, through a different herdr subcommand:

```lua
-- herdr-nvim/lua/herdr-nvim/dispatch.lua:7-12
if opts.submit then
  -- agent prompt sends text and auto-submits (presses Enter for you)
  local r = exec({ "herdr", "agent", "prompt", pane_id, text })
```

review.nvim never calls `herdr agent prompt`. Its only auto-submit is in the tmux branch, gated on a
config flag:

```
=== auto_enter uses ===
  lua/review/config.lua:28:---@field auto_enter boolean Whether to send Enter key after pasting
  lua/review/config.lua:151:        auto_enter = false,
  lua/review/export/markdown.lua:322:                if cfg.tmux.auto_enter then
=== herdr Enter/send-keys ===
  lua/review/export/markdown.lua:324:                        vim.system({ "tmux", "send-keys", "-t", target, "Enter" })
```

`tmux` is not installed on this machine and herdr is the multiplexer, so that branch is unreachable here.
Inside herdr, review.nvim can only ever do what `<leader>As` does, and the operator presses Enter by
hand.

### 4.5 Survive an edit above it: fail, and this is the deciding row

One process, one buffer, both stores. A comment goes on line 5 in each, then three lines are inserted
above:

```
content at line 5 BEFORE: line 5

--- before the edit ---
review.nvim  stored line: 5
herdr-nvim   stored line: 5

--- after inserting 3 lines above ---
review.nvim  stored line: 5  -> now points at: line 2
herdr-nvim   stored line: 8  -> now points at: line 5

expected (the commented line moved 5 -> 8): line 8 = line 5
```

The mechanism is visible in six lines of each store. herdr-nvim sets an extmark and reads the position
back out of the buffer on every access:

```lua
-- herdr-nvim/lua/herdr-nvim/comments.lua:9-14
local extmark = vim.api.nvim_buf_set_extmark(bufnr, M.ns, start_line - 1, 0, {
  end_row = end_line,
  end_col = 0,
  right_gravity = false,
  end_right_gravity = true,
})
```

review.nvim stores the number and never revisits it (`quick_comments/state.lua:51-60`). It holds no
extmark at all: every `nvim_buf_set_extmark` in the plugin is decoration (highlights and virtual lines)
in its own diff and list buffers, there is no `nvim_buf_get_extmark_by_id` anywhere, and
`lua/review/quick_comments/` contains the string `extmark` zero times.

The gutter sign is not a counter-example. Signs do shift with the text, but `signs.update` unplaces every
sign and re-places it from the stored number (`quick_comments/signs.lua:33,46-52`), and it runs on every
`BufEnter` (`signs.lua:75-82`), so the sign snaps back to the stale line. Either way the record behind it
never moved.

The diff-pane comments re-anchor, but against the diff render rather than against the file.
`reanchor_comment` maps `original_line` to a display row and leaves `original_line` itself untouched
(`core/diff.lua:375-388`), so the source anchor is fixed for the comment's life exactly as the quick
comment's is.

**What this does to the prompt.** Same buffer, same edit, both stores rendered through their own
exporters:

````
=== what review.nvim would send (:Review qs / the 's' key) ===
# Quick Comments

## sample.txt

**Line 5** - 󰍩 Note
```
line 5
```
this line is wrong
=== what herdr-nvim would send (<leader>As / <leader>AS) ===
Code review comments from my editor:

1. /private/tmp/rv.QTHw/repo/sample.txt:8-8
   > line 5
   Comment: this line is wrong

Please address each comment. Reply with what you changed per item.
````

Both name the right code, and only one gives an address that is true of the file as it now stands.
review.nvim says line 5, where line 5 now holds `line 2`. It reads correct only because its fenced
context is a snapshot frozen at creation (`quick_comments/markdown.lua:35-39`), never re-read from the
buffer. herdr-nvim says 8, and pulls its snippet live through the extmark (`comments.lua:75-79`), so the
two can never drift apart.

This matters most in the case the flow was built for. `auto_refresh` exists because an agent is writing
files while the operator reads, and an agent writing above a commented line is the ordinary case, not the
corner one.

### 4.6 Persistence: real, and already covered

Quick comments survive a restart. The file is per repository and inside `.git`, so nothing needs
gitignoring:

```
=== review.nvim quick-comment persistence path ===
/private/tmp/rv.QTHw/repo/.git/review-comments.json
```

It round-trips across processes: a comment written by one probe run was reloaded by the next one's
`setup()` and appeared in its export. The stored record shows the anchor problem in the persisted form, a
bare integer with a frozen context string and nothing to re-derive either from:

```json
{
  "comments": {
    "/private/tmp/rv.QTHw/repo/sample.txt": [
      {
        "line": 5,
        "text": "this line is wrong",
        "context": "line 5",
        "id": "qc_1788649895_1",
        "type": "note",
        "created_at": 1788649895,
        "file": "/private/tmp/rv.QTHw/repo/sample.txt"
      }
    ]
  },
  "comment_id_counter": 1,
  "version": 1
}
```

herdr-nvim has no persistence whatever. Its store is a module-local table (`comments.lua:3`) and a
recursive search of its Lua for `writefile`, `readfile`, `json_encode`, `json.encode` or `stdpath`
returns nothing. A restart drops every pending comment.

So the capability is real. It is also the one the machine least needs a new plugin for, for the two
reasons section 1 gives: `clear_after_send` deletes comments on delivery anyway, and atlas.nvim's local
notes have been installed since 2026-09-05 with `delete_notes = false`, `<leader>gtn` to list them and a
`bin/atlas-notes` front end an agent can drive.

### 4.7 Project state

215 commits since 2026-01-25, the most recent 2026-09-04, which is the day before this evaluation. No
tags and no releases, so a pin can only be a commit. 211 of the 215 commits are by one author. It ships a
test suite and a CI workflow, and `:checkhealth review` is thorough.

Read plainly: actively developed, single maintainer, no release discipline yet. Not disqualifying on its
own, and not the reason for the verdict, but it is the second plugin in two evaluations that would have
to be tracked by commit rather than by tag.

______________________________________________________________________

## 5. What adopting would cost

An adopt outcome becomes its own pull request. Because the verdict is not to adopt, this section is the
shape of the work if the deciding row is ever fixed upstream, not a plan.

The cheap version is not available. Taking review.nvim only for its quick comments would still install
the whole plugin, and the rest of it is a git client: a diff browser, a file tree with staging, a branch
list with checkout and delete, a commit list with a soft reset, and push, pull, commit and amend keys.
Every one of those overlaps something already declared in `dot_config/nvim/lua/plugins/git.lua`: gitsigns
for hunks and staging, fugitive for everything else, gitlinker, octo, and atlas since 2026-09-05.
Adopting it would put a fifth git surface in the config to get a comment store.

The keymap work is small, because review.nvim binds nothing globally by default: it registers one
command, `:Review`, and creates a global key only where a `keymaps` entry is given a value. It would want
a `<leader>` prefix of its own under the section 8.2 rule, plus a group row in
`lua/plugins/which-key.lua`. Its in-buffer keys are buffer-local to its own floating windows and would
collide with nothing.

Two decisions an adopt pull request would own. Whether the annotator writes into review.nvim's store
instead of herdr-nvim's, which is not a swap of one call for another: `annotate.lua:267-268` adds a
comment and decorates it by id, where review.nvim's equivalent is a file path and a line number, and the
annotator would lose the extmark that makes `M.line()` worth calling. And whether `<leader>AS` survives,
since review.nvim cannot auto-submit inside herdr (4.4); keeping it would mean either an
`export.on_export` callback that shells `herdr agent prompt` directly, or accepting that the submit key
goes away.

______________________________________________________________________

## 6. Cleanup

The scratch tree at `/tmp/rv.QTHw`, holding both clones, the probe scripts, the throwaway git repository
and the redirected Neovim roots, was trashed at the end of the evaluation. Nothing under `~/.config/nvim`
or `~/.local/share/nvim` was written to or read at any point: both plugins came from fresh clones, the
incumbent at the commit `lazy-lock.json` pins. No herdr pane, tab or workspace was created, no agent was
sent anything, and the operator's running editor was never touched.
