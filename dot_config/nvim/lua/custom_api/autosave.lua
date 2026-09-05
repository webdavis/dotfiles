-- Whether auto-save.nvim may write a buffer, and the flag that tells lsp-format
-- an automatic write is under way (spec 5.3, item 42).
--
-- claudecode.nvim opens a proposed edit in a scratch buffer and treats a write
-- as "accepted". Auto-save fires on a timer, so without this rule an agent's
-- proposal is accepted by nobody, seconds after it appears. The exclusions are
-- the ones claudecode's own README documents for okuuva/auto-save.nvim.

local M = {}

--- Whether a buffer is safe for auto-save.nvim to write.
---@param name string the buffer's name, as `nvim_buf_get_name` returns it
---@param buftype string the buffer's `&buftype`
---@return boolean
function M.should_save(name, buftype)
  -- Lua patterns, so the parentheses and the dash are escaped with `%`.
  if name:match("%(proposed%)") or name:match("%(NEW FILE %- proposed%)") then
    return false
  end

  -- claudecode's diff buffers are `acwrite`, which catches a proposed buffer
  -- whose name a future version of the plugin spells differently.
  if buftype == "acwrite" then
    return false
  end

  return true
end

--- Raise the flag lsp-format's `BufWritePre` reads, for one automatic write.
---@param bufnr integer
function M.mark_write(bufnr)
  vim.b[bufnr].autosave_write = true

  -- auto-save.nvim drops the flag in its post event, but a `BufWritePre`
  -- handler that throws escapes the write and that event never runs. A flag
  -- left up would silently skip formatting on every later manual write of the
  -- buffer, so the clear is scheduled here as well. The write is synchronous,
  -- so this always runs after it, and on the normal path it is a no-op.
  vim.schedule(function()
    M.clear_write(bufnr)
  end)
end

--- Drop the flag, if the buffer is still around to carry it.
---@param bufnr integer
function M.clear_write(bufnr)
  if vim.api.nvim_buf_is_valid(bufnr) then
    vim.b[bufnr].autosave_write = nil
  end
end

return M
