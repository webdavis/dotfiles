---@class custom_api.clipboard
---Yanks that land in the system clipboard and tell the truth about what they
---copied.
---
---THE SELECTION IS YANKED WHERE IT STANDS, with no range and no `gv`. Three
---spellings were measured against charwise, linewise and blockwise selections
---(`tests/clipboard_spec.lua` pins the survivor):
---
---  * `:'<,'>y+`, the Ex range this replaced, is ALWAYS linewise, so selecting
---    one word copied its whole line. That was the reported bug.
---  * `normal! gv"+y` is wrong for a different reason: the callback of a
---    Visual-mode mapping runs while the mode is STILL visual and before `'<`
---    and `'>` are set, so `gv` there swaps in the PREVIOUS selection. On the
---    first selection of a session it copied nothing at all.
---  * `normal! "+y` yanks the live selection with its own mode, which is what
---    the operator's own hands would do, `curswant` and a ragged `<C-v>$` edge
---    included.
local M = {}

---The register every function here writes. Spelled once, because a second
---spelling is a yank that silently lands somewhere else.
local REGISTER = "+"

---How much yanked text a notification shows before it is cut short.
local PREVIEW_LIMIT = 60

---A one-line preview of yanked text, short enough to read in a notification.
---
---A whole paragraph or buffer in a notification body is a wall nobody reads,
---so a long first line is cut and a multi-line yank says how many lines it
---carried instead of printing them. The cut counts CHARACTERS rather than
---bytes, or it lands inside a multi-byte character and prints a broken glyph.
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

---Yank the visual selection to the clipboard, exactly as selected.
---
---Called from a Visual-mode mapping, while that selection is still live. The
---cursor is put back where it was, which is what the mapping this replaced
---claimed to do and did not.
---@return nil
function M.yank_selection()
  local cursor = vim.api.nvim_win_get_cursor(0)
  vim.cmd('normal! "' .. REGISTER .. "y")
  -- A yank adds and removes no lines, so the saved position still exists.
  vim.api.nvim_win_set_cursor(0, cursor)
end

---Yank the LINES the visual selection sits in to the clipboard, whole.
---
---The capital is Vim's own convention for the linewise form, so `<leader>y`
---copies what is selected and `Y` copies the lines it covers. Before this, `Y`
---in Visual mode reached neither the clipboard nor the selection: it fell
---through to the built-in linewise yank into the unnamed register.
---@return nil
function M.yank_selected_lines()
  vim.cmd('normal! "' .. REGISTER .. "Y")
end

---Yank from the cursor to the last non-blank character of the line.
---
---GUARDED, because `g_` has no target on a blank line and the unguarded form
---beeped instead of saying anything.
---@return nil
function M.yank_to_line_end()
  if vim.api.nvim_get_current_line():match("^%s*$") then
    vim.notify("Nothing to yank: the line is blank", vim.log.levels.WARN)
    return
  end
  vim.cmd('normal! "' .. REGISTER .. "yg_")
end

---Yank the text inside a delimiter pair to the clipboard.
---
---THE CLIPBOARD IS RESTORED WHEN THE TEXT OBJECT IS NOT THERE, which is the
---defect this replaces. The previous version yanked and then read the register
---back to report what it got, so `yi(` on a line holding no parentheses
---announced the PREVIOUS clipboard contents as freshly yanked. Emptying the
---register first makes an absent text object detectable, and putting the old
---value back means a failed yank costs the operator nothing.
---
---Ceiling: an EMPTY pair (`()`) reads as a miss, because Vim yanks nothing for
---either one and the register cannot tell them apart. The report is a warning
---and the clipboard is intact, which is the right outcome for both.
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
