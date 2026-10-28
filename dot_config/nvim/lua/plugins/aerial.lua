return {
  "stevearc/aerial.nvim",
  -- Treesitter-only buffers never fire LspAttach,
  -- so aerial loads on any file buffer to run `on_attach`.
  event = { "BufReadPost", "BufNewFile" },
  -- Every command aerial registers, so a typed one works before the plugin loads.
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
  -- `on_attach` sets `<leader>is`, `{` and `}` per buffer.
  -- stylua: ignore start
  keys = {
    { "<leader>ia", "<cmd>AerialToggle!<cr>",   desc = "Aerial: toggle sidebar (don't focus)" },
    { "<leader>iA", "<cmd>AerialToggle<cr>",    desc = "Aerial: toggle sidebar (and focus)" },
    { "<leader>io", "<cmd>AerialOpen!<cr>",     desc = "Aerial: open sidebar (don't focus)" },
    { "<leader>iO", "<cmd>AerialOpen<cr>",      desc = "Aerial: open sidebar (and focus)" },
    { "<leader>ic", "<cmd>AerialClose<cr>",     desc = "Aerial: close sidebar" },
    { "<leader>iC", "<cmd>AerialCloseAll<cr>",  desc = "Aerial: close all sidebars" },
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
          lhs = "<leader>is",
          rhs = function()
            require("aerial").snacks_picker({
              layout = {},
            })
          end,
          desc = "Aerial: Snacks picker",
        })
      end,
    }

    require("aerial").setup(opts)
  end,
}
