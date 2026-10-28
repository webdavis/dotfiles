-- Required per case, so a missing module fails each case by name.
local function autosave()
  return require("custom_api.autosave")
end

local function scratch()
  return vim.api.nvim_create_buf(false, true)
end

-- A real file buffer, because `BufWritePre` runs only on an actual write.
local function file_buffer()
  local dir = vim.fn.tempname()
  assert(vim.fn.mkdir(dir, "p") == 1, "could not create " .. dir)
  local buf = vim.api.nvim_create_buf(false, false)
  vim.api.nvim_buf_set_name(buf, dir .. "/written.txt")
  return buf
end

-- Runs one automatic write and reports what a format handler after `earlier` saw on the flag.
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

  -- claudecode marks every proposal buffer, on both diff paths.
  ["a claudecode proposal buffer is not auto-saved"] = function()
    local buf = scratch()
    vim.b[buf].claudecode_diff_tab_name = "lsp.lua (proposed)"
    assert(autosave().should_save(buf) == false)
  end,

  -- Octo and gitsigns use `acwrite` for buffers whose writes do real work.
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

  ["the write flag is up for the write itself"] = function()
    local buf = scratch()
    autosave().mark_write(buf)
    assert(vim.b[buf].autosave_write == true)
  end,

  -- A flag left up would skip formatting on every later manual write.
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

  -- A yielding handler runs scheduled callbacks before the format handler reads the flag.
  ["an automatic write is still flagged when an earlier handler yields"] = function()
    assert(flag_seen_by_formatter(file_buffer(), function()
      vim.wait(1)
    end) == true)
  end,

  -- Raises SafeState by hand, because under `nvim -l` Neovim never returns to its main loop.
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
