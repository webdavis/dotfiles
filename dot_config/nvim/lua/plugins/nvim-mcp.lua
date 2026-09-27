-- Loaded at startup so an agent's `nvim-mcp --connect auto` finds this Neovim's socket.
return {
  "linw1995/nvim-mcp",
  build = "cargo install --locked --path .",
  lazy = false,
  opts = {},
}
