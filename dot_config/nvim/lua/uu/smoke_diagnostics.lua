local M = {}

function M.errors(snapshot)
  local errors = {}
  if not snapshot.entered then
    errors[#errors + 1] = "VimEnter was not observed"
  end
  if not snapshot.drained then
    errors[#errors + 1] = "startup work drain timeout"
  end
  if snapshot.errmsg ~= "" then
    errors[#errors + 1] = snapshot.errmsg
  end
  vim.list_extend(errors, snapshot.notifications)
  for _, history in ipairs(snapshot.histories) do
    if not history.ok or type(history.entries) ~= "table" then
      errors[#errors + 1] = history.name .. " history unreadable: " .. tostring(history.entries)
    else
      for _, entry in ipairs(history.entries) do
        errors[#errors + 1] = history.name .. ": " .. tostring(entry)
      end
    end
  end
  return errors
end

function M.capture(entered, drained, notifications, earlier_error)
  local snapshot =
    { entered = entered, drained = drained, notifications = notifications, histories = {}, errmsg = vim.v.errmsg }
  if earlier_error ~= "" and earlier_error ~= snapshot.errmsg then
    snapshot.notifications[#snapshot.notifications + 1] = earlier_error
  end
  local function history(name, read, text)
    local ok, entries = pcall(read)
    if ok and type(entries) == "table" then
      local rendered = {}
      for _, entry in pairs(entries) do
        local rendered_ok, line = pcall(text, entry)
        if not rendered_ok then
          ok, rendered = false, line
          break
        end
        rendered[#rendered + 1] = line
      end
      entries = rendered
    end
    snapshot.histories[#snapshot.histories + 1] = { name = name, ok = ok, entries = entries }
  end
  if package.loaded["snacks.notifier"] then
    history("Snacks", function()
      return require("snacks.notifier").get_history({ filter = "error" })
    end, function(entry)
      return entry.msg
    end)
  end
  if package.loaded["noice"] then
    history("Noice", function()
      return require("noice.message.manager").get({ error = true }, { history = true })
    end, function(entry)
      return entry:content()
    end)
  end
  return snapshot
end

return M
