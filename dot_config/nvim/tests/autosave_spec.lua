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

  ["clearing the flag of a deleted buffer is not an error"] = function()
    local buf = scratch()
    autosave().mark_write(buf)
    vim.api.nvim_buf_delete(buf, { force = true })
    autosave().clear_write(buf)
  end,
}
