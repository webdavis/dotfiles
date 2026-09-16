---@class custom_api.dashboard_files
---The dashboard's file list, ranked by what the operator was most likely doing.
---
---THE DASHBOARD IS A DOORWAY, NOT A ROOM (operator 2026-09-15). Neovim is opened
---to work on a file, so the file list gets the prime rows and the verbs sit under
---it. That is why this module exists at all: the built-in `recent_files` section
---ranks by when a file was last OPENED, which counts every file ever glanced at
---equally with the three that carry unfinished work.
---
---The ranking is uncommitted files first, then recently opened ones. A file with
---unsaved-to-git changes is the strongest available signal for "this is why the
---editor is open", and it costs one `git status` rather than a heuristic.
local M = {}

---How many characters of a parent directory to show before it is cut short.
local DIRECTORY_LIMIT = 18

---The keys the rows below the first answer to. A row reached by its position
---needs no mnemonic, so they are just digits.
local ROW_KEYS = { "1", "2", "3", "4", "5", "6", "7", "8" }

---The first row is the resume row and takes Enter, because continuing what you
---were doing is the single most likely reason the editor is open. It is not a
---separate entry from the list: the top-ranked file IS what you resume, and a
---dedicated row repeating it would spend the best line in the dashboard saying
---the same thing twice.
local RESUME_KEY = "<CR>"
local RESUME_LABEL = "⏎"

---Run a command and return its stdout lines, or an empty list when it fails.
---
---A dashboard that errors is worse than a dashboard missing a section, so a
---non-zero exit, an absent binary and a repository-less directory all answer the
---same way: nothing to show.
---@param command string[] The argv words, no shell.
---@return string[] lines
---Git's own environment variables, scrubbed before every call.
---
---GIT_DIR OVERRIDES `-C`, and git exports GIT_DIR, GIT_WORK_TREE and
---GIT_INDEX_FILE into every hook it runs. A Neovim started from a hook would
---therefore list the HOOK's repository rather than the one the editor is in.
---The same leak is what made this module's own spec reconfigure the real
---repository while it ran (measured 2026-09-15): `git init` in a temporary
---directory wrote `core.bare = true` and a temporary `core.worktree` into the
---inherited GIT_DIR instead.
local GIT_ENVIRONMENT_TO_SCRUB = {
  "GIT_DIR",
  "GIT_WORK_TREE",
  "GIT_INDEX_FILE",
  "GIT_OBJECT_DIRECTORY",
  "GIT_COMMON_DIR",
}

---`command` wrapped in `env -u ...`, so no inherited git variable reaches it.
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

local function lines_from(command)
  local result = vim.system(without_git_environment(command), { text = true }):wait()
  if result.code ~= 0 or not result.stdout then
    return {}
  end
  -- NOT `vim.trim(result.stdout)` first. Trimming the whole output strips the
  -- leading space of the FIRST line, and `git status --porcelain` writes an
  -- unstaged change as " M path", so the first path came back a character short
  -- (`ot_config/...`). `trimempty` already drops the trailing newline's empty
  -- element, which is the only thing the trim was buying.
  return vim.split(result.stdout, "\n", { plain = true, trimempty = true })
end

---The working tree's changed files, newest-known-state first.
---
---`--porcelain` is the stable format, so the two status characters are columns 1
---and 2 and the path starts at column 4. A rename reads `R  old -> new`, and the
---new name is the one worth opening.
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
    -- A quoted path carries escapes rather than the name itself, and unquoting
    -- it correctly is more work than the row is worth, so it is skipped.
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

---The ranked file list: uncommitted work first, then recently opened files.
---
---Deduplicated by path, because a file that is both changed and recent must not
---take two rows, and the changed row is the one that carries the status marker.
---@param limit number How many rows the dashboard has room for.
---@return {path:string, status:string|nil}[] entries
function M.entries(limit)
  local entries, seen = {}, {}

  -- Normalized before comparing, because the dedupe is by path and the two
  -- sources spell a path differently: `git` reports it relative to the
  -- repository root, `vim.v.oldfiles` holds whatever was opened. KNOWN CEILING:
  -- normalizing does not resolve symlinks, so one file reached through a link
  -- and again through its target still takes two rows.
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

---A path split into the parts a row shows: the filename, and just enough of its
---parent to tell two files of the same name apart.
---
---The full path is deliberately NOT shown. Neovim's own abbreviation renders
---`~/workspaces/Ivy/webdavis/dotfiles/docs/acceptance/nvim-acceptance-drills.md`
---as `~/w/I/w/d/d/a/nvim-acceptance-drills.md`, which is unreadable, and the
---operator named that as the dashboard's worst legibility problem.
---@param path string An absolute path.
---@return string name The filename.
---@return string directory The parent directory, trimmed, with a trailing slash.
function M.split_for_display(path)
  local name = vim.fs.basename(path)
  local directory = vim.fs.basename(vim.fs.dirname(path))
  -- CHARACTERS, not bytes. `#directory` counts bytes, so a name with any
  -- multi-byte character was cut mid-character, and the ellipsis (three bytes
  -- itself) pushed the result past the limit it was meant to enforce.
  if vim.fn.strchars(directory) > DIRECTORY_LIMIT then
    directory = vim.fn.strcharpart(directory, 0, DIRECTORY_LIMIT - 1) .. "…"
  end
  return name, directory .. "/"
end

---One dashboard row per ranked file, keyed `1` through `9`.
---@param limit number How many rows the dashboard has room for.
---@param width number The dashboard's width in columns.
---@return snacks.dashboard.Item[] items
function M.section(limit, width)
  local items = {}
  for index, entry in ipairs(M.entries(math.min(limit, #ROW_KEYS + 1))) do
    local key = index == 1 and RESUME_KEY or ROW_KEYS[index - 1]
    local label = index == 1 and RESUME_LABEL or key
    local name, directory = M.split_for_display(entry.path)
    -- The row is three columns: the key, the filename, and the directory with
    -- the change marker right-aligned against the dashboard's edge.
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
