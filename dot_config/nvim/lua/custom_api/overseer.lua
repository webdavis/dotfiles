---@class custom_api.overseer
local M = {}

---Runs one shell command, or a list of them, as a single task.
---One command runs as a shell string, so quoted git format specifiers work.
---Several run in order through the orchestrator, which stops at the first failure.
---@param opts { cmds: string|string[] }
function M.overseer_runner(opts)
  opts = opts or {}
  local cmds = opts.cmds

  if type(cmds) == "string" then
    cmds = { cmds }
  elseif type(cmds) ~= "table" then
    error("'commands' parameter must be a string or a table")
  end

  if vim.tbl_isempty(cmds) then
    error("'commands' parameter must not be empty")
  end

  for i, c in ipairs(cmds) do
    if type(c) ~= "string" then
      error("Invalid command type at index " .. i .. ": " .. type(c))
    end
  end

  local components = { { "on_complete_notify", statuses = { "SUCCESS" } }, "default" }

  if #cmds == 1 then
    require("overseer")
      .new_task({
        name = cmds[1],
        cmd = cmds[1],
        components = components,
      })
      :start()
    return
  end

  local steps = {}
  for _, c in ipairs(cmds) do
    table.insert(steps, { cmd = c, name = c })
  end

  require("overseer")
    .new_task({
      name = table.concat(cmds, " then "),
      strategy = { "orchestrator", tasks = steps },
      components = components,
    })
    :start()
end

return M
