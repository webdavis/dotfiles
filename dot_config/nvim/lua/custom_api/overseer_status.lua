---@class custom_api.overseer_status
---The overseer task counter rendered by the witch-line statusline component.
---
---This lives in a module rather than in closures beside the component because
---witch-line's cache serializes a component's callbacks as bytecode WITHOUT
---their upvalues. A callback that closed over a local helper restored as
---`attempt to call upvalue 'counts' (a nil value)`, so the counter worked on a
---cold start and broke on every start after one. `require` is a global lookup,
---so callbacks that reach this module by name survive the roundtrip.
local M = {}

-- Running first, so a live task is the first thing read.
local ORDER = { "RUNNING", "FAILURE", "SUCCESS", "CANCELED" }
local ICONS = {
  RUNNING = "󰑮",
  FAILURE = "󰅚",
  SUCCESS = "󰄴",
  CANCELED = "",
}

---Task counts by status.
---@return table<string, integer>
function M.counts()
  -- `package.loaded` rather than require: overseer lazy-loads itself, and asking
  -- for it here would drag it in on every statusline redraw.
  if not package.loaded["overseer"] then
    return {}
  end
  local by_status = {}
  -- `unique` collapses repeat runs of one task, and wrapped background jobs stay
  -- out, so this counts what the task list shows.
  for _, task in ipairs(require("overseer.task_list").list_tasks({ unique = true })) do
    by_status[task.status] = (by_status[task.status] or 0) + 1
  end
  return by_status
end

---True when there is nothing to report, so the component hides itself.
---@return boolean
function M.is_idle()
  return vim.tbl_isempty(M.counts())
end

---One icon and count per status that has tasks.
---@return string
function M.render()
  local by_status = M.counts()
  local parts = {}
  for _, status in ipairs(ORDER) do
    if by_status[status] then
      table.insert(parts, ("%s %d"):format(ICONS[status], by_status[status]))
    end
  end
  return table.concat(parts, " ")
end

return M
