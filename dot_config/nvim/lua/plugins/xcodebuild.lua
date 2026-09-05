-- xcodebuild.nvim: build, run, test and (with nvim-dap) debug an Xcode project or a SwiftPM
-- package (including a Vapor server) from Neovim. `<leader>x` is its group; the audit at
-- docs/superpowers/plans/2026-09-01-nvim-overhaul-plan.md Task 3 found the plan's original eight
-- keymaps covered a quarter of the plugin's 66 commands. This file's keymap set and its reasoning
-- are recorded in the PR body, not here.

-- Save the CURRENT buffer's breakpoints, leaving every other file's entry alone.
--
-- The plugin's own `save_breakpoints()` walks `nvim_list_bufs()` and writes
-- `saved[name] = dap.breakpoints.get()[bufnr]` for each. An argument-list buffer that is
-- listed but not yet loaded has no dap breakpoints, so that assignment is nil and its saved
-- entry is deleted. Reproduced: `nvim A.swift B.swift`, toggle in A, and B's conditional
-- breakpoint is gone from breakpoints.json. Reading, updating one key and writing back is the
-- whole fix, and the three breakpoint mappings share it.
local function save_current_buffer_breakpoints()
  local path = require("xcodebuild.project.appdata").breakpoints_filepath
  local saved = {}
  if vim.fn.filereadable(path) == 1 then
    local decoded_ok, decoded = pcall(vim.fn.json_decode, table.concat(vim.fn.readfile(path), ""))
    if decoded_ok and type(decoded) == "table" then
      saved = decoded
    end
  end

  local bufnr = vim.api.nvim_get_current_buf()
  saved[vim.api.nvim_buf_get_name(bufnr)] = require("dap.breakpoints").get()[bufnr]

  -- Outside a configured project the appdata directory does not exist, the open fails and
  -- this returns without writing, which is what keeps the mappings safe on a stray Swift file.
  local fp = io.open(path, "w")
  if fp then
    fp:write(vim.fn.json_encode(saved))
    fp:close()
  end
end

return {
  {
    "wojciech-kulik/xcodebuild.nvim",
    commit = "633eb71c0b354581837025581b7261dbe5361226",
    dependencies = {
      "MunifTanjim/nui.nvim", -- required: health.lua marks it optional = false
      "folke/snacks.nvim", -- the picker every other surface in this config uses, selected in opts below
      "stevearc/oil.nvim", -- file-tree sync into the Xcode project
      "mfussenegger/nvim-dap", -- operator-approved 2026-09-02, see the spec 12.2 correction
      { "rcarriga/nvim-dap-ui", dependencies = { "nvim-neotest/nvim-nio" } },
    },
    ft = "swift",
    -- Enumerated literally (lazy.nvim needs the exact names): the operator's own minimum
    -- typeable-before-a-swift-buffer set, so `:XcodebuildSetup` works on a bare package
    -- directory and the others work from a non-swift buffer in an Xcode project.
    cmd = {
      "XcodebuildSetup",
      "XcodebuildPicker",
      "XcodebuildBuild",
      "XcodebuildBuildRun",
      "XcodebuildTest",
      "XcodebuildToggleLogs",
      "XcodebuildSelectScheme",
      "XcodebuildSelectDevice",
      "XcodebuildProjectManager",
      "XcodebuildCleanDerivedData",
      -- The comment on the keys list below says attach, detach and debug-without-build stay
      -- reachable "by name". Without an entry here that claim is false from a cold non-Swift
      -- buffer: the command does not exist yet and typing it is E492.
      "XcodebuildAttachDebugger",
      "XcodebuildDetachDebugger",
      "XcodebuildDebug",
    },
    cond = vim.fn.has("mac") == 1,
    opts = {
      -- The write half of report persistence: with this off the plugin never saves the report
      -- at all (`project/builder.lua:81`, `tests/runner.lua:199`), so last run's logs, marks
      -- and diagnostics are gone every session. The read half is a `VimEnter` autocmd, which
      -- an `ft = "swift"` spec always registers too late to fire, so `config` below calls
      -- `load_last_report()` directly. Both halves now work without loading eagerly.
      restore_on_start = true,
      -- Off by default upstream. `<leader>xv` toggles this, and a bound key that reports
      -- coverage is disabled and shows nothing is a dead key by another name; shipping "the
      -- Swift stack" with coverage dark by default was the audit's own named omission.
      code_coverage = { enabled = true },
      code_coverage_report = {
        -- Upstream's 60/30 thresholds are the conventional pair and there is no house number
        -- to prefer over them. Expanding the tree is the change: the report exists to be read.
        open_expanded = true,
      },
      project_config = {
        -- Both off upstream. On, the plugin finds the project config from a file anywhere
        -- below the project root rather than only at the cwd, and re-reads it when the cwd
        -- moves. Worth it for a workspace holding a package and an app side by side.
        search_in_parent_dirs = true,
        reload_on_cwd_change = true,
      },
      project_manager = {
        -- Off upstream. On, a new file is added to the xcodeproj nearest to it rather than to
        -- the one configured, which is what makes the file-tree sync correct when a workspace
        -- holds more than one project. With a single project it is a no-op.
        find_xcodeproj = true,
      },
      test_explorer = {
        -- Off upstream. A skipped test that is invisible reads as a test that does not exist.
        show_disabled_tests = true,
      },
      integrations = {
        -- pymobiledevice3 is installed (uv tool, declared in
        -- `.chezmoidata/system_packages_autoinstall.yaml`), which is what builds and runs apps
        -- on a physical device and on anything below iOS 17. That much works with no sudo.
        --
        -- BLOCKED, and do NOT follow `:h xcodebuild.ios17`: iOS 17+ secure-tunnel debugging
        -- stays unavailable until a reviewed sudo configuration exists. The documented recipe
        -- grants passwordless root to `~/Library/xcodebuild.nvim/remote_debugger`, and root
        -- owning that file is not enough. `~/Library` is user-writable, so the whole directory
        -- can be swapped for another one; and the helper resolves `pymobiledevice3` off PATH,
        -- where the uv shim, the Python interpreter and every dependency are user-owned. Either
        -- route runs attacker-chosen code as root.
        --
        -- What a configuration would have to provide before this is reconsidered: the helper
        -- copy root-owned in a root-owned directory OUTSIDE `$HOME`; the interpreter and the
        -- pymobiledevice3 entry point pinned to absolute root-owned paths inside the helper
        -- rather than resolved at run time; and a sudoers entry carrying `secure_path` so the
        -- rule cannot be redirected by the caller's environment. Nothing here installs any of
        -- that, and this branch has installed no sudo rule at all.
        pymobiledevice = { enabled = true },
        -- Scheme guessing makes `buildServer.json` describe the target actually being edited,
        -- so sourcekit-lsp resolves imports correctly in a multi-scheme project. The plugin's
        -- own switch is left OFF and the autocmd re-registered in `config` below, because
        -- `guess_scheme()` gates on `xcodeproj OR workingDirectory` while the work it then
        -- does needs an Xcode project. A configured Swift package sets only the second, so
        -- every `BufEnter *.swift` in a Vapor package raised "This operation is not supported
        -- for Swift Package" and "No targets found for the current file".
        xcode_build_server = { enabled = true, guess_scheme = false },
        -- DELIBERATELY off, and the one feature not turned on. codelldb is the pre-Xcode-16
        -- debug adapter; from Xcode 16 the plugin uses `lldb-dap` out of the toolchain and
        -- says so in `integrations/dap.lua:594`. This machine is Xcode 26.6, so enabling it
        -- would swap a current adapter for the legacy one. The path is filled in anyway, from
        -- the Mason install `lsp.lua` already declares, so the fallback is one boolean away
        -- and needs no path hunting on the day an old toolchain matters.
        codelldb = {
          enabled = false,
          codelldb_path = vim.fn.stdpath("data") .. "/mason/packages/codelldb/extension/adapter/codelldb",
        },
        -- The picker is chosen by first match down a fixed list (telescope, then snacks, then
        -- fzf-lua) in the plugin's own pickers.setup(). All three default to enabled, so snacks
        -- only wins today because neither of the others is installed: an accident, not a
        -- choice, and installing telescope later would silently move every Xcode picker off
        -- the surface the rest of this config uses. Turning the two off makes snacks the
        -- declared winner.
        telescope_nvim = { enabled = false },
        fzf_lua = { enabled = false },
      },
    },
    config = function(_, opts)
      -- `platform/macos.lua:22` looks for `stdbuf` by that exact name to stream a macOS app's
      -- logs without a debugger, which is how a Vapor server's output reaches the editor.
      -- Homebrew's coreutils installs it as `gstdbuf` and keeps the GNU names in a separate
      -- directory, so the check fails on a machine that has the tool.
      --
      -- APPENDED, never prepended. `vim.env.PATH` is inherited by every subprocess Neovim
      -- starts, git hooks and formatters included, so putting the GNU names first swaps the
      -- userland under all of them: measured, BSD `date -j -f` and `stat -f %m` both exit 1
      -- under a prepend and both work under an append. Appending still resolves `stdbuf`,
      -- because no earlier entry provides one. Coreutils ships no `sed`, so nothing here
      -- substitutes GNU sed either way.
      if vim.fn.executable("stdbuf") == 0 then
        local gnubin = "/opt/homebrew/opt/coreutils/libexec/gnubin"
        if vim.fn.isdirectory(gnubin) == 1 then
          vim.env.PATH = vim.env.PATH .. ":" .. gnubin
        end
      end

      require("xcodebuild").setup(opts)
      -- The read half of `restore_on_start`. Upstream hangs it on `VimEnter`, which has always
      -- fired by the time an `ft = "swift"` spec loads, so the plugin's own autocmd never runs
      -- and the saved report is never read back. Calling it here restores last run's logs,
      -- marks, quickfix and diagnostics at the moment the first Swift file opens instead.
      require("xcodebuild.project.appdata").load_last_report()
      -- The ordinary setup path never calls this; without it the debugger commands below do
      -- not exist at all, and checkhealth's debugger check has nothing to find.
      require("xcodebuild.integrations.dap").setup()

      -- `reload_on_cwd_change` above re-reads settings and the report when the project changes,
      -- but nothing reloads breakpoints, so after a switch the buffers still carry the previous
      -- project's. The plugin broadcasts the switch; this is the missing third restore.
      vim.api.nvim_create_autocmd("User", {
        group = vim.api.nvim_create_augroup("xcodebuild_breakpoints_on_cwd_change", { clear = true }),
        pattern = "XcodebuildCwdChanged",
        callback = function()
          require("xcodebuild.integrations.dap").load_breakpoints()
        end,
      })

      -- Scheme guessing, with the guard the plugin's own registration lacks: an Xcode project
      -- must be configured. `settings.xcodeproj` is exactly that, and is nil for a Swift
      -- package, which is what silences the per-keystroke errors in a Vapor package.
      vim.api.nvim_create_autocmd("BufEnter", {
        group = vim.api.nvim_create_augroup("xcodebuild_guess_scheme_when_xcodeproj", { clear = true }),
        pattern = "*.swift",
        callback = function()
          if require("xcodebuild.project.config").settings.xcodeproj then
            require("xcodebuild.integrations.xcode-build-server").guess_scheme()
          end
        end,
      })

      local dap, dapui = require("dap"), require("dapui")
      dapui.setup()
      dap.listeners.after.event_initialized["dapui_config"] = function()
        dapui.open()
      end
      dap.listeners.before.event_terminated["dapui_config"] = function()
        dapui.close()
      end
      dap.listeners.before.event_exited["dapui_config"] = function()
        dapui.close()
      end
    end,
    keys = {
      -- Build and run
      { "<leader>xb", "<cmd>XcodebuildBuild<cr>", desc = "Xcode: build" },
      { "<leader>xB", "<cmd>XcodebuildCleanBuild<cr>", desc = "Xcode: clean build" },
      { "<leader>xr", "<cmd>XcodebuildBuildRun<cr>", desc = "Xcode: build and run" },
      { "<leader>xc", "<cmd>XcodebuildCancel<cr>", desc = "Xcode: cancel action" },
      { "<leader>xD", "<cmd>XcodebuildCleanDerivedData<cr>", desc = "Xcode: clean derived data" },
      -- Test
      { "<leader>xt", "<cmd>XcodebuildTest<cr>", desc = "Xcode: test all" },
      -- The one visual-mode map in the group. `<cmd>` is load-bearing rather than stylistic
      -- here: the provider reads the range with `getpos("v")`, which only answers while visual
      -- mode is still active, and a `:` mapping leaves it before the command runs.
      { "<leader>xt", "<cmd>XcodebuildTestSelected<cr>", mode = "v", desc = "Xcode: test selected" },
      { "<leader>xT", "<cmd>XcodebuildTestNearest<cr>", desc = "Xcode: test nearest" },
      { "<leader>xC", "<cmd>XcodebuildTestClass<cr>", desc = "Xcode: test current class" },
      { "<leader>xf", "<cmd>XcodebuildTestFailing<cr>", desc = "Xcode: test failing" },
      { "<leader>x.", "<cmd>XcodebuildTestRepeat<cr>", desc = "Xcode: repeat last test run" },
      { "<leader>xe", "<cmd>XcodebuildTestExplorerToggle<cr>", desc = "Xcode: toggle test explorer" },
      -- Code coverage. `code_coverage.enabled` is turned on in opts above, and the inline marks
      -- were the only part of it reachable: the report and the two jumps have no keys today and,
      -- unlike most of the rest, the jumps are not in the action picker either. They stay inside
      -- the `<leader>x` group rather than taking global `[`/`]` pairs, which unimpaired and the
      -- treesitter textobject motions already crowd.
      { "<leader>xv", "<cmd>XcodebuildToggleCodeCoverage<cr>", desc = "Xcode: toggle code coverage" },
      { "<leader>xV", "<cmd>XcodebuildShowCodeCoverageReport<cr>", desc = "Xcode: show code coverage report" },
      { "<leader>x]", "<cmd>XcodebuildJumpToNextCoverage<cr>", desc = "Xcode: jump to next coverage mark" },
      { "<leader>x[", "<cmd>XcodebuildJumpToPrevCoverage<cr>", desc = "Xcode: jump to previous coverage mark" },
      -- Code. Not conveniences: sourcekit-lsp only returns a code action when the range it is
      -- given matches the diagnostic's range exactly, and Neovim sends the cursor position, so
      -- plain `vim.lsp.buf.code_action` silently returns nothing on Swift unless the cursor
      -- happens to sit on the right column. The plugin ships `integrations/lsp.lua` purely to
      -- find the current line's diagnostic and send its real range. Without these two keys that
      -- module is dead code and Swift code actions mostly do not appear.
      { "<leader>xa", "<cmd>XcodebuildCodeActions<cr>", desc = "Xcode: code actions for the line" },
      { "<leader>xq", "<cmd>XcodebuildQuickfixLine<cr>", desc = "Xcode: quickfix the line" },
      -- Debug (nvim-dap): build-and-launch under the debugger is the one daily action; attach,
      -- detach and debug-without-rebuilding stay command-only, reachable through the picker
      -- (`<leader>xx`) or by name, since they are rarer and every xcode letter is now spoken for.
      { "<leader>xg", "<cmd>XcodebuildBuildDebug<cr>", desc = "Xcode: build and debug" },
      -- Debugging a test had no route at all: the plugin registers user commands for the four
      -- app debugging actions but none for the six test ones, which exist only as Lua functions
      -- on the dap integration module, so neither a keymap nor the action picker could reach
      -- them. This is the one that gets used, the rest stay on the module.
      {
        "<leader>xG",
        function()
          require("xcodebuild.integrations.dap").debug_func_test()
        end,
        desc = "Xcode: debug nearest test",
      },
      -- Pairs with `<leader>xf`: re-run what failed, this time under the debugger. The other
      -- four test-debugging functions (all, target, class, selected) stay on the module, since
      -- a failing run is what sends anyone to a debugger.
      {
        "<leader>xF",
        function()
          require("xcodebuild.integrations.dap").debug_failing_tests()
        end,
        desc = "Xcode: debug failing tests",
      },
      -- Previews. Live now that the snacks image snack is on (snacks.lua). Rendering also
      -- needs the `xcodebuild-nvim-preview` Swift package in the project being previewed,
      -- which is a per-project dependency no editor config can supply.
      { "<leader>xi", "<cmd>XcodebuildPreviewGenerateAndShow<cr>", desc = "Xcode: generate and show preview" },
      { "<leader>xI", "<cmd>XcodebuildPreviewToggle<cr>", desc = "Xcode: toggle preview" },
      -- Snapshot tests. The plugin ships its own `getsnapshots` binary to build the diffs, so
      -- the only prerequisite is swift-snapshot-testing in the project.
      { "<leader>xS", "<cmd>XcodebuildFailingSnapshots<cr>", desc = "Xcode: failing snapshot tests" },
      -- Assets and macros. The assets manager needs `fd`, which is installed.
      { "<leader>xA", "<cmd>XcodebuildAssetsManager<cr>", desc = "Xcode: assets manager" },
      { "<leader>xM", "<cmd>XcodebuildApproveMacros<cr>", desc = "Xcode: approve Swift macros" },
      -- Attach and detach, live now that pymobiledevice3 is installed: this is how the
      -- debugger reaches an app already running, on a simulator or a device.
      { "<leader>xh", "<cmd>XcodebuildAttachDebugger<cr>", desc = "Xcode: attach debugger" },
      { "<leader>xH", "<cmd>XcodebuildDetachDebugger<cr>", desc = "Xcode: detach debugger" },
      -- Project
      { "<leader>xw", "<cmd>XcodebuildSetup<cr>", desc = "Xcode: setup wizard" },
      { "<leader>x?", "<cmd>XcodebuildShowConfig<cr>", desc = "Xcode: show current configuration" },
      { "<leader>xs", "<cmd>XcodebuildSelectScheme<cr>", desc = "Xcode: select scheme" },
      { "<leader>xd", "<cmd>XcodebuildSelectDevice<cr>", desc = "Xcode: select device" },
      -- The third project selector beside scheme and device, and routine on iOS, where a target
      -- usually carries more than one test plan.
      { "<leader>xP", "<cmd>XcodebuildSelectTestPlan<cr>", desc = "Xcode: select test plan" },
      { "<leader>xl", "<cmd>XcodebuildToggleLogs<cr>", desc = "Xcode: toggle logs" },
      { "<leader>xp", "<cmd>XcodebuildProjectManager<cr>", desc = "Xcode: project manager" },
      { "<leader>xo", "<cmd>XcodebuildOpenInXcode<cr>", desc = "Xcode: open in Xcode" },
      { "<leader>xE", "<cmd>XcodebuildEditEnvVars<cr>", desc = "Xcode: edit env vars" },
      { "<leader>xR", "<cmd>XcodebuildEditRunArgs<cr>", desc = "Xcode: edit run args" },
      -- The plugin's own answer to "66 commands": one key reaches most of the rest. It is
      -- contextual (build/test/scheme/device actions for the current project), not exhaustive:
      -- explicit log open/close, next/previous device and the quickfix commands are not in it.
      { "<leader>xx", "<cmd>XcodebuildPicker<cr>", desc = "Xcode: available actions" },
      -- Raw nvim-dap control, under the existing `<leader>D` "debug" group (which-key.lua:35),
      -- not `<leader>x`: starting a session with xg above is not "wired up" without a way to
      -- set a breakpoint and step through it. Kept clear of the existing Ds/Dt maps and the Dp
      -- profiler subgroup; a capital second letter pairs with its lowercase neighbor, the same
      -- pattern this file already uses (xt/xT, xb/xB).
      --
      -- The three breakpoint maps go through xcodebuild's own dap module rather than nvim-dap
      -- directly, because breakpoint persistence was only half wired: the `dap.setup()` call in
      -- config above installs a `BufReadPost *.swift` autocmd that LOADS breakpoints from the
      -- project's breakpoints.json, and nothing ever wrote that file. Each of these wrappers
      -- calls the plain nvim-dap function and then saves. Outside a configured project the save
      -- is a silent no-op (the file open fails and the function returns), so they stay safe on a
      -- Swift buffer that belongs to no Xcode project.
      {
        "<leader>Db",
        function()
          require("dap").toggle_breakpoint()
          save_current_buffer_breakpoints()
        end,
        desc = "Debug: toggle breakpoint",
      },
      {
        "<leader>DB",
        function()
          require("dap").set_breakpoint(vim.fn.input("Breakpoint condition: "))
          save_current_buffer_breakpoints()
        end,
        desc = "Debug: conditional breakpoint",
      },
      -- A log point: prints on hit instead of stopping, and `{expr}` interpolates. No equivalent
      -- existed, and it is the cheapest way to trace a Swift value without stopping the app.
      {
        "<leader>Dm",
        function()
          require("dap").set_breakpoint(nil, nil, vim.fn.input("Breakpoint message: "))
          save_current_buffer_breakpoints()
        end,
        desc = "Debug: toggle message breakpoint",
      },
      {
        "<leader>Dc",
        function()
          require("dap").continue()
        end,
        desc = "Debug: continue",
      },
      {
        "<leader>Do",
        function()
          require("dap").step_over()
        end,
        desc = "Debug: step over",
      },
      {
        "<leader>Di",
        function()
          require("dap").step_into()
        end,
        desc = "Debug: step into",
      },
      {
        "<leader>DO",
        function()
          require("dap").step_out()
        end,
        desc = "Debug: step out",
      },
      -- xcodebuild's terminate, not nvim-dap's: it also cancels the xcodebuild action that is
      -- still running behind the session and closes dap-ui, so stopping the debugger no longer
      -- leaves a build or test run going with nothing showing it.
      {
        "<leader>Dx",
        function()
          require("xcodebuild.integrations.dap").terminate_session()
        end,
        desc = "Debug: terminate",
      },
      {
        "<leader>Du",
        function()
          require("dapui").toggle()
        end,
        desc = "Debug: toggle dap-ui panel",
      },
    },
  },
}
