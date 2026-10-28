-- The formatter table and the format-on-save hook in `lua/plugins/conform.lua`,
-- plus the removal of the workaround they replaced.
--
-- conform.nvim is a plugin, absent under `nvim --clean`, so nothing here calls
-- into it: the spec table IS the mapping, and the hook is a plain function of
-- two switches. That a formatter really runs is proved by formatting real files,
-- not from here.
--
-- The expected filetypes are written out rather than read back off the spec, so
-- a row dropped from the plugin file fails this instead of agreeing with itself.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

-- Every filetype a none-ls formatting source covered before the migration, and
-- the formatter that took it over.
local EXPECTED = {
  astro = "prettierd",
  css = "prettierd",
  graphql = "prettierd",
  handlebars = "prettierd",
  html = "prettierd",
  htmlangular = "prettierd",
  javascript = "prettierd",
  javascriptreact = "prettierd",
  json = "prettierd",
  json5 = "prettierd",
  jsonc = "prettierd",
  less = "prettierd",
  lua = "stylua",
  luau = "stylua",
  markdown = "mdformat",
  ["markdown.mdx"] = "prettierd",
  nix = "nixfmt",
  ruby = "rubocop",
  scss = "prettierd",
  sh = "shfmt",
  svelte = "prettierd",
  swift = "swiftformat",
  terraform = "terraform_fmt",
  ["terraform-vars"] = "terraform_fmt",
  tf = "terraform_fmt",
  toml = "taplo",
  typescript = "prettierd",
  typescriptreact = "prettierd",
  vue = "prettierd",
  yaml = "prettierd",
  ["yaml.ansible"] = "ansible-lint",
}

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
---@param autosave_write boolean|nil value of auto-save.nvim's per-buffer flag
local function on_save(autoformat, autosave_write)
  local hook = opts().format_on_save
  assert(type(hook) == "function", "format_on_save is not a function")
  local bufnr = scratch()
  local saved = vim.g.autoformat_on_save
  vim.g.autoformat_on_save = autoformat
  vim.b[bufnr].autosave_write = autosave_write
  local ok, result = pcall(hook, bufnr)
  vim.g.autoformat_on_save = saved
  assert(ok, result)
  return result
end

return {
  ["every filetype the old sources formatted resolves a formatter"] = function()
    local by_ft = assert(opts().formatters_by_ft, "no formatters_by_ft")
    for filetype, formatter in pairs(EXPECTED) do
      local formatters = by_ft[filetype]
      assert(formatters and #formatters > 0, "no formatter for " .. filetype)
      assert(
        vim.tbl_contains(formatters, formatter),
        filetype .. " does not run " .. formatter .. ", it runs " .. table.concat(formatters, ", ")
      )
    end
  end,

  ["swift keeps both of the formatters its two sources ran"] = function()
    assert(
      vim.deep_equal(opts().formatters_by_ft.swift, { "swiftformat", "swiftlint" }),
      "swift does not run swiftformat then swiftlint"
    )
  end,

  ["yaml keeps both of the formatters its two sources ran"] = function()
    assert(
      vim.deep_equal(opts().formatters_by_ft.yaml, { "prettierd", "yamlfmt" }),
      "yaml does not run prettierd then yamlfmt"
    )
  end,

  -- The old per-filetype `exclude` lists kept a language server out of the queue
  -- wherever a dedicated formatter owned the filetype. "fallback" is the same
  -- rule stated once: the server formats only what the table does not name.
  ["a language server formats only a filetype with no formatter of its own"] = function()
    local defaults = assert(opts().default_format_opts, "no default_format_opts")
    assert(defaults.lsp_format == "fallback", "lsp_format is " .. tostring(defaults.lsp_format))
  end,

  ["an explicit write formats"] = function()
    local result = on_save(true, nil)
    assert(type(result) == "table", "an explicit write returned " .. tostring(result))
  end,

  ["a write formats nothing while autoformat-on-save is off"] = function()
    assert(on_save(false, nil) == nil, "formatted with autoformat-on-save off")
  end,

  -- auto-save.nvim writes on a timer, and reformatting a buffer the operator did
  -- not ask to write moves their cursor and their undo history.
  ["an automatic write formats nothing"] = function()
    assert(on_save(true, true) == nil, "formatted an automatic write")
  end,

  ["the deleted formatting workaround is gone"] = function()
    -- Spelled in pieces so this file is not itself a hit.
    local module = config_root .. "/lua/custom_api/lsp" .. "_format.lua"
    assert(vim.fn.filereadable(module) == 0, "the workaround module is still on disk")
  end,
}
