-- To profile Neovim startup time, launch Neovim with:
--
--   $ PROFILE=1 nvim
--
-- This generates timing information for plugin loading and initialization.
-- Ref: https://github.com/folke/snacks.nvim/blob/main/docs/profiler.md#profiling-neovim-startup
if vim.env.PROFILE then
  -- example for lazy.nvim
  -- change this to the correct path for your plugin manager
  local snacks = vim.fn.stdpath("data") .. "/lazy/snacks.nvim"
  vim.opt.rtp:append(snacks)
  require("snacks.profiler").startup({
    startup = {
      event = "VimEnter", -- stop profiler on this event. Defaults to `VimEnter`
      -- event = "UIEnter",
      -- event = "VeryLazy",
    },
  })
end

-- Global Helpers
local api = require("custom_api")
_G.map = api.util.map

-- Import Config
require("config.options")
require("config.keymaps")
require("config.autocmds")
require("config.lazy")
