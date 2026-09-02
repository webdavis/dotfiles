return {
  "mgierada/lazydocker.nvim",
  dependencies = { "akinsho/toggleterm.nvim" },
  config = function()
    require("lazydocker").setup({
      -- Border options: "single" | "double" | "shadow" | "curved".
      border = "curved",
      width = 0.8,
      height = 0.8,
    })
  end,
  event = "BufRead",
  keys = {
    {
      "<leader>dd",
      function()
        require("lazydocker").open()
      end,
      desc = "Lazydocker: open floating window",
    },
  },
}
