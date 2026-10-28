-- `PROFILE=1 nvim` profiles startup.
if vim.env.PROFILE then
  local snacks = vim.fn.stdpath("data") .. "/lazy/snacks.nvim"
  vim.opt.rtp:append(snacks)
  require("snacks.profiler").startup({
    startup = {
      event = "VimEnter",
    },
  })
end

-- Runs before config.options sets the leader, so require only the module that exports `map`.
_G.map = require("custom_api.keymap").map

require("config.options")
require("config.keymaps")
require("config.autocmds")
require("config.lazy")
