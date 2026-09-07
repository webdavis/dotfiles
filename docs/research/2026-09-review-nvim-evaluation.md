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

Neither overturns the default, and the first is a genuine gap that nothing else here fills. The atlas
evaluation asked this document to check whether atlas's local notes already cover it. They do not.
`bin/atlas-notes` requires `--target`, a pull request URL or an atlas reference, and its `--line` is the
"1-based line number in the pull request head" (`bin/atlas-notes:17-19`, enforced at line 84 where the
target is `required`). An atlas note hangs off a pull request that already exists, so it has nowhere to
put an annotation on uncommitted work in the current buffer, which is what the annotation flow holds.
review.nvim's persistence is a real capability the flow lacks.

The flow's own design limits how much that buys. Delivery is send-then-clear: `clear_after_send` defaults
to true (`init.lua:8`) and deletes every comment once it goes out, so a comment's intended life is one
review pass, measured in minutes. Persistence covers a crash or a restart inside that window. Section 5
is what it would cost, and the short version is that the plugin cannot be taken for its comment store
alone: the rest of it is a fifth git surface.

The deleted-line comment is the second gap, and it is unmatched as far as this evaluation measured. It
does not carry the decision either, because it is a question about reviewing a diff rather than about the
job the annotation flow does. That job, as `annotate.lua:1-8` states it, is writing down what the
operator would otherwise retype at an agent about a line of live code: its path and number, its
diagnostic, its enclosing function, its blame. A deleted line has no live code to point at, and the diff
is something the agent can read for itself.

**Everything else is a tie or a loss.** Commenting a line or a selection and listing are ties, and both
sides store the typed text unchanged (4.1). Pasting into the agent's input runs the same command on both
sides: review.nvim shells `herdr pane send-text` (`export/markdown.lua:452`) and so does herdr-nvim
(`dispatch.lua:15`).

Two losses sit on the delivery path. The submit variant has no equivalent: `<leader>AS` auto-submits
through `herdr agent prompt` (`dispatch.lua:9`), and review.nvim's only auto-submit is wired into its
tmux branch (`export/markdown.lua:322-324`), which is unreachable on a machine with no tmux. And the
routing is wider than it should be: herdr-nvim filters candidate agents by `HERDR_WORKSPACE_ID`
(`agents.lua:14-26`) where review.nvim keeps every pane the daemon reports
(`export/markdown.lua:365-385`), so with eight project workspaces configured its picker can send one
repository's comments to an agent working in another (4.3).

**Reconsider when any of these becomes true.** review.nvim re-anchors its quick comments on buffer
changes, by extmark or an equivalent, which would remove the one disqualifying finding and reopen the
comparison on its merits. Or the annotation flow acquires a need to comment on deleted lines, in which
case this is the plugin that already does it and section 5 is the shape of the work. Or herdr-nvim's
in-memory store starts losing work the operator cares about, which would be an argument for persisting
the EXISTING store, not for importing a second one.

______________________________________________________________________

## 2. How it was tested

review.nvim was cloned into a `mktemp -d` and loaded only in headless Neovim (v0.12.5) with all four
roots redirected into that directory (`XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_STATE_HOME`,
`XDG_CACHE_HOME`). It was never declared through chezmoi, never written into `~/.config/nvim` or
`~/.local/share/nvim`, and never loaded into the operator's running editor. herdr-nvim and atlas.nvim
were cloned the same way and checked out at the commits this repository pins, rather than read out of the
live data directory, so every side of every comparison comes from a clean checkout at a known commit.

The probes ran in two sittings and the pasted output names two scratch roots, `/tmp/rv.QTHw` for the
first and `/tmp/rv2.miyr` for the second, which added the complete-flow measurement in 4.5 and the atlas
check in 4.6. Both were trashed (section 6).

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

The complete-flow probe in 4.5 carries the same guard on the annotator, verified the same way:

```
=== mutation: annotator unreachable ===
PROBE FAIL: custom_api.annotate not reachable: module 'custom_api.annotate' not found:
	no field package.preload['custom_api.annotate']
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

- **Comment a line or a selection (`<leader>Ac`): tie.** Both comment a line and both comment a range,
  and both store the typed text unchanged. review.nvim adds comment types and templates, which are
  conveniences on the text rather than capability. The diagnostic, function and blame enrichment is not
  part of this row: it belongs to `<leader>Cx` (4.1).
- **List (`<leader>Al`): tie.** Both open an interactive list with jump, edit and delete on single keys.
  review.nvim's is a persistent side panel, herdr-nvim's is a transient float. Neither can do something
  the other cannot (4.2).
- **Paste into the agent's input (`<leader>As`): same command, wider routing.** Both shell
  `herdr pane send-text` with no Enter and pick the agent with `vim.ui.select`. review.nvim offers agents
  from every workspace where herdr-nvim filters to the current one, so its picker can reach an agent in
  the wrong repository (4.3).
- **Send and submit (`<leader>AS`): fail, no equivalent.** herdr-nvim auto-submits through
  `herdr agent prompt`. review.nvim's auto-submit is tmux-only; inside herdr it always leaves the prompt
  unsent (4.4).
- **Survive an edit above it: fail, and this is the deciding row.** The herdr-nvim comment moved 5 to 8
  and kept pointing at its line. The review.nvim comment stayed on 5 and now points at different text.
  The incumbent's win is partial: the extmark moves, but an address baked into the comment text by
  `<leader>Cx` does not (4.5).

______________________________________________________________________

## 4. Row detail

### 4.1 Comment a line or a selection: tie

Both sides do the action, and both store the text the operator typed and nothing more.

`<leader>Ac` is bound in normal and visual mode (`init.lua:22-23`) onto `comment_line` and
`comment_selection`. Both funnel into `add_comment`, which opens an input and hands the result straight
to the store: `ui.input_comment(function(text) comments.add(bufnr, start_line, end_line, text) end)`
(`init.lua:30-46`). No enrichment happens on this path.

review.nvim reaches the same place through `:Review qc`, which dispatches on the range and calls
`quick_comments.add(opts.line1, opts.line2)` or `quick_comments.add()` (`commands.lua:51-57`). `M.add`
resolves the range from the cursor when no argument is given, refuses a buffer with no file name, and
captures the line content as `context` before opening its input (`quick_comments/init.lua:18-35`). Its
`submit` closure trims trailing blank lines and stores the text with that context
(`quick_comments/init.lua:79-99`).

**The enrichment belongs to a different key, and an earlier revision of this document attributed it to
the wrong one.** The four composed parts, the `@path:line` mention, the diagnostic, the enclosing
function and the blame commit (`annotate.lua:260-265`), are what `annotate.line()` builds, and
`annotate.line()` is bound to `<leader>Cx` (`plugins/claudecode.lua:88-95`), not to `<leader>Ac`. So the
comment-action row is a tie on content as well as on action. The enrichment comparison is a separate
question, and on it review.nvim has no equivalent of `<leader>Cx` at all: its Lua holds no reference to
diagnostics or blame anywhere.

```
=== enrichment sources in review.nvim's comment paths ===
  vim.diagnostic   0 hits anywhere in lua/
  treesitter       lua/review/ui/diff_view.lua:57:---Namespace for treesitter syntax highlights
  blame            0 hits anywhere in lua/
```

The one treesitter use is syntax highlighting inside its own diff buffers, not an enclosing-function
lookup.

review.nvim does add comment types (note, fix, question) and eight canned templates on `<C-t>`, which
herdr-nvim has no equivalent of. Both are conveniences on the text rather than new capability.

### 4.2 List: tie

`<leader>Al` opens herdr-nvim's `comment_list`, where hover auto-jumps to each comment, `<CR>` edits and
`d` deletes (`init.lua:69-80`). review.nvim's `:Review qp` calls `quick_comments.toggle_panel()`
(`commands.lua:58-60`), and the panel binds its keys directly on its own buffer: `q` and `<Esc>` close,
`<CR>` jumps, `L` previews, `d` deletes, `e` edits, `c` copies and `s` sends
(`quick_comments/panel.lua:179,183,188,201,253,265,292,305`).

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

### 4.3 Paste into the agent's input: the same command, a wider candidate list

This row is not a near-match, it is the same transport. herdr-nvim's non-submit branch runs
`herdr pane send-text <pane_id> <text>` (`dispatch.lua:15`). review.nvim's herdr branch runs
`{ "herdr", "pane", "send-text", agent.pane_id, content }` (`export/markdown.lua:452`). Both discover
agents with `herdr agent list`, both present them through `vim.ui.select` when the target is ambiguous,
and both deliberately send no Enter so the operator reads the prompt before submitting it.

**The candidate list is not the same list, and this is a regression rather than a preference.**
herdr-nvim scopes the agents to the current workspace before anything else: it reads `HERDR_WORKSPACE_ID`
and keeps an agent only when its `workspace_id` matches, or when the variable is unset
(`agents.lua:14-26`). review.nvim applies no such filter. `parse_agents` keeps every pane the daemon
reports that carries a `pane_id` (`export/markdown.lua:365-385`), and the string `workspace` appears
nowhere in its Lua. On a machine running eight project workspaces, the review.nvim picker offers agents
from all of them, so the comments from one repository can be sent into an agent working in another.
herdr-nvim cannot make that mistake.

Two smaller differences on the same path. herdr-nvim resolves silently when the target is unambiguous,
preferring a single agent in the current tab via `HERDR_TAB_ID` and only then falling back to the picker
(`agents.lua:38-49`), where review.nvim asks every time on the interactive path: its skip-the-picker
branch is gated on a `silent` flag that auto-sends with exactly one agent and refuses outright with more
(`export/markdown.lua:495-503`). And herdr-nvim warns when the chosen agent is mid-task and sends anyway
(`init.lua:115-117`), which review.nvim has no notion of.

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
config flag. Verbatim grep output, source indentation intact, so the last line runs past the wrap width:

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

**The comparison above is between the two STORES, and the complete flow is worse than its store.** The
probe added the herdr-nvim comment through `comments.add`, which is what `<leader>Ac` does. The enriched
annotator does something else: `annotate.line()` formats `@%s:%d` from the file and line and puts it
inside the comment's own TEXT (`annotate.lua:261`), and text is the one field the extmark cannot touch.
Running the real `annotate.line()` through the same edit shows both halves at once:

```
--- the annotation as stored, before the edit ---
extmark line: 5
stored text:  @sample.txt:5 | blame 61d8758 seed the fixture

--- after inserting 3 lines above ---
extmark line: 8  (moved, and points at: line 5)
stored text:  @sample.txt:5 | blame 61d8758 seed the fixture
```

So the prompt that reaches the agent carries two addresses for one comment, and after an edit they
disagree:

```
1. /private/tmp/rv2.miyr/repo/sample.txt:8-8
   > line 5
   Comment: @sample.txt:5 | blame 61d8758 seed the fixture
```

The header is right and the embedded mention is stale. This does not change the verdict, because the
store is still the thing being compared and review.nvim's stales in both places while herdr-nvim's stales
in one. It does mean the honest claim is narrower than "the annotation flow survives an edit above it".
The extmark survives; an address baked into the comment text does not, and `<leader>Cx` bakes one in.
`<leader>Ac` stores no address at all, so it has nothing to go stale.

This matters most in the case the flow was built for. `auto_refresh` exists because an agent is writing
files while the operator reads, and an agent writing above a commented line is the ordinary case, not the
corner one.

### 4.6 Persistence: a real gap, and atlas does not close it

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

So the capability is real, and the obvious substitute does not cover it. atlas.nvim's local notes have
been installed since 2026-09-05 with `delete_notes = false` and `<leader>gtn`, and the atlas evaluation
pointed here on exactly this question, but an atlas note is scoped to a pull request. `bin/atlas-notes`
documents `--target` as a "Pull request URL or canonical Atlas reference" and `--line` as a "1-based line
number in the pull request head" (`bin/atlas-notes:17-19`), and `add` calls
`target(required(options, "target"))` at line 84, so the target is not optional. There is no way to file
an atlas note against uncommitted work in the current buffer, which is what the annotation flow holds.

`clear_after_send` is what limits the value of that persistence. It deletes every comment on delivery, so
the window being protected is one review pass rather than a working history. Section 5 is what buying
that window would cost.

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

Three decisions an adopt pull request would own. Whether the annotator writes into review.nvim's store
instead of herdr-nvim's, which is not a swap of one call for another: `annotate.lua:267-268` adds a
comment and decorates it by id, where review.nvim's equivalent is a file path and a line number, and the
annotator would lose the extmark that makes `M.line()` worth calling. Whether `<leader>AS` survives,
since review.nvim cannot auto-submit inside herdr (4.4); keeping it would mean either an
`export.on_export` callback that shells `herdr agent prompt` directly, or accepting that the submit key
goes away. And how the agent picker gets scoped back to the current workspace (4.3), which
`export.on_export` could also own, since a callback replaces the built-in herdr path entirely and could
filter on `HERDR_WORKSPACE_ID` the way `agents.lua` does.

______________________________________________________________________

## 6. Cleanup

Both scratch trees, `/tmp/rv.QTHw` and `/tmp/rv2.miyr`, holding the three clones, the probe scripts, the
throwaway git repositories and the redirected Neovim roots, were trashed at the end of their sittings.
Nothing under `~/.config/nvim` or `~/.local/share/nvim` was written to or read at any point: every plugin
came from a fresh clone at the commit this repository pins. The one file read out of the working tree was
the annotation flow's own source, `dot_config/nvim/`, appended to a headless runtimepath so
`annotate.line()` could be measured; nothing was written back to it. No herdr pane, tab or workspace was
created, no agent was sent anything, and the operator's running editor was never touched.
