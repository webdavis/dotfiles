-- `n` stays forward and `N` backward in every mode, including the hlslens normal-mode remap.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

local LINES = { "target", "aaa", "target", "bbb", "target" }

-- The search-count message would otherwise interleave with the runner's report.
vim.opt.shortmess:append("S")

local lens_starts

-- Supplies the global `map()` and a fake hlslens, so the shipped mappings run.
local function install_mappings()
  lens_starts = 0
  _G.map = require("custom_api.keymap").map
  package.loaded["hlslens"] = {
    setup = function() end,
    start = function()
      lens_starts = lens_starts + 1
    end,
  }
  -- Loads in startup order, so hlslens re-maps normal mode after the keymaps.
  dofile(config_root .. "/lua/config/keymaps.lua")
  dofile(config_root .. "/lua/plugins/hlslens.lua").config()
end

-- Matches on lines 1, 3 and 5, cursor between the last two, after a BACKWARD search. A direction-stable `n` lands on 5.
local function backward_search_buffer()
  local buf = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, LINES)
  vim.api.nvim_win_set_buf(0, buf)
  vim.api.nvim_win_set_cursor(0, { 4, 0 })
  vim.fn.setreg("/", "target")
  vim.v.searchforward = 0
  return buf
end

-- `normal` without a bang uses the mappings. `silent!` keeps the search echo out of the report.
local function feed(keys)
  vim.cmd("silent! normal " .. keys)
end

return {
  ["normal-mode n stays forward after a backward search"] = function()
    install_mappings()
    backward_search_buffer()
    feed("n")
    local line = vim.api.nvim_win_get_cursor(0)[1]
    assert(line == 5, "n landed on line " .. line .. ", want 5 (forward)")
  end,

  ["normal-mode N stays backward after a backward search"] = function()
    install_mappings()
    backward_search_buffer()
    feed("N")
    local line = vim.api.nvim_win_get_cursor(0)[1]
    assert(line == 3, "N landed on line " .. line .. ", want 3 (backward)")
  end,

  ["normal-mode n still starts the lens"] = function()
    install_mappings()
    backward_search_buffer()
    feed("n")
    assert(lens_starts == 1, "hlslens.start() ran " .. lens_starts .. " times, want once")
  end,

  ["operator-pending n agrees with normal-mode n"] = function()
    install_mappings()
    backward_search_buffer()
    feed('"ayn')
    local yanked = vim.fn.getreg("a")
    assert(yanked:sub(1, 3) == "bbb", "yn took " .. vim.inspect(yanked) .. ", want the forward region")
  end,
}
