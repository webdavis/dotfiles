-- custom_api.util's pure string helpers (spec 6.3), plus the arity and trimming
-- of run_shell_command, the caller the trim fix actually narrowed.

local util = require("custom_api.util")

-- `#` is undefined on a table with an embedded nil, so the count comes from
-- `select` instead.
local function collect(...)
  return select("#", ...), { ... }
end

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

  ["map has left util for custom_api.keymap"] = function()
    assert(util.map == nil, "util.map is still " .. type(util.map))
    assert(type(require("custom_api.keymap").map) == "function", "custom_api.keymap.map is missing")
  end,

  ["overseer_runner has left util for custom_api.overseer"] = function()
    assert(util.overseer_runner == nil, "util.overseer_runner is still " .. type(util.overseer_runner))
    assert(
      type(require("custom_api.overseer").overseer_runner) == "function",
      "custom_api.overseer.overseer_runner is missing"
    )
  end,

  ["run_shell_command returns the exit code and the trimmed output, and nothing else"] = function()
    -- This is the caller the trim fix narrowed from three values to two: gsub's
    -- substitution count used to ride out of `trim` and become a third result.
    -- The shell is real because running a command is what this function is for.
    local count, values = collect(util.run_shell_command({ cmd = "printf '  out  '" }))
    assert(count == 2, "returned " .. count .. " values")
    assert(values[1] == 0, "exit code was " .. tostring(values[1]))
    assert(values[2] == "out", "output was " .. tostring(values[2]))
  end,
}
