-- The Ivy vault is not synced to this Mac yet; it lands in ~/workspaces/Ivy
-- (memory: scalebar-on-hold). Loading only for notes under that folder keeps
-- every other markdown file free of vault behaviour.
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
    -- markview.nvim already renders markdown; two renderers fight over conceal.
    ui = { enable = false },
    -- custom_api.markdown_task (`<C-CR>`) is the one task toggle; without this,
    -- obsidian's `<CR>` toggles a checkbox too.
    checkbox = { enabled = false },
  },
}
