-- neotest: one test-runner surface over per-language adapters, under the `<leader>t` group.
-- Adapters ship only once proven against a real project; the PR body records which were proven
-- and which were held back, with the reason.
--
-- The per-language adapters below load on their own filetype rather than at startup, and neotest
-- requires whichever are still unloaded when it first runs.
local javascript_filetypes = {
  "javascript",
  "javascriptreact",
  "typescript",
  "typescriptreact",
}

--- The shapes a node:test import takes. `from "node:test"` ends both the one-line and the
--- multiline form of a static import, including a TypeScript `import type`.
local node_test_import_patterns = {
  "require%s*%(%s*['\"]node:test['\"]",
  "import%s*%(?%s*['\"]node:test['\"]",
  "from%s+['\"]node:test['\"]",
}

--- Whether `file_path` imports or requires `node:test`.
---
--- Plain `io` rather than `vim.fn.readfile`: `is_test_file` runs inside neotest's async contexts,
--- where a Vimscript call raises E5560 and a `pcall` around it would read as "no import". The whole
--- file is read rather than a prefix, because a license or generated preamble can push a real
--- import past any cutoff, and a missed import hands the file to a runner that cannot run it.
---
--- Comments come out before the match, so a note that merely names the module does not read as an
--- import. The strip is textual and knows nothing about string literals, so a `//` inside one ends
--- that line early; no import statement survives that shape anyway.
---@param file_path string
---@return boolean
local function imports_node_test(file_path)
  local handle = io.open(file_path, "r")
  if not handle then
    return false
  end
  local source = handle:read("*a") or ""
  handle:close()
  source = source:gsub("/%*.-%*/", "\n"):gsub("//[^\n]*", "")
  for _, pattern in ipairs(node_test_import_patterns) do
    if source:match(pattern) then
      return true
    end
  end
  return false
end

--- The one filename rule the three JavaScript adapters are asked through.
---
--- Their pinned matchers disagree. vitest claims `.e2e.` and node:test and jest claim
--- `.e2e-spec.`, `.unit.`, `.regression.` and `.integration.` instead, and none of the three
--- claims `.mjs` or `.cjs`. Deciding ownership with one adapter's rule and standing down with
--- another's is what leaves a file with no claimant, so the union is asked once, here.
local javascript_test_patterns = {}
for _, kind in ipairs({ "spec", "e2e", "e2e%-spec", "test", "unit", "regression", "integration" }) do
  for _, extension in ipairs({ "js", "jsx", "mjs", "cjs", "coffee", "ts", "tsx", "mts", "cts" }) do
    javascript_test_patterns[#javascript_test_patterns + 1] = "%." .. kind .. "%." .. extension .. "$"
  end
end

---@param file_path string?
---@return boolean
local function is_javascript_test_file(file_path)
  if not file_path then
    return false
  end
  -- Every pinned matcher opens with this one, whatever the extension.
  if file_path:match("__tests__") then
    return true
  end
  for _, pattern in ipairs(javascript_test_patterns) do
    if file_path:match(pattern) then
      return true
    end
  end
  return false
end

--- The runner a decoded package.json names, through its dependency lists or its scripts.
---@param manifest table
---@return "vitest"|"jest"|"node"|nil
local function runner_named_by(manifest)
  local names = {}
  for _, field in ipairs({ "dependencies", "devDependencies" }) do
    if type(manifest[field]) == "table" then
      for name in pairs(manifest[field]) do
        names[#names + 1] = name
      end
    end
  end
  if type(manifest.scripts) == "table" then
    for _, command in pairs(manifest.scripts) do
      names[#names + 1] = tostring(command)
    end
  end

  -- Substring matches, because a runner is named by more than its own package: `@vitest/ui` and
  -- `vitest run` both name vitest, `ts-jest` and `jest --ci` both name jest. "vitest" never
  -- appears inside a jest name, so asking for it first is what settles a package declaring both.
  local declared = table.concat(names, " ")
  if declared:find("vitest", 1, true) then
    return "vitest"
  end
  if declared:find("jest", 1, true) then
    return "jest"
  end
  if declared:find("node:test", 1, true) or declared:find("node --test", 1, true) then
    return "node"
  end
  return nil
end

--- The runner named by the nearest package.json that names one, walking outward from `path`.
---
--- Outward rather than the nearest manifest alone, because a package that names nothing declares
--- nothing: stopping at an empty `fixtures/package.json` loses the vitest its parent declares, and
--- the adapters' own detection does not recover it, since that reads the working directory and the
--- git root rather than the intermediate package. Walking is also what keeps a nested package that
--- DOES name a runner ahead of an ancestor that names another.
---
--- Plain `io` at each level, for the same async reason as the import read above, and it doubles as
--- the test for whether a manifest is there at all.
---@param path string a file or a directory
---@return "vitest"|"jest"|"node"|nil runner
---@return string|nil the path of a manifest that could not be parsed
local function declared_runner(path)
  local directories = { path }
  for parent in vim.fs.parents(path) do
    directories[#directories + 1] = parent
  end
  for _, directory in ipairs(directories) do
    local manifest = directory .. "/package.json"
    local handle = io.open(manifest, "r")
    if handle then
      local contents = handle:read("*a")
      handle:close()
      local ok, decoded = pcall(vim.json.decode, contents)
      if not ok or type(decoded) ~= "table" then
        return nil, manifest
      end
      local runner = runner_named_by(decoded)
      if runner then
        return runner
      end
    end
  end
  return nil
end

--- The adapter that owns `file_path`, or nil when none of the three does.
---
--- ONE answer, which all three predicates are derived from. Ownership and standing down were
--- separate rules before, and any disagreement between them left a file either fought over or,
--- more often, claimed by nobody. The rules, in order:
---
---   * a file that imports node:test is node:test's, whatever the project declares, because
---     node:test attaches to every package.json and no filename rule tells its files from
---     vitest's or jest's;
---   * otherwise the nearest package that names a runner names the owner, so a nested jest
---     package inside a vitest repository keeps its own files;
---   * a package naming both is a migration TO vitest, and its config is the one new test files
---     are written against, so vitest takes it.
---@param file_path string?
---@return "neotest-nodejs"|"neotest-vitest"|"neotest-jest"|nil
local function owner_of(file_path)
  if not is_javascript_test_file(file_path) then
    return nil
  end
  if imports_node_test(file_path) then
    return "neotest-nodejs"
  end
  local runner = declared_runner(file_path)
  if runner == "vitest" then
    return "neotest-vitest"
  end
  if runner == "jest" then
    return "neotest-jest"
  end
  return nil
end

--- The JavaScript adapters, each under the runner a package.json declares for it.
local javascript_adapters = {
  ["neotest-vitest"] = "vitest",
  ["neotest-jest"] = "jest",
  ["neotest-nodejs"] = "node",
}

--- Run everything under the working directory through ONE named adapter.
---
--- neotest takes a directory without asking any adapter's `is_test_file`, then walks its adapter
--- map with `pairs`, so leaving the choice to it makes a package with two attached adapters run a
--- different one between presses. An adapter id is `<name>:<root>` and `run.run` takes one
--- verbatim, so naming it is what settles the run. The nearest declaring package.json names it;
--- with nothing declared and more than one adapter attached the choice is the operator's, because
--- a wrong silent pick reads as a runner that lost its tests.
local function run_all_tests()
  local neotest = require("neotest")
  local directory = vim.fn.getcwd()

  -- A manifest nobody can parse is not the same answer as a manifest naming no runner. Saying so
  -- and stopping beats a chooser built on a file that could not be read. `owner_of` treats the
  -- same result as "no runner" without a word, because it is asked once per file during discovery
  -- and would say it hundreds of times.
  local runner, unparseable = declared_runner(directory)
  if unparseable then
    vim.notify("neotest: could not parse " .. unparseable, vim.log.levels.ERROR)
    return
  end

  -- Asked of each adapter rather than resolved here, so the id matches the one neotest builds
  -- from the same call.
  local javascript, other = {}, {}
  for _, adapter in ipairs(require("neotest.config").adapters) do
    local root = adapter.root(directory)
    if root then
      local entry = { name = adapter.name, id = ("%s:%s"):format(adapter.name, root) }
      table.insert(javascript_adapters[adapter.name] and javascript or other, entry)
    end
  end

  for _, entry in ipairs(javascript) do
    if javascript_adapters[entry.name] == runner then
      return neotest.run.run({ directory, adapter = entry.id })
    end
  end
  if #javascript == 1 then
    return neotest.run.run({ directory, adapter = javascript[1].id })
  end
  if #javascript == 0 then
    return neotest.run.run(directory)
  end

  vim.ui.select(javascript, {
    prompt = "Neotest: run all tests with",
    format_item = function(entry)
      return entry.name
    end,
  }, function(choice)
    if choice then
      neotest.run.run({ directory, adapter = choice.id })
    end
  end)
end

return {
  {
    "nvim-neotest/neotest",
    commit = "27bf921498043f7ecd821d6db68d05de244bbd02",
    dependencies = {
      -- The busted and Swift adapters ship a rockspec listing nvim-nio, and lazy.nvim turns a
      -- rockspec dependency into a top-level spec, which would make nvim-nio load at startup.
      { "nvim-neotest/nvim-nio", lazy = true },
      { "nvim-neotest/neotest-python", commit = "1b56ca4ba51c6014f986d6548ee629bdc95589d1" },
      { "fredrikaverpil/neotest-golang", commit = "65b2be63c3de00e6f15c05388ebdc8d603dbd727" },
    },
    -- stylua: ignore start
    keys = {
      { "<leader>tt", function() require("neotest").run.run() end, desc = "Neotest: run nearest test" },
      { "<leader>tf", function() require("neotest").run.run(vim.fn.expand("%")) end, desc = "Neotest: run file" },
      { "<leader>ta", run_all_tests, desc = "Neotest: run all tests" },
      { "<leader>ts", function() require("neotest").summary.toggle() end, desc = "Neotest: toggle summary" },
      { "<leader>to", function() require("neotest").output.open({ enter = true }) end, desc = "Neotest: open output" },
      { "<leader>tS", function() require("neotest").run.stop() end, desc = "Neotest: stop run" },
    },
    -- stylua: ignore end
    config = function()
      -- Ownership is one answer, `owner_of`, and each adapter is asked for its own name. Nothing
      -- else decides: a file cannot be a test file for the adapter standing down and not for the
      -- one that would take it, which is how files ended up with no claimant at all.
      --
      -- vitest is copied rather than constructed. Its `__call` puts its own dependency gate AHEAD
      -- of the predicate the caller supplies, and that gate cannot see a runner declared by an
      -- intermediate package, so constructing it would veto the answer before it was asked. jest
      -- and node:test both hand their `is_test_file` straight to the configured predicate, so they
      -- take the supported route.
      local vitest = vim.tbl_extend("force", require("neotest-vitest"), {
        is_test_file = function(file_path)
          return owner_of(file_path) == "neotest-vitest"
        end,
      })
      local jest = require("neotest-jest")({
        isTestFile = function(file_path)
          return owner_of(file_path) == "neotest-jest"
        end,
      })
      local node = require("neotest-nodejs")({
        isTestFile = function(file_path)
          return owner_of(file_path) == "neotest-nodejs"
        end,
      })

      -- Construction audit at these pins. Only neotest-golang REQUIRES the call: its
      -- `M.Adapter.options` is assigned inside `__call` alone (init.lua:241) and read by
      -- `filter_dir` (init.lua:49), so the bare module raises on any Go module with a
      -- subdirectory. The rest carry their options without it: neotest-python IS a constructed
      -- adapter at load (init.lua:70), vitest and jest assign their defaults at load and let
      -- `__call` override only what the caller supplies, busted's config module starts at its
      -- own defaults (config.lua:17), and the Swift adapter's `__call` only sets a log level.
      require("neotest").setup({
        adapters = {
          require("neotest-python"),
          require("neotest-golang")({}),
          vitest,
          jest,
          node,
          require("neotest-busted"),
          require("neotest-swift-testing"),
        },
      })
    end,
  },
  { "marilari88/neotest-vitest", commit = "c3c69715da4b158069fd4262083e7219a5c14cfb", ft = javascript_filetypes },
  { "nvim-neotest/neotest-jest", commit = "0e7979d51301dfae5ef839d771bd28cf593fde3f", ft = javascript_filetypes },
  { "AkisArou/neotest-nodejs", commit = "68e558dff61f7ac630f55ab63a092c1767965386", ft = javascript_filetypes },
  { "MisanthropicBit/neotest-busted", commit = "9efddcee53d255cef8937541808eccd464772f80", ft = "lua" },
  {
    -- Codeberg, not GitHub: the GitHub repository has been an archived redirect since 2026-04-28.
    url = "https://codeberg.org/mmllr/neotest-swift-testing",
    commit = "5b2d7efea43cb0d66d97de65b9ebc7b1db4659fd",
    ft = "swift",
  },
}
