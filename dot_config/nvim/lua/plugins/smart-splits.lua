-- Ctrl-h/j/k/l move across Neovim splits and herdr panes. At a split edge, the herdr backend in
-- lua/smart-splits/mux/herdr.lua runs `herdr pane focus`.
return {
  "mrjones2014/smart-splits.nvim",
  lazy = false,
  opts = {
    multiplexer_integration = "herdr",
    at_edge = "stop",
    disable_multiplexer_nav_when_zoomed = true,
  },
  config = function(_, opts)
    local ss = require("smart-splits")
    ss.setup(opts)
    vim.keymap.set("n", "<C-h>", ss.move_cursor_left, { desc = "Nav left (split/pane)" })
    vim.keymap.set("n", "<C-j>", ss.move_cursor_down, { desc = "Nav down (split/pane)" })
    vim.keymap.set("n", "<C-k>", ss.move_cursor_up, { desc = "Nav up (split/pane)" })
    vim.keymap.set("n", "<C-l>", ss.move_cursor_right, { desc = "Nav right (split/pane)" })
    vim.keymap.set("n", "<C-\\>", ss.move_cursor_previous, { desc = "Nav to previous split/pane" })
  end,
}
