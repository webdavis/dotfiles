local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

local function conform_spec()
  for _, spec in ipairs(dofile(config_root .. "/lua/plugins/conform.lua")) do
    if spec[1] == "stevearc/conform.nvim" then
      return spec
    end
  end
  error("no stevearc/conform.nvim spec in lua/plugins/conform.lua")
end

local function opts()
  return assert(conform_spec().opts, "the conform.nvim spec has no `opts`")
end

local function scratch()
  return vim.api.nvim_create_buf(false, true)
end

---Run the spec's `format_on_save` hook with both switches set.
---@param autoformat boolean value of the global autoformat toggle
---@param save_kind string|nil the kind of save pantry's autosave is making, if any
local function on_save(autoformat, save_kind)
  local hook = opts().format_on_save
  assert(type(hook) == "function", "format_on_save is not a function")
  local bufnr = scratch()
  local saved = vim.g.autoformat_on_save
  vim.g.autoformat_on_save = autoformat
  vim.b[bufnr].pantry_save = save_kind
  local ok, result = pcall(hook, bufnr)
  vim.g.autoformat_on_save = saved
  assert(ok, result)
  return result
end

---Whether a Markdown formatter is offered for a file, given the context conform passes it.
---conform names its temp copy in `filename`, so only the buffer still carries the real name.
---@param formatter string name of a formatter the Markdown row lists
---@param path string path of the file being formatted
local function formats(formatter, path)
  local condition = (opts().formatters[formatter] or {}).condition
  if condition == nil then
    return true
  end
  local bufnr = scratch()
  vim.api.nvim_buf_set_name(bufnr, path)
  local temp_copy = vim.fs.dirname(path) .. "/.conform.1234567." .. vim.fs.basename(path)
  local offered = condition(nil, { buf = bufnr, filename = temp_copy })
  vim.api.nvim_buf_delete(bufnr, { force = true })
  return offered
end

return {
  ["an explicit write formats"] = function()
    local result = on_save(true, nil)
    assert(type(result) == "table", "an explicit write returned " .. tostring(result))
  end,

  ["a write formats nothing while autoformat-on-save is off"] = function()
    assert(on_save(false, nil) == nil, "formatted with autoformat-on-save off")
  end,

  -- A pause save happens while the work goes on, and formatting then would move the cursor.
  ["a pause save formats nothing"] = function()
    assert(on_save(true, "pause") == nil, "formatted a pause save")
  end,

  ["a leave save formats"] = function()
    assert(type(on_save(true, "leave")) == "table", "did not format a leave save")
  end,

  -- Wrapping would split the one-rule-per-line files, and nothing joins the lines again.
  ["no Markdown formatter touches a rule file"] = function()
    for _, formatter in ipairs(opts().formatters_by_ft.markdown) do
      for _, name in ipairs({ "AGENTS.md", "TASKS.md" }) do
        assert(not formats(formatter, "/repo/" .. name), formatter .. " formats " .. name)
      end
    end
  end,

  ["every Markdown formatter still formats other Markdown files"] = function()
    for _, formatter in ipairs(opts().formatters_by_ft.markdown) do
      assert(formats(formatter, "/repo/README.md"), formatter .. " skips README.md")
      assert(formats(formatter, "/repo/docs/MY_TASKS.md"), formatter .. " skips MY_TASKS.md")
    end
  end,
}
