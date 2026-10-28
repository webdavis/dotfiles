---@class custom_api.clipboard
local M = {}

local REGISTER = "+"

local PREVIEW_LIMIT = 60

---Cuts by characters so a multi-byte glyph stays whole.
---@param text string The yanked text.
---@return string preview One line, never longer than the limit plus a count.
function M.summarize(text)
  local lines = vim.split(text, "\n", { plain = true })
  local first = lines[1] or ""
  if vim.fn.strchars(first) > PREVIEW_LIMIT then
    first = vim.fn.strcharpart(first, 0, PREVIEW_LIMIT) .. "..."
  end
  if #lines > 1 then
    return first .. " (" .. #lines .. " lines)"
  end
  return first
end

---Yanks the live selection, because `gv` in a Visual-mode mapping reselects the previous one.
---@return nil
function M.yank_selection()
  local cursor = vim.api.nvim_win_get_cursor(0)
  vim.cmd('normal! "' .. REGISTER .. "y")
  -- A yank adds and removes no lines, so the saved position still exists.
  vim.api.nvim_win_set_cursor(0, cursor)
end

---@return nil
function M.yank_selected_lines()
  vim.cmd('normal! "' .. REGISTER .. "Y")
end

---Without a count, warns on a blank line, where `g_` has no target.
---@return nil
function M.yank_to_line_end()
  local count = vim.v.count > 0 and tostring(vim.v.count) or ""
  if count == "" and vim.api.nvim_get_current_line():match("^%s*$") then
    vim.notify("Nothing to yank: the line is blank", vim.log.levels.WARN)
    return
  end
  vim.cmd('normal! "' .. REGISTER .. count .. "yg_")
end

---Empties the register first so a missing text object shows, then restores it.
---An empty pair `()` also counts as a miss.
---@param delimiter string One delimiter character, for example `(` or `"`.
---@param label string What to call the pair when it is missing.
---@return boolean yanked Whether anything was actually copied.
function M.yank_inside(delimiter, label)
  local saved = vim.fn.getreg(REGISTER)
  local saved_type = vim.fn.getregtype(REGISTER)
  vim.fn.setreg(REGISTER, "")

  local ok = pcall(vim.cmd, 'normal! "' .. REGISTER .. "yi" .. delimiter)
  local yanked = vim.fn.getreg(REGISTER)

  if not ok or yanked == "" then
    vim.fn.setreg(REGISTER, saved, saved_type)
    vim.notify("Nothing to yank: no " .. label .. " here", vim.log.levels.WARN)
    return false
  end

  vim.notify("Yanked: " .. M.summarize(yanked), vim.log.levels.INFO)
  return true
end

return M
