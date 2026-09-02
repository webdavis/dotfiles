return {
  {
    "folke/flash.nvim",
    ---@type Flash.Config
    opts = {
      modes = {
        search = {
          enabled = false,
        },
        char = {
          enabled = false,
          jump_labels = true,
        },
      },
      label = {
        uppercase = true,
      },
    },
    keys = {
      -- stylua: ignore start
      { "<leader>/f", mode = { "n", "x", "o" }, function() require("flash").jump() end, desc = "Flash: Search" },
      { "gs", mode = { "n", "x", "o" }, function() require("flash").treesitter() end, desc = "Flash: Visually select Treesitter objects" },
      { "r", mode = "o", function() require("flash").remote() end, desc = "Flash: choose remote location for text operation" },
      { "R", mode = { "o", "x" }, function() require("flash").treesitter_search() end, desc = "Treesitter Search" },
      { "<c-s>", mode = { "c" }, function() require("flash").toggle() end, desc = "Flash: Toggle flash style search" },
      -- stylua: ignore end

      -- Simulate nvim-treesitter incremental selection.
      {
        "<c-space>",
        mode = { "n", "o", "x" },
        function()
          require("flash").treesitter({
            actions = {
              ["<c-space>"] = "next",
              ["<BS>"] = "prev",
            },
          })
        end,
        desc = "Treesitter Incremental Selection",
      },
    },
  },
}
