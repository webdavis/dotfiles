-- Whether auto-save.nvim may write a buffer, and the flag that tells lsp-format
-- an automatic write is under way (spec 5.3, item 42).
--
-- claudecode.nvim opens a proposed edit in a scratch buffer and treats a write
-- as "accepted". Auto-save fires on a timer, so without this rule an agent's
-- proposal is accepted by nobody, seconds after it appears.

local M = {}

--- Whether a buffer is safe for auto-save.nvim to write.
---@param _name string the buffer's name; deliberately not read, see below
---@param buftype string the buffer's `&buftype`
---@return boolean
function M.should_save(_name, buftype)
  -- The buftype decides, never the name. At the pinned claudecode every
  -- writable proposal buffer is `acwrite`; the one legacy `(New)` buffer is
  -- `nofile`, which Neovim refuses to write at all, so no name rule is needed
  -- to protect it. A name rule would only stand between auto-save and an
  -- ordinary file that happens to live under a `(proposed)` directory. The name
  -- stays in the signature so a spec can prove it is not consulted.
  return buftype ~= "acwrite"
end

--- Raise the flag lsp-format's `BufWritePre` reads, for one automatic write.
---@param bufnr integer
function M.mark_write(bufnr)
  vim.b[bufnr].autosave_write = true

  -- auto-save.nvim drops the flag in its post event, but a `BufWritePre`
  -- handler that throws escapes the write and that event never runs. A flag
  -- left up would silently skip formatting on every later manual write of the
  -- buffer, so the clear is scheduled here as well; on the normal path the post
  -- event has already run by then and this is a no-op.
  vim.schedule(function()
    -- `vim.schedule` is not an after-write barrier. An earlier `BufWritePre`
    -- handler that yields (a wait, a nested write) pumps the event loop mid
    -- write, which runs this callback while the write is still going. Clearing
    -- here would let the format handler further down the chain read no flag and
    -- format the automatic write. `SafeState` is the real barrier: it fires once
    -- Neovim is idle again, which is necessarily after the write.
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

--- Drop the flag, if the buffer is still around to carry it.
---@param bufnr integer
function M.clear_write(bufnr)
  if vim.api.nvim_buf_is_valid(bufnr) then
    vim.b[bufnr].autosave_write = nil
  end
end

return M
