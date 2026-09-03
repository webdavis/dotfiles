-- init.lua requires one custom_api module to publish `_G.map`, and it does that
-- BEFORE `config.options` assigns vim.g.mapleader. Whatever it requires must
-- therefore install nothing, or mappings are created against a leader that does
-- not exist yet. The module name is read out of init.lua rather than spelled
-- here, so this pins the bootstrap's actual load, not a copy of it.

-- The runner prepends `<config_root>/lua/?.lua` to package.path.
local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

local modes = { "n", "i", "v", "x", "s", "o", "c", "t" }

local function keymaps()
  local seen = {}
  for _, mode in ipairs(modes) do
    for _, keymap in ipairs(vim.api.nvim_get_keymap(mode)) do
      seen[mode .. " " .. keymap.lhs] = true
    end
  end
  return seen
end

local function forget_custom_api()
  for name in pairs(package.loaded) do
    if name == "custom_api" or name:match("^custom_api%.") then
      package.loaded[name] = nil
    end
  end
end

return {
  ["the module init.lua requires for `map` installs no keymaps"] = function()
    local init = assert(io.open(config_root .. "/init.lua")):read("a")
    local module_name =
      assert(init:match('require%("(custom_api[%w_%.]*)"%)'), "init.lua requires no custom_api module")

    forget_custom_api()
    local before = keymaps()
    require(module_name)
    local after = keymaps()

    local added = {}
    for lhs in pairs(after) do
      if not before[lhs] then
        table.insert(added, lhs)
      end
    end
    table.sort(added)

    assert(
      #added == 0,
      ("requiring %s installed %d keymaps: %s"):format(module_name, #added, table.concat(added, ", "))
    )
    assert(
      vim.g.mapleader == nil,
      "requiring " .. module_name .. " set vim.g.mapleader to " .. tostring(vim.g.mapleader)
    )
  end,
}
