-- Neovim and the sidebar launcher both call setup. Keep mapping ownership here.
return {
  "ChmaraX/herdr-nvim",
  lazy = false,
  opts = { prefix = "<leader>A", keymaps = false },
  keys = {
    {
      "<leader>Ac",
      function()
        require("herdr-nvim").comment_selection()
      end,
      mode = "x",
      desc = "herdr-nvim: comment selection",
    },
    {
      "<leader>Ac",
      function()
        require("herdr-nvim").comment_line()
      end,
      desc = "herdr-nvim: comment line",
    },
    {
      "<leader>Al",
      function()
        require("herdr-nvim").list_comments()
      end,
      desc = "herdr-nvim: list comments",
    },
    {
      "<leader>As",
      function()
        require("herdr-nvim").send_all({ submit = false })
      end,
      desc = "herdr-nvim: paste comments to agent",
    },
    {
      "<leader>AS",
      function()
        require("herdr-nvim").send_all({ submit = true })
      end,
      desc = "herdr-nvim: send comments to agent",
    },
  },
}
