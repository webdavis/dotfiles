-- Loads only for notes in the Ivy vault, so other markdown files keep their usual behaviour.
local vault = vim.fn.expand("~/workspaces/Ivy")

return {
  "obsidian-nvim/obsidian.nvim",
  version = "*",
  event = { "BufReadPre " .. vault .. "/*.md", "BufNewFile " .. vault .. "/*.md" },
  cmd = "Obsidian",
  ---@module 'obsidian'
  ---@type obsidian.config
  opts = {
    legacy_commands = false,
    workspaces = { { name = "Ivy", path = vault } },
    -- markview.nvim already renders markdown, and two renderers fight over hidden text.
    ui = { enable = false },
    -- custom_api.markdown_task (`<C-CR>`) is the one task toggle; without this,
    -- obsidian's `<CR>` toggles a checkbox too.
    checkbox = { enabled = false },
  },
}
