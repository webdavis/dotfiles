---The keymap-layer error boundary: turns `error()` and `nil, message` results into notifications.
local function collect(...)
  return select("#", ...), { ... }
end

---@param fn fun():any The call to run.
---@param opts { label: string } `label` names the call in the notification.
---@return any ... `fn`'s values, or nothing when it failed.
return function(fn, opts)
  opts = opts or {}
  local label = opts.label
  if type(label) ~= "string" or label == "" then
    error("custom_api.try: `label` is required")
  end

  local count, results = collect(xpcall(fn, debug.traceback))

  -- A thrown error is a bug, and `debug.traceback` already added the traceback to its message.
  if not results[1] then
    vim.notify(("[%s] %s"):format(label, results[2]), vim.log.levels.ERROR, { title = label })
    return
  end

  -- A `nil, message` result is an expected failure, so it reports no traceback.
  if results[2] == nil and type(results[3]) == "string" then
    vim.notify(("[%s] %s"):format(label, results[3]), vim.log.levels.WARN, { title = label })
    return
  end

  return unpack(results, 2, count)
end
