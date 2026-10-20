-- herdr-nvim-2, linked from its checkout until the repository is public. The
-- plugin maps no keys, so every `<leader>a` key is declared here.

-- `annotate_line` returns its refusal rather than notifying it.
local function annotate(send)
  local id, reason = require("herdr-nvim-2").annotate_line({ send = send })
  if not id then
    vim.notify("herdr-nvim-2: " .. reason, vim.log.levels.WARN)
  end
end

return {
  dir = "/Users/stephen/workspaces/Ivy/webdavis/herdr-nvim-2",
  name = "herdr-nvim-2",
  lazy = false,
  opts = {},
  keys = {
    {
      "<leader>ac",
      function()
        require("herdr-nvim-2").comment_line()
      end,
      desc = "herdr-nvim-2: comment line",
    },
    {
      "<leader>ac",
      function()
        require("herdr-nvim-2").comment_selection()
      end,
      mode = "x",
      desc = "herdr-nvim-2: comment selection",
    },
    {
      "<leader>al",
      function()
        require("herdr-nvim-2").list_comments()
      end,
      desc = "herdr-nvim-2: list comments",
    },
    {
      "<leader>aS",
      function()
        require("herdr-nvim-2").send()
      end,
      desc = "herdr-nvim-2: send comments to agent",
    },
    {
      "<leader>ai",
      function()
        require("herdr-nvim-2").ref_line()
      end,
      desc = "herdr-nvim-2: reference line at agent cursor",
    },
    {
      "<leader>ai",
      function()
        require("herdr-nvim-2").ref_selection()
      end,
      mode = "x",
      desc = "herdr-nvim-2: reference selection at agent cursor",
    },
    {
      "<leader>ad",
      function()
        annotate(false)
      end,
      desc = "herdr-nvim-2: annotate line",
    },
    {
      "<leader>aD",
      function()
        annotate(true)
      end,
      desc = "herdr-nvim-2: annotate line and send it",
    },
    {
      "<leader>ap",
      function()
        require("herdr-nvim-2").send_paragraph()
      end,
      desc = "herdr-nvim-2: send paragraph",
    },
    {
      "<leader>ap",
      function()
        require("herdr-nvim-2").send_selection()
      end,
      mode = "x",
      desc = "herdr-nvim-2: send selection",
    },
    {
      "<leader>aq",
      function()
        require("herdr-nvim-2").queue_menu()
      end,
      desc = "herdr-nvim-2: queued prompts",
    },
    {
      "<leader>ao",
      function()
        require("herdr-nvim-2").panel()
      end,
      desc = "herdr-nvim-2: comment panel",
    },
    {
      "]r",
      function()
        require("herdr-nvim-2").next_comment()
      end,
      desc = "herdr-nvim-2: next comment",
    },
    {
      "[r",
      function()
        require("herdr-nvim-2").prev_comment()
      end,
      desc = "herdr-nvim-2: previous comment",
    },
  },
}
