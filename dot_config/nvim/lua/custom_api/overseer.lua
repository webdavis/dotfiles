---@class custom_api.overseer
---Overseer task orchestration.
local M = {}

---Turn a list of shell commands into ONE runnable task.
---
---Two shapes, because the callers have two needs.
---
---A single command stays a shell string. Every one-command caller here passes
---git format specifiers wrapped in single quotes
---(`--pretty=format:'%C(yellow)%h%C(reset)'`), which only mean anything to a
---shell; handing those to argv would run them literally.
---
---Two or more commands go to overseer's orchestrator strategy rather than being
---joined with `;`. `;` runs the next command whether or not the last one
---worked, so `git init ; gh repo create` created the GitHub repository even when
---`git init` had failed. The orchestrator runs them in order, STOPS at the first
---failure, and gives each step its own status and output.
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
