-- Drives the real generator against this repo's own justfile.

local provider = require("overseer.template.user.just_gates")

local repo_root = vim.fs.dirname(vim.fs.dirname(vim.fs.dirname(vim.fs.dirname(vim.fs.abspath(arg[0])))))

---@return string[] gates The recipes `just ship` runs, in order
local function ship_gates()
  local gates = {}
  local in_ship = false
  for _, line in ipairs(vim.fn.readfile(vim.fs.joinpath(repo_root, "justfile"))) do
    if line:match("^ship:") then
      in_ship = true
    elseif in_ship then
      local gate = line:match("^%s+just (%S+)$")
      if not gate then
        break
      end
      table.insert(gates, gate)
    end
  end
  return gates
end

return {
  ["the task runs the gates of the ship recipe in the same order"] = function()
    local templates = provider.generator({ dir = repo_root })
    assert(type(templates) == "table", tostring(templates))

    local task_gates = {}
    for _, step in ipairs(templates[1].builder().strategy.tasks) do
      table.insert(task_gates, step.cmd[2])
    end

    local expected = ship_gates()
    assert(vim.tbl_contains(expected, "lint-secrets"), "ship recipe was not parsed: " .. vim.inspect(expected))
    assert(
      vim.deep_equal(task_gates, expected),
      ("task runs %s, ship runs %s"):format(vim.inspect(task_gates), vim.inspect(expected))
    )
  end,
}
