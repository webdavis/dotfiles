-- `just ship`, but one task per gate, through the orchestrator strategy.
--
-- The recipe runs the three gates as three lines of one shell command, so a
-- failure gives one status and one pile of output to read. The orchestrator
-- strategy runs them as separate tasks in the same order, so each gate carries
-- its own status, its own output buffer and its own quickfix, and a red run says
-- which gate went red without reading anything.
--
-- The orchestrator over the `dependencies` component, which reaches the same end:
-- dependencies attach to a task that then runs LAST, which reads backwards for a
-- pipeline whose whole point is the order, and the orchestrator's own task shows
-- the sequence as children in the task list.

local GATES = { "lint-check", "test", "lint-actions-security" }

---@param dir string
---@return string|nil path The justfile that defines every gate
local function justfile_with_gates(dir)
  local candidates = vim.fs.find(function(name)
    local lower = name:lower()
    return lower == "justfile" or lower == ".justfile"
  end, { upward = true, type = "file", path = dir, limit = math.huge })

  for _, path in ipairs(candidates) do
    local text = table.concat(vim.fn.readfile(path), "\n")
    local complete = true
    for _, gate in ipairs(GATES) do
      -- A recipe is a line starting at column one with the name and a colon.
      if not text:match("\n" .. vim.pesc(gate) .. "[^\n]*:") and not text:match("^" .. vim.pesc(gate) .. "[^\n]*:") then
        complete = false
        break
      end
    end
    if complete then
      return path
    end
  end
end

---@type overseer.TemplateFileProvider
return {
  cache_key = function(opts)
    return justfile_with_gates(opts.dir)
  end,
  generator = function(opts)
    local justfile = justfile_with_gates(opts.dir)
    if not justfile then
      return "No justfile defining " .. table.concat(GATES, ", ")
    end
    local cwd = vim.fs.dirname(justfile)

    local steps = {}
    for _, gate in ipairs(GATES) do
      table.insert(steps, { cmd = { "just", gate }, cwd = cwd, name = "just " .. gate })
    end

    return {
      {
        name = "just ship (one task per gate)",
        desc = "The three gates CI runs, in CI order, each with its own status and quickfix",
        tags = { "TEST" },
        builder = function()
          return {
            name = "just ship (one task per gate)",
            cwd = cwd,
            strategy = { "orchestrator", tasks = steps },
          }
        end,
      },
    }
  end,
}
