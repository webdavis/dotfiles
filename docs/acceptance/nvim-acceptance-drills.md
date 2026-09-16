# Neovim acceptance drills

Two drills, both run by hand at the operator's own keyboard inside a live Herdr (terminal workspace
manager) session. A headless start or a private test fixture cannot show what these show: rendered screen
output, live pane routing, and an editor watching an agent pane it is actually connected to. A synthetic
or headless run does not establish rendered acceptance (docs/remaining-work.md, task 65).

## Drill one: B103 pane-move routing

In plain terms: Neovim sends the current paragraph to the Claude agent in its own tab. The bug (B103) was
that it kept sending to the tab it started in, even after the pane moved somewhere else. This drill moves
the Neovim pane to a new tab and sends again, to check that the send now follows the pane, not its
launch-time tab.

Mechanically: the shared herdr seam (`dot_config/nvim/lua/custom_api/herdr.lua`) must resolve the agent
in the pane's CURRENT tab. [Pull request #543](https://github.com/webdavis/dotfiles/pull/543) fixed this
by reading the live pane's `tab_id` from `herdr pane current --current` and filtering the workspace's
Claude agents against it (`herdr.lua:163-176`), replacing a call into the third-party `herdr-nvim`
plugin's `agents.resolve()`, which read the pane's launch-time `HERDR_TAB_ID` environment variable and
never updated it.

Deployment: local `main` contains the merge (`41423df8`); the fix is live on this machine already.

### Setup

1. Open a Herdr session, or attach to the one already running: `herdr`.
1. Find your workspace id: run `herdr workspace list` and read the `workspace_id` column (for example
   `w2W`). Use it below as `<workspace>`.
1. Create a second tab in the same workspace:
   `herdr tab create --workspace <workspace> --label b103-tab-b --no-focus`. Its `tab_id` is in the JSON
   output (for example `w2W:t2`); call it `<tab-b>`. The tab you started in is `<tab-a>`.
1. In tab A's pane, start Claude: `herdr agent start claude-a --kind claude --pane <tab-a-pane>`.
1. In tab B's pane, start a second Claude:
   `herdr agent start claude-b --kind claude --pane <tab-b-pane>`.
1. Run `herdr agent list` and check both rows: each must show `workspace_id = <workspace>` and the
   `tab_id` of the tab it started in. **Pass**: two Claude agents, one per tab. **Fail**: stop and fix
   the setup first; a drill run against a bad setup proves nothing.
1. Open Neovim in tab A on a scratch file with unsaved text: `nvim /tmp/b103-drill.md`, type a line, do
   not save.

### Procedure

1. In the Neovim buffer, press `<leader>Cp` in normal mode to send the paragraph to the Claude agent
   (`dot_config/nvim/lua/plugins/claudecode.lua:87`, via `custom_api.herdr.send_selection_or_paragraph`).
   Watch both Claude panes. **Pass**: the text appears in tab A's pane only. **Fail**: it appears in tab
   B, or in neither.
1. Find the Neovim pane's id: run `:echo $HERDR_PANE_ID` inside Neovim, or cross-check with
   `herdr pane list --workspace <workspace>`. Then move that pane to tab B, leaving the Claude panes
   where they are: `herdr pane move <nvim-pane-id> --tab <tab-b> --split right --no-focus`. Confirm the
   move landed: `herdr pane current --pane <nvim-pane-id>` must report `tab_id = <tab-b>`. **Do not**
   check `$HERDR_TAB_ID` from inside Neovim to confirm this: it is set once when the pane starts and
   never updates, which is exactly the staleness B103 is about (`herdr.lua:46-47`, and the B103 bullet in
   `docs/remaining-work.md`).
1. In the same still-open, still-unsaved buffer, edit the line so the text is visibly different from step
   1 (a reused stale send should not be mistaken for a fresh one), then send again with `<leader>Cp`.
   Watch both panes. **Pass**: the new text appears in tab B's Claude pane, the tab Neovim is now in.
   **Fail**: it appears in tab A's Claude pane, the tab Neovim used to be in. That is B103's defect:
   routing followed the stale launch-time tab instead of the live one.
1. Confirm independently: run `herdr pane current --current` from a shell in the Neovim pane's terminal,
   and compare its `tab_id` against `herdr agent list`'s `tab_id` for whichever Claude pane received the
   text in step 3. **Pass**: they match. **Fail**: they differ.

### Open question

No Herdr or Neovim surface prints "agent X resolved this send" as a labelled line; the only observable
this drill found is which pane's terminal receives the sent text, watched directly. If a future Herdr or
plugin version adds a resolution log or a visible confirmation naming the target pane, prefer it over
watching both terminals.

## Drill two: task 65's Neovim overhaul acceptance record

In plain terms: this drill watches the real, on-screen Neovim rather than a headless test, because a
headless run cannot see a popup render, a build finish, or a notification arrive. Task 65
(docs/remaining-work.md) lists exactly the things the private, headless audit in
[`docs/research/2026-09-nvim-overhaul-acceptance.md`](../research/2026-09-nvim-overhaul-acceptance.md)
could not check for that reason.

The ledger's expected which-key groups (`X = xcode`, `d = do`) are stale. The current source
(`dot_config/nvim/lua/plugins/which-key.lua`) declares `x = xcode` (line 55), `d = docker` (line 34),
plus `X = diagnostics/quickfix` (line 56), `A = herdr` (line 31), `C = claude` (line 33), `t = test`
(line 52) and `L = lazy` (line 45). Step 4 below checks the current declarations, not the stale ones.

### Procedure

1. **Five silent starts, rendered.** Quit any running Neovim. Launch it five times in a row (`nvim`,
   `:q`, repeat), watching the screen each time. **Pass**: all five starts are silent, no error, warning,
   or stack trace. **Fail**: any one of the five prints something.
1. **Full-plugin health output.** Start Neovim. Run `:Lazy! load all` to load every deferred plugin,
   including the `VeryLazy` ones. Then run `:checkhealth` and read the full report. **Pass**: no failures
   beyond the ones already listed in the "Health and deployment differences" table of the existing
   acceptance record. **Fail**: a new, uncatalogued error or failed check appears.
1. **Quiescent startup comparison.** With no other agents or heavy processes running, run
   `nvim --startuptime /tmp/b103-startuptime.log +q` and read the last line's total: that is the cold
   start. Run it again right after for a warm figure. Compare both against the retained baseline of
   164.178 ms and the synthetic warm-start figures already on record (94.373 ms source, 95.845 ms
   installed copy); the pass condition for the warm figure is `after < baseline - 10`. Separately, start
   Neovim normally, run `:Lazy! load all`, and time that render by eye or with `:Lazy profile`. **Pass**:
   the warm figure still beats `baseline - 10`, and the rendered figure with every `VeryLazy` plugin
   loaded is recorded (task 65 sets no threshold for it, only that it be recorded). **Fail**: the warm
   figure regresses past `baseline - 10`.
1. **Rendered which-key groups.** In a normal buffer, press `<leader>` and hold, or press it and wait for
   the which-key popup. Read the rendered group list. **Pass**: `x` is labelled `xcode`, `d` is `docker`,
   `X` is `diagnostics/quickfix`, `A` is `herdr`, `C` is `claude`, `t` is `test`, `L` is `lazy`, all
   seven. **Fail**: any label differs, including the stale `X = xcode` / `d = do` pairing task 65
   flagged.
1. **Both agent loops, unsaved buffer.** Open a scratch buffer and type unsaved text.
   - Claude loop: press `<leader>Cc` to launch or attach
     (`dot_config/nvim/lua/plugins/claudecode.lua:98`). Confirm the Claude pane connects (`/ide` succeeds
     or a new pane opens and connects). Press `<leader>Cp` to send the paragraph (line 87); confirm it
     lands in the Claude pane. Select a range and press `<leader>Cs` in visual mode (line 70) to exercise
     `ClaudeCodeSend`; confirm a diff or send occurs. Press `<leader>Cy` / `<leader>Cn` (lines 72-73) to
     accept or deny a proposed diff, if one opens; confirm the buffer updates (accept) or stays unsaved
     and unchanged (deny). Put the agent into a blocked state (mid-approval), press `<leader>Cp` again,
     and confirm the refusal message from `custom_api.herdr.M.send`'s `blocked` verdict (`herdr.lua`, the
     `VERDICTS` table) and that nothing was typed into the agent's pane.
   - Herdr annotation loop: select a range and press `<leader>Ac` in visual mode to comment it; press
     `<leader>Ac` in normal mode on a line to comment it
     (`dot_config/nvim/lua/plugins/herdr-nvim.lua:8-19`); press `<leader>Al` to list comments (line 23);
     press `<leader>As` to paste them to the agent without submitting, and `<leader>AS` to send and
     submit (lines 30, 37). Confirm each lands in the agent pane as expected, and that `As` does not
     submit while `AS` does. **Pass**: every keymap above does what its description says, on screen,
     against a real agent pane. **Fail**: any keymap silently no-ops, errors, or sends to the wrong pane.
1. **Swift and custom-plugin behavior.** In a Swift/Xcode project buffer, exercise the `<leader>x` xcode
   group: build, and run a test. Look for real UIKit or Vapor compiler diagnostics, a real build or test
   result, and on-save formatting firing (write the buffer, confirm the formatter ran). **Pass**:
   build/test completes and reports a real result; formatting is visible on save. **Fail**: no
   diagnostics, a build that silently does nothing, or a save with no formatting. Separately, for the
   custom pns.nvim plugin, trigger a notification-worthy event (for example, let a long-running command
   in an agent pane finish) and confirm a rendered notification arrives through it.
1. **Clean-home apply, then a quiet repeat.** Using a throwaway `HOME` with no prior Neovim state, run
   the bootstrap/full apply this configuration ships (the installer PR #385 delivers). **Pass**: the
   apply completes and Neovim starts cleanly against that fresh home. Run the same apply again
   immediately. **Pass**: the second run is quiet, reporting nothing changed, rather than re-doing work
   or erroring. **Fail**: either run errors, or the repeat is not quiet.
1. **Inventory-to-pull-request mapping.** The operator confirms each inventory row has a merged pull
   request, against the mapping table in `docs/research/2026-09-nvim-overhaul-acceptance.md`.

## Results

Record what was observed and the date.

- Drill one run on: **2026-09-15**. Run by the operator, two Claude agents in two fresh throwaway tabs of
  workspace `wW`, labelled `b103-a` and `b103-b`, in tabs `wW:t37` and `wW:t38`. Setup: **pass**.
  Procedure step 1 (send from the launch tab): **pass**, text landed in tab A's agent only. Step 2 (move
  the Neovim pane to tab B): **pass**, once corrected to the `--split right` form this document now
  carries (the drill's original `--tab`-only form failed against the installed `herdr` binary mid-run).
  Step 3 (edit and resend from the moved pane): **pass**, the visibly different text landed in tab B's
  agent, the tab Neovim had just moved into, not tab A. This is the acceptance: routing followed the
  pane's live tab rather than its launch-time tab. Step 4 (independent `tab_id` cross-check): **pass**.
- Drill two run on: **\_\_\_\_**. Pass/fail per step:
