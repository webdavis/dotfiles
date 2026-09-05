-- custom_api.autosave (spec 5.3, item 42): auto-save.nvim keeps its hands off
-- the buffers claudecode.nvim opens for a proposed edit, so a diff is resolved
-- by `<leader>Cy`/`<leader>Cn` and never by a write that fired on a timer, and
-- an automatic write announces itself so lsp-format can stand down.
--
-- `should_save` takes the buffer's name and buftype as arguments rather than
-- reading them off a buffer, so the exclusion rule needs no buffer, no plugin
-- and no claudecode session to test.

-- Required per case rather than once at the top of the file, so a missing
-- module fails every case by name instead of aborting the run before the first.
local function autosave()
  return require("custom_api.autosave")
end

local function scratch()
  return vim.api.nvim_create_buf(false, true)
end

-- A real file buffer under Neovim's own temp tree, because the case below has to
-- perform an actual write for `BufWritePre` handlers to run at all.
local function file_buffer()
  local dir = vim.fn.tempname()
  assert(vim.fn.mkdir(dir, "p") == 1, "could not create " .. dir)
  local buf = vim.api.nvim_create_buf(false, false)
  vim.api.nvim_buf_set_name(buf, dir .. "/written.txt")
  return buf
end

-- Runs one automatic write of `bufnr` the way the plugin does, and reports what
-- a format handler sitting after `earlier` would have seen on the flag.
local function flag_seen_by_formatter(bufnr, earlier)
  local group = vim.api.nvim_create_augroup("AutosaveSpecWrite", { clear = true })
  if earlier then
    vim.api.nvim_create_autocmd("BufWritePre", { group = group, pattern = "*", callback = earlier })
  end
  local seen
  vim.api.nvim_create_autocmd("BufWritePre", {
    group = group,
    pattern = "*",
    callback = function(args)
      seen = vim.b[args.buf].autosave_write
    end,
  })
  vim.api.nvim_buf_call(bufnr, function()
    autosave().mark_write(bufnr)
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { "changed" })
    pcall(function()
      vim.cmd("silent! write!")
    end)
    autosave().clear_write(bufnr)
  end)
  vim.api.nvim_del_augroup_by_id(group)
  return seen
end

return {
  -- ── which buffers auto-save may write ──

  ["a claudecode diff buffer is not auto-saved"] = function()
    assert(autosave().should_save("lua/plugins/lsp.lua", "acwrite") == false)
  end,

  ["an ordinary file buffer is auto-saved"] = function()
    assert(autosave().should_save("lua/plugins/lsp.lua", "") == true)
  end,

  -- The rule is the buftype, never the name: at the pinned claudecode every
  -- writable proposal buffer is `acwrite`, and a name rule would strand an
  -- ordinary file that merely spells one of those words.
  ["an ordinary file under a (proposed) directory is auto-saved"] = function()
    assert(autosave().should_save("/work/(proposed)/file.lua", "") == true)
  end,

  ["an ordinary file named like a proposed buffer is auto-saved"] = function()
    assert(autosave().should_save("notes (NEW FILE - proposed).md", "") == true)
  end,

  -- ── the write flag lsp-format reads ──

  ["the write flag is up for the write itself"] = function()
    local buf = scratch()
    autosave().mark_write(buf)
    assert(vim.b[buf].autosave_write == true)
  end,

  -- auto-save.nvim raises the flag before its write and drops it after, but a
  -- BufWritePre handler that throws escapes that write and the post event never
  -- runs. A flag left up would silently disable formatting for every later
  -- manual write on the buffer, so raising it also schedules its own clear.
  ["the write flag comes back down when the post event never fires"] = function()
    local buf = scratch()
    autosave().mark_write(buf)
    assert(
      vim.wait(1000, function()
        return vim.b[buf].autosave_write == nil
      end, 10),
      "flag stayed up after the post event was skipped"
    )
  end,

  ["an automatic write is still flagged when no earlier handler yields"] = function()
    assert(flag_seen_by_formatter(file_buffer(), nil) == true)
  end,

  -- A `BufWritePre` handler that yields pumps the event loop, which runs anything
  -- merely scheduled from the pre event. A clear that rides on `vim.schedule`
  -- alone therefore lands BEFORE the format handler reads the flag, and the
  -- automatic write is formatted after all.
  ["an automatic write is still flagged when an earlier handler yields"] = function()
    assert(flag_seen_by_formatter(file_buffer(), function()
      vim.wait(1)
    end) == true)
  end,

  ["clearing the flag of a deleted buffer is not an error"] = function()
    local buf = scratch()
    autosave().mark_write(buf)
    vim.api.nvim_buf_delete(buf, { force = true })
    autosave().clear_write(buf)
  end,
}
