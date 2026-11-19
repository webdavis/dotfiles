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

local function assert_select(keymap, lhs, opts)
  assert(
    vim.deep_equal(keymap, { "actions.select", opts = opts }),
    lhs .. " is " .. vim.inspect(keymap) .. ", want select with " .. vim.inspect(opts)
  )
end

return {
  ["turns off the default keys that other tools own"] = function()
    local keymaps = setup_options().keymaps
    for _, lhs in ipairs({ "<C-h>", "<C-l>", "<C-s>", "<C-t>" }) do
      assert(keymaps[lhs] == false, lhs .. " is " .. vim.inspect(keymaps[lhs]) .. ", want false")
    end
  end,

  ["opens in splits and a new tab"] = function()
    local keymaps = setup_options().keymaps
    assert_select(keymaps["<C-x>"], "<C-x>", { horizontal = true })
    assert_select(keymaps["<C-v>"], "<C-v>", { vertical = true })
    assert_select(keymaps["<C-y>"], "<C-y>", { tab = true })
  end,

  ["refreshes on localleader r and never on R"] = function()
    local keymaps = setup_options().keymaps
    assert(keymaps["<localleader>r"] == "actions.refresh", "got " .. vim.inspect(keymaps["<localleader>r"]))
    assert(keymaps["R"] == nil, "R is bound to " .. vim.inspect(keymaps["R"]))
  end,

  ["watches the filesystem for changes"] = function()
    assert(setup_options().watch_for_changes == true, "watch_for_changes is not true")
  end,
}
