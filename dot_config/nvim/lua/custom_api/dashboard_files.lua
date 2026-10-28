---@class custom_api.dashboard_files
---The dashboard file list: uncommitted files first, then recently opened ones.
local M = {}

local DIRECTORY_LIMIT = 18

local ROW_KEYS = { "1", "2", "3", "4", "5", "6", "7", "8" }

---Enter resumes the top-ranked file.
local RESUME_KEY = "<CR>"
local RESUME_LABEL = "⏎"

---Git exports these into hooks, and GIT_DIR overrides `-C`, so scrub them.
local GIT_ENVIRONMENT_TO_SCRUB = {
  "GIT_DIR",
  "GIT_WORK_TREE",
  "GIT_INDEX_FILE",
  "GIT_OBJECT_DIRECTORY",
  "GIT_COMMON_DIR",
}

---@param command string[] The argv words, no shell.
---@return string[] wrapped
local function without_git_environment(command)
  local wrapped = { "env" }
  for _, name in ipairs(GIT_ENVIRONMENT_TO_SCRUB) do
    table.insert(wrapped, "-u")
    table.insert(wrapped, name)
  end
  return vim.list_extend(wrapped, command)
end

---@param command string[] The argv words, no shell.
---@return string[] lines
local function lines_from(command)
  local result = vim.system(without_git_environment(command), { text = true }):wait()
  if result.code ~= 0 or not result.stdout then
    return {}
  end
  -- Split untrimmed, because a porcelain line can start with a space.
  return vim.split(result.stdout, "\n", { plain = true, trimempty = true })
end

---Changed files, with a rename listed by its new name.
---@return {path:string, status:string}[] changed
function M.changed_files()
  local root = vim.fs.root(vim.fn.getcwd(), ".git")
  if not root then
    return {}
  end

  local changed = {}
  for _, line in ipairs(lines_from({ "git", "-C", root, "status", "--porcelain" })) do
    local status, path = line:sub(1, 2), line:sub(4)
    local _, renamed_to = path:match("^(.+) %-> (.+)$")
    path = renamed_to or path
    -- Skip quoted paths, which carry escapes instead of the name.
    if not path:match('^"') then
      table.insert(changed, { path = vim.fs.joinpath(root, path), status = vim.trim(status) })
    end
  end
  return changed
end

---Recently opened files that still exist, most recent first.
---@return string[] paths
function M.recent_files()
  local recent = {}
  for _, path in ipairs(vim.v.oldfiles or {}) do
    if vim.fn.filereadable(path) == 1 then
      table.insert(recent, path)
    end
  end
  return recent
end

---Uncommitted files first, then recent ones, one row per path.
---@param limit number How many rows the dashboard has room for.
---@return {path:string, status:string|nil}[] entries
function M.entries(limit)
  local entries, seen = {}, {}

  -- Normalize so git and oldfiles spell a path the same way.
  local function add(path, status)
    path = vim.fs.normalize(path)
    if seen[path] or #entries >= limit then
      return
    end
    seen[path] = true
    table.insert(entries, { path = path, status = status })
  end

  for _, changed in ipairs(M.changed_files()) do
    add(changed.path, changed.status)
  end
  for _, path in ipairs(M.recent_files()) do
    add(path, nil)
  end

  return entries
end

---The filename plus a short parent directory, enough to tell same-named files apart.
---@param path string An absolute path.
---@return string name The filename.
---@return string directory The parent directory, trimmed, with a trailing slash.
function M.split_for_display(path)
  local name = vim.fs.basename(path)
  local directory = vim.fs.basename(vim.fs.dirname(path))
  -- Count characters so a multi-byte name is cut cleanly.
  if vim.fn.strchars(directory) > DIRECTORY_LIMIT then
    directory = vim.fn.strcharpart(directory, 0, DIRECTORY_LIMIT - 1) .. "…"
  end
  return name, directory .. "/"
end

---One dashboard row per ranked file.
---@param limit number How many rows the dashboard has room for.
---@param width number The dashboard's width in columns.
---@return snacks.dashboard.Item[] items
function M.section(limit, width)
  local items = {}
  for index, entry in ipairs(M.entries(math.min(limit, #ROW_KEYS + 1))) do
    local key = index == 1 and RESUME_KEY or ROW_KEYS[index - 1]
    local label = index == 1 and RESUME_LABEL or key
    local name, directory = M.split_for_display(entry.path)
    -- Right-align the directory and change marker against the dashboard edge.
    local marker = entry.status and (" " .. entry.status) or ""
    local spent = vim.fn.strdisplaywidth("  " .. label .. "  " .. name .. directory .. marker)
    table.insert(items, {
      key = key,
      action = ":e " .. vim.fn.fnameescape(entry.path),
      file = entry.path,
      text = {
        { "  " .. label .. "  ", hl = "key" },
        { name, hl = "file" },
        { string.rep(" ", math.max(1, width - spent)) },
        { directory, hl = "dir" },
        { marker, hl = "special" },
      },
    })
  end
  return items
end

return M
