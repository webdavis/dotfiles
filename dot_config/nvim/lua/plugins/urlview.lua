return {
  "axieax/urlview.nvim",
  cmd = "UrlView",
  keys = {
    {
      "<leader>UU",
      "<cmd>UrlView buffer action=system<cr>",
      desc = "UrlView: open selected URL in the browser",
      silent = true,
    },
    {
      "<leader>Uo",
      "<cmd>UrlView buffer action=system<cr>",
      desc = "UrlView: open selected URL in the browser (alias of <leader>UU)",
      silent = true,
    },
    {
      "<leader>Ul",
      "<cmd>UrlView lazy action=system<cr>",
      desc = "UrlView: open an installed plugin's page",
      silent = true,
    },
    -- These entries only load the plugin, because urlview binds `[u` and `]u` itself.
    -- Listing them here also keeps vim-unimpaired from taking them first.
    { "[u", desc = "Previous URL" },
    { "]u", desc = "Next URL" },
  },
  config = function()
    require("urlview").setup({})
  end,
}
