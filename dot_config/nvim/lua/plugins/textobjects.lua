return {
  {
    "kylechui/nvim-surround",
    version = "^4.0.0",
    event = "VeryLazy",
    config = function()
      require("nvim-surround").setup({})
    end,
  },
  {
    "wellle/targets.vim",
    -- Loaded at startup, because a text object must answer on the first press.
    lazy = false,
  },
  {
    "chrisgrieser/nvim-various-textobjs",
    -- Loaded at startup, because this plugin alone owns `im`, `am`, `ik` and `ak`, and they must
    -- work from the first keystroke.
    lazy = false,
    opts = {
      keymaps = {
        -- The default set installs at `VeryLazy`, in the callback below.
        useDefaults = false,
      },
    },
    config = function(_, opts)
      local various_textobjs = require("various-textobjs")

      various_textobjs.setup(opts)

      -- Only these four at startup, because the full default set would take keys from other plugins.
      local EARLY_KEYS = {
        { lhs = "im", textobj = "chainMember", scope = "inner" },
        { lhs = "am", textobj = "chainMember", scope = "outer" },
        { lhs = "ik", textobj = "key", scope = "inner" },
        { lhs = "ak", textobj = "key", scope = "outer" },
      }
      for _, early in ipairs(EARLY_KEYS) do
        map({
          mode = { "o", "x" },
          lhs = early.lhs,
          rhs = function()
            various_textobjs[early.textobj](early.scope)
          end,
          desc = ("Various Textobjs: %s %s"):format(early.scope, early.textobj),
        })
      end

      -- Installing the defaults at `VeryLazy` leaves other plugins their keys during startup.
      -- `setup` with `useDefaults = true` installs them and replaces the four above.
      local function install_defaults()
        various_textobjs.setup({ keymaps = { useDefaults = true } })

        map({
          mode = "n",
          lhs = "gx",
          rhs = function()
            various_textobjs.url()

            -- `url()` enters visual mode only when it finds a URL.
            local foundURL = vim.fn.mode() == "v"
            if not foundURL then
              return
            end

            local url = vim.fn.getregion(vim.fn.getpos("."), vim.fn.getpos("v"), { type = "v" })[1]
            vim.ui.open(url)

            vim.cmd.normal({ "v", bang = true })
          end,
          desc = "Various Textobjs: open URL",
        })
      end

      -- `:Lazy reload` runs this after `VeryLazy` has fired, so install the defaults directly.
      if vim.g.did_very_lazy then
        install_defaults()
      else
        vim.api.nvim_create_autocmd("User", {
          pattern = "VeryLazy",
          once = true,
          callback = install_defaults,
        })
      end
    end,
  },
}
