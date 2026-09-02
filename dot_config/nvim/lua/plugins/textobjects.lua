return {
  {
    "kylechui/nvim-surround",
    version = "^3.0.0", -- Use for stability; omit to use `main` branch for the latest features
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
    event = "VeryLazy",
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
