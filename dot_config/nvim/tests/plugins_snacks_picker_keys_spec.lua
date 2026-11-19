local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

---@return table
local function picker_windows()
  for _, plugin in ipairs(dofile(config_root .. "/lua/plugins/snacks.lua")) do
    if plugin[1] == "folke/snacks.nvim" then
      return plugin.opts.picker.win
    end
  end
  error("no folke/snacks.nvim spec")
end

return {
  ["the prompt opens the selection in a new tab on ctrl-y"] = function()
    local keys = picker_windows().input.keys
    assert(vim.deep_equal(keys["<c-y>"], { "tab", mode = { "n", "i" } }), "<c-y> is " .. vim.inspect(keys["<c-y>"]))
    assert(keys["<c-t>"] == false, "<c-t> is " .. vim.inspect(keys["<c-t>"]) .. ", want false")
  end,

  ["the list opens the selection in a new tab on ctrl-y"] = function()
    local keys = picker_windows().list.keys
    assert(keys["<c-y>"] == "tab", "<c-y> is " .. vim.inspect(keys["<c-y>"]))
    assert(keys["<c-t>"] == false, "<c-t> is " .. vim.inspect(keys["<c-t>"]) .. ", want false")
  end,
}
