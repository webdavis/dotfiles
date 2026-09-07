local Report = require("uu.report")
local Config = require("lazy.core.config")
local Plugin = require("lazy.core.plugin")
local Manager = require("lazy.manage")

local function plugin_report()
  local plugins = {}
  for name, plugin in pairs(Config.plugins) do
    plugins[#plugins + 1] = { name = name, updates = plugin._.updates, errors = Plugin.has_errors(plugin) }
  end
  return Report.plugin_lines(plugins)
end

local function installed()
  local plugins = {}
  for name, plugin in pairs(Config.plugins) do
    if plugin._.installed and not plugin._.is_local then
      plugins[name] = plugin.dir
    end
  end
  return plugins
end

local function run()
  local args = _G.arg or {}
  local config = vim.fn.fnamemodify(args[0], ":p:h:h:h")
  local options = {
    config = config,
    auto_commit = false,
    recovery = vim.fn.stdpath("state") .. "/uu/plugins-" .. vim.fn.sha256(config) .. ".json",
    plugins = installed,
    update = function()
      Manager.update({ wait = true, show = false })
      return plugin_report()
    end,
  }
  local index = 1
  while args[index] do
    if args[index] == "--auto-commit" then
      options.auto_commit = true
    elseif args[index] == "--repo" then
      index = index + 1
      options.repo = assert(args[index], "--repo needs an absolute repository path")
      assert(options.repo:sub(1, 1) == "/", "--repo needs an absolute repository path")
    else
      error("unknown plugins argument: " .. args[index])
    end
    index = index + 1
  end
  assert(not options.auto_commit or options.repo, "--auto-commit requires --repo")
  local result = require("uu.writeback").run(options)
  if result.kind == "check" then
    Manager.check({ wait = true, show = false })
    local lines, status = plugin_report()
    vim.list_extend(result.lines, lines)
    result.status = status
  end
  return result.lines, result.status
end

local ok, lines, status = pcall(run)
if not ok then
  io.write("plugin lane failed: ", tostring(lines), "\n")
  io.flush()
  vim.cmd("cquit " .. Report.FAILED)
end
for _, line in ipairs(lines) do
  io.write(line, "\n")
end
io.flush()
vim.cmd("cquit " .. status)
