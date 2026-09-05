-- custom_api.autosave (spec 5.3, item 42): auto-save.nvim keeps its hands off
-- the buffers claudecode.nvim opens for a proposed edit, so a diff is resolved
-- by `<leader>Cy`/`<leader>Cn` and never by a write that fired on a timer, and
-- an automatic write announces itself so lsp-format can stand down.
--
-- `should_save` identifies a proposal by the marker claudecode sets on it, not
-- by buftype: `acwrite` is a shared buftype that Octo and gitsigns also use for
-- buffers whose writes are real work, so rejecting it would disable auto-save
-- well outside claudecode.

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

  -- claudecode marks every proposal buffer it opens, on both the native and the
  -- unified diff paths (`diff.lua:710`, `diff_inline.lua:287` at the pin).
  ["a claudecode proposal buffer is not auto-saved"] = function()
    local buf = scratch()
    vim.b[buf].claudecode_diff_tab_name = "lsp.lua (proposed)"
    assert(autosave().should_save(buf) == false)
  end,

  -- Octo gives every issue, pull request and discussion buffer `acwrite`, and
  -- writing one pushes the edit to GitHub; gitsigns uses it for the editable
  -- index diff. Neither carries claudecode's marker, and both keep auto-save.
  ["an acwrite buffer without the marker is auto-saved"] = function()
    local buf = scratch()
    vim.bo[buf].buftype = "acwrite"
    vim.bo[buf].filetype = "octo"
    assert(autosave().should_save(buf) == true)
  end,

  ["an ordinary file buffer is auto-saved"] = function()
    assert(autosave().should_save(scratch()) == true)
  end,

  ["an ordinary file whose name looks like a proposal is auto-saved"] = function()
    local buf = scratch()
    vim.api.nvim_buf_set_name(buf, "/work/(proposed)/notes (NEW FILE - proposed).md")
    assert(autosave().should_save(buf) == true)
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

  -- The two hazards together, which is the combination nothing else covers: the
  -- flag is raised from inside an autocmd (so the scheduled clear sees an
  -- executing autocommand and defers), an earlier handler yields (so that
  -- deferral is actually taken), and a later one throws (so auto-save's post
  -- event never arrives to clear it). `SafeState` is then the only thing left
  -- that can drop the flag, and it has to.
  --
  -- The event is raised here rather than waited for: under `nvim -l` the editor
  -- never returns to its main loop, so Neovim itself never emits `SafeState`.
  -- What belongs to this config is the one-shot handler and what it does, and
  -- that is what driving the event exercises.
  ["a write that yields and then throws drops the flag once SafeState arrives"] = function()
    local buf = file_buffer()
    local group = vim.api.nvim_create_augroup("AutosaveSpecSafeState", { clear = true })
    vim.api.nvim_create_autocmd("BufWritePre", {
      group = group,
      pattern = "*",
      callback = function()
        vim.wait(1)
      end,
    })
    vim.api.nvim_create_autocmd("BufWritePre", { group = group, pattern = "*", command = "throw 'spec'" })
    vim.api.nvim_create_autocmd("User", {
      group = group,
      pattern = "AutosaveSpecMark",
      callback = function()
        autosave().mark_write(buf)
      end,
    })

    vim.api.nvim_buf_call(buf, function()
      vim.api.nvim_exec_autocmds("User", { pattern = "AutosaveSpecMark" })
      vim.api.nvim_buf_set_lines(buf, 0, -1, false, { "changed" })
      pcall(function()
        vim.cmd("silent! write!")
      end)
    end)
    vim.api.nvim_del_augroup_by_id(group)

    assert(vim.b[buf].autosave_write == true, "the flag was dropped during the write, before the format handler")
    vim.api.nvim_exec_autocmds("SafeState", {})
    assert(vim.b[buf].autosave_write == nil, "the flag was never dropped, so formatting stays off for this buffer")
  end,

  ["clearing the flag of a deleted buffer is not an error"] = function()
    local buf = scratch()
    autosave().mark_write(buf)
    vim.api.nvim_buf_delete(buf, { force = true })
    autosave().clear_write(buf)
  end,
}
