local function report()
  return require("uu.report")
end

return {
  ["a package that fired install:failed is a failure although the completion list names it"] = function()
    local lines, status = report().mason_lines({
      packages = { { name = "formatter", event = "install:failed", reason = "owned compiler refused" } },
      completed = { "formatter" },
    })
    assert(status == report().FAILED and table.concat(lines, "\n"):find("formatter: owned compiler refused", 1, true))
  end,
  ["a package that fired install:success is listed as updated"] = function()
    local lines, status = report().mason_lines({ packages = { { name = "formatter", event = "install:success" } } })
    assert(status == report().OK and lines[1] == "formatter: updated")
  end,
  ["the servers sentence is present on a clean run"] = function()
    local lines, status = report().mason_lines({ packages = { { name = "formatter", event = "current" } } })
    assert(status == report().OK and lines[1] == "formatter: current")
    assert(lines[#lines] == "Language servers are managed by Mason; update them through :Mason.")
  end,
  ["the Mason entry keeps a failed install failed after its completion event"] = function()
    local fixture = dofile(vim.fn.fnamemodify(debug.getinfo(1, "S").source:sub(2), ":h") .. "/uu_fixture.lua")
    local child = fixture.module(
      "mason",
      [[
local handlers = {}
local tool = { name = "formatter", once = function(_, event, fn) handlers[event] = fn end }
package.loaded["mason-registry"] = { once = function() end, get_all_packages = function() return { tool } end }
package.loaded["lazy.core.config"] = { plugins = { ["mason-tool-installer.nvim"] = {} } }
package.loaded["lazy.core.plugin"] = { values = function() return { ensure_installed = { "formatter" } } end }
vim.api.nvim_create_user_command("MasonUpdate", function() end, {})
vim.api.nvim_create_user_command("MasonToolsUpdateSync", function()
  assert(handlers["install:failed"], "missing install failure subscription")("owned installer failed")
  assert(handlers["install:success"], "missing install success subscription")()
  vim.api.nvim_exec_autocmds("User", { pattern = "MasonToolsUpdateCompleted", data = { "formatter" } })
  error("owned command threw after failure event")
end, {})
]]
    )
    assert(
      child.code == 1 and child.stdout:find("formatter: owned installer failed", 1, true),
      child.stdout .. child.stderr
    )
  end,
  ["a failed Mason registry refresh refuses installs and retains the servers sentence"] = function()
    local fixture = dofile(vim.fn.fnamemodify(debug.getinfo(1, "S").source:sub(2), ":h") .. "/uu_fixture.lua")
    local child = fixture.module(
      "mason",
      [[
local failed
package.loaded["mason-registry"] = { once = function(_, event, callback) assert(event == "update:failed"); failed = callback end }
vim.api.nvim_create_user_command("MasonUpdate", function() failed("owned registry refused") end, {})
vim.api.nvim_create_user_command("MasonToolsUpdateSync", function() error("unexpected install") end, {})
]]
    )
    assert(child.code == 1 and child.stdout:find("owned registry refused", 1, true), child.stdout .. child.stderr)
    assert(child.stdout:find("Language servers are managed by Mason", 1, true), child.stdout)
  end,
}
