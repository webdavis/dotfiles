-- Range maps stay off `<leader>s`, the snacks picker group, and substituted text reaches the yanky ring.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

local function spec()
  return dofile(config_root .. "/lua/plugins/substitute.lua")
end

---Every (mode, lhs) pair the spec claims.
local function claimed()
  local keys = {}
  for _, row in ipairs(spec().keys) do
    local modes = row.mode or "n"
    modes = type(modes) == "table" and modes or { modes }
    for _, mode in ipairs(modes) do
      keys[mode .. " " .. row[1]] = row[2]
    end
  end
  return keys
end

return {
  ["the substitute operators are bound in the modes they work in"] = function()
    local keys = claimed()
    for _, id in ipairs({ "n s", "n ss", "n S", "x s" }) do
      assert(type(keys[id]) == "function", "not bound to a callable: " .. id)
    end
  end,
  ["the exchange operators and the cancel key are bound"] = function()
    local keys = claimed()
    for _, id in ipairs({ "n sx", "n sxx", "n sxc", "x X" }) do
      assert(type(keys[id]) == "function", "not bound to a callable: " .. id)
    end
  end,
  ["the range family is bound in both modes plus the word variant"] = function()
    local keys = claimed()
    for _, id in ipairs({ "n <leader>S", "x <leader>S", "n <leader>SS" }) do
      assert(type(keys[id]) == "function", "not bound to a callable: " .. id)
    end
  end,
  ["nothing is bound on the snacks picker prefix"] = function()
    for id in pairs(claimed()) do
      local lhs = id:sub(3)
      assert(lhs:sub(1, 9) ~= "<leader>s", "claims the snacks picker prefix: " .. lhs)
    end
  end,
  ["the substituted text is handed to yanky"] = function()
    local opts = spec().opts
    -- A function, because the yanky integration module exists only after lazy.nvim loads yanky.
    assert(type(opts) == "function", "opts is a table, so it is resolved before yanky is loaded")

    local asked = false
    local real = package.loaded["yanky.integration"]
    package.loaded["yanky.integration"] = {
      substitute = function()
        asked = true
        return function() end
      end,
    }
    local ok, resolved = pcall(opts)
    package.loaded["yanky.integration"] = real

    assert(ok, resolved)
    assert(asked, "opts never asked yanky for its substitute callback")
    assert(type(resolved.on_substitute) == "function", "on_substitute is not a callback")
  end,
  ["yanky loads first"] = function()
    assert(vim.tbl_contains(spec().dependencies, "gbprod/yanky.nvim"))
  end,
}
