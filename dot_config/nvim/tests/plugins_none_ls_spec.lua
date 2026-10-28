-- What none-ls is still allowed to register, from `lua/plugins/lsp.lua`.
--
-- It stays installed for its two CODE-ACTION sources, which neither conform.nvim
-- nor nvim-lint replaces, and for nothing else. A formatter put back here would
-- compete with conform for the buffer; a diagnostic would publish into the
-- shared LSP namespace instead of nvim-lint's own. So the interesting assertion
-- is negative, and the double is built to make it: every builtin category is a
-- recorder, so the config cannot read one without this spec seeing it.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

---Load the none-ls spec against a recording `null-ls` double, run its `config`,
---and hand back every builtin it asked for plus the sources it registered.
local function registered()
  local asked, sources = {}, nil

  local function category(name)
    return setmetatable({}, {
      __index = function(_, builtin)
        local reference = name .. "." .. builtin
        table.insert(asked, reference)
        return reference
      end,
    })
  end

  local double = {
    builtins = {
      code_actions = category("code_actions"),
      completion = category("completion"),
      diagnostics = category("diagnostics"),
      formatting = category("formatting"),
    },
    setup = function(config)
      sources = config.sources
    end,
  }

  local saved = package.loaded["null-ls"]
  package.loaded["null-ls"] = double

  local spec
  for _, candidate in ipairs(dofile(config_root .. "/lua/plugins/lsp.lua")) do
    if candidate[1] == "nvimtools/none-ls.nvim" then
      spec = candidate
    end
  end
  assert(spec, "no nvimtools/none-ls.nvim spec in lua/plugins/lsp.lua")

  local ok, err = pcall(assert(spec.config, "the none-ls spec has no `config`"), spec, spec.opts)
  package.loaded["null-ls"] = saved
  assert(ok, err)

  return asked, assert(sources, "none-ls was set up without a `sources` list")
end

return {
  ["none-ls registers the two code-action sources"] = function()
    local _, sources = registered()
    table.sort(sources)
    assert(
      vim.deep_equal(sources, { "code_actions.gitsigns", "code_actions.refactoring" }),
      "registered " .. table.concat(sources, ", ")
    )
  end,

  ["none-ls registers no formatter and no diagnostic"] = function()
    local asked = registered()
    for _, reference in ipairs(asked) do
      assert(reference:match("^code_actions%."), "the none-ls config still reaches for " .. reference)
    end
  end,
}
