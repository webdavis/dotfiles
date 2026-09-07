local Report = require("uu.report")
local Config = require("lazy.core.config")
local Plugin = require("lazy.core.plugin")
local Manager = require("lazy.manage")

local function run()
  Manager.check({ wait = true, show = false })
  local plugins = {}
  for name, plugin in pairs(Config.plugins) do
    plugins[#plugins + 1] = { name = name, updates = plugin._.updates, errors = Plugin.has_errors(plugin) }
  end
  return Report.plugin_lines(plugins)
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
