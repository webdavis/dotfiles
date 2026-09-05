return {
  "Wansmer/treesj",
  dependencies = { "nvim-treesitter/nvim-treesitter" },
  -- treesj creates its three commands from inside `setup()`, which `config`
  -- runs, so they have to be named up front or the first use does nothing.
  cmd = { "TSJJoin", "TSJSplit", "TSJToggle" },
  -- These rows are the mappings themselves, not a second copy of them: lazy.nvim
  -- installs the placeholder at startup and sets the real mapping from the same
  -- row on the first press. `require` sits inside each callback because the
  -- plugin is not on the runtimepath until that press loads it.
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
