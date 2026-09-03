---@class custom_api.util
---My custom Neovim API.
local M = {}

-- ╭──────╮
-- │  API │
-- ╰──────╯

---Remove surrounding whitespace from `s`.
---
---The outer parentheses are load-bearing: they drop `gsub`'s substitution
---count, which every caller in tail position would otherwise return as a
---second value of its own.
function M.trim(s)
  return ((s or ""):gsub("^%s*(.-)%s*$", "%1"))
end

---Remove surrounding whitespace and convert `s` to lowercase.
function M.sanitize_input(s)
  return M.trim(s):lower()
end

function M.get_cwd_basename()
  return vim.fn.fnamemodify(vim.fn.getcwd(), ":t")
end

function M.copy_to_system_clipboard(data)
  vim.fn.setreg("+", data)
end

function M.normalize(message)
  message = M.trim(message)
  return (message ~= "" and message) or nil
end

function M.run_shell_command(opts)
  opts = opts or error("Missing `command` argument. Provide a table with a `command` field.")
  local cmd = opts.cmd
  local notify_error = opts.notify_error

  local command_string
  if type(cmd) == "table" then
    command_string = table.concat(cmd, " ")
  elseif type(cmd) == "string" then
    command_string = cmd
  else
    error(("Invalid `command` type: %s. Must be a string or table."):format(type(cmd)))
  end

  local output = vim.fn.system(command_string)
  local exit_code = vim.v.shell_error

  if exit_code ~= 0 and notify_error then
    if type(notify_error) == "boolean" then
      return nil, output
    end
  end

  return exit_code, M.trim(output)
end

return M
