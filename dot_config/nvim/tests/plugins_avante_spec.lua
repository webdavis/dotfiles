-- Every key needs a callable rhs, and Avante keeps its own maps off because aerial owns `<leader>a`.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

local function spec()
  return dofile(config_root .. "/lua/plugins/avante.lua")
end

local function has_key(value, wanted)
  if type(value) ~= "table" then
    return false
  end
  for key, child in pairs(value) do
    if key == wanted or has_key(child, wanted) then
      return true
    end
  end
  return false
end

return {
  ["the Claude Code provider needs no credential in this spec"] = function()
    local plugin = spec()
    assert(plugin.opts.provider == "claude-code")
    for _, key in ipairs({ "providers", "api_key", "api_key_name" }) do
      assert(not has_key(plugin, key), "the spec declares " .. key)
    end
  end,
  ["lazy owns every global avante key with a callable"] = function()
    local plugin = spec()
    assert(plugin.opts.behaviour.auto_set_keymaps == false)
    for _, row in ipairs(plugin.keys) do
      assert(type(row[2]) == "function", row[1] .. " has no callable")
      assert(row[1]:sub(1, 9) == "<leader>v", row[1] .. " is outside the avante prefix")
      assert(row[1]:sub(1, 9) ~= "<leader>a", row[1] .. " claims the aerial prefix")
    end
  end,
  ["the source build and its two hard dependencies are declared"] = function()
    local plugin = spec()
    assert(plugin.build == "make")
    assert(vim.tbl_contains(plugin.dependencies, "nvim-lua/plenary.nvim"))
    assert(vim.tbl_contains(plugin.dependencies, "MunifTanjim/nui.nvim"))
  end,
}
