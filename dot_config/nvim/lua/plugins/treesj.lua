return {
  "Wansmer/treesj",
  dependencies = { "nvim-treesitter/nvim-treesitter" },
  config = function()
    local treesj = require("treesj")

    treesj.setup({
      use_default_keymaps = false,
      max_join_length = 500,
    })

    -- stylua: ignore start
    map({ mode = "n", lhs = "<leader>jt", rhs = function() treesj.toggle({ split = { recursive = true } }) end, desc = "TreeSJ: toggle (recursive)" })
    map({ mode = "n", lhs = "<leader>jT", rhs = function() treesj.toggle() end, desc = "TreeSJ: toggle" })
    map({ mode = "n", lhs = "<leader>js", rhs = function() treesj.split({ split = { recursive = true } }) end, desc = "TreeSJ: split (recursive)" })
    map({ mode = "n", lhs = "<leader>jS", rhs = function() treesj.split() end, desc = "TreeSJ: split" })
    map({ mode = "n", lhs = "<leader>jj", rhs = function() treesj.join() end, desc = "TreeSJ: join" })
  end,
}
