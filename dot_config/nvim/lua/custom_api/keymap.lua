---@class custom_api.keymap
local M = {}

---@param rhs string|fun():any
---@return boolean
local is_command_silent = function(rhs)
  return not (type(rhs) == "string" and rhs:match("^:"))
end

---@class custom_api.keymap.map
---@field mode string|table
---@field lhs string|table
---@field rhs string|fun():any
---@field desc string|nil
---@field sequence boolean|nil Maps `rhs` as keys to type instead of a command to run.
---@field expr boolean|nil
---@field nowait boolean|nil
---@field remap boolean|nil

---Wraps `vim.keymap.set` with `noremap`, several `lhs` keys per call, and `silent` unless `rhs` starts with `:`.
---@param opts custom_api.keymap.map
---@return nil
function M.map(opts)
  local mode = opts.mode
  local lhs_list = opts.lhs
  local rhs = opts.rhs
  local sequence = opts.sequence

  local desc = opts.desc

  if type(lhs_list) == "string" then
    lhs_list = { lhs_list }
  end

  local keymap_opts = { noremap = true, desc = desc, silent = is_command_silent(rhs) }

  for k, v in pairs(opts) do
    if not (k == "mode" or k == "lhs" or k == "rhs" or k == "sequence") then
      keymap_opts[k] = v
    end
  end

  for _, lhs in ipairs(lhs_list) do
    if sequence or not keymap_opts.silent then
      vim.keymap.set(mode, lhs, rhs, keymap_opts)
    else
      vim.keymap.set(mode, lhs, function()
        if type(rhs) == "function" then
          return rhs()
        elseif type(rhs) == "string" then
          vim.cmd(rhs)
        end
      end, keymap_opts)
    end
  end
end

return M
