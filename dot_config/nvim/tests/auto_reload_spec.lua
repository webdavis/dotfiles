-- Real files and a real `vim.uv` watch, because the test covers the libuv event reaching `checktime`.

-- Required per case, so a missing module fails each case by name.
local function auto_reload()
  return require("custom_api.auto_reload")
end

local function write(path, text)
  local handle = assert(io.open(path, "w"), "could not write " .. path)
  handle:write(text)
  handle:close()
end

-- Each file gets its own directory, which Neovim removes on exit. The rename case stages its file
-- in the same directory, because `os.rename` cannot move a file across filesystems.
local function temp_file(text)
  local dir = vim.fn.tempname()
  assert(vim.fn.mkdir(dir, "p") == 1, "could not create " .. dir)
  local path = dir .. "/watched.txt"
  write(path, text)
  return path
end

local function first_line(bufnr)
  return vim.api.nvim_buf_get_lines(bufnr, 0, 1, false)[1]
end

local function open(path)
  vim.cmd.edit(path)
  return vim.api.nvim_get_current_buf()
end

local function reaches(bufnr, text, timeout)
  return vim.wait(timeout or 500, function()
    return first_line(bufnr) == text
  end, 10)
end

-- libuv registers the watch on the next loop poll, so pump the loop before writing.
local function arm()
  vim.wait(20)
end

return {
  ["a watched buffer follows an in-place write"] = function()
    local module = auto_reload()
    local path = temp_file("one\n")
    local bufnr = open(path)
    module.watch(bufnr)
    arm()

    write(path, "two two\n")

    assert(reaches(bufnr, "two two"), "the buffer still reads " .. tostring(first_line(bufnr)))
    module.unwatch(bufnr)
  end,

  ["a watched buffer follows every writer that replaces the file by rename"] = function()
    -- The second rename matters: macOS watches the inode, so only a re-armed watch fires again.
    local module = auto_reload()
    local path = temp_file("one\n")
    local bufnr = open(path)
    module.watch(bufnr)
    arm()

    for _, expected in ipairs({ "two two", "three three three" }) do
      local staged = path .. ".staged"
      write(staged, expected .. "\n")
      assert(os.rename(staged, path))
      assert(
        reaches(bufnr, expected),
        ("after renaming %q into place the buffer reads %s"):format(expected, tostring(first_line(bufnr)))
      )
      arm()
    end

    module.unwatch(bufnr)
  end,

  ["an unwatched buffer stops following its file"] = function()
    local module = auto_reload()
    local path = temp_file("one\n")
    local bufnr = open(path)
    module.watch(bufnr)
    -- Armed first, so this covers `unwatch` stopping a live watch.
    arm()
    module.unwatch(bufnr)

    write(path, "two two\n")

    assert(not reaches(bufnr, "two two", 300), "the buffer reloaded after unwatch")
  end,

  ["watching a buffer twice keeps the one handle"] = function()
    -- Every `BufWritePost` calls `watch` again.
    local module = auto_reload()
    local bufnr = open(temp_file("one\n"))

    module.watch(bufnr)
    local handle = module.handles[bufnr]
    assert(handle, "watch started no handle")
    module.watch(bufnr)
    assert(module.handles[bufnr] == handle, "the second watch replaced the handle and leaked the first")

    module.unwatch(bufnr)
    assert(module.handles[bufnr] == nil, "unwatch left the handle in the table")
    -- A stopped handle that is never closed stays alive in the loop.
    assert(handle:is_closing(), "unwatch stopped the handle but never closed it")
  end,

  ["the augroup watches a normal file buffer without a manual watch call"] = function()
    -- Goes through the real autocmds and never calls `watch` itself.
    local module = auto_reload()
    require("config.autocmds")
    local path = temp_file("one\n")
    local bufnr = open(path)
    assert(module.handles[bufnr], "BufReadPost started no watch on a normal file buffer")
    arm()

    write(path, "two two\n")

    assert(reaches(bufnr, "two two"), "the buffer still reads " .. tostring(first_line(bufnr)))

    module.unwatch(bufnr)
    -- Delete the augroup so later cases do not depend on it.
    vim.api.nvim_del_augroup_by_name("nvim_config_auto_reload")
  end,
}
