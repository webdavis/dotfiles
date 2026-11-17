-- A plugin that ships in the same repo as a program. An apply keeps its checkout on the commit that program was built from.
local M = {}

local function checkout(name)
  return vim.fn.stdpath("data") .. "/paired/" .. name
end

function M.is_present(name)
  return vim.fn.isdirectory(checkout(name) .. "/lua") == 1
end

-- The lazy.nvim fields that load the plugin from its checkout, and leave it out until an apply has made one.
function M.spec(name)
  return {
    dir = checkout(name),
    name = name,
    enabled = function()
      return M.is_present(name)
    end,
  }
end

return M
