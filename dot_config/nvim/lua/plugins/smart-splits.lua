-- Seamless Ctrl-h/j/k/l across Neovim splits AND herdr panes.
--
-- smart-splits owns the keys inside nvim: move to the adjacent split, or — when
-- already at a split edge — hand off to the multiplexer. The handoff goes through
-- a custom herdr backend (lua/smart-splits/mux/herdr.lua) that calls
-- `herdr pane focus`. On the herdr side, the same keys run herdr-smart-nav.sh,
-- which forwards the keystroke back here when the focused pane is nvim, else moves
-- the herdr pane directly. One set of keys, no boundary to think about.
--
-- Replaces alexghergh/nvim-tmux-navigation (tmux-only: it shells out to
-- `tmux select-pane`, which herdr can't receive).
return {
  "mrjones2014/smart-splits.nvim",
  lazy = false,
  opts = {
    multiplexer_integration = "herdr",
    -- Stop at the outermost edge instead of wrapping around, matching the
    -- behavior of the previous nvim-tmux-navigation setup.
    at_edge = "stop",
    -- Don't navigate out of a zoomed split/pane (preserves disable_when_zoomed).
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
