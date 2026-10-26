-- `]f` and `[f`: open the next or previous file in the current file's folder.
--
-- Files sort the way people count (`test2` before `test10`), images, archives,
-- lock files and whatever git ignores are skipped, and a jump past either end
-- stops at the first or last file. Each jump shows `4/12 name` in the message
-- area.
local M = {}

local SKIPPED_EXTENSIONS = {}
-- stylua: ignore
for _, extension in ipairs({
  "png", "jpg", "jpeg", "gif", "webp", "ico", "icns", "pdf", "zip", "gz", "tar", "tgz", "xz", "dmg",
  "mp3", "mp4", "mov", "woff", "woff2", "ttf", "otf", "lock", "swp", "swo",
}) do
  SKIPPED_EXTENSIONS[extension] = true
end

local SKIPPED_NAMES = {
  [".DS_Store"] = true,
  ["lazy-lock.json"] = true,
  ["package-lock.json"] = true,
  ["pnpm-lock.yaml"] = true,
}

local function skipped(name)
  return SKIPPED_NAMES[name] or SKIPPED_EXTENSIONS[(name:match("%.([^.]+)$") or ""):lower()]
end

-- Digit runs padded to one width, so they compare as numbers.
local function sort_key(name)
  return (name:lower():gsub("%d+", function(digits)
    return ("0"):rep(math.max(0, 20 - #digits)) .. digits
  end))
end

local function by_sort_key(a, b)
  local key_a, key_b = sort_key(a), sort_key(b)
  if key_a ~= key_b then
    return key_a < key_b
  end
  return a < b
end

local function ignored_by_git(dir, names)
  local result = vim
    .system({ "git", "-C", dir, "check-ignore", "--stdin" }, { stdin = table.concat(names, "\n") })
    :wait()
  local ignored = {}
  -- Exit 0 lists ignored names, 1 means none, anything else (not a repo) leaves the list as it is.
  if result.code == 0 then
    for line in vim.gsplit(result.stdout, "\n", { trimempty = true }) do
      ignored[line] = true
    end
  end
  return ignored
end

local function folder_files(dir, current)
  local names = {}
  for name, kind in vim.fs.dir(dir) do
    local is_file = kind == "file" or (kind == "link" and vim.fn.isdirectory(vim.fs.joinpath(dir, name)) == 0)
    if is_file and (name == current or not skipped(name)) then
      names[#names + 1] = name
    end
  end
  local ignored = ignored_by_git(dir, names)
  names = vim.tbl_filter(function(name)
    return name == current or not ignored[name]
  end, names)
  table.sort(names, by_sort_key)
  return names
end

---@param offset integer how many files to move: positive for later, negative for earlier
function M.jump(offset)
  local path = vim.api.nvim_buf_get_name(0)
  local dir = path ~= "" and vim.fs.dirname(path) or vim.fn.getcwd()
  local current = path ~= "" and vim.fs.basename(path) or ""
  local names = folder_files(dir, current)
  if #names == 0 then
    vim.api.nvim_echo({ { "No files in " .. vim.fn.fnamemodify(dir, ":~") } }, false, {})
    return
  end

  -- A buffer that isn't in the list (unnamed, or a folder) sits between the
  -- names that sort around it.
  local index = vim.fn.index(names, current) + 1
  if index == 0 then
    local before = #vim.tbl_filter(function(name)
      return by_sort_key(name, current)
    end, names)
    index = offset > 0 and before or before + 1
  end

  local target = math.min(math.max(index + offset, 1), #names)
  if names[target] == current then
    local edge = offset > 0 and "last" or "first"
    vim.api.nvim_echo({ { ("%d/%d %s (%s file)"):format(target, #names, current, edge) } }, false, {})
    return
  end
  vim.cmd.edit(vim.fn.fnameescape(vim.fs.joinpath(dir, names[target])))
  vim.api.nvim_echo({ { ("%d/%d %s"):format(target, #names, names[target]) } }, false, {})
end

return M
