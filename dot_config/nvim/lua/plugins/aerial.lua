return {
  "stevearc/aerial.nvim",
  -- aerial attaches from its own `BufEnter`/`WinEnter` autocmds, and its first
  -- backend is treesitter, so before this branch it attached to any buffer with
  -- a parser, not only an LSP one. The plugin therefore has to be loaded by the
  -- time a file buffer is entered, or `on_attach` never runs and the buffer-local
  -- `{` and `}` jumps never appear. `LspAttach` would miss every treesitter-only
  -- buffer; these two events cover a file read and a new file, and neither fires
  -- on a startup with no file, which is what keeps aerial off the startup path.
  event = { "BufReadPost", "BufNewFile" },
  -- Every command aerial registers, so one typed at the command line still
  -- works while the plugin is unloaded.
  cmd = {
    "AerialClose",
    "AerialCloseAll",
    "AerialGo",
    "AerialInfo",
    "AerialNavClose",
    "AerialNavOpen",
    "AerialNavToggle",
    "AerialNext",
    "AerialOpen",
    "AerialOpenAll",
    "AerialPrev",
    "AerialToggle",
  },
  -- These eight carry their own mappings, so each key is declared once rather
  -- than here and again in `config`. lazy.nvim installs the placeholder from
  -- the entry at startup and the real mapping from the same entry on load.
  -- `<leader>as`, `{` and `}` are not here: `on_attach` owns them, they have
  -- never existed before aerial attached to a buffer, and the event above is
  -- what gets them back.
  -- stylua: ignore start
  keys = {
    { "<leader>at", "<cmd>AerialToggle!<cr>",   desc = "Aerial: toggle sidebar (don't focus)" },
    { "<leader>aa", "<cmd>AerialToggle!<cr>",   desc = "Aerial: toggle sidebar (don't focus)" },
    { "<leader>aT", "<cmd>AerialToggle<cr>",    desc = "Aerial: toggle sidebar (and focus)" },
    { "<leader>aA", "<cmd>AerialToggle<cr>",    desc = "Aerial: toggle sidebar (and focus)" },
    { "<leader>ao", "<cmd>AerialOpen!<cr>",     desc = "Aerial: open sidebar (don't focus)" },
    { "<leader>aO", "<cmd>AerialOpen<cr>",      desc = "Aerial: open sidebar (and focus)" },
    { "<leader>ac", "<cmd>AerialClose<cr>",     desc = "Aerial: close sidebar" },
    { "<leader>aC", "<cmd>AerialCloseAll<cr>",  desc = "Aerial: close all sidebars" },
  },
  -- stylua: ignore end
  dependencies = {
    "nvim-treesitter/nvim-treesitter",
    "nvim-tree/nvim-web-devicons",
    "folke/snacks.nvim",
  },
  config = function()
    local opts = {
      layout = {
        min_width = 20,
      },
      on_attach = function(bufnr)
        map({ mode = "n", lhs = "{", rhs = "AerialPrev", desc = "Aerial: jump to next code object", buffer = bufnr })
        map({ mode = "n", lhs = "}", rhs = "AerialNext", desc = "Aerial: jump to previous code object", buffer = bufnr })

        map({
          mode = "n",
          lhs = "<leader>as",
          rhs = function()
            require("aerial").snacks_picker({
              layout = {
                -- preset = "dropdown",
                -- preview = false,
              },
            })
          end,
          desc = "Aerial: Snacks picker",
        })
      end,
    }

    require("aerial").setup(opts)
  end,
}
