-- `VeryLazy` is safe because the port floor 49152 sits above the ports local probes hit.
-- A headless session logs at `warn`, keeping stderr empty. An interactive one keeps `info` for `:ClaudeCodeStatus`.

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

-- Runs the plugin `init` first, as lazy.nvim does, then attaches a UI with the fake logger present or absent.
-- Returns the level the fake was handed, or nil.
local function fake_logger(record)
  return {
    setup = function(conf)
      record.level = conf.log_level
      record.calls = (record.calls or 0) + 1
    end,
  }
end

local function level_after_ui_attaches(logger_loaded)
  local record = {}
  local real = package.loaded["claudecode.logger"]
  dofile(config_root .. "/lua/plugins/claudecode.lua").init()
  package.loaded["claudecode.logger"] = logger_loaded and fake_logger(record) or nil
  vim.api.nvim_exec_autocmds("UIEnter", {})
  package.loaded["claudecode.logger"] = real
  return record.level
end

-- The UI attaches before the plugin loads, and a later attach must still restore the level.
local function level_after_early_and_late_attach()
  local record = {}
  local real = package.loaded["claudecode.logger"]
  dofile(config_root .. "/lua/plugins/claudecode.lua").init()
  package.loaded["claudecode.logger"] = nil
  vim.api.nvim_exec_autocmds("UIEnter", {})
  package.loaded["claudecode.logger"] = fake_logger(record)
  vim.api.nvim_exec_autocmds("UIEnter", {})
  vim.api.nvim_exec_autocmds("UIEnter", {})
  package.loaded["claudecode.logger"] = real
  return record.level, record.calls
end

return {
  ["a UI that attached before the plugin loaded does not spend the hook"] = function()
    local level, calls = level_after_early_and_late_attach()
    assert(level == "info", "late attach did not restore: " .. tostring(level))
    assert(calls == 1, "restore ran " .. tostring(calls) .. " times, want once")
  end,
  ["a UI attaching after the plugin loaded raises the level back to info"] = function()
    assert(level_after_ui_attaches(true) == "info")
  end,
  ["a UI attaching before the plugin loaded touches no logger"] = function()
    assert(level_after_ui_attaches(false) == nil)
  end,
  ["a headless session (no UI attached) logs at warn"] = function()
    assert(level_with_uis({}) == "warn")
  end,
  ["an interactive session (a UI attached) keeps info"] = function()
    assert(level_with_uis({ { chan = 1 } }) == "info")
  end,
  ["the websocket port cannot land where the listener probe reaches"] = function()
    -- Every port below 49152 gets probed.
    local _, opts = level_with_uis({ { chan = 1 } })
    local range = assert(opts.port_range, "the spec hands claudecode no port range")
    assert(range.min >= 49152, "the port floor is " .. range.min .. ", inside the probed range")
    assert(range.max <= 65535, "the port ceiling is " .. range.max .. ", past the last port")
    assert(range.min < range.max, "the range is empty or inverted")
  end,
  ["the terminal provider stays none either way"] = function()
    local _, headless = level_with_uis({})
    local _, interactive = level_with_uis({ { chan = 1 } })
    assert(headless.terminal.provider == "none" and interactive.terminal.provider == "none")
  end,
}
