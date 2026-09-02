-- The nvim half of ChmaraX/herdr-nvim (the herdr plugin is installed by the
-- dotfiles' run_after_53): line-anchored annotations sent back to agents.
return {
  "ChmaraX/herdr-nvim",
  opts = {
    prefix = "<leader>A", -- the default <leader>a collides with other maps here
  },
}
