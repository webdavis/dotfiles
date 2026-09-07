local function errors(change)
  local snapshot = { entered = true, drained = true, errmsg = "", notifications = {}, histories = {} }
  for key, value in pairs(change or {}) do
    snapshot[key] = value
  end
  local ok, result = pcall(function()
    return require("uu.smoke_diagnostics").errors(snapshot)
  end)
  assert(ok, tostring(result))
  return result
end

return {
  ["a healthy startup has no diagnostic failure"] = function()
    assert(#errors() == 0)
  end,
  ["an initialization error remains a startup failure"] = function()
    local result = errors({ errmsg = "owned init failed" })
    assert(#result == 1 and result[1]:find("owned init failed", 1, true))
  end,
  ["error notifications fail even when lazy task errors are false"] = function()
    local result = errors({ notifications = { "owned plugin failed" } })
    assert(#result == 1 and result[1]:find("owned plugin failed", 1, true))
  end,
  ["errors retained by loaded Snacks and Noice each fail startup"] = function()
    for _, name in ipairs({ "Snacks", "Noice" }) do
      local result = errors({ histories = { { name = name, ok = true, entries = { "owned late error" } } } })
      assert(#result == 1 and result[1]:find(name, 1, true) and result[1]:find("owned late error", 1, true))
    end
  end,
  ["an unreadable loaded notifier history fails closed"] = function()
    for _, history in ipairs({
      { name = "Snacks", ok = false, entries = "read failed" },
      {
        name = "Noice",
        ok = true,
        entries = false,
      },
    }) do
      local result = errors({ histories = { history } })
      assert(#result == 1 and result[1]:find("history unreadable", 1, true))
    end
  end,
  ["a missing VimEnter and a startup drain timeout each fail"] = function()
    local missing, timeout = errors({ entered = false }), errors({ drained = false })
    assert(type(missing[1]) == "string" and missing[1]:find("VimEnter", 1, true))
    assert(type(timeout[1]) == "string" and timeout[1]:find("timeout", 1, true))
  end,
}
