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

--- The test runner the package.json NEAREST `path` declares, read from its dependency lists and
--- its scripts, or nil when it names none.
---
--- Deliberately the nearest manifest alone, unlike the adapters' own detection, which also reads
--- the working directory and the git root. That difference is the whole point of this: it is what
--- lets a nested package that declares jest keep its own files inside a vitest repository instead
--- of losing them to an ancestor's choice.
---@param path string a file or a directory
---@return "vitest"|"jest"|"node"|nil
local function nearest_declared_runner(path)
  local root = vim.fs.root(path, "package.json")
  if not root then
    return nil
  end
  -- Plain `io`, for the same async reason as above.
  local handle = io.open(root .. "/package.json", "r")
  if not handle then
    return nil
  end
  local contents = handle:read("*a")
  handle:close()
  local ok, manifest = pcall(vim.json.decode, contents)
  if not ok or type(manifest) ~= "table" then
    return nil
  end

  local declared = {}
  for _, field in ipairs({ "dependencies", "devDependencies" }) do
    if type(manifest[field]) == "table" then
      for name in pairs(manifest[field]) do
        declared[#declared + 1] = name
      end
    end
  end
  if type(manifest.scripts) == "table" then
    for _, command in pairs(manifest.scripts) do
      declared[#declared + 1] = tostring(command)
    end
  end

  -- Substring matches, because a runner is named by more than its own package: `@vitest/ui` and
  -- `vitest run` both name vitest, `ts-jest` and `jest --ci` both name jest. "vitest" never
  -- appears inside a jest name, so asking for it first is what settles a package declaring both.
  local names = table.concat(declared, " ")
  if names:find("vitest", 1, true) then
    return "vitest"
  end
  if names:find("jest", 1, true) then
    return "jest"
  end
  if names:find("node:test", 1, true) or names:find("node --test", 1, true) then
    return "node"
  end
  return nil
end

--- The adapter each declared JavaScript runner belongs to.
local javascript_adapters = {
  vitest = "neotest-vitest",
  jest = "neotest-jest",
  node = "neotest-nodejs",
}

--- Run everything under the working directory through ONE named adapter.
---
--- neotest takes a directory without asking any adapter's `is_test_file`, then walks its adapter
--- map with `pairs`, so leaving the choice to it makes a package with two attached adapters run a
--- different one between presses. An adapter id is `<name>:<root>` and `run.run` takes one
--- verbatim, so naming it is what settles the run. The nearest package.json names it; with
--- nothing declared and more than one adapter attached the choice is the operator's, because a
--- wrong silent pick reads as a runner that lost its tests.
local function run_all_tests()
  local neotest = require("neotest")
  local directory = vim.fn.getcwd()

  -- Asked of each adapter rather than resolved here, so the id matches the one neotest builds
  -- from the same call.
  local attached = {}
  for _, adapter in ipairs(require("neotest.config").adapters) do
    local root = adapter.root(directory)
    if root then
      attached[#attached + 1] = { name = adapter.name, id = ("%s:%s"):format(adapter.name, root) }
    end
  end
  if #attached == 0 then
    return neotest.run.run(directory)
  end

  local declared = nearest_declared_runner(directory)
  local wanted = declared and javascript_adapters[declared]
  for _, entry in ipairs(attached) do
    if entry.name == wanted then
      return neotest.run.run({ directory, adapter = entry.id })
    end
  end
  if #attached == 1 then
    return neotest.run.run({ directory, adapter = attached[1].id })
  end

  vim.ui.select(attached, {
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
      -- Three adapters claim JavaScript and neotest walks its adapter map with `pairs`, so
      -- exactly one of them has to answer yes for any given file. The rules, in order:
      --
      --   * a file that imports node:test is node:test's, whatever the project declares, because
      --     node:test attaches to every package.json and no filename rule distinguishes its files
      --     from vitest's or jest's;
      --   * a package that declares jest and not vitest keeps its own files, so a nested jest
      --     package inside a vitest repository is not swallowed by the ancestor;
      --   * otherwise vitest wins over jest, because a repository carrying both runners is a
      --     migration TO vitest and its config is the one new test files are written against.
      --
      -- Only vitest consults the nearest manifest. jest already stands down wherever vitest
      -- claims, so narrowing vitest hands the nested package back to jest on its own, and the
      -- precedence lives in one place rather than two that can disagree.
      --
      -- Each adapter is asked through ITS OWN default predicate first. That answer carries the
      -- adapter's real dependency detection, which reads the working directory and the git root as
      -- well as the nearest manifest, and it is also what makes a nil path safe: all three defaults
      -- answer false for one, so the helpers above never see nil.
      local vitest = require("neotest-vitest")
      local vitest_claims = vitest.is_test_file
      local jest_claims = require("neotest-jest.jest-util").defaultIsTestFile
      local node_claims = require("neotest-nodejs.node-util").defaultIsTestFile

      -- vitest ASSIGNS a new closure over this predicate rather than rebinding an upvalue, so the
      -- default captured above stays callable and composing against it does not recurse.
      vitest({
        is_test_file = function(file_path)
          return vitest_claims(file_path)
            and not imports_node_test(file_path)
            and nearest_declared_runner(file_path) ~= "jest"
        end,
      })
      local jest = require("neotest-jest")({
        isTestFile = function(file_path)
          return jest_claims(file_path) and not imports_node_test(file_path) and not vitest.is_test_file(file_path)
        end,
      })
      local node = require("neotest-nodejs")({
        isTestFile = function(file_path)
          return node_claims(file_path) and imports_node_test(file_path)
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
