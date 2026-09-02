return {
  "axieax/urlview.nvim",
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
      desc = "UrlView: open LazyVim plugin page",
    })
  end,
}
