---The keymap-layer error boundary (spec 6.1).
---
---`git`, `github` and `util` never call `vim.notify`: an operational failure
---(not a git repository, `gh` not logged in) is a `nil, message` result and a
---bug is an `error()`. This is the one place both become a message.
---
---The label is explicit data. Its predecessor, `helpers.wrap`, read the name
---off `debug.getinfo(fn, "n")`, which is nil for every local function in
---`custom_api`, so every report it ever wrote said "anonymous" (item 19).
---
---```lua
---local branch = custom_api.try(function()
---  return git.default_branch()
---end, { label = "git.default_branch" })
---```
---@param fn fun():any The call to run.
---@param opts { label: string } `label` names the call in the notification.
---@return any ... `fn`'s values, or nothing when it failed.

local function collect(...)
  return select("#", ...), { ... }
end

return function(fn, opts)
  opts = opts or {}
  local label = opts.label
  if type(label) ~= "string" or label == "" then
    error("custom_api.try: `label` is required")
  end

  local count, results = collect(xpcall(fn, debug.traceback))

  -- A bug: `debug.traceback` has already prefixed the message to the traceback.
  if not results[1] then
    vim.notify(("[%s] %s"):format(label, results[2]), vim.log.levels.ERROR, { title = label })
    return
  end

  -- An operational failure: a result, not a bug, so no traceback to report.
  if results[2] == nil and type(results[3]) == "string" then
    vim.notify(("[%s] %s"):format(label, results[3]), vim.log.levels.WARN, { title = label })
    return
  end

  return unpack(results, 2, count)
end
