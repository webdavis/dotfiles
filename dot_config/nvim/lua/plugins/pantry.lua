return {
  "webdavis/pantry.nvim",
  lazy = false,
  -- Loads first, so ui2 shows and records the messages other plugins print while they start.
  priority = 2000,
  dependencies = { "nvim-treesitter/nvim-treesitter" },
  opts = {
    rsi = {},
    jumps = {},
    markdown = {},
    messages = {},
    linelimit = {},
    focus = {},
    autosave = {
      mode = "off",
      skip = {
        "gitcommit",
        "NvimTree",
        "Outline",
        "alpha",
        "dashboard",
        "lazygit",
        "neo-tree",
        "oil",
        "prompt",
        "toggleterm",
        "harpoon",
      },
      -- A write to a claudecode.nvim proposed-edit buffer accepts the edit.
      condition = function(buf)
        return vim.b[buf].claudecode_diff_tab_name == nil
      end,
    },
  },
  keys = {
    {
      "<leader>ux",
      function()
        require("pantry.focus").toggle()
      end,
      desc = "Toggle focus on the code around the cursor",
    },
    {
      "<leader>uv",
      function()
        local autosave = require("pantry.autosave")
        autosave.toggle()
        vim.notify("Autosave is " .. (autosave.is_enabled() and "on" or "off"))
      end,
      desc = "Toggle Autosave",
    },
  },
}
