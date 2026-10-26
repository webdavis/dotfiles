---The one Markdown task toggle. A plain line becomes `- [ ] line`, a list item
---gains a box, and a task flips between `[ ]` and `[x]`.
local M = {}

---@param line string
---@return string new_line
---@return integer shift Bytes added before the cursor.
function M.toggle(line)
  local prefix = line:match("^(%s*[-*+] )") or line:match("^(%s*%d+[.)] )")
  if not prefix then
    local indent, text = line:match("^(%s*)(.*)$")
    return indent .. "- [ ] " .. text, 6
  end

  local box = line:sub(#prefix + 1, #prefix + 3)
  local rest = line:sub(#prefix + 4)
  if box == "[ ]" then
    return prefix .. "[x]" .. rest, 0
  end
  if box == "[x]" or box == "[X]" then
    return prefix .. "[ ]" .. rest, 0
  end
  return prefix .. "[ ] " .. line:sub(#prefix + 1), 4
end

---Toggle the task on the cursor's line, keeping the cursor on the same
---character of the text.
---@return nil
function M.apply()
  local row, col = unpack(vim.api.nvim_win_get_cursor(0))
  local line, shift = M.toggle(vim.api.nvim_get_current_line())
  vim.api.nvim_set_current_line(line)
  vim.api.nvim_win_set_cursor(0, { row, col + shift })
end

return M
