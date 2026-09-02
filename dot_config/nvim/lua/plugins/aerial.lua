return {
  "stevearc/aerial.nvim",
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
