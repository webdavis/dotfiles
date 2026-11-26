local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

---@return table
local function picker_options()
  for _, plugin in ipairs(dofile(config_root .. "/lua/plugins/snacks.lua")) do
    if plugin[1] == "folke/snacks.nvim" then
      return plugin.opts.picker
    end
  end
  error("no folke/snacks.nvim spec")
end

local function assert_unbound(keys, lhs)
  assert(keys[lhs] == false, lhs .. " is " .. vim.inspect(keys[lhs]) .. ", want false")
end

return {
  ["the prompt, the list and the explorer leave ctrl-t to another tool"] = function()
    local picker = picker_options()
    assert_unbound(picker.win.input.keys, "<c-t>")
    assert_unbound(picker.win.list.keys, "<c-t>")
    assert_unbound(picker.sources.explorer.win.list.keys, "<c-t>")
  end,

  ["the projects prompt reopens itself in a new tab on ctrl-y"] = function()
    local keys = picker_options().sources.projects.win.input.keys
    assert_unbound(keys, "<c-t>")
    local key = keys["<c-y>"]
    assert(vim.deep_equal(key.mode, { "n", "i" }), "<c-y> modes are " .. vim.inspect(key.mode))

    local closed, reopened = false, false
    local saved_snacks = _G.Snacks
    _G.Snacks = {
      notify = function() end,
      picker = {
        projects = function()
          reopened = true
        end,
      },
    }
    local tabs_before = #vim.api.nvim_list_tabpages()
    local ok, err = pcall(key[1], {
      close = function()
        closed = true
      end,
    })
    local tabs_after = #vim.api.nvim_list_tabpages()
    _G.Snacks = saved_snacks
    vim.cmd("silent! tabonly")
    assert(ok, err)
    assert(tabs_after == tabs_before + 1, "opened " .. (tabs_after - tabs_before) .. " tabs, want 1")
    assert(closed and reopened, "the picker was not closed and reopened")
  end,
}
