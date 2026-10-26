return {
  "axieax/urlview.nvim",
  -- `UrlView` is the only command urlview registers (urlview/command.lua).
  cmd = "UrlView",
  -- Each row IS the mapping: lazy sets a placeholder at startup and installs this
  -- same rhs when the plugin loads, so there is no second copy in `config`.
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
    -- Trigger-only, no rhs: urlview binds `[u` and `]u` itself inside `setup`
    -- (urlview/jump.lua), so these placeholders only load it on the first
    -- press, which then lands on urlview's own jump.
    { "[u", desc = "Previous URL" },
    { "]u", desc = "Next URL" },
  },
  config = function()
    require("urlview").setup({})
  end,
}
