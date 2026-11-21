local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

local no_ui = {}
local one_ui = { { chan = 1 } }

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

local function fake_logger(record)
  return {
    setup = function(conf)
      record.level = conf.log_level
      record.calls = (record.calls or 0) + 1
    end,
  }
end

local function attach_ui()
  vim.api.nvim_exec_autocmds("UIEnter", {})
end

local function level_logger_received_after_ui_attaches(logger_loaded)
  local record = {}
  local real = package.loaded["claudecode.logger"]
  dofile(config_root .. "/lua/plugins/claudecode.lua").init()
  package.loaded["claudecode.logger"] = logger_loaded and fake_logger(record) or nil
  attach_ui()
  package.loaded["claudecode.logger"] = real
  return record.level
end

local function level_after_early_and_late_attach()
  local record = {}
  local real = package.loaded["claudecode.logger"]
  dofile(config_root .. "/lua/plugins/claudecode.lua").init()
  package.loaded["claudecode.logger"] = nil
  attach_ui()
  package.loaded["claudecode.logger"] = fake_logger(record)
  attach_ui()
  attach_ui()
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
    assert(level_logger_received_after_ui_attaches(true) == "info")
  end,
  ["a UI attaching before the plugin loaded touches no logger"] = function()
    assert(level_logger_received_after_ui_attaches(false) == nil)
  end,
  ["a headless session logs at warn, which keeps stderr empty"] = function()
    assert(level_with_uis(no_ui) == "warn")
  end,
  ["an interactive session logs at info, which :ClaudeCodeStatus shows"] = function()
    assert(level_with_uis(one_ui) == "info")
  end,
  ["the websocket port cannot land where the moshi-hook probe reaches"] = function()
    local moshi_hook_highest_port_probed = 49151
    local highest_tcp_port = 65535
    local _, opts = level_with_uis(one_ui)
    local range = assert(opts.port_range, "the spec hands claudecode no port range")
    assert(range.min > moshi_hook_highest_port_probed, "the port floor is " .. range.min .. ", inside the probed range")
    assert(range.max <= highest_tcp_port, "the port ceiling is " .. range.max .. ", past the last port")
    assert(range.min < range.max, "the range is empty or inverted")
  end,
  ["the terminal provider stays none either way"] = function()
    local _, headless = level_with_uis(no_ui)
    local _, interactive = level_with_uis(one_ui)
    assert(headless.terminal.provider == "none" and interactive.terminal.provider == "none")
  end,
}
