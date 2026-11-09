return {
  "webdavis/pantry.nvim",
  lazy = false,
  -- Loads first, so ui2 shows and records the messages other plugins print while they start.
  priority = 2000,
  dependencies = { "nvim-treesitter/nvim-treesitter" },
  opts = {
    rsi = {},
    jumps = {},
    markdown = {},
    messages = {},
  },
}
