return {
  "Wansmer/treesj",
  dependencies = { "nvim-treesitter/nvim-treesitter" },
  -- treesj creates its three commands and, here, its five mappings from inside
  -- `setup()`, which `config` runs, so both have to be named up front or the
  -- first use does nothing; `desc` mirrors each `map()` call so the which-key
  -- popup reads the same before and after treesj loads.
  cmd = { "TSJJoin", "TSJSplit", "TSJToggle" },
  keys = {
    { "<leader>jj", desc = "TreeSJ: join" },
    { "<leader>jS", desc = "TreeSJ: split" },
    { "<leader>js", desc = "TreeSJ: split (recursive)" },
    { "<leader>jT", desc = "TreeSJ: toggle" },
    { "<leader>jt", desc = "TreeSJ: toggle (recursive)" },
  },
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
