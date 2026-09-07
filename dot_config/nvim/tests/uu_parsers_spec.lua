local function report()
  return require("uu.report")
end

return {
  ["a false wait result is a failure"] = function()
    local lines, status = report().parser_lines({ ok = false, updated = {}, current = {}, errors = {} })
    assert(status == report().FAILED and lines[1] == "parser update failed")
  end,
  ["a true result lists updated and current parsers"] = function()
    local lines, status = report().parser_lines({ ok = true, updated = { "lua" }, current = { "rust" }, errors = {} })
    assert(status == report().OK and vim.deep_equal(lines, { "lua: parser updated", "rust: parser current" }))
  end,
  ["the parsers entry waits and reports a normal false result with its compiler tail"] = function()
    local fixture = dofile(vim.fn.fnamemodify(debug.getinfo(1, "S").source:sub(2), ":h") .. "/uu_fixture.lua")
    local child = fixture.module(
      "parsers",
      [[
package.loaded["nvim-treesitter.config"] = { get_installed = function() return { "fixture" } end }
local waited = false
package.loaded["nvim-treesitter.install"] = { update = function(languages, options)
  assert(languages == nil and options.summary == true)
  return { wait = function() waited = true; return false end }
end }
package.loaded["nvim-treesitter.log"] = { show = function()
  assert(waited, "parser wait was omitted")
  vim.api.nvim_echo({ { "error(install/fixture): owned compiler tail" } }, false, {})
end }
]]
    )
    assert(child.code == 1 and child.stdout:find("owned compiler tail", 1, true), child.stdout .. child.stderr)
  end,
  ["the parsers entry attributes installer log contexts to their parser names"] = function()
    local fixture = dofile(vim.fn.fnamemodify(debug.getinfo(1, "S").source:sub(2), ":h") .. "/uu_fixture.lua")
    local child = fixture.module(
      "parsers",
      [[
package.loaded["nvim-treesitter.config"] = { get_installed = function() return { "fixture", "current" } end }
package.loaded["nvim-treesitter.install"] = { update = function() return { wait = function() return true end } end }
package.loaded["nvim-treesitter.log"] = { show = function() vim.api.nvim_echo({ { "info(install/fixture): Language installed" } }, false, {}) end }
]]
    )
    local expected = "fixture: parser updated\ncurrent: parser current\n"
    assert(child.code == 0 and child.stdout:sub(1, #expected) == expected, child.stdout .. child.stderr)
    assert(not child.stdout:find("fixture: parser current", 1, true), child.stdout)
  end,
}
