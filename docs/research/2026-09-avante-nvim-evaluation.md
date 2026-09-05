# avante.nvim evaluated against the agent lane

**Date:** 2026-09-05 · **Plan task:** 40a (sibling of the atlas evaluation) · **Spec:** 7.1 to 7.7 ·
**Machine:** dresden **Subject:** `yetone/avante.nvim` at `3f9737ac0b1bd553e39cf8b80ace0ae9ab49fa96`
(default branch `main` head, 34 commits past tag `v0.2.3`, read 2026-09-05) **Incumbent:** the agent lane
this overhaul is building, in three open pull requests: `claudecode.nvim` with `provider = "none"` (PR
337), the shared herdr seam plus the `claude --ide` launch helper (PR 344), and `nvim-mcp` with a
pane-aware socket resolver (PR 339)

This pull request adds this document and nothing else. No plugin was declared through chezmoi, the lock
was not touched, and no `chezmoi apply` was run.

This is the second version of this record. A review found seven overstated or misattributed claims in the
first; each is corrected here, and the specific claims are called out where they sat so the correction is
legible. The verdict is unchanged, but it now rests on a design-fit judgment and the churn count, not on
a capability the machine could not reach.

______________________________________________________________________

## 1. Verdict

# DO NOT ADOPT

Keep the agent lane. Avante is a capable, well-built Cursor-style assistant, and much of it works here: I
drove it live against a keyless local model and against the operator's Claude login to answer questions,
edit one file and two files, and resolve merge conflicts, all with no new secret. The reason to pass is
not that avante is weak. It is that avante answers a different question than the one this config has
decided to ask.

**Deciding rationale, one count.** The agent lane puts the agent in a herdr pane and has it edit the LIVE
Neovim buffer: `nvim-mcp` reads and writes the open buffer over the remote-procedure-call socket (PR
339), `claudecode.nvim` proposes diffs the operator accepts in the editor (PR 337), and the seam sends a
selection to the agent's pane (PR 344). Avante is the opposite arrangement: the agent lives in an editor
sidebar or chat buffer, and its edits land on DISK, which the open buffer then has to catch up to. That
is the Agent Client Protocol chat-buffer model that spec 7.6 named and DEFERRED, on the stated grounds
that an agent inside a Neovim chat buffer is the inverse of this setup. Adopting avante is adopting the
deferred design. This count needs no capability claim; it is a design-fit judgment, and it is the whole
verdict.

**Two supporting counts, both smaller than the first version of this document claimed.**

- **A disk-write divergence, not a lost update.** On the Agent Client Protocol provider, the agent's own
  edit tool writes files to disk directly, and avante does not reconcile the open buffer, so an unsaved
  buffer and the disk diverge (4.6). This is real and it is the same shape as the count above: the lane
  edits the live buffer, avante edits the disk under it. But it is NOT a silent lost update. I tested the
  auto-save case with timing (4.7): with auto-save enabled, an external write during the debounce window
  makes auto-save's own write refuse with "the file has been changed since reading it," so nothing is
  silently overwritten. The operator is left to reconcile a flagged divergence, not to discover a loss.
- **Churn and build weight against a recently-transferred project.** The repository moved to an
  `avante-corp` organization (its release-note compare links point there), the README still opens with
  "still under active development... Expect some rough edges and instability," the head is 34 commits
  past the newest tag with 63 commits in the last 30 days, and it carries a Rust build that took just
  under three minutes here and whose build hook timed out on the first pass (4.9, 5.1). That is weight to
  carry for a plugin whose role the lane already fills three ways.

**Removed from the first version: the key-requirement count.** The first draft said avante's native diff
engine was untestable without an Anthropic or OpenAI key and made that a deciding count. That was wrong.
Native providers are reachable with no new secret two ways: a keyless local model (Ollama, present on
this machine), and the operator's Claude subscription through `auth_type = "max"`, which is a browser
OAuth flow to claude.ai, not an API key (source at `providers/claude.lua:47-48`). And the conflict
resolution the engine is built around has no provider dependency at all: I drove it keyless (4.2). The
count is deleted.

What avante does better than the lane, recorded so it is not re-litigated: it is a single plugin that
gives a full in-editor chat sidebar, `@`-mention context, slash commands, chat history, a repo map, and a
native merge-conflict resolver, where the lane is three plugins plus a resolver plus a seam. Its Agent
Client Protocol integration genuinely works against the operator's local `claude` with no new secret. If
spec 7.6 is ever revisited and the goal becomes an agent in a Neovim chat buffer, avante is a strong
candidate, and this document should be re-read rather than the evaluation redone.

______________________________________________________________________

## 2. How it was tested

Avante was installed by hand for the day, outside chezmoi, into a throwaway Neovim root at `/tmp/av` with
its own `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_STATE_HOME` and `XDG_CACHE_HOME`. That root held a copy
of this branch's `dot_config/nvim` with the two `dot_` source names rendered, plus one extra spec file,
`lua/plugins/zz-avante-eval.lua`, pinning avante to the commit above and building it with `make`. The
real `~/.local/share/nvim` was copied in with `cp -Rc` so lazy had only avante and its dependencies to
clone. Nothing was written to `~/.config/nvim`, `~/.local/share/nvim`, or the branch's `dot_config/nvim`.

Every live probe ran in a DETACHED headless Neovim (`nvim --headless --listen <socket>` with the
throwaway roots), driven from outside by `nvim --server <socket> --remote-expr`. No pane, tab or
workspace was opened in the operator's herdr session, and the operator's Neovim was never attached to.
This corrects the first evaluation, which created herdr workspaces and surfaced an avante prompt to the
operator; the findings below were all captured over the socket and from on-disk files, so none of them
depend on a visible screen.

Two no-secret providers were used. The Agent Client Protocol path to the local `claude` binary carried
the agent rows: avante's `claude-code` provider spawning `@zed-industries/claude-agent-acp` for the
buffer, edit, multi-file and project rows. Ollama (`qwen3.5:4b`, already pulled) was declared as a
keyless NATIVE provider for the native-editing attempt. A third path, the operator's Claude subscription
via `auth_type = "max"`, was NOT exercised because it opens a browser OAuth flow the operator would have
to complete; it is recorded as available-without-a-new-secret rather than tested. Copilot was tried in
the first evaluation and returned 401; that token does not carry avante's use. No new secret was created,
no key was read or printed, and KeePassXC was never unlocked.

______________________________________________________________________

## 3. Outcome table

Native editing (avante's own diff engine) and Agent Client Protocol editing (the agent's own tools) are
now distinct rows, because they are different code paths with different results. "Native" means a
provider avante talks to directly (Ollama here, or Claude via API or subscription); "ACP" means the
agent-in-an-adapter path.

- **Ask about the current buffer (ACP): pass.** The agent read the open file with its Read tool and
  answered correctly (4.1).
- **Conflict resolution, the native diff engine's apply step (keyless): pass.** Driven with no provider
  and no network: a buffer of conflict markers resolved correctly to either side (4.2).
- **End-to-end native edit, model proposes then the sidebar applies (Ollama): undecided.** The range edit
  call was accepted, but the 4-billion-parameter local model produced no usable apply within the time
  budget (4.3). Not an avante limit; a model-and-harness limit, recorded as undecided.
- **Selection editing, the range invocation (ACP): call accepted, buffer result undecided.**
  `api.edit(request, line1, line2)` takes explicit ranges and needs no visual-mode transition, correcting
  the first version's "errored" claim (4.3). Over ACP the call returned cleanly but produced no buffer
  change in 120 seconds; the buffer-update mechanism (`nvim_buf_set_lines`) is the native path, untested
  here.
- **A multi-file change (ACP): pass.** The agent edited two distinct files, `calc.py` and `calc2.py`, in
  one turn (4.5).
- **A multi-file change (native diff engine): undecided.** Not reached, for the same reason the native
  end-to-end row is undecided.
- **A project-wide question needing context (ACP): pass.** The agent used Glob to find a file across the
  repo and answered (4.4).
- **Startup cost, lazy: pass (advisory).** Avante on `cmd`/`keys` adds nothing measurable; on `VeryLazy`
  its exclusive self-time is about 7 ms warm (4.8). Spec 9.1 gates the warm-median difference, which the
  batches here do not resolve, so this is advisory, not a gate result.
- **Keymap collisions with `<leader>a`: fail.** Avante's defaults claim seventeen `<leader>a*` maps and
  interleave with aerial's group; five aerial maps survive only because lazy's guard skips a pre-bound
  key (4.10).
- **Unsaved-buffer divergence when a disk write races an open buffer (ACP): observed; not a silent
  loss.** The agent's native edit tool wrote disk and the open unsaved buffer did not reload (4.6). With
  auto-save enabled and timed, the auto-save write is refused with a file-changed warning, so no silent
  lost update occurs (4.7).

Counts: five pass (ask, keyless conflict resolution, multi-file ACP, project-wide question, startup),
three undecided (native end-to-end edit, ACP selection buffer-update, multi-file native), one fail
(keymap collisions), one observed-divergence (not a pass or fail: a real behavior, milder than the first
version claimed).

______________________________________________________________________

## 4. Row detail

### 4.1 Ask about the current buffer (ACP): pass

`require("avante.api").ask({ question = ... })` about the open `2026-09-atlas-nvim-evaluation.md` buffer
opened the sidebar, the adapter spawned, the agent used its Read tool, and it answered `KEEP`, which
matches that document's verdict. The turn header read `ACP: claude-code`, confirming the context reached
the model.

### 4.2 Conflict resolution, keyless: pass

The first version marked native editing untested and implied a key was needed. That was too broad: the
conflict-resolution step has no provider check and no network call, and I drove it with no provider
configured at all. The apply path registers the buffer first (`avante.diff.add_visited_buffer`) and then
parses it (`avante.diff.process`); the first version's attempt failed only because it skipped the
register step, which is why `parse_buffer` indexed a nil at `diff.lua:325`.

With a buffer of hand-written markers:

```
conflict.py:  def add(a, b) / <<<<<<< HEAD / return a + b / ======= /
              """Return the sum...""" / return a + b / >>>>>>> Snippet
process(bufnr)        -> conflict_count = 1
choose("theirs")      -> conflict_count = 0, buffer = def add(a,b) / """Return the sum...""" / return a + b
choose("ours")        -> conflict_count = 0, buffer = def add(a,b) / return a + b
```

Both sides resolved correctly, buffer-only, no disk write, no network. So the diff engine's resolution
mechanics work with no key. The one part that needs a provider is the LLM PROPOSING the snippet that
becomes those markers; the resolution itself does not.

### 4.3 Selection editing, the range invocation

The first version said `AvanteEdit` errored and implied the selection path was unusable. That was a
harness error, not an avante one: `api.edit(request, line1, line2)` takes explicit line ranges
(`selection.lua:250-262`) and needs no visual-mode transition; the first version called it with no range,
so it fell through to `get_visual_selection_and_range`, which returned nil outside visual mode.

Called correctly with a range, `api.edit("...", 1, 2)` returned `ok = true` with the selection object
set. Over the ACP provider, though, it produced no buffer change in 120 seconds: the selection-edit path
expects the provider to stream a code block that lands through `nvim_buf_set_lines` (`selection.lua:175`,
buffer-only, no disk write), and the ACP agent replies with tool calls rather than a plain code block. So
the range invocation is usable (correcting the "errored" claim), but the ACP selection-edit OUTCOME is
undecided, separate from the conflict application of 4.2.

### 4.4 A project-wide question needing context (ACP): pass

`AvanteAsk` for the filename of the octo evaluation document produced a `Glob` for `*octo*` (no match), a
`Glob` for `*atlas*` (one match), and the correct answer naming `2026-09-atlas-nvim-evaluation.md`. This
is the agent's own repository search, not avante's `@codebase` repo map (which is a native-provider
feature). The reachable provider answers project-wide questions by having the agent search.

### 4.5 A multi-file change (ACP): pass

The first version marked this a pass on two edits to one file, which the review correctly rejected: two
edits to one file do not show multi-file. Re-run touching two files. With `calc.py` (add/sub) and
`calc2.py` (mul) both open, one `AvanteAsk` to docstring the function in each produced both edits in 15
seconds:

```
calc.py:  add + sub each gain a triple-quoted docstring
calc2.py: mul gains a triple-quoted docstring
```

So multi-file editing over ACP is a genuine pass. It is the agent's own tools doing it, not avante's diff
engine, which is why the native multi-file row stays undecided.

### 4.6 Unsaved-buffer divergence, and why it is not the fs handler

The first version assigned this collision to avante's own filesystem handler and to an unguarded
`vim.cmd("edit")`. Both attributions were wrong, and the review caught both.

What actually happens: the tested adapter and Claude Code use their own native Read/Edit tools, which
write to disk directly through the OS. They do NOT call avante's Agent Client Protocol filesystem handler
at `llm.lua:1308` (that handler only fires on an ACP `fs/write_text_file` request, which the native tools
do not send). So avante is never told the file changed, and the open buffer is never reloaded.
Reproduced: an unsaved `# UNSAVED_LOCAL_EDIT_SENTINEL` in the buffer, the agent appended
`# AGENT_WROTE_THIS` to disk, and afterward disk had the agent's line while the buffer kept the
operator's unsaved one. The divergence is real.

The unguarded `vim.cmd("edit")` I cited is a SEPARATE, INSPECTED risk, not the cause of the above.
Verified independently: `vim.cmd("edit")` on a modified buffer throws `E37: No write since last change`
(a `pcall` returns `ok = false`), rather than silently skipping the reload as the first version claimed.
That handler only runs on the ACP fs path, which the native tools bypass, so it was not on the path of
the observed divergence at all. The two are now separated and the causal link is dropped.

### 4.7 The auto-save case, timed: no silent lost update

The first version claimed a lost update with a pending auto-save. Its throwaway config had auto-save
`enabled = false`, so no timer was ever armed; the claim was unsupported. Re-run with auto-save ENABLED
(the config's real setting is off, but the operator toggles it on with `<leader>uv`) and the timing
recorded:

```
auto-save on, debounce 1000 ms, write_all_buffers false
T0        buffer edited to BUFFER_EDIT via real keystrokes (InsertLeave arms the deferred save), modified=true
T0+0.2s   external write to the same file on disk (stands in for the agent's native edit), inside the window
T0+1.8s   disk = EXTERNAL_AGENT_WRITE ; buffer = BUFFER_EDIT, modified=true
          :messages -> "WARNING: The file has been changed since reading it!!!"
```

Auto-save's own `silent! write` (`auto-save.nvim` at the pinned commit) hit Vim's changed-on-disk guard
and REFUSED to overwrite. So the pending auto-save does not silently lose the agent's write, and it does
not silently clobber the operator's buffer either: it leaves a flagged divergence for the operator to
resolve. A subsequent manual `:w` meets the same guard and prompts. So the correct label is
unsaved-buffer divergence, potential-not-observed loss, and the pending-auto-save behavior is now tested
rather than asserted.

### 4.8 Startup cost

The first version summed the `self+sourced` column and reported about 24 ms, which double-counts the
imports each avante file pulls in. The exclusive `self` column is the right measure. Five `VeryLazy`
runs, spec 9.1 method:

| Run           | Exclusive self-time   |
| ------------- | --------------------- |
| 1 (cold)      | 12.1 ms               |
| 2 to 5 (warm) | 7.8, 7.6, 6.6, 7.7 ms |

So avante's own warm startup cost is about 7 ms, not 24. Spec 9.1 gates the difference in the whole
config's WARM MEDIAN, and the batches here (avante-absent 200 to 216 ms across two batches, avante on
`VeryLazy` 213 ms) sit inside their own noise and do not resolve a 7 ms move. The first version's
"exceeds the 10 ms tolerance, so it must be `cmd`/`keys`" claim is therefore removed; the placement
guidance is now advisory. Placing avante on `cmd`/`keys` is still the natural choice (it has no
must-be-present-at-startup requirement, unlike claudecode's lock file), but the startup numbers do not
compel it.

### 4.9 Release velocity and project state

The repository is `yetone/avante.nvim` on the surface, but its release-note compare links point at
`avante-corp/avante.nvim`, so the project has moved to an organization. The head read today is 34 commits
past the newest tag `v0.2.3`, with 63 commits in the last 30 days and 123 in the last 90. Tags do not sit
on `main`: `v0.2.1` through `v0.2.3` live on a `release-v0.2` branch and are not ancestors of `main`,
while `v0.2.0` is the newest ancestor tag. Release bodies carry only a compare link, no curated
breaking-change notes. The README still says "still under active development... Expect some rough edges
and instability." A pin here would be a commit on `main`, tracked like claudecode's.

### 4.10 Keymap collisions

After `VeryLazy` fired avante in a copy of the live config, avante's default keymaps and aerial's
`<leader>a` group interleave. Avante won seventeen `<leader>a*` normal-mode maps (24 avante-owned global
maps in all). Aerial kept five (`ac`, `ao`, `aO`, `aA`, `aT`), and only because avante uses lazy's
`safe_keymap_set`, which skips a key a lazy `keys` handler already claimed (`utils/init.lua:223-254`).
The result is one `<leader>a` prefix answering to two plugins with no pattern: `<leader>at` toggles the
avante sidebar, `<leader>aT` toggles aerial. `<leader>a` is aerial's which-key group; `<leader>A` is
herdr and `<leader>C` is claudecode, both taken. An adopt would remap avante's whole group to a free
prefix. Avante's in-sidebar keys are buffer-local and collide with nothing.

### 4.11 The reproduction, pinned

The first version asserted the adapter ran the operator's `claude` binary under two `ACP_*` settings.
Both were wrong; the review caught it and the source confirms the correction.

- **Adapter:** `@zed-industries/claude-agent-acp` version `0.23.1`. The spec now pins `@0.23.1` rather
  than resolving `npx -y` unversioned.
- **Runtime:** the adapter bundles `@anthropic-ai/claude-agent-sdk` `0.2.83`, whose `cli.js` reports
  Claude Code runtime `VERSION "2.1.83"`. That is the runtime that ran, not the operator's `claude`
  (which is `2.1.257`). The first version's operator-binary assertion is removed.
- **Configuration:** the adapter reads `CLAUDE_CODE_EXECUTABLE` (and `MAX_THINKING_TOKENS`, `IS_SANDBOX`,
  `CLAUDE_CONFIG_DIR`), and it takes permissions from its settings manager. Both settings the first
  version named, `ACP_PATH_TO_CLAUDE_CODE_EXECUTABLE` and `ACP_PERMISSION_MODE`, appear zero times in the
  adapter's code and are ignored. The eval spec now sets neither, so the adapter located `claude` through
  its own resolver and ran its bundled runtime.

______________________________________________________________________

## 5. What adopting would cost, if the verdict were reversed

One pull request adding a `lua/plugins/avante.lua` spec pinned to a `main` commit, built with `make` (the
prebuilt fallback is unreliable, 5.1), lazy on `cmd`/`keys`, with:

- **A provider.** No new secret is strictly required: a keyless local model (Ollama) or the operator's
  Claude subscription through `auth_type = "max"` (browser OAuth, no API key) both drive the native
  engine. An API key buys a stronger native model and Fast Apply, but is not a precondition.
- **A keymap group of its own,** remapped off aerial's `<leader>a` prefix, with a which-key group row
  (4.10).
- **Dependencies.** `nui.nvim`, `snacks.nvim`, `nvim-web-devicons`/`mini.icons` are already in the lock.
  New: `render-markdown.nvim` (or wire the `Avante` filetype into this config's `markview.nvim`),
  `img-clip.nvim` for image paste, `copilot.lua` only if the Copilot provider is used, and `blink.compat`
  or `blink-cmp-avante` to feed avante's completion sources into this config's blink.cmp.
- **A buffer-reconciliation decision.** Because the agent path writes disk under the open buffer (4.6),
  an adopt would owe a reload strategy that does not lose unsaved edits, plus the spec 7.5 question of
  whether avante or the MCP lane owns buffer editing. Two editors of one buffer is a divergence the lane
  avoids by design.

### 5.1 The build step

The Rust libraries (`avante_tokenizers`, `avante_templates`, `avante_repo_map`, `avante_html2md`) build
from `crates/` with `make` and took 2 minutes 52 seconds here by hand; lazy's build hook (default
120-second timeout) terminated the first pass mid-compile, so a fresh install needs a longer build
timeout or the prebuilt fallback. That fallback (`build.sh`) resolves the newest tag to `v0.2.0` and,
here, returned an empty download URL, so it fetched nothing. A pin on `main` past `v0.2.0` therefore must
build from source with a raised timeout.

______________________________________________________________________

## 6. Cleanup

The throwaway root at `/tmp/av` and the avante source checkout were trashed at the end of the evaluation;
the copilot auth symlink into `~/.config/github-copilot` was removed before trashing so the real
directory was never touched. All live probes ran in detached headless Neovim instances against the
throwaway roots, quit at the end; no herdr pane, tab or workspace was created this round, and the
operator's Neovim was never attached to. Nothing under `~/.config/nvim` or `~/.local/share/nvim` was
written to; both were read-only inputs, the second only as the source of a `cp -Rc`.
