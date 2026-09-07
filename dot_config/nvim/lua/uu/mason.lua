local Report = require("uu.report")

local function run()
  local registry = require("mason-registry")
  local refresh_error
  registry:once("update:failed", function(reason)
    refresh_error = vim.inspect(reason)
  end)
  vim.cmd("MasonUpdate")
  assert(not refresh_error, "Mason registry update failed: " .. tostring(refresh_error))

  local plugin = require("lazy.core.config").plugins["mason-tool-installer.nvim"]
  local options = require("lazy.core.plugin").values(plugin, "opts", false)
  local events = {}
  for _, item in ipairs(options.ensure_installed or {}) do
    local name = type(item) == "table" and item[1] or item
    if type(item) ~= "table" or item.condition == nil then
      events[name] = { name = name, event = "current" }
    end
  end
  for _, package in ipairs(registry.get_all_packages()) do
    package:once("install:success", function()
      if not events[package.name] or events[package.name].event ~= "install:failed" then
        events[package.name] = { name = package.name, event = "install:success" }
      end
    end)
    package:once("install:failed", function(reason)
      events[package.name] = {
        name = package.name,
        event = "install:failed",
        reason = (type(reason) == "string" and reason or vim.inspect(reason)):sub(-2048),
      }
    end)
  end
  local completed = false
  vim.api.nvim_create_autocmd("User", {
    pattern = "MasonToolsUpdateCompleted",
    once = true,
    callback = function()
      completed = true
    end,
  })
  local update_ok, update_error = true
  if #(options.ensure_installed or {}) > 0 then
    update_ok, update_error = pcall(vim.cmd, "MasonToolsUpdateSync")
    if update_ok then
      assert(completed, "Mason tools update returned without completion")
    end
  end
  local packages, changed = {}, false
  for _, package in pairs(events) do
    packages[#packages + 1] = package
    changed = changed or package.event == "install:success"
  end
  local lines, status = Report.mason_lines({ packages = packages })
  if not update_ok and status == Report.OK then
    table.insert(lines, 1, "Mason update failed: " .. tostring(update_error))
    status = Report.FAILED
  end
  if changed then
    local notice = Report.restart_notice(Report.other_instances(Report.running_sockets(), vim.uv.os_getpid()))
    if notice then
      lines[#lines + 1] = notice
    end
  end
  return lines, status
end

local ok, lines, status = pcall(run)
if not ok then
  local failure = "Mason update failed: " .. tostring(lines)
  lines = Report.mason_lines({ packages = {} })
  table.insert(lines, 1, failure)
  status = Report.FAILED
end
for _, line in ipairs(lines) do
  io.write(line, "\n")
end
io.flush()
vim.cmd("cquit " .. status)
