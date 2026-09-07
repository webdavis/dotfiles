local report = require("uu.report")
local config = vim.fn.stdpath("config")
local root = vim.fn.fnamemodify(config, ":h:h")

local function read(path)
  local file = assert(io.open(path, "rb"))
  local bytes = file:read("*a")
  assert(file:close())
  return bytes
end

local function write(path, bytes)
  local file = assert(io.open(path, "wb"))
  assert(file:write(bytes))
  assert(file:close())
end

local function finish(status)
  io.flush()
  vim.cmd("cquit " .. status)
end

if arg and arg[1] == "prepare" then
  local ok, why = pcall(function()
    require("lazy.manage").update({ wait = true, show = false })
    local plugins = {}
    for name, plugin in pairs(require("lazy.core.config").plugins) do
      plugins[#plugins + 1] = { name = name, errors = require("lazy.core.plugin").has_errors(plugin) }
    end
    local lines, status = report.plugin_lines(plugins)
    for _, line in ipairs(lines) do
      io.write(line, "\n")
    end
    if status ~= report.OK then
      error("candidate plugin preparation failed")
    end
  end)
  if not ok then
    io.write(tostring(why), "\n")
  end
  finish(ok and report.OK or report.FAILED)
else
  local diagnostics = require("uu.smoke_diagnostics")
  local notifications, earlier_error = {}, vim.v.errmsg
  local notify = vim.notify
  vim.notify = function(message, level, options)
    if level == vim.log.levels.ERROR then
      notifications[#notifications + 1] = tostring(message)
    end
    return notify(message, level, options)
  end
  local function verify()
    local request = vim.json.decode(read(root .. "/run.json"))
    local result = { run = request.run, lock = read(config .. "/lazy-lock.json"), vim_enter = true }
    vim.api.nvim_exec_autocmds("User", { pattern = "VeryLazy", modeline = false })
    local started, turns = vim.uv.hrtime(), 0
    local drained = vim.wait(500, function()
      turns = turns + 1
      if turns < 2 or (vim.uv.hrtime() - started) / 1e6 < 100 then
        return false
      end
      for _, plugin in pairs((package.loaded["lazy.core.config"] or {}).plugins or {}) do
        for _, task in ipairs(plugin._ and plugin._.tasks or {}) do
          if task:running() then
            return false
          end
        end
      end
      return true
    end, 10)
    local snapshot = diagnostics.capture(true, drained, notifications, earlier_error)
    result.errors = diagnostics.errors(snapshot)
    write(root .. "/startup.json", vim.json.encode(snapshot) .. "\n")
    vim.cmd("silent checkhealth")
    local health = vim.api.nvim_buf_get_lines(0, 0, -1, false)
    write(root .. "/checkhealth.txt", table.concat(health, "\n") .. "\n")
    result.health_errors, result.health_warnings = report.health_counts(health)
    write(root .. "/completion.json", vim.json.encode(result) .. "\n")
    io.write("candidate lock sha256: ", vim.fn.sha256(result.lock), "; run: ", request.run, "\n")
    for _, why in ipairs(result.errors) do
      io.write("startup error: ", why, "; ", root .. "/startup.json\n")
    end
    finish(#result.errors == 0 and report.OK or report.FAILED)
  end
  vim.api.nvim_create_autocmd("VimEnter", {
    once = true,
    callback = function()
      vim.schedule(function()
        local ok, why = pcall(verify)
        if not ok then
          io.write("smoke verification failed: ", tostring(why), "\n")
          finish(report.FAILED)
        end
      end)
    end,
  })
end
