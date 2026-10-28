-- Runs `just ship` as one task per gate, so a failure names its gate.

local GATES = { "lint-gate", "test", "lint-actions-security" }

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

-- No `cache_key`, because the built-in `just` provider keys on the same justfile
-- and the cache would mix the two.
---@type overseer.TemplateFileProvider
return {
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
