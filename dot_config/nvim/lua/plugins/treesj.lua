return {
  "Wansmer/treesj",
  dependencies = { "nvim-treesitter/nvim-treesitter" },
  -- treesj creates these commands in `setup()`, so they are listed to trigger the load.
  cmd = { "TSJJoin", "TSJSplit", "TSJToggle" },
  -- stylua: ignore start
  keys = {
    { "<leader>jt", function() require("treesj").toggle({ split = { recursive = true } }) end, desc = "TreeSJ: toggle (recursive)", silent = true },
    { "<leader>jT", function() require("treesj").toggle() end, desc = "TreeSJ: toggle", silent = true },
    { "<leader>js", function() require("treesj").split({ split = { recursive = true } }) end, desc = "TreeSJ: split (recursive)", silent = true },
    { "<leader>jS", function() require("treesj").split() end, desc = "TreeSJ: split", silent = true },
    { "<leader>jj", function() require("treesj").join() end, desc = "TreeSJ: join", silent = true },
  },
  -- stylua: ignore end
  config = function()
    require("treesj").setup({
      use_default_keymaps = false,
      max_join_length = 500,
    })
  end,
}
