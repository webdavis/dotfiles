return {
  "folke/lazydev.nvim",
  version = "1.*",
  dependencies = {
    "folke/snacks.nvim",
  },
  ft = "lua",
  cmd = "LazyDev",
  opts = {
    library = {
      { path = "${3rd}/luv/library", words = { "vim%.uv" } },
      { path = "snacks.nvim", words = { "Snacks" } },
      -- Loads in every Lua file, because every file under lua/plugins is a lazy.nvim spec.
      "lazy.nvim",
    },
  },
}
