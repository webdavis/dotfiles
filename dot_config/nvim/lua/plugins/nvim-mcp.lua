-- Loaded right after the first screen, so an agent's `nvim-mcp --connect auto` finds this Neovim's
-- socket. Its setup runs `git rev-parse` to name the socket, about 10 ms that stays off startup.
return {
  "linw1995/nvim-mcp",
  build = "cargo install --locked --path .",
  event = "VeryLazy",
  opts = {},
}
