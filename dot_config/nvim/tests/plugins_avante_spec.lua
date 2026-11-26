-- Avante keeps its own maps off because aerial owns `<leader>a`.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

local function spec()
  return dofile(config_root .. "/lua/plugins/avante.lua")
end

return {
  ["lazy owns every global avante key with a callable"] = function()
    local plugin = spec()
    assert(plugin.opts.behaviour.auto_set_keymaps == false)
    for _, row in ipairs(plugin.keys) do
      assert(type(row[2]) == "function", row[1] .. " has no callable")
      assert(row[1]:sub(1, 9) == "<leader>v", row[1] .. " is outside the avante prefix")
      assert(row[1]:sub(1, 9) ~= "<leader>a", row[1] .. " claims the aerial prefix")
    end
  end,
}
