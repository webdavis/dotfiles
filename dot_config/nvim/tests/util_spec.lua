-- custom_api.util's pure string helpers (spec 6.3).

local util = require("custom_api.util")

return {
  ["trim strips surrounding whitespace"] = function()
    assert(util.trim("  hi there  ") == "hi there")
  end,

  ["trim reads a missing string as empty"] = function()
    assert(util.trim(nil) == "")
  end,

  ["trim returns only the trimmed string"] = function()
    -- gsub's second return value is the substitution count. Leaking it makes
    -- every tail-position caller a two-value expression, and util.lua's own
    -- run_shell_command returned three values because of it.
    local first, second = util.trim(" x ")
    assert(first == "x", "trimmed to " .. tostring(first))
    assert(second == nil, "leaked a second value: " .. tostring(second))
  end,

  ["sanitize_input trims and lowercases"] = function()
    assert(util.sanitize_input("  HeLLo World  ") == "hello world")
  end,

  ["normalize returns the trimmed message"] = function()
    assert(util.normalize("  a message\t") == "a message")
  end,

  ["normalize reads a blank message as nil"] = function()
    assert(util.normalize("   \n  ") == nil)
  end,
}
