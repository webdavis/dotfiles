-- nvim-lint is absent under `nvim --clean`, so its module is faked.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

---Run the nvim-lint config against a fake `lint` module.
local function configured()
  local fake = { linters_by_ft = {}, linters = { luacheck = { args = {} } }, calls = {} }
  fake.try_lint = function(names, opts)
    table.insert(fake.calls, { names = names, opts = opts })
  end

  local saved = package.loaded["lint"]
  local saved_parser = package.loaded["lint.parser"]
  package.loaded["lint"] = fake
  package.loaded["lint.parser"] = {
    for_sarif = function()
      return "sarif-parser"
    end,
  }

  local spec
  for _, candidate in ipairs(dofile(config_root .. "/lua/plugins/nvim-lint.lua")) do
    if candidate[1] == "mfussenegger/nvim-lint" then
      spec = candidate
    end
  end
  assert(spec, "no mfussenegger/nvim-lint spec in lua/plugins/nvim-lint.lua")

  local ok, err = pcall(assert(spec.config, "the nvim-lint spec has no `config`"), spec, spec.opts)
  package.loaded["lint"] = saved
  package.loaded["lint.parser"] = saved_parser
  assert(ok, err)

  return fake
end

local function run_event(fake, event)
  for _, autocmd in ipairs(vim.api.nvim_get_autocmds({ group = "NvimLintGroup", event = event })) do
    if autocmd.callback then
      autocmd.callback({ event = event })
    end
  end
  return assert(fake.calls[#fake.calls], event .. " ran no linter")
end

return {
  ["only a write runs the disk-backed EmmyLua checker"] = function()
    local fake = configured()
    local insert = run_event(fake, "InsertLeave")
    assert(not insert.opts.filter({ name = "emmylua_check" }), "InsertLeave ran emmylua_check on stale disk content")
    assert(insert.opts.filter({ name = "luacheck" }), "InsertLeave stopped the stdin linter")

    local write = run_event(fake, "BufWritePost")
    assert(write.opts.filter({ name = "emmylua_check" }), "BufWritePost did not run emmylua_check")
  end,

  ["linting is triggered by a write"] = function()
    local fake = configured()
    local events = {}
    for _, autocmd in ipairs(vim.api.nvim_get_autocmds({ group = "NvimLintGroup" })) do
      events[autocmd.event] = true
      if autocmd.callback then
        autocmd.callback({ event = autocmd.event })
      end
    end
    assert(events.BufWritePost, "nothing lints on a write")
    assert(#fake.calls > 0, "the autocmd ran no linter")
  end,
}
