return {
  {
    "kylechui/nvim-surround",
    version = "^4.0.0", -- Use for stability; omit to use `main` branch for the latest features
    event = "VeryLazy",
    config = function()
      require("nvim-surround").setup({
        -- Configuration here, or leave empty to use defaults
      })
    end,
  },
  {
    "wellle/targets.vim",
  },
  {
    "chrisgrieser/nvim-various-textobjs",
    -- Eager, not `VeryLazy`. This plugin is the only owner of `im`, `am`, `ik`
    -- and `ak` now that treesitter's shadowed declarations are gone, and on
    -- `VeryLazy` those four answered nothing at all for the first tens of
    -- milliseconds of a session: measured 0 of 4 selecting anything from a
    -- VimEnter keystroke, against 4 of 4 while treesitter still declared them.
    -- Loading at startup gives the single owner a mapping from the first
    -- keystroke, which restoring the second owner would not: that would put the
    -- same four keys back under two plugins with a handover at `VeryLazy`, so
    -- `im` would mean "inner function" early and "inner chainMember" later.
    lazy = false,
    opts = {
      keymaps = {
        useDefaults = true,
      },
    },
    config = function(_, opts)
      local various_textobjs = require("various-textobjs")

      various_textobjs.setup(opts)

      map({
        mode = "n",
        lhs = "gx",
        rhs = function()
          -- Find and select next URL.
          various_textobjs.url()

          -- Only switches to visual mode when textobj found.
          local foundURL = vim.fn.mode() == "v"
          if not foundURL then
            return
          end

          local url = vim.fn.getregion(vim.fn.getpos("."), vim.fn.getpos("v"), { type = "v" })[1]
          vim.ui.open(url) -- requires nvim 0.10

          -- Leave visual mode.
          vim.cmd.normal({ "v", bang = true })
        end,
        desc = "Various Textobjs: open URL",
      })
    end,
  },
}
