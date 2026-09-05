# avante.nvim evaluated against the agent lane

**Date:** 2026-09-05 · **Plan task:** 40a (sibling of the atlas evaluation) · **Spec:** 7.1 to 7.7 ·
**Machine:** dresden **Subject:** `yetone/avante.nvim` at `3f9737ac0b1bd553e39cf8b80ace0ae9ab49fa96`
(default branch `main` head, 34 commits past tag `v0.2.3`, read 2026-09-05) **Incumbent:** the agent lane
this overhaul is building, in three open pull requests: `claudecode.nvim` with `provider = "none"` (PR
337), the shared herdr seam plus the `claude --ide` launch helper (PR 344), and `nvim-mcp` with a
pane-aware socket resolver (PR 339)

This pull request adds this document and nothing else. No plugin was declared through chezmoi, the lock
was not touched, and no `chezmoi apply` was run.

______________________________________________________________________

## 1. Verdict

# DO NOT ADOPT

Keep the agent lane. Avante is a capable Cursor-style assistant, and if this were an empty config it
would be a reasonable pick. It is not an empty config: the overhaul is already building an agent lane
with a specific shape, and avante is the inverse of that shape at the one point that matters most.

**Deciding rationale.** The agent lane's central rule is spec 7.5: a file open in a Neovim buffer is
edited through the Neovim tools in its current unsaved state, never by a disk write that races the
buffer. Every piece of the lane serves that rule. `claudecode.nvim` proposes diffs the operator accepts
in the editor and never writes a proposal buffer to disk (PR 337 built an auto-save exclusion for exactly
this). `nvim-mcp` lets the agent read and edit the LIVE buffer over the remote-procedure-call socket (PR
339). The seam sends a selection to the agent in its herdr pane (PR 344). Avante, on the only provider
reachable here, does the opposite: its agent edits files by writing them to disk and reloading the
buffer, and it has no coordination with an unsaved buffer at all. I reproduced the collision live (4.6):
with an unsaved edit in a `calc.py` buffer, the avante agent appended a line to `calc.py` on disk, and
afterward the disk held the agent's change while the buffer still held the operator's unsaved one, with
no marker, no diff and no merge. That is the precise failure spec 7.5 exists to prevent, and avante
builds it in rather than guards against it.

**The second count is that the feature avante is famous for could not be reached on this machine, and
would cost a secret to reach.** Avante's headline is its native diff apply and conflict resolution: the
sidebar renders a change as `<<<<<<<`/`>>>>>>>` conflict markers and the operator resolves them with
`co`/`ct`/`ca`/`cb` (verified in the pinned source, 4.2), plus Fast Apply through a Morph model. That
whole flow belongs to the DIRECT API providers (claude, openai, copilot). It is NOT what runs on an Agent
Client Protocol provider, where the agent's own edit tool writes to disk instead. The only no-secret
provider available here is the Agent Client Protocol path to the local `claude` binary, so the native
diff engine, the thing that would most distinguish avante from the lane, is untested and untestable here
without an Anthropic or OpenAI key (Copilot was tried and returned 401, 4.1). Adopting avante for its
diff engine means adopting a key to feed it.

**The third count is churn against a fast-moving, recently-transferred project.** The repository moved to
an `avante-corp` organization (the compare links in its own release notes point there), the README still
opens with "the plugin is still under active development... Expect some rough edges and instability," and
the head is 34 commits past the newest tag with 63 commits in the last 30 days (4.7). Its five optional
dependencies (nui, snacks, render-markdown, img-clip, copilot.lua) and a Rust build step that took just
under three minutes on this machine (4.5) are real weight to carry for a plugin whose one differentiating
feature this machine cannot use.

What avante does better than the lane, recorded so it is not re-litigated: it is a single plugin that
gives a full in-editor chat sidebar, `@`-mention context, slash commands, chat history and a repo map,
where the lane is three plugins plus a resolver plus a seam. And its ACP integration genuinely works: I
drove it live to answer a question about the current buffer (4.1), edit a file (4.3), make a multi-edit
change (4.4) and answer a project-wide question by searching the repo (4.5), all against the local
`claude` binary with no new secret. If the goal were "one plugin, agent in a chat buffer, edits to disk,"
avante would win. That goal is the ACP hedge of spec 7.6, which was DEFERRED precisely because an agent
inside a Neovim chat buffer is the inverse of this setup. Avante is a well-built answer to a question
this config has already decided not to ask.

**Reconsider when any of these becomes true.** Spec 7.6 is revisited and cross-agent portability in a
Neovim chat buffer becomes the goal, at which point avante's ACP integration is a strong candidate and
this document should be re-read rather than the evaluation redone. Or a funded API key enters the
workflow AND the operator wants an in-editor diff-apply UI that the agent-in-a-pane lane does not
provide, at which point avante's native conflict flow becomes reachable and worth its own trial. Neither
is true today.

______________________________________________________________________

## 2. How it was tested

Avante was installed by hand for the day, outside chezmoi, into a throwaway Neovim root at `/tmp/av` with
its own `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_STATE_HOME` and `XDG_CACHE_HOME`. That root held a copy
of this branch's `dot_config/nvim` with the two `dot_` source names rendered, plus one extra spec file,
`lua/plugins/zz-avante-eval.lua`, pinning avante to the commit above and building it with `make`. The
real `~/.local/share/nvim` was copied in with `cp -Rc` so lazy had only avante and its dependencies to
clone. Nothing was written to `~/.config/nvim`, `~/.local/share/nvim`, or the branch's `dot_config/nvim`.

The incumbent side, the agent lane, was set up in a second throwaway root at `/tmp/cc` built the same way
from the `nvim-herdr-seam` branch (PR 344, which stacks on PR 337), so the comparison is like with like:
`claudecode.nvim` at its pinned `2390c6e`, the shared seam `custom_api/herdr.lua`, and the `<leader>C*`
keymaps.

Both editors were driven live inside real herdr workspaces created for the evaluation and closed at the
end, never touching the operator's own panes. Avante was additionally driven through a Neovim listen
socket (`nvim --listen`, then `nvim --server ... --remote-expr`), because avante's sidebar renders poorly
in a 64-column pane and the socket let me fire its API and read the result and edited buffers directly
rather than scraping a cramped screen.

The provider question decided which rows could run. Two no-secret paths exist. Copilot was tried first,
because `~/.config/github-copilot` is authenticated for other tools; avante reached it and the request
failed 401 "Bad credentials" (4.1), so that token does not carry avante's use. The Agent Client Protocol
path to the local `claude` binary was then used for every live avante row: avante's `claude-code` ACP
provider spawning `@zed-industries/claude-agent-acp` (pinned by `npx -y`) with
`ACP_PATH_TO_CLAUDE_CODE_EXECUTABLE` pointing at the operator's `claude` and
`ACP_PERMISSION_MODE=bypassPermissions`. No new secret was created, no key was read or printed, and
KeePassXC was never unlocked.

`:checkhealth avante` on the throwaway root, with the ACP provider and snacks input configured:

```
avante.nvim ~
- ✅ OK Found required plugin: nvim-lua/plenary.nvim
- ✅ OK Found required plugin: MunifTanjim/nui.nvim
- ✅ OK Found icons plugin (nvim-web-devicons or mini.icons)
- ✅ OK Found configured input provider: snacks.nvim
- ❌ ERROR Copilot provider is configured but neither copilot.lua nor copilot.vim is installed
- ✅ OK All essential TreeSitter parsers are installed
```

The Copilot line is an artifact of an earlier probe that set `provider = "copilot"`; avante reads the
Copilot token from `~/.config/github-copilot/apps.json` directly and needs `copilot.lua` only for this
health line, not to function. On the ACP provider the health check is clean but for that stale line.

______________________________________________________________________

## 3. Outcome table

Every avante row ran on the Agent Client Protocol provider (the local `claude` binary), the only
no-secret path. The seam rows ran against `claudecode.nvim` plus `custom_api/herdr.lua` from PR 344.

- **Ask about the current buffer: pass, both.** avante: `AvanteAsk` opened the sidebar, the ACP agent
  read the open file with its Read tool and answered correctly (4.1). seam: a paragraph sent with
  `<leader>Cp` reached a real Claude agent in its herdr pane, which replied (4.9).
- **Edit a selection and apply the diff: undecided for avante's native flow, pass for the ACP edit.**
  avante: `AvanteEdit`'s selection capture needs interactive visual mode and errored when driven over the
  socket (4.3); the native conflict-marker apply belongs to the API providers and was not reachable
  without a key. The ACP edit path works: the agent edited the file with its own tool (4.3). seam:
  claudecode proposes a diff the operator accepts with `<leader>Cy`; the at-mention and diff wiring were
  verified live in PR 337, the accept keystroke needs a running `claude --ide` diff and is taken on that
  PR's evidence.
- **A multi-file change: pass, both, agent-driven.** avante: the ACP agent made a two-edit change to one
  file and wrote it to disk (4.4); it can touch many files the same way, because the edits are the
  agent's tools, not avante's. seam plus nvim-mcp: the agent edits any buffer over the MCP socket (PR
  339); not re-driven here.
- **A project-wide question needing context: pass, both.** avante: the ACP agent used Glob to find a file
  across the repo and answered (4.5). This is the agent's own search, not avante's `@codebase` repo map,
  which needs an API provider. seam: the agent in the pane has the same repo tools.
- **Startup cost, lazy: pass.** With avante triggered on `cmd`/`keys` it adds nothing measurable to the
  warm median; on `VeryLazy` it adds about 24 ms of its own startup work (4.8), which is over the spec
  9.1 gate's 10 ms tolerance, so avante belongs on `cmd`/`keys`, not `VeryLazy`.
- **Keymap collisions with `<leader>a`: fail.** Avante's default keymaps claim seventeen `<leader>a*`
  maps and interleave with aerial's, which owns `<leader>a` as its which-key group; five aerial maps
  survive only because lazy's guard skips a key aerial already bound (4.10).
- **Change proposed while an auto-save is pending: fail for avante.** The ACP agent's disk write raced an
  unsaved buffer and the two diverged silently (4.6); avante has no auto-save coordination. The lane is
  built to avoid exactly this (spec 7.5, PR 337's auto-save exclusion), so this is a pass for the
  incumbent.

Counts: two clean passes for avante (ask, project-wide question), two agent-driven passes (multi-file,
ACP edit), one undecided (native diff apply), two fails (keymap collisions, auto-save collision), one
pass on startup once placed correctly.

______________________________________________________________________

## 4. Row detail

### 4.1 Ask about the current buffer: pass (and the Copilot 401)

Copilot was tried first. With `provider = "copilot"`, avante loaded, built the request, and tried to
refresh the Copilot token, which failed:

```
copilot.lua:219: Failed to get success response: {
  body = '{ "message": "Bad credentials", ... "status": "401" }',
  status = 401
}
```

So the token in `~/.config/github-copilot/apps.json` does not authorize avante's Copilot use. This is a
genuine end-to-end reach of the request path, failing at the credential boundary, the same shape the
atlas evaluation saw with GitLab.

On the Agent Client Protocol provider it worked. `require("avante.api").ask({ question = ... })` about
the open `2026-09-atlas-nvim-evaluation.md` buffer opened the sidebar, the ACP adapter spawned, and the
agent used its Read tool and answered `KEEP`, which matches that document's verdict. The sidebar result
buffer showed the turn header `ACP: claude-code` and the selected file, confirming the context reached
the model.

### 4.2 Avante's native diff and conflict flow (source read, not run)

Avante's differentiator is verified in the pinned source rather than run, because it needs an API key.
`lua/avante/sidebar.lua:760` (`insert_conflict_contents`) writes `<<<<<<< HEAD` and `>>>>>>> Snippet`
markers into the code buffer, and `lua/avante/diff.lua` matches `^<<<<<<<` and `^>>>>>>>` and binds the
resolution keys from `Config.mappings.diff` (`co` ours, `ct` theirs, `ca` all theirs, `cb` both, `cc`
cursor, `]x`/`[x` to move between conflicts). Fast Apply (`behaviour.enable_fastapply`) routes edits
through a Morph model (`MORPH_API_KEY`) for instant application. All of this is the API-provider path;
none of it runs on an ACP provider, where the agent's own tools do the editing.

### 4.3 Edit a selection and apply the diff: undecided (native), pass (ACP)

`require("avante.api").edit(...)` driven over the socket errored at
`selection.lua:133: attempt to index field 'selection' (a nil value)`, because avante captures the visual
selection through interactive mode-change autocommands that a scripted `normal! V` does not register.
This is a harness limitation, not an avante defect, but it means the `AvanteEdit` selection path was not
exercised end to end here, and the diff-apply step behind it is the API-provider flow of 4.2, which no
key reached.

The ACP edit path did run (4.4): the agent edited the file with its own Edit tool. So "apply the diff" on
the reachable provider is "the agent writes the file," not "the operator resolves a conflict marker."

### 4.4 A multi-file change: pass (agent-driven)

With an unmodified `calc.py` open, `AvanteAsk` requesting a docstring on both functions produced, over
about 30 seconds: a Read of `calc.py`, then two `Edit calc.py` tool calls, then `DONE`. The file on disk
gained both docstrings and the buffer reloaded to match:

```
def add(a, b):
    """Return the sum of a and b."""
    return a + b

def sub(a, b):
    """Return the difference of a and b."""
    return a - b
```

The edits are the ACP agent's own tools writing to disk. The write handler is
`lua/avante/llm.lua:1308-1329`: `io.open(abs_path, "w")`, write the content, then for every buffer whose
name is that path, `vim.api.nvim_buf_call(buf, function() vim.cmd("edit") end)` to reload it. This scales
to many files trivially, because it is the agent editing, not avante's diff engine. It is also the
mechanism behind the collision in 4.6.

### 4.5 A project-wide question needing context: pass

`AvanteAsk` asking for the filename of the octo evaluation document produced, over about 20 seconds, a
`Glob` for `docs/research/*octo*` (no match), a `Glob` for `docs/research/*atlas*` (one match), and the
correct answer naming `2026-09-atlas-nvim-evaluation.md`. This is the ACP agent's own repository search,
not avante's `@codebase` repo map (which is the API-provider RAG feature, and optionally a dockerized RAG
service). The reachable provider answers project-wide questions by having the agent search, which works.

### 4.6 A change proposed while an auto-save is pending: fail (the deciding row)

This is the row the verdict rests on. I put an unsaved edit into the `calc.py` buffer and then asked the
ACP agent to change the same file on disk.

Before: the buffer's first line was `# UNSAVED_LOCAL_EDIT_SENTINEL`, `modified = true`, and the disk file
did not contain that line. The agent then appended `# AGENT_WROTE_THIS` to `calc.py` with its edit tool.
After:

```
disk (calc.py):        ends with `# AGENT_WROTE_THIS`, no sentinel
buffer (calc.py):      first line still `# UNSAVED_LOCAL_EDIT_SENTINEL`, modified = true,
                       8 lines, no `# AGENT_WROTE_THIS`
```

The write handler's `vim.cmd("edit")` (no bang) cannot reload a modified buffer, so the reload silently
did not happen and the two states diverged. From here `:w` overwrites the agent's change and `:e!`
discards the operator's, a classic lost update, with nothing in avante to warn or merge. The agent lane
is engineered against exactly this: spec 7.5 forbids a disk write that races an unsaved buffer, PR 337
added an auto-save exclusion so proposal buffers are never written, and PR 339's `nvim-mcp` has the agent
edit the LIVE buffer over the socket. Avante inverts the design at its most load-bearing point.

### 4.7 Release velocity and project state

The repository is `yetone/avante.nvim` on the surface, but its own release notes compare against
`avante-corp/avante.nvim`, so the project has moved to an organization. It is fast-moving: the head read
today is 34 commits past the newest tag `v0.2.3`, with 63 commits in the last 30 days and 123 in the last
90\. Tags do not sit on `main`: `v0.2.1`, `v0.2.2` and `v0.2.3` live on a `release-v0.2` branch and are
NOT ancestors of `main`, while `v0.2.0` is the newest tag that is (its merge base with `main` is
`43aa8ee`). The release bodies carry only a "Full Changelog" compare link, no curated breaking-change
notes, so a bump is read by reading commits. The README still says "still under active development...
Expect some rough edges and instability." A pin here would be a commit on `main`, tracked like
claudecode's, with a protocol or config break surfacing at bump time.

### 4.8 Startup cost

Measured with the spec 9.1 method (five headless runs, `doautocmd User VeryLazy`, warm median of runs 2
to 5), from an empty benchmark directory, in three configurations:

| Configuration                | Warm median                 |
| ---------------------------- | --------------------------- |
| avante absent                | 200 to 216 ms (two batches) |
| avante on `event = VeryLazy` | 213 ms                      |
| avante on `cmd` and `keys`   | 214 ms                      |

The medians sit inside the batch-to-batch noise, so the warm median does not separate them. Avante's own
attributed startup work, summed from the `--startuptime` lines, is about 24 ms when `VeryLazy` fires it
(24.5, 22.6, 25.0, 27.9 ms across runs). That is over the spec 9.1 gate's 10 ms tolerance, so avante must
be triggered by `cmd`/`keys`, where it costs nothing until invoked, not by `VeryLazy`. Unlike claudecode,
which needs `VeryLazy` so its lock file exists before a CLI connects (spec 9), avante has no
must-be-present-at-startup requirement, so `cmd`/`keys` is available to it.

### 4.9 The seam side, run live for parity

To compare like with like, the seam rows ran against `claudecode.nvim` plus `custom_api/herdr.lua` from
PR 344. `<leader>Cc` (launch or attach) created a Claude agent named `claude-w1b-p2` in a new pane in the
workspace and wrote a `~/.claude/ide/<port>.lock` file, matching spec 7.2. `<leader>Cp` on a two-line
paragraph reached that agent as one submitted prompt, and it replied `PONG`, with the sentinel line
outside the paragraph not sent. This is the same read-and-answer capability avante's 4.1 shows, reached
through the pane rather than a sidebar. The diff-accept keystroke (`<leader>Cy`) needs a running
`claude --ide` proposing a diff and is taken on PR 337's live at-mention and diff verification rather
than re-driven here.

### 4.10 Keymap collisions

After `VeryLazy` fired avante in a copy of the live config, avante's default keymaps and aerial's
`<leader>a` group interleave. Avante won seventeen `<leader>a*` normal-mode maps (`aa`, `an`, `ar`, `af`,
`aS`, `ad`, `as`, `aR`, `aC`, `aB`, `ah`, `aM`, `am`, `az`, `at`, `a?`, and `ae` in visual mode),
reporting 24 avante-owned global maps in all. Aerial kept five (`ac` close, `ao` open, `aO`
open-and-focus, `aA` and `aT` toggle-and-focus), and only because avante uses lazy's `safe_keymap_set`,
which skips a key a lazy `keys` handler already claimed (`utils/init.lua:223-254`). The result is one
`<leader>a` prefix answering to two plugins with no pattern to it: `<leader>at` toggles the avante
sidebar, `<leader>aT` toggles aerial. `<leader>a` is aerial's which-key group in
`lua/plugins/which-key.lua`; `<leader>A` is herdr and `<leader>C` is claudecode, both taken. An adopt
would have to remap avante's whole group to a free prefix rather than accept the collision. Avante's
in-sidebar keys (`co`/`ct`, `A`, `a`, `@`, and so on) are buffer-local to its own windows and collide
with nothing.

______________________________________________________________________

## 5. What adopting would cost, if the verdict were reversed

Recorded so an adopt is scoped rather than open-ended. It would be one pull request adding a
`lua/plugins/avante.lua` spec pinned to a `main` commit, built with `make` (or the prebuilt fallback,
5.1), lazy on `cmd`/`keys` (4.8), with:

- **A provider and its secret.** For the native diff engine (4.2), an Anthropic or OpenAI key in
  KeePassXC, pulled into the environment the way other secrets are; the agent lane needs no such key
  because it reuses the operator's `claude` login. For the no-secret ACP path, no key, but then avante is
  a second agent-in-a-buffer beside the herdr-pane agents, which is the 7.6 hedge, not this lane.
- **A keymap group of its own.** Remap the whole `<leader>a*` set off aerial's prefix to a free one and
  add a which-key group row (4.10).
- **Dependencies.** `nui.nvim`, `snacks.nvim`, `nvim-web-devicons`/`mini.icons` are already in the lock.
  New ones: `render-markdown.nvim` (or wire the `Avante` filetype into the config's existing
  `markview.nvim`), `img-clip.nvim` for image paste, `copilot.lua` only if the Copilot provider is used,
  and `blink.compat` or `blink-cmp-avante` to feed avante's `@`/`/`/`#` completion sources into this
  config's blink.cmp (it uses blink, not nvim-cmp).
- **The auto-save reconciliation that does not exist.** Because the ACP write path writes disk under the
  buffer (4.6), an adopt would owe the same auto-save exclusion PR 337 built for claudecode, plus a
  buffer-reload strategy that does not lose unsaved edits. This is net-new work avante does not provide.
- **The spec 7.5 rule.** Either avante is registered as the buffer-editing path and 7.5 rewritten around
  it, or avante and the MCP lane both write the same buffers and the rule breaks. Two editors of one
  buffer is the collision 7.5 forbids.

### 5.1 The build step

The Rust libraries (`avante_tokenizers`, `avante_templates`, `avante_repo_map`, `avante_html2md`) build
from `crates/` with `make` and took 2 minutes 52 seconds on this machine by hand; the lazy build hook's
own default 120-second timeout terminated the first pass mid-compile, so a fresh install needs either a
longer build timeout or the prebuilt-binary fallback. That fallback (`build.sh`) downloads a release
asset for the newest tag, and here it resolved the newest tag to `v0.2.0` and fell over: the release API
returned an empty download URL, so no library was fetched. A pin on `main` past `v0.2.0` therefore cannot
rely on the prebuilt path and must build from source with a raised timeout. This is one more moving part
for a plugin whose differentiator this machine cannot use.

______________________________________________________________________

## 6. Cleanup

The two throwaway roots at `/tmp/av` and `/tmp/cc`, and the avante source checkout used to read its code,
were trashed at the end of the evaluation. The copilot auth symlink into `~/.config/github-copilot` was
removed before trashing, so the real directory was never touched. The two throwaway herdr workspaces and
the Claude agent spawned in one of them were closed, and the `~/.claude/ide` lock file my test wrote is
gone. Nothing under `~/.config/nvim` or `~/.local/share/nvim` was written to at any point; both were
read-only inputs, the second only as the source of a `cp -Rc`.
