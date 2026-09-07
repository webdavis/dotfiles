local M = { OK = 0, FAILED = 1, PENDING = 100 }

function M.plugin_lines(plugins)
  local lines = {}
  local current, pending, failed = 0, 0, 0
  for _, plugin in ipairs(plugins) do
    if plugin.errors then
      failed = failed + 1
      lines[#lines + 1] = plugin.name .. ": plugin check failed"
    elseif plugin.updates then
      pending = pending + 1
      lines[#lines + 1] = plugin.name .. ": updates available"
    else
      current = current + 1
    end
  end
  table.sort(lines)
  lines[#lines + 1] = ("plugins: %d current, %d pending, %d failed"):format(current, pending, failed)
  return lines, failed > 0 and M.FAILED or pending > 0 and M.PENDING or M.OK
end

function M.other_instances(sockets, own_pid)
  local seen, count = {}, 0
  for _, socket in ipairs(sockets) do
    local pid = tonumber(socket:match("nvim%.(%d+)%.0$"))
    if pid and pid ~= own_pid and not seen[pid] then
      count = count + 1
      seen[pid] = true
    end
  end
  return count
end

function M.running_sockets()
  local parent = vim.fn.fnamemodify(vim.fn.stdpath("run"), ":h")
  return vim.fn.glob(parent .. "/*/nvim.*.0", false, true)
end

function M.restart_notice(count)
  if count > 0 then
    return ("%d Neovim instance(s) were running during this update; restart them to load the new versions"):format(
      count
    )
  end
end

function M.health_counts(lines)
  local errors, warnings = 0, 0
  for _, line in ipairs(lines) do
    local severity = line:match("^%s*%-?%s*(%u+)%f[%W]")
    if severity == "ERROR" then
      errors = errors + 1
    elseif severity == "WARNING" then
      warnings = warnings + 1
    end
  end
  return errors, warnings
end

local function cell(value)
  return ((value or ""):gsub("[\t\r\n]", " "))
end

function M.keymap_rows(maps_by_mode)
  local rows = {}
  for mode, maps in pairs(maps_by_mode) do
    for _, map in ipairs(maps) do
      rows[#rows + 1] = table.concat({ cell(mode), cell(map.lhs), cell(map.rhs), cell(map.desc) }, "\t")
    end
  end
  table.sort(rows)
  return rows
end

local function indexed(rows)
  local result = {}
  for _, row in ipairs(rows) do
    local key = row:match("^([^\t]*\t[^\t]*)\t")
    if key then
      result[key] = row
    end
  end
  return result
end

function M.keymap_diff(before, after)
  local old, new = indexed(before), indexed(after)
  local added, removed = {}, {}
  for key, row in pairs(new) do
    if not old[key] then
      added[#added + 1] = row
    end
  end
  for key, row in pairs(old) do
    if not new[key] then
      removed[#removed + 1] = row
    end
  end
  table.sort(added)
  table.sort(removed)
  return added, removed
end

function M.commit_allowed(porcelain, symbolic_ref)
  if porcelain:find("%S") then
    return false, "source lock is dirty"
  end
  if not symbolic_ref:match("^refs/heads/[^%s]+%s*$") then
    return false, "HEAD is detached"
  end
  return true
end

function M.mason_lines(events)
  local lines, failed = {}, false
  for _, package in ipairs(events.packages) do
    if package.event == "install:failed" then
      failed = true
      lines[#lines + 1] = package.name .. ": " .. package.reason
    elseif package.event == "install:success" then
      lines[#lines + 1] = package.name .. ": updated"
    elseif package.event == "current" then
      lines[#lines + 1] = package.name .. ": current"
    end
  end
  table.sort(lines)
  lines[#lines + 1] = "Language servers are managed by Mason; update them through :Mason."
  return lines, failed and M.FAILED or M.OK
end

function M.parser_lines(summary)
  local lines = {}
  for _, error in ipairs(summary.errors) do
    lines[#lines + 1] = error
  end
  for _, name in ipairs(summary.updated) do
    lines[#lines + 1] = name .. ": parser updated"
  end
  for _, name in ipairs(summary.current) do
    lines[#lines + 1] = name .. ": parser current"
  end
  if not summary.ok and #summary.errors == 0 then
    lines[#lines + 1] = "parser update failed"
  end
  return lines, summary.ok and #summary.errors == 0 and M.OK or M.FAILED
end

return M
