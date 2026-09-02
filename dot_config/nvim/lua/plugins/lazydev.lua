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
      -- every file under lua/plugins is a lazy.nvim spec, so these types are
      -- always wanted: an entry with no words is loaded globally
      "lazy.nvim",
    },
  },
}
