-- Reload a buffer when something outside Neovim writes its file (spec 5.4).
--
-- `checktime` on focus change and cursor movement never fires while Neovim
-- sits idle in one herdr pane and an agent writes the file from another, so
-- the file itself is watched. The augroup in `config/autocmds.lua` decides
-- which buffers get a watch; this module owns the handles.

local M = {}

-- The live `vim.uv` fs_event handles, keyed by buffer number. Public because a
-- leaked second handle is invisible from anywhere else, and the spec asserts
-- against it.
M.handles = {}

local start

-- Returns the handle, or nil when this buffer has no file to watch.
start = function(bufnr)
  local name = vim.api.nvim_buf_get_name(bufnr)
  local path = vim.uv.fs_realpath(name)
  if not path then
    -- A path that still exists but does not resolve is the case worth saying
    -- out loud: a watch that silently never starts is worse than no watch,
    -- because the buffer then goes stale with nothing to show for it. A name
    -- that resolves to nothing (an unwritten buffer, a deleted file) is the
    -- ordinary case and stays quiet.
    if name ~= "" and vim.uv.fs_lstat(name) then
      vim.notify_once(
        "auto_reload: cannot resolve " .. name .. ", buffer will not follow the file",
        vim.log.levels.WARN
      )
    end
    return nil
  end

  local handle = vim.uv.new_fs_event()
  if not handle then
    vim.notify_once("auto_reload: no fs_event handle available", vim.log.levels.WARN)
    return nil
  end

  handle:start(path, {}, function()
    vim.schedule(function()
      if not vim.api.nvim_buf_is_loaded(bufnr) then
        M.unwatch(bufnr)
        return
      end
      vim.cmd("checktime " .. bufnr)
      -- The watch is on the inode, so a writer that renames a new file into
      -- place (most formatters, and any agent that writes a temp file and
      -- moves it) leaves this handle on an inode nothing will ever touch
      -- again. Re-arm after every event, on the path as it is now.
      M.unwatch(bufnr)
      M.handles[bufnr] = start(bufnr)
    end)
  end)

  return handle
end

-- Idempotent: a second call while a watch is live is a no-op, so the repeated
-- `BufWritePost` on one buffer cannot leak a handle per write.
function M.watch(bufnr)
  if M.handles[bufnr] then
    return
  end
  M.handles[bufnr] = start(bufnr)
end

-- A no-op on a buffer that was never watched.
function M.unwatch(bufnr)
  local handle = M.handles[bufnr]
  if not handle then
    return
  end
  M.handles[bufnr] = nil
  handle:stop()
  handle:close()
end

return M
