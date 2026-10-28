local try = require("custom_api.try")

-- Runs `try` with `vim.notify` recorded. Returns the notified text, then what `try` returned.
-- `select` counts the values, because `#` is undefined with an embedded nil.
local function collect(...)
  return select("#", ...), { ... }
end

local function run(fn, opts)
  local messages = {}
  local real_notify = vim.notify
  vim.notify = function(message)
    table.insert(messages, message)
  end
  local count, results = collect(pcall(try, fn, opts))
  vim.notify = real_notify
  assert(results[1], results[2])
  return table.concat(messages, "\n"), unpack(results, 2, count)
end

local function blows_up()
  error("the call blew up")
end

local function reports_failure()
  return nil, "not a git repository"
end

return {
  ["reports the explicit label of a raising call"] = function()
    local text = run(blows_up, { label = "git.default_branch" })
    assert(text:find("git.default_branch", 1, true), "label missing from: " .. text)
    assert(text:find("the call blew up", 1, true), "message missing from: " .. text)
  end,

  ["never falls back to a reflected name"] = function()
    local text = run(blows_up, { label = "git.default_branch" })
    assert(not text:find("anonymous", 1, true), "reported a reflected name: " .. text)
  end,

  ["includes the traceback of a raising call"] = function()
    local text = run(blows_up, { label = "git.default_branch" })
    assert(text:find("stack traceback:", 1, true), "no traceback in: " .. text)
  end,

  ["reports an operational failure under the same label"] = function()
    local text = run(function()
      return nil, "not a git repository"
    end, { label = "git.default_branch" })
    assert(text:find("git.default_branch", 1, true), "label missing from: " .. text)
    assert(text:find("not a git repository", 1, true), "message missing from: " .. text)
  end,

  ["passes a successful call's values through untouched"] = function()
    local text, branch = run(function()
      return "main"
    end, { label = "git.default_branch" })
    assert(text == "", "notified on success: " .. text)
    assert(branch == "main", "returned " .. tostring(branch))
  end,

  ["passes every value of a successful call through, not just the first"] = function()
    local _, branch, remote = run(function()
      return "main", "origin"
    end, { label = "git.default_branch" })
    assert(branch == "main", "first value was " .. tostring(branch))
    assert(remote == "origin", "second value was " .. tostring(remote))
  end,

  ["returns nothing at all after a failed call"] = function()
    -- Return nothing on failure, so a caller's `if not result` guard fires.
    local after_bug = select("#", run(blows_up, { label = "git.default_branch" })) - 1
    assert(after_bug == 0, "returned " .. after_bug .. " values after a bug")

    local after_failure = select("#", run(reports_failure, { label = "git.default_branch" })) - 1
    assert(after_failure == 0, "returned " .. after_failure .. " values after an operational failure")
  end,

  ["refuses a call with no label"] = function()
    -- A missing label is a caller bug.
    local ok = pcall(try, blows_up, {})
    assert(not ok, "accepted a call with no label")
  end,
}
