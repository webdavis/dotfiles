---@class custom_api.overseer_status
---The overseer task counter rendered by the lualine statusline component.
local M = {}

-- Running first, so a live task is the first thing read.
local ORDER = { "RUNNING", "FAILURE", "SUCCESS", "CANCELED" }
local ICONS = {
  RUNNING = "󰑮",
  FAILURE = "󰅚",
  SUCCESS = "󰄴",
  CANCELED = "",
}

---@return table<string, integer>
function M.counts()
  -- `package.loaded`, so a statusline redraw never loads overseer.
  if not package.loaded["overseer"] then
    return {}
  end
  local by_status = {}
  -- `list_tasks`, not `unique`, so two runs of one command both count.
  for _, task in ipairs(require("overseer.task_list").list_tasks({})) do
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
