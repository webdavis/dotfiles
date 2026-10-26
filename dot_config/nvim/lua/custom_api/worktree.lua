-- The name of the linked git worktree the current file belongs to, for the statusline.
--
-- git is asked in the background once per folder and the answer is kept, so drawing the
-- statusline never waits on git. The main checkout of a repo has no worktree name.
local M = {}

local names = {} ---@type table<string, string|false|true> folder -> name, false (none), true (asking)

local function folder()
  local path = vim.api.nvim_buf_get_name(0)
  local dir = path ~= "" and vim.fs.dirname(path) or vim.fn.getcwd()
  return vim.fn.isdirectory(dir) == 1 and dir or vim.fn.getcwd()
end

---@return string|nil name of the worktree, nil in a main checkout or outside git
function M.name()
  local dir = folder()
  local name = names[dir]
  if name == nil then
    names[dir] = true
    vim.system(
      { "git", "-C", dir, "rev-parse", "--path-format=absolute", "--git-dir", "--git-common-dir", "--show-toplevel" },
      { text = true },
      function(result)
        local lines = vim.split(result.stdout or "", "\n", { trimempty = true })
        -- A linked worktree keeps its own git dir inside the common one; the main checkout's are the same.
        local linked = result.code == 0 and #lines == 3 and lines[1] ~= lines[2]
        names[dir] = linked and vim.fs.basename(lines[3]) or false
        vim.schedule(function()
          if package.loaded.lualine then
            require("lualine").refresh({ place = { "statusline" } })
          end
        end)
      end
    )
    return nil
  end
  return type(name) == "string" and name or nil
end

return M
