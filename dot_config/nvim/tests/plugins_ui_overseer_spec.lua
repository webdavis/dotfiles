-- The overseer task counter in lualine's statusline (`lua/plugins/ui.lua`).
--
-- The component stays hidden until there is something to report, and the
-- counting is read out of a faked task list, so these cases need neither
-- overseer nor lualine running.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

---Run `fn(component)` with overseer faked, and the fake still in place while it
---runs: the component's callbacks `require` at call time, not at build time.
---@param tasks table[] What `list_tasks` should answer
---@param fn fun(component: table)
local function with_tasks(tasks, fn)
  local names = { "overseer.task_list", "overseer" }
  local saved = {}
  for _, name in ipairs(names) do
    saved[name] = { package.loaded[name] }
  end

  local list_opts
  package.loaded["overseer.task_list"] = {
    list_tasks = function(opts)
      list_opts = opts
      return tasks
    end,
  }
  _G.__list_opts = function()
    return list_opts
  end
  -- The counter reads `package.loaded` rather than requiring overseer, so this
  -- is what decides whether it considers itself live at all.
  package.loaded["overseer"] = {}

  local ok, err = pcall(function()
    local spec = dofile(config_root .. "/lua/plugins/ui.lua")
    for _, plugin in ipairs(spec) do
      if plugin[1] == "nvim-lualine/lualine.nvim" then
        fn(plugin.opts.sections.lualine_x[1])
        return
      end
    end
    error("the lualine spec is gone from plugins/ui.lua")
  end)

  for _, name in ipairs(names) do
    package.loaded[name] = saved[name][1]
  end
  assert(ok, err)
end

---@param n integer
---@param status string
---@return table[]
local function tasks(n, status)
  local list = {}
  for _ = 1, n do
    table.insert(list, { status = status })
  end
  return list
end

return {
  ["it counts every task, not one per name"] = function()
    -- `list_tasks({ unique = true })` returns the OLDER completed task sharing a
    -- name, so running one shell command twice, succeeding then failing, showed
    -- only the success. Both tasks are visible in the list; both must be counted.
    with_tasks({ { status = "SUCCESS" }, { status = "FAILURE" } }, function(component)
      local text = component[1]()
      assert(text:match("\u{f0134}"), "the success is missing from " .. text)
      assert(text:match("\u{f015a}"), "the failure is missing from " .. text)
      local opts = _G.__list_opts()
      assert(not (opts and opts.unique), "list_tasks was asked for unique tasks, which hides the newer one")
    end)
  end,

  ["the counter is the first right-hand component"] = function()
    with_tasks({}, function(component)
      assert(type(component[1]) == "function" and type(component.cond) == "function", "not a function component")
    end)
  end,

  ["it hides when there are no tasks"] = function()
    with_tasks({}, function(component)
      assert(not component.cond(), "the counter is visible with an empty task list")
    end)
  end,

  ["it shows a count per status"] = function()
    with_tasks(vim.list_extend(tasks(2, "RUNNING"), tasks(1, "FAILURE")), function(component)
      assert(component.cond(), "the counter is hidden with three tasks")
      local text = component[1]()
      assert(text:match("2"), "the running count is missing from " .. text)
      assert(text:match("1"), "the failure count is missing from " .. text)
    end)
  end,

  ["it orders running before finished"] = function()
    with_tasks(vim.list_extend(tasks(1, "SUCCESS"), tasks(1, "RUNNING")), function(component)
      local text = component[1]()
      local running, success = text:find("\u{f046e}"), text:find("\u{f0134}")
      assert(running and success, "both glyphs should be present in " .. text)
      assert(running < success, "running should be reported before success: " .. text)
    end)
  end,

  ["each status keeps a real glyph, not a stripped empty string"] = function()
    -- These are Nerd Font private-use codepoints, which have been silently
    -- dropped in transit before now.
    with_tasks(tasks(1, "CANCELED"), function(component)
      local text = component[1]()
      assert(text:match("\u{f46e}"), "the canceled glyph is missing from " .. vim.inspect(text))
    end)
  end,

  ["a status with no tasks contributes nothing"] = function()
    with_tasks(tasks(1, "SUCCESS"), function(component)
      local text = component[1]()
      assert(not text:match("\u{f015a}"), "a failure glyph appeared with no failures: " .. text)
    end)
  end,
}
