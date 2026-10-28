-- The direction of `n` and `N` in every mode that maps them.
--
-- `lua/config/keymaps.lua` wants the vim-galore recipe that keeps `n` always
-- forward and `N` always backward whichever way the search was started, and it
-- sets that in normal, visual and operator-pending mode.
-- `lua/plugins/hlslens.lua` then re-maps normal mode alone, so before this spec
-- `n` moved backward after a `?` search while `dn` operated forward. The two
-- disagreeing is the defect; the fix keeps hlslens's documented composition and
-- puts the direction-stable motion inside it.
--
-- The spec exercises the real mappings rather than reading their text: it runs
-- the plugin file's `config` with hlslens faked, seeds a backward search
-- (`v:searchforward == 0`) and feeds the keys.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

local LINES = { "target", "aaa", "target", "bbb", "target" }

-- The search-count message would otherwise interleave with the runner's report.
vim.opt.shortmess:append("S")

local lens_starts

-- `config` calls the global `map()` that `init.lua` installs, and requires
-- hlslens itself. Both are supplied here, so the mappings under test are the
-- shipped ones.
local function install_mappings()
  lens_starts = 0
  _G.map = require("custom_api.keymap").map
  package.loaded["hlslens"] = {
    setup = function() end,
    start = function()
      lens_starts = lens_starts + 1
    end,
  }
  -- The startup order: `init.lua` runs `config/keymaps.lua` first, then
  -- lazy.nvim loads hlslens, whose `config` re-maps normal mode over it.
  dofile(config_root .. "/lua/config/keymaps.lua")
  dofile(config_root .. "/lua/plugins/hlslens.lua").config()
end

-- A buffer whose matches sit on lines 1, 3 and 5, the cursor between the last
-- two, and a search register that remembers a BACKWARD search. A
-- direction-stable `n` goes to line 5; a plain `n` goes to line 3.
local function backward_search_buffer()
  local buf = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, LINES)
  vim.api.nvim_win_set_buf(0, buf)
  vim.api.nvim_win_set_cursor(0, { 4, 0 })
  vim.fn.setreg("/", "target")
  vim.v.searchforward = 0
  return buf
end

-- `normal` without a bang so the mappings under test are used, and `silent!` so
-- the motion's own search echo stays out of the runner's report.
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
