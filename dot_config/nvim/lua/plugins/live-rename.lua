return {
  "saecki/live-rename.nvim",
  config = function()
    local live_rename = require("live-rename")

    -- stylua: ignore start
    map({ mode = "n", lhs = "<leader>rn", rhs = live_rename.rename, desc = "Live Rename: normal mode" })
    map({ mode = "n", lhs = "<leader>rN", rhs = live_rename.map({ text = "", insert = true }), desc = "Live Rename: insert mode" })
  end,
}
