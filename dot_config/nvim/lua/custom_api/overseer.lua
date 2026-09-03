---@class custom_api.overseer
---Overseer task orchestration.
local M = {}

function M.overseer_runner(opts)
  opts = opts or {}
  local cmds = opts.cmds
  local operator = opts.operator or ";"

  if type(cmds) == "string" then
    cmds = { cmds }
  elseif type(cmds) ~= "table" then
    error("'commands' parameter must be a string or a table")
  end

  for i, c in ipairs(cmds) do
    if type(c) ~= "string" then
      error("Invalid command type at index " .. i .. ": " .. type(c))
    end
  end

  local cmd_str = table.concat(cmds, " " .. operator .. " ")

  -- Create the orchestrator task:
  require("overseer")
    .new_task({
      name = "**Command Orchestrator:** `" .. cmd_str .. "`",
      cmd = cmd_str,
      components = { { "on_complete_notify", statuses = { "SUCCESS" } }, "default" },
    })
    :start()
end

return M
