-- Each adapter loads with its filetype, and neotest loads any adapter still missing on its first run.
local javascript_filetypes = {
  "javascript",
  "javascriptreact",
  "typescript",
  "typescriptreact",
}

--- The tree-sitter language for each JavaScript-family extension. `.tsx` needs the `tsx` grammar,
--- because the `typescript` one turns everything after the first JSX element into an error.
local javascript_languages = {
  js = "javascript",
  jsx = "javascript",
  mjs = "javascript",
  cjs = "javascript",
  ts = "typescript",
  mts = "typescript",
  cts = "typescript",
  tsx = "tsx",
}

--- The import, re-export, `require` and dynamic `import` shapes that load node:test. Matching the
--- parse tree keeps strings, comments and regular expressions from counting as imports.
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

--- Memoized, because compiling the query is the slow part and the routing runs per file.
local compiled_queries = {}
local function node_test_query(language)
  if compiled_queries[language] == nil then
    local ok, query = pcall(vim.treesitter.query.parse, language, node_test_query_text)
    compiled_queries[language] = ok and query or false
  end
  return compiled_queries[language] or nil
end

--- TypeScript erases type-only imports, so they say nothing about the runner the file uses.
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

--- Import answers keyed by path, size and modification time, because all three adapters ask
--- about the same file and parsing is the slow part.
local parsed_imports = {}

--- Whether `file_path` imports `node:test`. Uses `io`, because a Vimscript call raises E5560 in
--- neotest's async contexts.
---@param file_path string
---@return boolean
local function imports_node_test(file_path)
  local language = javascript_languages[file_path:match("%.([^.]+)$") or ""]
  local query = language and node_test_query(language)
  if not query then
    return false
  end
  local stat = vim.uv.fs_stat(file_path)
  if not stat then
    return false
  end
  local key = ("%s\0%d\0%d\0%d"):format(file_path, stat.size, stat.mtime.sec, stat.mtime.nsec)
  local remembered = parsed_imports[key]
  if remembered ~= nil then
    return remembered
  end

  local handle = io.open(file_path, "r")
  if not handle then
    return false
  end
  local source = handle:read("*a") or ""
  handle:close()

  -- Every query shape compares a fragment against `node:test`, so a file without that text needs no parse.
  if not source:find("node:test", 1, true) then
    parsed_imports[key] = false
    return false
  end

  local parsed, parser = pcall(vim.treesitter.get_string_parser, source, language)
  if not parsed then
    return false
  end
  parsed_imports[key] = false
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
      parsed_imports[key] = true
      return true
    end
  end
  return false
end

--- The union of the three JavaScript adapters' filename rules, so every test file has one owner.
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

--- The decoded package.json at `manifest`, or nil. The second value is true when the file exists
--- but cannot be parsed. Uses `io`, because a Vimscript call raises E5560 in async contexts.
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

  -- Substring matches catch names like `@vitest/ui` and `jest --ci`. vitest is checked first so it
  -- wins when a package declares both.
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
--- The walk stops at the repository root, because a manifest above it belongs to another project.
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
--- The walk has no depth limit. Skipping node_modules and hidden directories keeps it small.
---@param directory string
---@param runner string
---@return string[]
local function packages_declaring_other_runners(directory, runner)
  local conflicting = {}
  for name, kind in
    vim.fs.dir(directory, {
      depth = math.huge,
      -- `dirname` is relative to `directory`, so match on its last component.
      skip = function(dirname)
        local name = vim.fs.basename(dirname)
        return name ~= "node_modules" and not vim.startswith(name, ".")
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

--- The adapter that owns `file_path`, or nil. A file importing node:test is node:test's.
--- Otherwise the nearest package naming a runner decides, and vitest wins when one names both.
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

--- Languages whose parser build timed out this session, so later presses skip the wait.
local parser_wait_failed = {}

--- The neotest module, after installing the buffer's tree-sitter parser. Every adapter finds tests
--- with tree-sitter, and a missing parser gives an empty result with no message. The wait comes
--- before requiring neotest, because its client starts discovery on the first call.
---@return table
local function neotest_with_parser()
  local language = vim.treesitter.language.get_lang(vim.bo.filetype) or vim.bo.filetype
  -- `language.add` returns false for a missing parser instead of raising, so check its value.
  local loaded, added = pcall(vim.treesitter.language.add, language)
  if loaded and added then
    return require("neotest")
  end
  -- Skip the wait when no grammar exists for this filetype or a build already timed out.
  if
    not parser_wait_failed[language]
    and vim.list_contains(require("nvim-treesitter.config").get_available(), language)
  then
    vim.notify("neotest: installing the " .. language .. " parser", vim.log.levels.INFO)
    -- `force` builds even when a leftover queries directory looks installed. The time limit keeps a
    -- stuck build from holding the editor. `install` reports a build with no parser as false.
    local finished, installed = require("nvim-treesitter").install({ language }, { force = true }):pwait(30000)
    if not (finished and installed) then
      parser_wait_failed[language] = true
    end
  end
  return require("neotest")
end

--- Runs everything under the working directory through one named adapter, because neotest
--- otherwise picks a different attached adapter between presses. The nearest package.json
--- names it. With none declared and several attached, the user chooses.
local function run_all_tests()
  local neotest = neotest_with_parser()
  local directory = vim.fn.getcwd()

  -- An unparseable manifest stops the run with an error instead of a guess.
  local runner, unparseable = declared_runner(directory)
  if unparseable then
    vim.notify("neotest: could not parse " .. unparseable, vim.log.levels.ERROR)
    return
  end

  -- One adapter id covers one tree, so packages with different runners need separate runs.
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

  -- Each adapter reports its own root so the id matches neotest's. Roots outside the repository are refused.
  local repository = vim.fs.root(directory, ".git")
  local function inside_repository(root)
    return not repository or root == repository or vim.startswith(root, repository .. "/")
  end

  local javascript, other, refused = {}, {}, false
  for _, adapter in ipairs(require("neotest.config").adapters) do
    local root = adapter.root(directory)
    if root and javascript_adapters[adapter.name] and not inside_repository(root) then
      root, refused = nil, true
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
  local function choose(candidates)
    vim.ui.select(candidates, {
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

  -- A JavaScript adapter attaches to any package.json, so adapters with their own root marker win.
  if #other == 1 then
    return neotest.run.run({ directory, adapter = other[1].id })
  end
  if #other > 1 then
    return choose(other)
  end

  if #javascript == 1 then
    return neotest.run.run({ directory, adapter = javascript[1].id })
  end
  -- neotest's own choice would pick the refused adapter again.
  if refused then
    vim.notify(
      ("neotest: every JavaScript adapter here roots outside %s, so there is nothing to run. Run from the package that owns those tests."):format(
        repository
      ),
      vim.log.levels.WARN
    )
    return
  end
  if #javascript == 0 then
    return neotest.run.run(directory)
  end

  choose(javascript)
end

return {
  {
    "nvim-neotest/neotest",
    commit = "27bf921498043f7ecd821d6db68d05de244bbd02",
    dependencies = {
      "webdavis/pns.nvim",
      -- Lazy, because lazy.nvim turns the nvim-nio rockspec dependency into a startup spec.
      { "nvim-neotest/nvim-nio", lazy = true },
      { "nvim-neotest/neotest-python", commit = "1b56ca4ba51c6014f986d6548ee629bdc95589d1" },
      { "fredrikaverpil/neotest-golang", commit = "65b2be63c3de00e6f15c05388ebdc8d603dbd727" },
      -- A dependency, so it is on the runtime path before `config` builds the adapter list.
      { "webdavis/neotest-bashunit", commit = "4c07ce1c94a53be4a5e8c959fd759432c81039f6" },
    },
    -- stylua: ignore start
    keys = {
      { "<leader>tt", function() neotest_with_parser().run.run() end, desc = "Neotest: run nearest test" },
      { "<leader>tf", function() neotest_with_parser().run.run(vim.fn.expand("%")) end, desc = "Neotest: run file" },
      { "<leader>ta", run_all_tests, desc = "Neotest: run all tests" },
      { "<leader>ts", function() neotest_with_parser().summary.toggle() end, desc = "Neotest: toggle summary" },
      { "<leader>to", function() require("neotest").output.open({ enter = true }) end, desc = "Neotest: open output" },
      { "<leader>tS", function() require("neotest").run.stop() end, desc = "Neotest: stop run" },
    },
    -- stylua: ignore end
    config = function()
      -- Each adapter claims a file only when `owner_of` names it. vitest is copied, not constructed,
      -- because its `__call` adds a dependency check that misses runners declared by nested packages.
      -- The queries compile here, on the main loop, because tree-sitter's first load raises E5560 in
      -- async contexts.
      node_test_query("javascript")
      node_test_query("typescript")
      node_test_query("tsx")

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

      -- Discovery retries until rust-analyzer has loaded the workspace, because
      -- rustaceanvim treats an early empty tree as final. After the first real tree, every call
      -- returns on its first try.
      local rustaceanvim_neotest = require("rustaceanvim.neotest")
      local warmed = false
      local rust = vim.tbl_extend("force", rustaceanvim_neotest, {
        discover_positions = function(file_path)
          if warmed then
            local ok, tree = pcall(rustaceanvim_neotest.discover_positions, file_path)
            return ok and tree or nil
          end
          local deadline = vim.uv.hrtime() + 10e9
          while true do
            local ok, tree = pcall(rustaceanvim_neotest.discover_positions, file_path)
            if ok and tree and #tree:children() > 0 then
              warmed = true
              return tree
            end
            if vim.uv.hrtime() >= deadline then
              warmed = true
              return ok and tree or nil
            end
            require("nio").sleep(400)
          end
        end,
      })

      -- Classpath and compilation come from jdtls. First use needs one manual `:NeotestJava setup`,
      -- which downloads the JUnit jar.
      local java = require("neotest-java")({})

      local elixir = require("neotest-elixir")

      local zig = require("neotest-zig")

      -- neotest-golang must be called, because it sets its options only inside `__call`.
      require("neotest").setup({
        consumers = { pns = require("pns.integrations.neotest").consumer },
        adapters = {
          require("neotest-python"),
          require("neotest-golang")({}),
          vitest,
          jest,
          node,
          rust,
          java,
          elixir,
          zig,
          require("neotest-bashunit"),
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
    -- The GitHub repository is an archived redirect to this one.
    url = "https://codeberg.org/mmllr/neotest-swift-testing",
    commit = "5b2d7efea43cb0d66d97de65b9ebc7b1db4659fd",
    ft = "swift",
  },
  { "mrcjkb/rustaceanvim", commit = "a968f5133b8b24f481de12f08cd79420d1ace559", ft = "rust" },
  {
    "rcasia/neotest-java",
    commit = "71354dd2c3f59bcc2301528dfccbbfa2b85bb870",
    ft = "java",
    dependencies = {
      -- neotest-java's health check requires this client for classpath and compilation.
      { "mfussenegger/nvim-jdtls", commit = "6e9d953f0b82bccdb834cfde0e893f3119c22592" },
      -- neotest-java's health check requires this for debugging.
      { "theHamsta/nvim-dap-virtual-text", commit = "fbdb48c2ed45f4a8293d0d483f7730d24467ccb6", opts = {} },
    },
  },
  { "jfpedroza/neotest-elixir", commit = "a242aebeaa6997c1c149138ff77f6cacbe33b6fc", ft = "elixir" },
  -- This adapter reads the test output Zig itself ships, so it keeps working across Zig releases.
  { "webdavis/neotest-zig", commit = "0deabad8bc9d08c7e70a6a3b7c0153ad76a52006", ft = "zig" },
}
