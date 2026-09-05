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
  -- These eight are bound in `config`, so before this branch they existed from
  -- startup even with no file open. `<leader>as`, `{` and `}` are not here: they
  -- are bound by `on_attach`, so they have never existed before aerial attached
  -- to a buffer, and the event above is what gets them back.
  keys = {
    { "<leader>at", desc = "Aerial: toggle sidebar (don't focus)" },
    { "<leader>aa", desc = "Aerial: toggle sidebar (don't focus)" },
    { "<leader>aT", desc = "Aerial: toggle sidebar (and focus)" },
    { "<leader>aA", desc = "Aerial: toggle sidebar (and focus)" },
    { "<leader>ao", desc = "Aerial: open sidebar (don't focus)" },
    { "<leader>aO", desc = "Aerial: open sidebar (and focus)" },
    { "<leader>ac", desc = "Aerial: close sidebar" },
    { "<leader>aC", desc = "Aerial: close all sidebars" },
  },
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

    -- stylua: ignore start
    map({ mode = "n", lhs = { "<leader>at", "<leader>aa" }, rhs = "AerialToggle!", desc = "Aerial: toggle sidebar (don't focus)" })
    map({ mode = "n", lhs = { "<leader>aT", "<leader>aA" }, rhs = "AerialToggle", desc = "Aerial: toggle sidebar (and focus)" })
    map({ mode = "n", lhs = "<leader>ao", rhs = "AerialOpen!", desc = "Aerial: open sidebar (don't focus)" })
    map({ mode = "n", lhs = "<leader>aO", rhs = "AerialOpen", desc = "Aerial: open sidebar (and focus)" })
    map({ mode = "n", lhs = "<leader>ac", rhs = "AerialClose", desc = "Aerial: close sidebar" })
    map({ mode = "n", lhs = "<leader>aC", rhs = "AerialCloseAll", desc = "Aerial: close all sidebars" })
  end,
}
