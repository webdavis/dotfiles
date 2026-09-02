local M = {}

---Collects multiple return values or arguments into a table.
---
---The returned table contains all values and a `.n` field indicating
---the number of elements, making it compatible with LuaJIT / Lua 5.1
---environments that lack `table.pack`.
---
---Example:
---```lua
---local t = pack(1, 2, 3)
---print(t[1], t[2], t[3], t.n)  --> 1  2  3  3
---```
---
---@vararg any Values to pack into a table.
---@return table A table containing all arguments with a `.n` field.
local function pack(...)
  return { n = select("#", ...), ... }
end

---Wraps a function to automatically log notifications with its module and function name.
---The wrapped function will:
---  1. Catch runtime errors (`error()`) and notify via `vim.notify`.
---  2. Handle soft errors by returning `nil, "message"` and notifying automatically.
---  3. Preserve all return values on success.
---@param module_name string The name of the module (e.g., "custom_api.github").
---@param fn function The function to wrap.
---@return function The wrapped function with automatic error logging.
function M.wrap(module_name, fn, opts)
  opts = opts or {}

  return function(call_opts)
    call_opts = call_opts or {}

    local log_level = call_opts.log_level or opts.log_level or vim.log.levels.ERROR
    local quiet = call_opts.quiet or opts.quiet or false
    local notify_title = { title = module_name }

    -- Safely call the function will all arguments.
    local ok, results = pcall(function()
      return pack(fn(call_opts))
    end)

    local func_name = debug.getinfo(fn, "n").name or "anonymous"
    local full_name = string.format("%s.%s", module_name, func_name)

    -- Handle hard errors:
    if not ok then
      if not quiet then
        vim.notify(string.format("Error [`%s`]: %s", full_name, results), log_level, notify_title)
      end
      return nil
    end

    -- Find first string result:
    local message
    for i = 2, results.n do
      if type(results[i]) == "string" then
        message = results[i]
        break
      end
    end

    -- Handle soft errors:
    if results[1] == nil and message then
      if not quiet then
        vim.notify(string.format("Error [`%s`]: %s", full_name, results[2]), log_level, notify_title)
      end
      return nil
    end

    return unpack(results, 1, results.n)
  end
end

return M
