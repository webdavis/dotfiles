-- The linter table in `lua/plugins/nvim-lint.lua`.
--
-- nvim-lint is a plugin, absent under `nvim --clean`, so its module is faked:
-- the spec's `config` writes `linters_by_ft` onto it and registers one autocmd,
-- and both are what this reads back. Whether a linter really reports is proved
-- by linting a file with a deliberate error, not from here.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

-- Every filetype a none-ls diagnostic source covered before the migration, and
-- the nvim-lint linter that took it over. `dotenv_linter` and `eslint` are
-- absent on purpose: the first was registered for `sh` and then disabled for
-- `sh`, so it ran on nothing, and the second duplicated the eslint language
-- server.
local EXPECTED = {
  dockerfile = "hadolint",
  yaml = "actionlint",
  ["yaml.ansible"] = "ansible_lint",
}

---Load the nvim-lint spec against a faked `lint` module, run its `config`, and
---hand back the fake plus the autocmds the config registered.
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
  ["every filetype the old sources diagnosed resolves a linter"] = function()
    local by_ft = configured().linters_by_ft
    for filetype, linter in pairs(EXPECTED) do
      local linters = by_ft[filetype]
      assert(linters and #linters > 0, "no linter for " .. filetype)
      assert(
        vim.tbl_contains(linters, linter),
        filetype .. " does not run " .. linter .. ", it runs " .. table.concat(linters, ", ")
      )
    end
  end,

  -- none-ls said this with `disabled_filetypes`; nvim-lint matches a filetype
  -- exactly, so the compound filetype having its own row is what keeps
  -- actionlint away from an Ansible playbook.
  ["an Ansible playbook is not handed to actionlint"] = function()
    local ansible = configured().linters_by_ft["yaml.ansible"]
    assert(not vim.tbl_contains(ansible, "actionlint"), "actionlint runs on yaml.ansible")
  end,

  ["Lua runs luacheck and EmmyLua together"] = function()
    local fake = configured()
    local lua = fake.linters_by_ft.lua
    assert(vim.deep_equal(lua, { "luacheck", "emmylua_check" }), "Lua linters are " .. vim.inspect(lua))
    assert(vim.tbl_contains(fake.linters.luacheck.args, "--config"), "luacheck has no explicit config")
    assert(
      vim.tbl_contains(fake.linters.luacheck.args, vim.fn.stdpath("config") .. "/.luacheckrc"),
      "luacheck does not use the deployed treefmt config"
    )
  end,

  ["EmmyLua reads the written file and publishes SARIF diagnostics"] = function()
    local linter = assert(configured().linters.emmylua_check, "no emmylua_check linter")
    assert(linter.cmd == "emmylua_check", "command is " .. tostring(linter.cmd))
    assert(linter.stdin == false, "emmylua_check must read the file on disk")
    assert(linter.ignore_exitcode == true, "diagnostics must not become a runner warning")
    assert(linter.parser == "sarif-parser", "emmylua_check does not use nvim-lint's SARIF parser")
    assert(vim.tbl_contains(linter.args, "--severity"), "severity floor is missing")
    assert(vim.tbl_contains(linter.args, "error"), "EmmyLua is not limited to gate errors")
  end,

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
