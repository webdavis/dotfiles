local M = {}
local uv = vim.uv

function M.read(path)
  local file = assert(io.open(path, "rb"), "could not read " .. path)
  local text = assert(file:read("*a"))
  assert(file:close())
  return text
end

local function sync_directory(path)
  local fd = assert(uv.fs_open(vim.fn.fnamemodify(path, ":h"), "r", 0))
  local ok, why = uv.fs_fsync(fd)
  assert(uv.fs_close(fd))
  assert(ok, why)
end

local serial = 0
function M.write(path, text)
  serial = serial + 1
  local temp = path .. ".tmp." .. uv.os_getpid() .. "." .. serial
  local fd = assert(uv.fs_open(temp, "wx", 384))
  local ok, why = pcall(function()
    local offset = 0
    while offset < #text do
      local written = assert(uv.fs_write(fd, text:sub(offset + 1), offset))
      assert(written > 0, "file write made no progress")
      offset = offset + written
    end
    assert(uv.fs_fsync(fd))
  end)
  local closed, close_error = uv.fs_close(fd)
  assert(ok, why)
  assert(closed, close_error)
  assert(uv.fs_rename(temp, path))
  sync_directory(path)
end

function M.save(path, record)
  vim.fn.mkdir(vim.fn.fnamemodify(path, ":h"), "p", 448)
  M.write(path, vim.json.encode(record) .. "\n")
end

function M.load(path)
  local stat, why, code = uv.fs_stat(path)
  if not stat then
    if code == "ENOENT" or code == "ENOTDIR" then
      return nil
    end
    error(why)
  end
  local record = vim.json.decode(M.read(path))
  assert(
    type(record) == "table"
      and record.version == 1
      and type(record.repo) == "string"
      and type(record.config) == "string"
      and type(record.lock) == "string",
    "unrecognized recovery record"
  )
  return record
end

function M.close(path)
  assert(uv.fs_rename(path, path .. ".closed"))
  sync_directory(path)
end

return M
