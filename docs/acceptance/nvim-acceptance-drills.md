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

1. **Five silent starts, rendered, with agents running.** Launch Neovim five times in a row (`nvim`,
   `:q`, repeat), watching the screen each time, on the machine as it normally is: agents from several
   harnesses live in other panes. That is the operator's standing condition, so it is the condition the
   step tests. **Pass**: all five starts are silent, no error, warning, or stack trace. **Fail**: any one
   of the five prints something. A message that appears only while agents are running is a defect to
   diagnose and fix in the configuration, not a reason to retry the step on an idle machine.
1. **Full-plugin health output.** Start Neovim. Run `:Lazy! load all` to load every deferred plugin,
   including the `VeryLazy` ones. Then run `:checkhealth` and read the full report. **Pass**: no failures
   beyond the ones already listed in the "Health and deployment differences" table of the existing
   acceptance record. **Fail**: a new, uncatalogued error or failed check appears.
1. **Startup comparison.** Run `nvim --startuptime /tmp/b103-startuptime.log +q` and read the last line's
   total: that is the cold start. Run it again right after for a warm figure. Run both on the machine as
   it normally is, agents included; the measured margin is wide enough that load does not decide the
   outcome (the 2026-09-15 run came in 68 ms under the threshold with agents running), so the step asks
   for no quiet window and no median of repeated samples. Compare both against the retained baseline of
   164.178 ms and the synthetic warm-start figures already on record (94.373 ms source, 95.845 ms
   installed copy); the pass condition for the warm figure is unchanged, `after < baseline - 10`, which
   is 154.178 ms. Separately, start Neovim normally, run `:Lazy! load all`, and time that render by eye
   or with `:Lazy profile`. **Pass**: the warm figure still beats `baseline - 10`, and the rendered
   figure with every `VeryLazy` plugin loaded is recorded (task 65 sets no threshold for it, only that it
   be recorded). **Fail**: the warm figure regresses past `baseline - 10`.
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

- Drill two run on: **2026-09-15**, by the operator, on the machine in its normal state with agents from
  several harnesses running. Three of the eight steps have results.

  **Step 1 (five silent starts): fail.** Two of the five starts printed claudecode.nvim messages. A WARN,
  twice:
  `WebSocket handshake failed for client ... Bad Request ... Bad WebSocket upgrade request: Missing or invalid Upgrade header`.
  And an ERROR:
  `vim.schedule callback: .../claudecode.nvim/lua/claudecode/server/client.lua:108: handle ... is already closing`.
  Both are the plugin's own server refusing and then closing a connection from a client it did not
  expect, which is what "agents are already running in other panes" looks like from inside a fresh
  Neovim. Not fixed here, and not noise: under this document's own premise a message that only appears
  under load is a defect. The open question is whose client is connecting, since claudecode.nvim's lock
  file is per-Neovim.

  **Step 2 (full-plugin health): fail**, with three findings absent from the acceptance record's "Health
  and deployment differences" table. Everything else in the report matched that table's documented
  exclusions (luarocks and hererocks, the snacks graphics, TeX and diagram tools, lazygit, the Xcode
  remote debugger helper), so those are not failures.

  1. **`kulala_http` parser fails to build. Host toolchain, not configuration.** `:Lazy! load all`
     printed `[nvim-treesitter/install/kulala_http]: Compiling parser` then
     `error: Error during "tree-sitter build": nil`, and kulala warned
     `Failed to install kulala_http parser` (`kulala.nvim/lua/kulala/config/init.lua:87`, reached from
     its `setup`). Reproduced outside Neovim with the same mason `tree-sitter` 0.27.0 over kulala's own
     grammar directory:
     `Parser compilation failed. Stderr: ld: tapi error: malformed file /Library/Developer/CommandLineTools/SDKs/MacOSX27.0.sdk/usr/lib/libSystem.B.tbd:4:20: error: unknown architecture arm64e.x1-macos`.
     The cause is not tree-sitter and not this configuration: NO C link succeeds on this machine right
     now. `clang -o hello hello.c` fails identically. `xcode-select -p` points at Xcode 26.6, whose
     linker is `ld-1267`, while the installed Command Line Tools are 27.0 and ship the SDK that clang
     resolves (`-syslibroot /Library/Developer/CommandLineTools/SDKs/MacOSX.sdk`); that SDK's
     `libSystem.tbd` names a target triple `ld-1267` cannot parse. The Command Line Tools' own linker is
     `ld-27037.1` and reads it fine:
     `DEVELOPER_DIR=/Library/Developer/CommandLineTools tree-sitter build -o parser.so` builds the
     grammar, exit 0. The repair is one operator command,
     `sudo xcode-select --switch /Library/Developer/CommandLineTools`, or an Xcode that matches macOS 27;
     both are the operator's call. Nothing in this repository is wrong and nothing here was changed. The
     warning was left in place deliberately: the only lever kulala offers is its `debug` option, which is
     all-or-nothing and would also silence every real request error it reports, and the message is true,
     correct and actionable. It is also not a recurring annoyance in ordinary use: kulala is
     `ft = { "http", "rest" }`, so it loads on an `.http` buffer, where a missing HTTP parser is exactly
     what the operator should hear about, and on the `:Lazy! load all` this drill performs. Once the
     toolchain is switched, the parser builds and the message is gone.

  1. **`rustaceanvim`'s health check crashed. Our configuration, fixed.**
     `rustaceanvim/lua/rustaceanvim/health.lua:78: attempt to index local 'result' (a nil value)`. Line
     78 is `local obj = result:wait(1000)`, where `result` came from
     `pcall(vim.system, { binary, '--version' }, { text = true })` on line 74. The nil is ours:
     `lua/plugins/overseer.lua` enabled overseer's `experimental_wrap_builtins` for every command, and
     that wrapper replaces `vim.system` with one that turns the call into an overseer task and returns
     the task's process handle. For a binary that cannot be spawned the handle is nil and the ENOENT
     `vim.system` documents as a raise is swallowed, so rustaceanvim first concludes the optional
     `lspmux` IS installed and then indexes nil. Measured with overseer loaded:
     `pcall(vim.system, { "lspmux", "--version" }, { text = true })` returns `true, nil`, against
     `true, <table>` for a binary that exists. The condition now declines a command whose binary is not
     executable, handing that case back to the builtin, which raises as documented, and leaves every
     runnable command wrapped. With the fix in place `:Lazy! load all` followed by
     `:checkhealth rustaceanvim` reports no errors. The pin did not move.

  1. **`codesnap` downloads an unpinned binary at load time. Recorded, unchanged.**
     `Downloading codesnap library for mac-aarch64_generator.so...` then
     `Library downloaded successfully!`. The download is `codesnap.nvim/lua/codesnap/fetch.lua:55`, a
     `curl -L` of
     `https://github.com/mistricky/codesnap.nvim/releases/download/v<version>/<os>-<arch>_generator.so`,
     called from `fetch.ensure_lib` (line 109) via `module.generator_file_path`
     (`lua/codesnap/module.lua:34`), which `lua/codesnap/init.lua:10` calls at the top level, so it runs
     the moment the plugin is required rather than on first snapshot. There is no checksum, no signature
     and no pin: the version string is read out of the checked-out `project.toml` and the only skip
     condition is a `lua/libs/.version` file that already matches. A GitHub release asset is mutable, so
     the artifact this plugin executes is whatever that tag serves at fetch time, while every other
     plugin here is pinned to a commit. The plugin exposes no option to pin or verify it. Also worth
     knowing: the `build = "make"` step this repository declares is now dead. It runs
     `scripts/apply_generator.sh`, which copies the commit-pinned `lua/mac-aarch64generator.so` to
     `lua/generator.so`, and the loader never looks there. The two artifacts are not the same build
     either, 6,359,152 bytes committed against 29,677,552 downloaded, so swapping the pinned one into
     `lua/libs/` is a change that needs proving rather than an equivalence. Dispositions, operator's
     call: accept the download, drop the plugin, or make the `build` step stage the commit-pinned
     artifact as `lua/libs/mac-aarch64_generator.so` with a matching `.version` and prove a snapshot
     still renders.

  **Step 3 (startup comparison): pass, comfortably.** `--- NVIM STARTED --- 086.328` against a threshold
  of 154.178 ms (the retained 164.178 ms baseline less the plan's 10 ms margin), measured with agents
  running. That is 68 ms of margin, and it also beats both recorded synthetic warm figures, 94.373 ms
  source and 95.845 ms installed copy. Evidence that the threshold survives load, which is why step 3 no
  longer asks for a quiet machine.

  **Step 8 (inventory to pull request mapping): pass.** All 59 pull requests that
  `docs/research/2026-09-nvim-overhaul-acceptance.md` links by `pull/<n>` are merged, verified per
  number: `merged_at` is non-null on all 59 and `merged` is true on all 59. The trap that nearly produced
  a wrong answer: `state` reads `closed` for a merged pull request AND for an abandoned one, so `state`
  alone shows "59 closed" and looks alarming. `merged_at` is the field that answers the question. The
  four `custom #1` to `custom #4` rows and the `#13` appendix anchor are not pull request references.

  **Steps 4, 5, 6 and 7: not run.**

  **One open question for the operator, unchanged by this run.** `neotest-java` reports two errors,
  `jdtls` missing and `nvim-dap-virtual-text` missing, and mason reports no Java runtime at all, both
  `java` and `javac` unavailable, so that adapter cannot work on this machine. Either install a Java
  runtime plus `jdtls`, which makes the adapter usable and the errors go away, or drop the Java adapter
  from `lua/plugins/neotest.lua`, which removes the errors and the capability together. Nothing about it
  was changed.
