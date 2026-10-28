-- custom_api.auto_reload (spec 5.4): a buffer follows its file when something
-- outside Neovim writes it, including a writer that replaces the file by
-- rename, and stops following once the watch is dropped.
--
-- Real files and a real `vim.uv` watch, because the behavior under test IS the
-- libuv event reaching `checktime`; a fake would answer for nothing. `vim.wait`
-- is what pumps that loop, and it is safe here: the runner is `nvim -l`, which
-- runs after startup, not a `-c` command evaluated before `VimEnter`.

-- Required per case rather than once at the top of the file, so a missing
-- module fails every case by name instead of aborting the run before the first.
local function auto_reload()
  return require("custom_api.auto_reload")
end

-- A fresh directory per file, under Neovim's own temp tree: short, and removed
-- when this process exits. The rename source is a sibling, since `os.rename`
-- cannot cross a filesystem.
local function write(path, text)
  local handle = assert(io.open(path, "w"), "could not write " .. path)
  handle:write(text)
  handle:close()
end

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

-- libuv registers a new fs_event with kqueue on the loop's NEXT poll, not
-- inside `start`, so a write issued in the same tick as the watch is missed.
-- Production never meets that window (the loop runs continuously between the
-- autocmd and the agent's write, and again between a re-arm and the next one),
-- but a spec that writes immediately would watch nothing and pass or fail for
-- the wrong reason, so it pumps the loop first.
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
    -- The SECOND rename is the case that matters. macOS watches the inode, so
    -- a watch that is not re-armed sits on the inode the first rename orphaned
    -- and never fires again; a first rename passing proves nothing.
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
    -- Armed first, so what this case pins is `unwatch` stopping a live watch
    -- rather than a watch that was never registered.
    arm()
    module.unwatch(bufnr)

    write(path, "two two\n")

    assert(not reaches(bufnr, "two two", 300), "the buffer reloaded after unwatch")
  end,

  ["watching a buffer twice keeps the one handle"] = function()
    -- Every `BufWritePost` calls `watch` again, so a `watch` that started a
    -- second handle would leak one per write, with nothing outside the handle
    -- table able to see it.
    local module = auto_reload()
    local bufnr = open(temp_file("one\n"))

    module.watch(bufnr)
    local handle = module.handles[bufnr]
    assert(handle, "watch started no handle")
    module.watch(bufnr)
    assert(module.handles[bufnr] == handle, "the second watch replaced the handle and leaked the first")

    module.unwatch(bufnr)
    assert(module.handles[bufnr] == nil, "unwatch left the handle in the table")
    -- Dropping the table entry is not enough. A handle that is stopped but never
    -- closed stays alive in the loop for the life of the process, one per buffer
    -- ever watched, and nothing outside the handle itself can see it.
    assert(handle:is_closing(), "unwatch stopped the handle but never closed it")
  end,

  ["the augroup watches a normal file buffer without a manual watch call"] = function()
    -- The cases above drive the module directly, which leaves the augroup in
    -- `config/autocmds.lua` unpinned: delete its `BufReadPost` autocmd and all
    -- four still pass while the feature is dead in every real Neovim. This one
    -- goes through the real entry point and never calls `watch` itself.
    local module = auto_reload()
    require("config.autocmds")
    local path = temp_file("one\n")
    local bufnr = open(path)
    assert(module.handles[bufnr], "BufReadPost started no watch on a normal file buffer")
    arm()

    write(path, "two two\n")

    assert(reaches(bufnr, "two two"), "the buffer still reads " .. tostring(first_line(bufnr)))

    module.unwatch(bufnr)
    -- Every other case owns its own watches, so the augroup goes again: what the
    -- runner happens to run after this must not depend on it being live.
    vim.api.nvim_del_augroup_by_name("nvim_config_auto_reload")
  end,
}
