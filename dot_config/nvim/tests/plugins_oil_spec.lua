local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

---Run the oil spec `config` with oil faked, and return the options it hands to `oil.setup`.
---@return table
local function setup_options()
  local captured
  local saved_oil, saved_map = package.loaded["oil"], _G.map
  package.loaded["oil"] = {
    setup = function(opts)
      captured = opts
    end,
  }
  _G.map = function() end
  local ok, err = pcall(function()
    for _, plugin in ipairs(dofile(config_root .. "/lua/plugins/oil.lua")) do
      if plugin[1] == "stevearc/oil.nvim" then
        plugin.config()
      end
    end
  end)
  package.loaded["oil"], _G.map = saved_oil, saved_map
  assert(ok, err)
  return assert(captured, "the spec never called oil.setup")
end

return {
  ["turns off the default keys that other tools own"] = function()
    local keymaps = setup_options().keymaps
    for _, lhs in ipairs({ "<C-h>", "<C-l>", "<C-s>", "<C-t>" }) do
      assert(keymaps[lhs] == false, lhs .. " is " .. vim.inspect(keymaps[lhs]) .. ", want false")
    end
  end,
}
