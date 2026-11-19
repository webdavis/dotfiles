-- Reload a buffer when another process writes its file, even while Neovim sits idle.

local M = {}

-- Live fs_event handles by buffer number, public so tests can count them.
M.handles = {}

local start

-- Returns the handle, or nil when this buffer has no file to watch.
start = function(bufnr)
  local name = vim.api.nvim_buf_get_name(bufnr)
  local missing = name ~= "" and not vim.uv.fs_lstat(name)
  -- A missing file is awaited through its directory, because a watch needs an existing path.
  local path = vim.uv.fs_realpath(missing and vim.fs.dirname(name) or name)
  if not path then
    -- Warn only when the file exists but its path does not resolve.
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
      if missing and not vim.uv.fs_lstat(name) then
        return
      end
      if missing and not vim.bo[bufnr].modified then
        -- `checktime` never loads a file created after its buffer, so read it afresh.
        vim.api.nvim_buf_call(bufnr, vim.cmd.edit)
      else
        vim.cmd("checktime " .. bufnr)
      end
      -- The watch follows the inode, so re-arm on the current path after each event.
      M.unwatch(bufnr)
      M.handles[bufnr] = start(bufnr)
    end)
  end)

  return handle
end

-- Idempotent, so each BufWritePost reuses the live handle.
function M.watch(bufnr)
  if M.handles[bufnr] then
    return
  end
  M.handles[bufnr] = start(bufnr)
end

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
