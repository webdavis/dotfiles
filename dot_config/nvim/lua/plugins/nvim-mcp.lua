-- Loaded right after the first screen, so an agent's `nvim-mcp --connect auto` finds this Neovim's
-- socket.
return {
  "linw1995/nvim-mcp",
  build = "cargo install --locked --path .",
  event = "VeryLazy",
  opts = {},
}
