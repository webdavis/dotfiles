-- Saves only the current buffer's breakpoints, because the plugin's `save_breakpoints()` erases
-- the saved entry of every listed buffer that is not loaded yet.
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

  -- Outside a configured project the open fails and nothing is written.
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
      "webdavis/pns.nvim",
      "MunifTanjim/nui.nvim",
      "folke/snacks.nvim",
      "stevearc/oil.nvim", -- file-tree sync into the Xcode project
      "mfussenegger/nvim-dap",
      { "rcarriga/nvim-dap-ui", dependencies = { "nvim-neotest/nvim-nio" } },
    },
    ft = "swift",
    -- Commands usable before a Swift buffer opens: `:XcodebuildSetup` on a bare package directory
    -- and the rest from any buffer in an Xcode project.
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
      -- Listed so attach, detach and debug work by name from a non-Swift buffer.
      "XcodebuildAttachDebugger",
      "XcodebuildDetachDebugger",
      "XcodebuildDebug",
    },
    cond = vim.fn.has("mac") == 1,
    opts = {
      -- Saves the report so logs, marks and diagnostics survive a restart. `config` reads it back.
      restore_on_start = true,
      code_coverage = { enabled = true },
      code_coverage_report = {
        open_expanded = true,
      },
      project_config = {
        -- For a workspace that holds a package and an app side by side.
        search_in_parent_dirs = true,
        reload_on_cwd_change = true,
      },
      project_manager = {
        -- Adds a new file to its nearest xcodeproj, for workspaces with more than one project.
        find_xcodeproj = true,
      },
      test_explorer = {
        -- Shows skipped tests, so they read as skipped rather than missing.
        show_disabled_tests = true,
      },
      integrations = {
        -- Debugging on a recent iOS device runs the plugin's `remote_debugger` helper as root.
        -- For this to work, a sudoers rule must let that helper run without a password.
        pymobiledevice = { enabled = true },
        -- Scheme guessing keeps buildServer.json on the edited target. The plugin's own trigger stays
        -- off, because it raises errors in a Swift package. `config` registers a guarded one instead.
        xcode_build_server = { enabled = true, guess_scheme = false },
        -- Off, because the plugin debugs with the toolchain's `lldb-dap`. The path stays so codelldb
        -- works again by setting `enabled = true`.
        codelldb = {
          enabled = false,
          codelldb_path = vim.fn.stdpath("data") .. "/mason/packages/codelldb/extension/adapter/codelldb",
        },
        -- Turning these off makes snacks the picker, matching the rest of this config.
        telescope_nvim = { enabled = false },
        fzf_lua = { enabled = false },
      },
    },
    config = function(_, opts)
      -- `stdbuf` must stay off PATH, because the plugin then launches apps through
      -- `stdbuf -o0 open`, which crashes on Apple silicon.
      require("xcodebuild").setup(opts)
      -- The plugin reads the report on `VimEnter`, which has fired before an `ft = "swift"` spec loads.
      require("xcodebuild.project.appdata").load_last_report()
      -- Registers the debugger commands, which the ordinary setup path skips.
      require("xcodebuild.integrations.dap").setup()

      -- Reload breakpoints when the project changes, since the plugin reloads only settings and the
      -- report. Gated on the breakpoints file changing, because the loader adds without clearing.
      local restored_breakpoints_file = require("xcodebuild.project.appdata").breakpoints_filepath
      vim.api.nvim_create_autocmd("User", {
        group = vim.api.nvim_create_augroup("xcodebuild_breakpoints_on_cwd_change", { clear = true }),
        pattern = "XcodebuildCwdChanged",
        callback = function()
          local path = require("xcodebuild.project.appdata").breakpoints_filepath
          if path == restored_breakpoints_file then
            return
          end
          restored_breakpoints_file = path
          -- Only buffers without live breakpoints, because the loader adds beside existing ones.
          local existing = require("dap.breakpoints").get()
          for _, buf in ipairs(vim.api.nvim_list_bufs()) do
            if not existing[buf] then
              require("xcodebuild.integrations.dap").load_breakpoints(buf)
            end
          end
        end,
      })

      -- Guess the scheme only for a complete Xcode project. `settings.xcodeproj` is nil for a Swift
      -- package, and `validate_project` catches a setup cancelled after picking the project.
      vim.api.nvim_create_autocmd("BufEnter", {
        group = vim.api.nvim_create_augroup("xcodebuild_guess_scheme_when_xcodeproj", { clear = true }),
        pattern = "*.swift",
        callback = function()
          if not require("xcodebuild.project.config").settings.xcodeproj then
            return
          end
          if require("xcodebuild.helpers").validate_project({ requiresXcodeproj = true, silent = true }) then
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
      { "<leader>xb", "<cmd>XcodebuildBuild<cr>", desc = "Xcode: build" },
      { "<leader>xB", "<cmd>XcodebuildCleanBuild<cr>", desc = "Xcode: clean build" },
      { "<leader>xr", "<cmd>XcodebuildBuildRun<cr>", desc = "Xcode: build and run" },
      { "<leader>xc", "<cmd>XcodebuildCancel<cr>", desc = "Xcode: cancel action" },
      { "<leader>xD", "<cmd>XcodebuildCleanDerivedData<cr>", desc = "Xcode: clean derived data" },
      { "<leader>xt", "<cmd>XcodebuildTest<cr>", desc = "Xcode: test all" },
      -- `<cmd>` keeps visual mode active, which the provider needs for `getpos("v")`.
      { "<leader>xt", "<cmd>XcodebuildTestSelected<cr>", mode = "v", desc = "Xcode: test selected" },
      { "<leader>xT", "<cmd>XcodebuildTestNearest<cr>", desc = "Xcode: test nearest" },
      { "<leader>xC", "<cmd>XcodebuildTestClass<cr>", desc = "Xcode: test current class" },
      { "<leader>xf", "<cmd>XcodebuildTestFailing<cr>", desc = "Xcode: test failing" },
      { "<leader>x.", "<cmd>XcodebuildTestRepeat<cr>", desc = "Xcode: repeat last test run" },
      { "<leader>xe", "<cmd>XcodebuildTestExplorerToggle<cr>", desc = "Xcode: toggle test explorer" },
      -- Coverage jumps stay in the `<leader>x` group, since global `[`/`]` pairs are crowded.
      { "<leader>xv", "<cmd>XcodebuildToggleCodeCoverage<cr>", desc = "Xcode: toggle code coverage" },
      { "<leader>xV", "<cmd>XcodebuildShowCodeCoverageReport<cr>", desc = "Xcode: show code coverage report" },
      { "<leader>x]", "<cmd>XcodebuildJumpToNextCoverage<cr>", desc = "Xcode: jump to next coverage mark" },
      { "<leader>x[", "<cmd>XcodebuildJumpToPrevCoverage<cr>", desc = "Xcode: jump to previous coverage mark" },
      -- sourcekit-lsp returns a code action only for the diagnostic's exact range, which these send.
      { "<leader>xa", "<cmd>XcodebuildCodeActions<cr>", desc = "Xcode: code actions for the line" },
      { "<leader>xq", "<cmd>XcodebuildQuickfixLine<cr>", desc = "Xcode: quickfix the line" },
      -- Attach, detach and debug-without-build are reachable through `<leader>xx` or by name.
      { "<leader>xg", "<cmd>XcodebuildBuildDebug<cr>", desc = "Xcode: build and debug" },
      -- The plugin has no command for test debugging, so this calls its Lua function.
      {
        "<leader>xG",
        function()
          require("xcodebuild.integrations.dap").debug_func_test()
        end,
        desc = "Xcode: debug nearest test",
      },
      -- Pairs with `<leader>xf` and re-runs the failing tests under the debugger.
      {
        "<leader>xF",
        function()
          require("xcodebuild.integrations.dap").debug_failing_tests()
        end,
        desc = "Xcode: debug failing tests",
      },
      -- Previews need the `xcodebuild-nvim-preview` Swift package in the project.
      { "<leader>xi", "<cmd>XcodebuildPreviewGenerateAndShow<cr>", desc = "Xcode: generate and show preview" },
      { "<leader>xI", "<cmd>XcodebuildPreviewToggle<cr>", desc = "Xcode: toggle preview" },
      -- Snapshot diffs need swift-snapshot-testing in the project.
      { "<leader>xS", "<cmd>XcodebuildFailingSnapshots<cr>", desc = "Xcode: failing snapshot tests" },
      -- The assets manager needs `fd`.
      { "<leader>xA", "<cmd>XcodebuildAssetsManager<cr>", desc = "Xcode: assets manager" },
      { "<leader>xM", "<cmd>XcodebuildApproveMacros<cr>", desc = "Xcode: approve Swift macros" },
      -- Attach reaches an app already running on a simulator or device.
      { "<leader>xh", "<cmd>XcodebuildAttachDebugger<cr>", desc = "Xcode: attach debugger" },
      { "<leader>xH", "<cmd>XcodebuildDetachDebugger<cr>", desc = "Xcode: detach debugger" },
      { "<leader>xw", "<cmd>XcodebuildSetup<cr>", desc = "Xcode: setup wizard" },
      { "<leader>x?", "<cmd>XcodebuildShowConfig<cr>", desc = "Xcode: show current configuration" },
      { "<leader>xs", "<cmd>XcodebuildSelectScheme<cr>", desc = "Xcode: select scheme" },
      { "<leader>xd", "<cmd>XcodebuildSelectDevice<cr>", desc = "Xcode: select device" },
      { "<leader>xP", "<cmd>XcodebuildSelectTestPlan<cr>", desc = "Xcode: select test plan" },
      { "<leader>xl", "<cmd>XcodebuildToggleLogs<cr>", desc = "Xcode: toggle logs" },
      { "<leader>xp", "<cmd>XcodebuildProjectManager<cr>", desc = "Xcode: project manager" },
      { "<leader>xo", "<cmd>XcodebuildOpenInXcode<cr>", desc = "Xcode: open in Xcode" },
      { "<leader>xE", "<cmd>XcodebuildEditEnvVars<cr>", desc = "Xcode: edit env vars" },
      { "<leader>xR", "<cmd>XcodebuildEditRunArgs<cr>", desc = "Xcode: edit run args" },
      -- The plugin's contextual action picker reaches most other commands.
      { "<leader>xx", "<cmd>XcodebuildPicker<cr>", desc = "Xcode: available actions" },
      -- Raw nvim-dap control lives in the `<leader>D` debug group. The breakpoint maps save after
      -- each change, because the plugin loads breakpoints.json on `BufReadPost` and never writes it.
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
      -- A log point prints on hit instead of stopping, and `{expr}` interpolates.
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
      -- xcodebuild's terminate also cancels the running xcodebuild action and closes dap-ui.
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
