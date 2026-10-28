return {
  "saecki/live-rename.nvim",
  keys = {
    {
      "<leader>rn",
      function()
        require("live-rename").rename()
      end,
      desc = "Live Rename: normal mode",
    },
    {
      "<leader>rN",
      function()
        require("live-rename").rename({ text = "", insert = true })
      end,
      desc = "Live Rename: insert mode",
    },
  },
}
