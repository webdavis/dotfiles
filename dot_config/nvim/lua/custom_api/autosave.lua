-- Keeps auto-save off claudecode proposed-edit buffers, where a write means "accept".

local M = {}

---@param bufnr integer
---@return boolean
function M.should_save(bufnr)
  -- Checks the claudecode marker rather than the `acwrite` buftype, because Octo and gitsigns
  -- buffers use that buftype too and should still auto-save.
  return vim.b[bufnr].claudecode_diff_tab_name == nil
end

--- Raise the flag conform's format-on-save hook reads, for one automatic write.
---@param bufnr integer
function M.mark_write(bufnr)
  vim.b[bufnr].autosave_write = true

  -- Also clear on a schedule, because a throwing `BufWritePre` skips the auto-save post event.
  vim.schedule(function()
    -- While a write is still running, wait for SafeState so the format hook still sees the flag.
    if vim.fn.state("x") == "" then
      M.clear_write(bufnr)
      return
    end

    vim.api.nvim_create_autocmd("SafeState", {
      once = true,
      callback = function()
        M.clear_write(bufnr)
      end,
    })
  end)
end

---@param bufnr integer
function M.clear_write(bufnr)
  if vim.api.nvim_buf_is_valid(bufnr) then
    vim.b[bufnr].autosave_write = nil
  end
end

return M
