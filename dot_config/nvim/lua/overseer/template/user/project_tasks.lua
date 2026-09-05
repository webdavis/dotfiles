-- Project-local tasks, read from `.overseer/tasks.json` or `overseer.json`.
--
-- Overseer has no project-local task file of its own at the pinned commit. Its
-- documented answer is `exrc` plus `register_template`, which runs a project's
-- Lua at startup for every directory opened. This reads JSON instead: a project
-- file is DATA, so checking one out from a repository cannot run anything. The
-- `.vscode/tasks.json` support the plugin already ships is the other half, and
-- both can be present at once.
--
-- Format, either file:
--
--   {
--     "tasks": [
--       {
--         "name": "deploy staging",        -- required
--         "cmd": ["./deploy.sh", "stage"], -- required, list (argv) or string (shell)
--         "desc": "Push to the staging environment",
--         "cwd": "server",                 -- relative to the file, or absolute
--         "env": { "TARGET": "stage" },
--         "components": ["default"],
--         "tags": ["BUILD"]                -- BUILD, RUN, TEST or CLEAN
--       }
--     ]
--   }

local FILENAMES = { ".overseer/tasks.json", "overseer.json" }
local VALID_TAGS = { BUILD = true, RUN = true, TEST = true, CLEAN = true }

---Find the nearest project task file at or above `dir`.
---@param dir string
---@return string|nil path
local function find_task_file(dir)
  local dirs = { dir }
  for parent in vim.fs.parents(dir) do
    table.insert(dirs, parent)
  end
  for _, candidate_dir in ipairs(dirs) do
    for _, name in ipairs(FILENAMES) do
      local candidate = vim.fs.joinpath(candidate_dir, name)
      if vim.fn.filereadable(candidate) == 1 then
        return candidate
      end
    end
  end
end

---Resolve an entry's `cwd` against the directory that declared it.
---@param root string
---@param cwd string
---@return string
local function resolve_cwd(root, cwd)
  local expanded = vim.fs.normalize(cwd)
  if vim.startswith(expanded, "/") then
    return expanded
  end
  return vim.fs.normalize(vim.fs.joinpath(root, cwd))
end

---A task entry is a trust boundary: it comes out of a file in someone's
---repository. Anything malformed is skipped with a reason rather than raising,
---so one bad entry cannot take the whole task picker down with it.
---@param entry any
---@param index integer
---@return table|nil task
---@return string|nil problem
local function validate(entry, index)
  local where = ("entry %d"):format(index)
  if type(entry) ~= "table" then
    return nil, where .. " is not an object"
  end
  if type(entry.name) ~= "string" or entry.name == "" then
    return nil, where .. " has no name"
  end
  where = ("%q"):format(entry.name)
  local cmd = entry.cmd
  if type(cmd) == "table" then
    if vim.tbl_isempty(cmd) then
      return nil, where .. " has an empty cmd"
    end
    for _, word in ipairs(cmd) do
      if type(word) ~= "string" then
        return nil, where .. " has a non-string word in cmd"
      end
    end
  elseif type(cmd) ~= "string" or cmd == "" then
    return nil, where .. " has no cmd"
  end
  for _, field in ipairs({ "desc", "cwd" }) do
    if entry[field] ~= nil and type(entry[field]) ~= "string" then
      return nil, ("%s has a non-string %s"):format(where, field)
    end
  end
  if entry.env ~= nil and type(entry.env) ~= "table" then
    return nil, where .. " has a non-object env"
  end
  if entry.components ~= nil and type(entry.components) ~= "table" then
    return nil, where .. " has a non-list components"
  end
  for _, tag in ipairs(entry.tags or {}) do
    if not VALID_TAGS[tag] then
      return nil, ("%s has unknown tag %q"):format(where, tostring(tag))
    end
  end
  return entry
end

---@type overseer.TemplateFileProvider
return {
  cache_key = function(opts)
    return find_task_file(opts.dir)
  end,
  generator = function(opts)
    local path = find_task_file(opts.dir)
    if not path then
      return "No .overseer/tasks.json or overseer.json found"
    end
    local root = vim.fs.dirname(path)
    if vim.fs.basename(root) == ".overseer" then
      root = vim.fs.dirname(root)
    end

    local ok, decoded = pcall(vim.json.decode, table.concat(vim.fn.readfile(path), "\n"))
    if not ok or type(decoded) ~= "table" or type(decoded.tasks) ~= "table" then
      return path .. " is not an object with a tasks list"
    end

    local templates = {}
    local problems = {}
    for index, entry in ipairs(decoded.tasks) do
      local task, problem = validate(entry, index)
      if not task then
        table.insert(problems, problem)
      else
        table.insert(templates, {
          name = task.name,
          desc = task.desc,
          tags = task.tags,
          builder = function()
            return {
              cmd = task.cmd,
              -- A RELATIVE cwd is relative to the file that declared it, so the
              -- same entry means the same directory wherever it is run from. An
              -- absolute one is left alone: joining it under the project turned
              -- "/tmp" into "<project>/tmp", which either fails to start or runs
              -- somewhere nobody asked for. `normalize` expands `~` first, so a
              -- home-relative path counts as absolute here.
              cwd = task.cwd and resolve_cwd(root, task.cwd) or root,
              env = task.env,
              components = task.components or { "default" },
            }
          end,
        })
      end
    end

    if not vim.tbl_isempty(problems) then
      vim.notify(
        ("%s: skipped %d task(s)\n%s"):format(path, #problems, table.concat(problems, "\n")),
        vim.log.levels.WARN,
        { title = "Overseer" }
      )
    end
    return templates
  end,
}
