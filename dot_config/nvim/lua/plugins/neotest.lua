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

--- The tree-sitter language a JavaScript-family file is parsed with, or nil when the routing has
--- nothing to parse it as.
---
--- `.tsx` is read as `typescript`. The `tsx` grammar is not among the ones this config installs,
--- and the typescript one still parses the import block of a `.tsx` file; only the JSX below it
--- becomes an error node, which nothing here reads. `.coffee` has no grammar at all, so a
--- CoffeeScript test file can never be node:test's.
local javascript_languages = {
  js = "javascript",
  jsx = "javascript",
  mjs = "javascript",
  cjs = "javascript",
  ts = "typescript",
  mts = "typescript",
  cts = "typescript",
  tsx = "typescript",
}

--- The four shapes that make node:test a runtime dependency: a static import, a re-export, a
--- `require` call and a dynamic `import` call.
---
--- Asked of the parse tree rather than of the text. A regular expression, a template substitution,
--- a literal nested inside one, a comment and an ordinary string are each their own node type, so
--- none of them can be read as an import, and an import written after any of them is still found.
local node_test_query_text = [[
  (import_statement source: (string (string_fragment) @specifier)) @declaration
  (export_statement source: (string (string_fragment) @specifier)) @declaration
  (call_expression
    function: (identifier) @callee
    arguments: (arguments (string (string_fragment) @specifier))
    (#eq? @callee "require"))
  (call_expression
    function: (import)
    arguments: (arguments (string (string_fragment) @specifier)))
]]

--- The compiled query for `language`, or nil when its grammar is not installed. Memoized, because
--- compiling is what makes the FIRST call expensive and the routing runs per file.
local compiled_queries = {}
local function node_test_query(language)
  if compiled_queries[language] == nil then
    local ok, query = pcall(vim.treesitter.query.parse, language, node_test_query_text)
    compiled_queries[language] = ok and query or false
  end
  return compiled_queries[language] or nil
end

--- Whether an import or re-export is type-only. TypeScript erases those before anything runs, so
--- they name the module at compile time and say nothing about the runner the file uses.
---@param declaration TSNode?
---@return boolean
local function is_type_only(declaration)
  if not declaration then
    return false
  end
  for child in declaration:iter_children() do
    if child:type() == "type" then
      return true
    end
  end
  return false
end

--- Whether `file_path` imports or requires `node:test`.
---
--- Plain `io` rather than `vim.fn.readfile`: `is_test_file` runs inside neotest's async contexts,
--- where a Vimscript call raises E5560 and a `pcall` around it would read as "no import".
---@param file_path string
---@return boolean
local function imports_node_test(file_path)
  local language = javascript_languages[file_path:match("%.([^.]+)$") or ""]
  local query = language and node_test_query(language)
  if not query then
    return false
  end
  local handle = io.open(file_path, "r")
  if not handle then
    return false
  end
  local source = handle:read("*a") or ""
  handle:close()

  local parsed, parser = pcall(vim.treesitter.get_string_parser, source, language)
  if not parsed then
    return false
  end
  for _, match in query:iter_matches(parser:parse()[1]:root(), source) do
    local specifier, declaration
    for id, nodes in pairs(match) do
      local node = type(nodes) == "table" and nodes[1] or nodes
      if query.captures[id] == "specifier" then
        specifier = node
      elseif query.captures[id] == "declaration" then
        declaration = node
      end
    end
    if
      specifier
      and vim.treesitter.get_node_text(specifier, source) == "node:test"
      and not is_type_only(declaration)
    then
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

--- The decoded package.json at `manifest`, or nil when there is none. The second value is true
--- when the file is there and no JSON parser can read it, which is not the same answer as absent.
---
--- Plain `io`, because `is_test_file` runs inside neotest's async contexts and inside `vim.uv`
--- callbacks, where a Vimscript call raises E5560. Opening the file doubles as the test for
--- whether a manifest is there at all.
---@param manifest string
---@return table|nil, boolean
local function read_manifest(manifest)
  local handle = io.open(manifest, "r")
  if not handle then
    return nil, false
  end
  local contents = handle:read("*a")
  handle:close()
  local ok, decoded = pcall(vim.json.decode, contents)
  if not ok or type(decoded) ~= "table" then
    return nil, true
  end
  return decoded, false
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
--- The walk stops at the repository, because a package.json above it belongs to someone else: a
--- nested repository that declares no runner would otherwise run its tests under whatever an
--- unrelated ancestor happens to declare. `.git` is matched by name, so a worktree or submodule,
--- where it is a FILE rather than a directory, bounds the walk the same way.
---
---@param path string a file or a directory
---@return "vitest"|"jest"|"node"|nil runner
---@return string|nil the path of a manifest that could not be parsed
local function declared_runner(path)
  local directories = { path }
  for parent in vim.fs.parents(path) do
    directories[#directories + 1] = parent
  end
  local repository = vim.fs.root(path, ".git")
  for _, directory in ipairs(directories) do
    local manifest = directory .. "/package.json"
    local decoded, unparseable = read_manifest(manifest)
    if unparseable then
      return nil, manifest
    end
    if decoded then
      local runner = runner_named_by(decoded)
      if runner then
        return runner
      end
    end
    if directory == repository then
      break
    end
  end
  return nil
end

--- The packages under `directory` that name a runner other than `runner`, relative to it.
---
--- Only asked when `directory` itself names one, so an ordinary monorepo whose root names nothing
--- is not in conflict with the packages inside it.
---
--- Depth four reaches `packages/<name>/package.json` and one level below, which is where a
--- monorepo puts them. A package buried deeper goes unseen and the run proceeds, which is the
--- behaviour this replaces rather than a new way to be wrong.
---@param directory string
---@param runner string
---@return string[]
local function packages_declaring_other_runners(directory, runner)
  local conflicting = {}
  for name, kind in
    vim.fs.dir(directory, {
      depth = 4,
      skip = function(dirname)
        return dirname ~= "node_modules" and dirname ~= ".git"
      end,
    })
  do
    if kind == "file" and name ~= "package.json" and vim.fs.basename(name) == "package.json" then
      local decoded = read_manifest(directory .. "/" .. name)
      local declared = decoded and runner_named_by(decoded)
      if declared and declared ~= runner then
        conflicting[#conflicting + 1] = vim.fs.dirname(name)
      end
    end
  end
  table.sort(conflicting)
  return conflicting
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

  -- One adapter id covers one tree, so a root-level run over packages naming different runners
  -- either skips the odd ones out or runs their tests under a runner ownership never gave them.
  -- Neither is worth guessing at, so the operator is told where to run instead.
  if runner then
    local conflicting = packages_declaring_other_runners(directory, runner)
    if #conflicting > 0 then
      vim.notify(
        ("neotest: this directory declares %s, but another runner is declared under %s. Run from a package root."):format(
          runner,
          table.concat(conflicting, ", ")
        ),
        vim.log.levels.WARN
      )
      return
    end
  end

  -- Asked of each adapter rather than resolved here, so the id matches the one neotest builds
  -- from the same call. A JavaScript adapter roots on the nearest package.json, which can sit
  -- outside the repository entirely; those tests are not this repository's, so that root is
  -- refused on the same boundary the manifest walk stops at.
  local repository = vim.fs.root(directory, ".git")
  local function inside_repository(root)
    return not repository or root == repository or vim.startswith(root, repository .. "/")
  end

  local javascript, other = {}, {}
  for _, adapter in ipairs(require("neotest.config").adapters) do
    local root = adapter.root(directory)
    if root and javascript_adapters[adapter.name] and not inside_repository(root) then
      root = nil
    end
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
  -- Nothing here declares a JavaScript runner, so an attached JavaScript adapter is attached only
  -- because some package.json exists, which a Python project can carry by accident. One other
  -- adapter rooted here is the answer, and the chooser only ever offers JavaScript adapters,
  -- because JavaScript is the only ambiguity this arbitrates.
  if #other == 1 then
    return neotest.run.run({ directory, adapter = other[1].id })
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
      --
      -- Audited at this pin: the copy keeps `root`, `filter_dir`, `build_spec`, `results` and the
      -- default command, config, environment and working-directory closures, and drops only the
      -- callable metatable, which nothing here needs. WHEN THE PIN MOVES, re-audit for options
      -- initialized inside `__call`, methods reachable only through the metatable, and mutable
      -- table fields the copy would share rather than own.
      -- Compiled here, on the main loop, and not on first use. Tree-sitter's own first load
      -- creates an augroup, and `is_test_file` is asked from async contexts where that raises
      -- E5560; the error would be caught and read as "this file imports nothing".
      node_test_query("javascript")
      node_test_query("typescript")

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
