return {
  "axieax/urlview.nvim",
  -- `UrlView` is the only command urlview registers (urlview/command.lua). The
  -- `desc` values repeat the ones `config` sets below, so which-key labels the key
  -- the same before and after the plugin loads.
  cmd = "UrlView",
  keys = {
    { "<leader>UU", desc = "UrlView: open selected URL in the browser" },
    { "<leader>Uo", desc = "UrlView: open selected URL in the browser" },
    { "<leader>Ul", desc = "UrlView: open an installed plugin's page" },
    -- urlview sets `[u` and `]u` itself (urlview/jump.lua), and vim-unimpaired
    -- claims the same pair only when `maparg` finds them free (its `s:Map`
    -- guard). Eager urlview got there first; a lazy one would hand both keys to
    -- unimpaired's URL encode and decode. Declaring them here puts lazy's
    -- placeholder in `maparg` before unimpaired's VeryLazy load, so the guard
    -- still skips and a press still reaches urlview's own jump.
    { "[u", desc = "Previous URL" },
    { "]u", desc = "Next URL" },
  },
  config = function()
    require("urlview").setup({})

    map({
      mode = "n",
      lhs = { "<leader>UU", "<leader>Uo" },
      rhs = "UrlView buffer action=system",
      desc = "UrlView: open selected URL in the browser",
    })

    map({
      mode = "n",
      lhs = "<leader>Ul",
      rhs = "UrlView lazy action=system",
      desc = "UrlView: open an installed plugin's page",
    })
  end,
}
