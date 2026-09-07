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
    -- Eager, and it stays that way. Its surface is operator-pending and visual
    -- text objects installed by its own plugin file, with no <Plug> mappings to
    -- put behind a trigger, and no filetype or command of its own. A `keys` list
    -- would have to name every object it declares (the quote, argument, pair,
    -- separator and tag families, each in `i`/`a` with the `n`/`l` variants),
    -- and every one of those is a key someone reaches for mid-edit, where a
    -- first-press load is exactly the pause a text object must not have.
    lazy = false,
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
        -- The default set is installed at `VeryLazy` instead, by the callback
        -- below. Only the four keys the eager load exists for are installed at
        -- startup.
        useDefaults = false,
      },
    },
    config = function(_, opts)
      local various_textobjs = require("various-textobjs")

      various_textobjs.setup(opts)

      -- The four keys, and only those. The whole default set at startup would
      -- take `n`, `R`, `r`, `in`, `an` and `Q` off their earlier owners for the
      -- length of the startup window, put this plugin's `gx` over the system
      -- handler, cover targets.vim's quote, argument and color objects, and
      -- reverse `ai` and `ii` against Snacks' scope objects, which install at
      -- `UIEnter`. Measured: every one of those changed hands, and `ai` and `ii`
      -- stayed changed for the whole session.
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

      -- `VeryLazy` is where the defaults and `gx` landed before this plugin
      -- loaded eagerly, so installing them there leaves every other owner the
      -- startup window it had. `setup` is the only public way in: it installs
      -- the default set when `useDefaults = true` merges over the config already
      -- in place, and its own `im`, `am`, `ik` and `ak` replace the four above.
      local function install_defaults()
        various_textobjs.setup({ keymaps = { useDefaults = true } })

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
      end

      -- A `:Lazy reload` re-runs this config after `VeryLazy` has already fired,
      -- and an autocmd for an event that will not come again would drop the
      -- defaults for the rest of the session.
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
