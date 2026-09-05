-- The log level `lua/plugins/claudecode.lua` hands claudecode.nvim.
--
-- INFO goes through `nvim_echo`, which is stderr in a headless run, so a
-- headless session gets `warn` and the zero-stderr startup gate holds; an
-- interactive session keeps the plugin's own `info`, because `:ClaudeCodeStatus`
-- answers at INFO. The spec reads the level back out of the plugin file's
-- `opts` with the UI list faked each way, so the shipped rule is what is tested.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

local function level_with_uis(uis)
  local real = vim.api.nvim_list_uis
  vim.api.nvim_list_uis = function()
    return uis
  end
  local ok, opts = pcall(function()
    return dofile(config_root .. "/lua/plugins/claudecode.lua").opts()
  end)
  vim.api.nvim_list_uis = real
  assert(ok, opts)
  return opts.log_level, opts
end

return {
  ["a headless session (no UI attached) logs at warn"] = function()
    assert(level_with_uis({}) == "warn")
  end,
  ["an interactive session (a UI attached) keeps info"] = function()
    assert(level_with_uis({ { chan = 1 } }) == "info")
  end,
  ["the terminal provider stays none either way"] = function()
    local _, headless = level_with_uis({})
    local _, interactive = level_with_uis({ { chan = 1 } })
    assert(headless.terminal.provider == "none" and interactive.terminal.provider == "none")
  end,
}
