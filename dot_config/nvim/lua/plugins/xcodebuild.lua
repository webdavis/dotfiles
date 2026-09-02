-- xcodebuild.nvim: build, run, test and (with nvim-dap) debug an Xcode project or a SwiftPM
-- package (including a Vapor server) from Neovim. `<leader>x` is its group; the audit at
-- docs/superpowers/plans/2026-09-01-nvim-overhaul-plan.md Task 3 found the plan's original eight
-- keymaps covered a quarter of the plugin's 66 commands. This file's keymap set and its reasoning
-- are recorded in the PR body, not here.
return {
  {
    "wojciech-kulik/xcodebuild.nvim",
    commit = "633eb71c0b354581837025581b7261dbe5361226",
    dependencies = {
      "MunifTanjim/nui.nvim", -- required: health.lua marks it optional = false
      "folke/snacks.nvim", -- the picker every other surface in this config uses, selected in opts
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
      -- Lazy loading on `ft = "swift"` misses `VimEnter`: opening `nvim` and then `:e Foo.swift`
      -- registers this feature's autocmd after `VimEnter` has already fired, so the previous
      -- run's logs, marks and diagnostics silently never restore. Upstream's own recommended
      -- spec has no lazy trigger at all for exactly this reason. Loading eagerly to make the
      -- default true would put the whole plugin on every startup for a feature that only saves
      -- one manual re-run; not worth it here, so the default is turned off explicitly instead.
      restore_on_start = false,
      -- Off by default upstream. `<leader>xv` toggles this, and a bound key that reports
      -- coverage is disabled and shows nothing is a dead key by another name; shipping "the
      -- Swift stack" with coverage dark by default was the audit's own named omission.
      code_coverage = { enabled = true },
      integrations = {
        -- Needs pymobiledevice3 (not installed, a pipx tool) plus passwordless sudo for a
        -- helper script this machine does not grant. Set explicitly so the config states the
        -- decision instead of relying on device_proxy's own is_installed() fallback.
        pymobiledevice = { enabled = false },
        -- The plugin resolves its picker in order (telescope, snacks, fzf-lua) and telescope IS
        -- in this config: it is an octo dependency (git.lua). Left on, every Xcode scheme, device
        -- and action picker would be the one telescope window in a snacks config. Octo opts out
        -- of it the same way, with `picker = "snacks"`.
        telescope_nvim = { enabled = false },
      },
    },
    config = function(_, opts)
      require("xcodebuild").setup(opts)
      -- The ordinary setup path never calls this; without it the debugger commands below do
      -- not exist at all, and checkhealth's debugger check has nothing to find.
      require("xcodebuild.integrations.dap").setup()

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
      { "<leader>xT", "<cmd>XcodebuildTestNearest<cr>", desc = "Xcode: test nearest" },
      { "<leader>xf", "<cmd>XcodebuildTestFailing<cr>", desc = "Xcode: test failing" },
      { "<leader>xe", "<cmd>XcodebuildTestExplorerToggle<cr>", desc = "Xcode: toggle test explorer" },
      { "<leader>xv", "<cmd>XcodebuildToggleCodeCoverage<cr>", desc = "Xcode: toggle code coverage" },
      -- Debug (nvim-dap): build-and-launch under the debugger is the one daily action; attach,
      -- detach and debug-without-rebuilding stay command-only, reachable through the picker
      -- (`<leader>xx`) or by name, since they are rarer and every xcode letter is now spoken for.
      { "<leader>xg", "<cmd>XcodebuildBuildDebug<cr>", desc = "Xcode: build and debug" },
      -- Project
      { "<leader>xs", "<cmd>XcodebuildSelectScheme<cr>", desc = "Xcode: select scheme" },
      { "<leader>xd", "<cmd>XcodebuildSelectDevice<cr>", desc = "Xcode: select device" },
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
      {
        "<leader>Db",
        function()
          require("dap").toggle_breakpoint()
        end,
        desc = "Debug: toggle breakpoint",
      },
      {
        "<leader>DB",
        function()
          require("dap").set_breakpoint(vim.fn.input("Breakpoint condition: "))
        end,
        desc = "Debug: conditional breakpoint",
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
      {
        "<leader>Dx",
        function()
          require("dap").terminate()
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
