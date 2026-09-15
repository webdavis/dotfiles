# Neovim acceptance drills

Two drills, both run by hand at the operator's own keyboard inside a live Herdr (terminal workspace
manager) session. A headless start or a private test fixture cannot show what these show: rendered screen
output, live pane routing, and an editor watching an agent pane it is actually connected to. A synthetic
or headless run does not establish rendered acceptance (docs/remaining-work.md, task 65).

## Drill one: B103 pane-move routing

Proves that after a pane moves to a different tab in the same Herdr workspace, the shared herdr seam
(`dot_config/nvim/lua/custom_api/herdr.lua`) resolves the agent in the pane's CURRENT tab, not the tab it
launched in. [Pull request #543](https://github.com/webdavis/dotfiles/pull/543) fixed this by reading the
live pane's `tab_id` from `herdr pane current --current` and filtering the workspace's Claude agents
against it (`herdr.lua:163-176`), replacing a call into the third-party `herdr-nvim` plugin's
`agents.resolve()`, which read the pane's launch-time `HERDR_TAB_ID` environment variable and never
updated it. Before the fix, a pane moved from tab A to tab B, sent through `<leader>Cp`, still resolved
the Claude agent that had been in tab A.

Deployment: local `main` contains the merge (`41423df8`); the fix is live on this machine already. This
drill is the live acceptance PR #543 itself calls out as still open.

### Setup

1. Open a Herdr session: `herdr` (or attach to the persistent one already running).
1. Note the current workspace id: `herdr workspace list`, and read the `workspace_id` column for the
   workspace you are in (for example `w2W`). Substitute it for `<workspace>` below.
1. Create a second tab in the same workspace, with a shell at its prompt:
   `herdr tab create --workspace <workspace> --label b103-tab-b --no-focus` Read its `tab_id` from the
   command's JSON output (for example `w2W:t2`); substitute it for `<tab-b>`. The tab you started in is
   `<tab-a>`.
1. In tab A's pane, start Claude: `herdr agent start claude-a --kind claude --pane <tab-a-pane>`.
1. In tab B's pane, start a second Claude:
   `herdr agent start claude-b --kind claude --pane <tab-b-pane>`.
1. Confirm both are visible and correctly scoped: `herdr agent list`. Each row must show
   `workspace_id = <workspace>` and the `tab_id` of the tab it was started in. **Pass**: two Claude
   agents, one per tab, both in this workspace. **Fail**: stop and fix the setup before continuing; a
   drill run against a bad setup proves nothing.
1. Open Neovim in tab A, in a scratch buffer with unsaved text: `nvim /tmp/b103-drill.md`, type a line of
   text, do not save.

### Procedure

1. From the Neovim buffer in tab A, send the buffer's paragraph to the Claude agent: press `<leader>Cp`
   in normal mode (`dot_config/nvim/lua/plugins/claudecode.lua:87`, routed through
   `custom_api.herdr.send_selection_or_paragraph` → `M.send` → `M.agent_pane`). Watch both panes. **Look
   for**: the text you typed appears in the tab A Claude pane's terminal, submitted. **Pass**: it appears
   in tab A only. **Fail**: it appears in tab B, or in neither.
1. Move the Neovim pane itself from tab A into tab B, without moving the Claude agent panes:
   `herdr pane move <nvim-pane-id> --tab <tab-b> --no-focus`. Confirm the move landed:
   `herdr pane current --pane <nvim-pane-id>` must report `tab_id = <tab-b>`.
1. Back in the same, still-open, still-unsaved Neovim buffer (do not restart Neovim), edit the line so
   the text is visibly different from step 1 (so a reused stale send cannot be mistaken for a fresh one),
   then send again with `<leader>Cp`. Watch both panes. **Look for**: which pane's terminal receives the
   new text. **Pass**: the new text appears in the tab B Claude pane, the tab this editor is now in.
   **Fail**: the new text appears in the tab A Claude pane, the tab this editor used to be in. That is
   exactly B103's defect: routing followed the stale launch-time tab instead of the live one.
1. As a second, independent confirmation, run `herdr pane current --current` from a shell in the Neovim
   pane's terminal (or trust the JSON `herdr.lua` itself reads) and compare its `tab_id` against
   `herdr agent list`'s `tab_id` for whichever Claude pane received the text in step 3. **Pass**: they
   match. **Fail**: they differ.

### Open question

No Herdr or Neovim surface prints "agent X resolved this send" as a labelled line; the only observable
this drill found is which pane's terminal receives the sent text, watched directly. If a future Herdr or
plugin version adds a resolution log or a visible confirmation naming the target pane, prefer it over
watching both terminals.

## Drill two: task 65's Neovim overhaul acceptance record

Task 65 (docs/remaining-work.md) requires closing the items the private, headless audit in
[`docs/research/2026-09-nvim-overhaul-acceptance.md`](../research/2026-09-nvim-overhaul-acceptance.md)
could not: rendered, on-screen behavior a headless run cannot observe.

The ledger's expected which-key groups (`X = xcode`, `d = do`) are stale. The current source
(`dot_config/nvim/lua/plugins/which-key.lua`) declares `x = xcode` (line 55), `d = docker` (line 34),
plus `X = diagnostics/quickfix` (line 56), `A = herdr` (line 31), `C = claude` (line 33), `t = test`
(line 52) and `L = lazy` (line 45). Step 4 below checks the current declarations, not the stale ones.

### Procedure

1. **Five silent starts, rendered.** Quit any running Neovim. Launch it interactively five times in a row
   (`nvim`, `:q`, repeat), each time in front of the screen. **Look for**: any error, warning, or stack
   trace in the command line or on exit. **Pass**: all five starts are silent. **Fail**: any one of the
   five prints something.
1. **Full-plugin health output.** In a fresh interactive start, run `:Lazy! load all` first (loads every
   deferred plugin, including the `VeryLazy` ones), then `:checkhealth`. Read the full report. **Pass**:
   no failures beyond the ones already catalogued in the "Health and deployment differences" table of the
   existing acceptance record. **Fail**: a new, uncatalogued error or failed check appears.
1. **Quiescent startup comparison.** With no other agents or heavy processes running, measure a cold
   start: `nvim --startuptime /tmp/b103-startuptime.log +q` then read the last line's total. Repeat once
   more back to back for a warm figure. Compare both against the retained baseline of 164.178 ms and the
   synthetic warm-start figures already on record (94.373 ms source, 95.845 ms installed copy), keeping
   the plan's `after < baseline - 10` pass condition for the warm figure. Separately, start Neovim
   normally, run `:Lazy! load all`, and time that render by eye or with `:Lazy profile`. **Pass**: the
   warm figure still beats `baseline - 10`, and the rendered figure with every `VeryLazy` plugin loaded
   is recorded (no threshold defined for it; task 65 asks only that it be recorded). **Fail**: the warm
   figure regresses past `baseline - 10`.
1. **Rendered which-key groups.** In a normal buffer, press `<leader>` and hold, or press it and wait for
   the which-key popup. Read the rendered group list. **Look for**: `x` labelled `xcode`, `d` labelled
   `docker`, `X` labelled `diagnostics/quickfix`, `A` labelled `herdr`, `C` labelled `claude`, `t`
   labelled `test`, `L` labelled `lazy`. **Pass**: all seven match. **Fail**: any label differs,
   including the stale `X = xcode` / `d = do` pairing task 65 flagged.
1. **Both agent loops, unsaved buffer.** Open a scratch buffer, type unsaved text.
   - Claude loop: press `<leader>Cc` to launch or attach
     (`dot_config/nvim/lua/plugins/claudecode.lua:98`); confirm the Claude pane connects (`/ide` succeeds
     or a new pane opens and connects). Press `<leader>Cp` to send the paragraph (line 87); confirm it
     lands in the Claude pane. Select a range and press `<leader>Cs` in visual mode (line 70) to exercise
     `ClaudeCodeSend`; confirm a diff or send occurs. Use `<leader>Cy` / `<leader>Cn` (lines 72-73) to
     accept or deny a proposed diff if one opens; confirm the buffer updates (accept) or is left unsaved
     and unchanged (deny). Put the agent into a blocked state (mid-approval) and press `<leader>Cp`
     again; confirm the refusal message from `custom_api.herdr.M.send`'s `blocked` verdict (`herdr.lua`,
     the `VERDICTS` table) and that nothing was typed into the agent's pane.
   - Herdr annotation loop: select a range and press `<leader>Ac` in visual mode to comment it, press
     `<leader>Ac` in normal mode on a line to comment it
     (`dot_config/nvim/lua/plugins/herdr-nvim.lua:8-19`), `<leader>Al` to list comments (line 23),
     `<leader>As` to paste them to the agent without submitting and `<leader>AS` to send and submit
     (lines 30, 37). Confirm each lands in the agent pane as expected and that `As` does not submit while
     `AS` does. **Pass**: every keymap above does what its description says, on screen, against a real
     agent pane. **Fail**: any keymap silently no-ops, errors, or sends to the wrong pane.
1. **Swift and custom-plugin behavior.** In a Swift/Xcode project buffer, exercise the `<leader>x` xcode
   group: build, and run a test. **Look for**: real UIKit or Vapor compiler diagnostics, a real build or
   test result, on-save formatting firing (write the buffer, confirm the formatter ran). **Pass**:
   build/test completes and reports a real result; formatting is visible on save. **Fail**: no
   diagnostics, a build that silently does nothing, or a save with no formatting. For the custom pns.nvim
   plugin, trigger a notification-worthy event (for example, let a long-running command in an agent pane
   finish) and confirm a rendered notification arrives through it.
1. **Clean-home apply, then a quiet repeat.** Using a throwaway `HOME` with no prior Neovim state, run
   the bootstrap/full apply this configuration ships (the installer PR #385 delivers). **Pass**: the
   apply completes and Neovim starts cleanly against that fresh home. Run the same apply again
   immediately. **Pass**: the second run is quiet (reports nothing changed) rather than re-doing work or
   erroring. **Fail**: either run errors, or the repeat is not quiet.
1. **Inventory-to-pull-request mapping.** The operator confirms each inventory row has a merged pull
   request, against the mapping table in `docs/research/2026-09-nvim-overhaul-acceptance.md`.

## Results

Record what was observed and the date.

- Drill one run on: **\_\_\_\_**. Pass/fail per step:
- Drill two run on: **\_\_\_\_**. Pass/fail per step:
