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
  ["the prompt opens the selection in a new tab on ctrl-y"] = function()
    local keys = picker_options().win.input.keys
    assert(vim.deep_equal(keys["<c-y>"], { "tab", mode = { "n", "i" } }), "<c-y> is " .. vim.inspect(keys["<c-y>"]))
    assert_unbound(keys, "<c-t>")
  end,

  ["the list opens the selection in a new tab on ctrl-y"] = function()
    local keys = picker_options().win.list.keys
    assert(keys["<c-y>"] == "tab", "<c-y> is " .. vim.inspect(keys["<c-y>"]))
    assert_unbound(keys, "<c-t>")
  end,

  ["the explorer list leaves ctrl-t unbound"] = function()
    assert_unbound(picker_options().sources.explorer.win.list.keys, "<c-t>")
  end,

  ["the github and undo prompts copy on alt-y and leave ctrl-y unbound"] = function()
    local sources = picker_options().sources
    for name, copy in pairs({ gh_issue = "gh_yank", gh_pr = "gh_yank", undo = "yank_add" }) do
      local keys = sources[name].win.input.keys
      assert(
        vim.deep_equal(keys["<a-y>"], { copy, mode = { "n", "i" } }),
        name .. " <a-y> is " .. vim.inspect(keys["<a-y>"])
      )
      assert_unbound(keys, "<c-y>")
      assert_unbound(sources[name].win.list.keys, "<c-y>")
    end
  end,

  ["the undo prompt copies deleted lines on alt-shift-y"] = function()
    local keys = picker_options().sources.undo.win.input.keys
    assert(
      vim.deep_equal(keys["<a-Y>"], { "yank_del", mode = { "n", "i" } }),
      "<a-Y> is " .. vim.inspect(keys["<a-Y>"])
    )
    assert_unbound(keys, "<c-s-y>")
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
