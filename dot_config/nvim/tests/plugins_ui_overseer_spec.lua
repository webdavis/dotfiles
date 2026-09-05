-- The overseer task counter appended to witch-line's statusline in
-- `lua/plugins/ui.lua`.
--
-- Two things are worth pinning: the component is APPENDED to witch-line's own
-- default list rather than replacing it, and it stays hidden until there is
-- something to report. The counting itself is read out of a faked task list, so
-- these cases need neither overseer nor witch-line running.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

local DEFAULTS = { "mode", "file.name", "git.branch", "%=", "cursor.pos" }

---Run `fn(component, components)` with witch-line and overseer faked, and the
---fakes still in place while it runs. The component's `hidden` and `update`
---close over `require` calls made at call time, not at build time, so restoring
---`package.loaded` before the assertions would silently measure the real
---overseer instead of the task list under test.
---@param tasks table[] What `list_tasks` should answer
---@param fn fun(component: table, components: table)
local function with_tasks(tasks, fn)
  local names = { "witch-line.constant.default", "overseer.task_list", "overseer" }
  local saved = {}
  for _, name in ipairs(names) do
    saved[name] = { package.loaded[name] }
  end

  package.loaded["witch-line.constant.default"] = DEFAULTS
  package.loaded["overseer.task_list"] = {
    list_tasks = function()
      return tasks
    end,
  }
  -- The component reads `package.loaded` rather than requiring overseer, so this
  -- is what decides whether it considers itself live at all.
  package.loaded["overseer"] = {}

  local ok, err = pcall(function()
    local spec = dofile(config_root .. "/lua/plugins/ui.lua")
    for _, plugin in ipairs(spec) do
      if plugin[1] == "sontungexpt/witch-line" then
        local components = plugin.opts().statusline.global
        fn(components[#components], components)
        return
      end
    end
    error("the witch-line spec is gone from plugins/ui.lua")
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

---Every name a function closes over.
---@param fn function
---@return string[]
local function upvalues(fn)
  local names, index = {}, 1
  while true do
    local name = debug.getupvalue(fn, index)
    if not name then
      return names
    end
    table.insert(names, name)
    index = index + 1
  end
end

return {
  ["the callbacks capture nothing"] = function()
    -- witch-line's cache serializes a component's callbacks as bytecode WITHOUT
    -- their upvalues, so anything captured comes back nil on a start that reads
    -- a populated cache: observed as `ui.lua:123: attempt to call upvalue
    -- 'counts' (a nil value)`. Reaching the module by name through `require`, a
    -- global lookup, is what survives that roundtrip.
    with_tasks({}, function(component)
      for _, field in ipairs({ "hidden", "update" }) do
        local captured = upvalues(component[field])
        assert(
          #captured == 0,
          ("%s captures %s, which the cache will not restore"):format(field, table.concat(captured, ", "))
        )
      end
    end)
  end,

  ["the default components are kept, with the counter appended"] = function()
    with_tasks({}, function(_, components)
      assert(#components == #DEFAULTS + 1, "got " .. #components .. " components, expected " .. (#DEFAULTS + 1))
      for index, name in ipairs(DEFAULTS) do
        assert(components[index] == name, ("component %d was %s"):format(index, tostring(components[index])))
      end
    end)
  end,

  ["the counter is the last component and names itself"] = function()
    with_tasks({}, function(component)
      assert(component.id == "overseer.tasks", "id was " .. tostring(component.id))
    end)
  end,

  ["it hides when there are no tasks"] = function()
    with_tasks({}, function(component)
      assert(component.hidden(), "the counter is visible with an empty task list")
    end)
  end,

  ["it shows a count per status"] = function()
    with_tasks(vim.list_extend(tasks(2, "RUNNING"), tasks(1, "FAILURE")), function(component)
      assert(not component.hidden(), "the counter is hidden with three tasks")
      local text = component.update()
      assert(text:match("2"), "the running count is missing from " .. text)
      assert(text:match("1"), "the failure count is missing from " .. text)
    end)
  end,

  ["it orders running before finished"] = function()
    with_tasks(vim.list_extend(tasks(1, "SUCCESS"), tasks(1, "RUNNING")), function(component)
      local text = component.update()
      local running, success = text:find("\u{f046e}"), text:find("\u{f0134}")
      assert(running and success, "both glyphs should be present in " .. text)
      assert(running < success, "running should be reported before success: " .. text)
    end)
  end,

  ["each status keeps a real glyph, not a stripped empty string"] = function()
    -- These are Nerd Font private-use codepoints, which have been silently
    -- dropped in transit before now.
    with_tasks(tasks(1, "CANCELED"), function(component)
      local text = component.update()
      assert(text:match("\u{f46e}"), "the canceled glyph is missing from " .. vim.inspect(text))
    end)
  end,

  ["a status with no tasks contributes nothing"] = function()
    with_tasks(tasks(1, "SUCCESS"), function(component)
      local text = component.update()
      assert(not text:match("\u{f015a}"), "a failure glyph appeared with no failures: " .. text)
    end)
  end,
}
