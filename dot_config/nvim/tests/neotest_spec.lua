-- The JavaScript adapter routing in lua/plugins/neotest.lua (spec 5.3), and the directory run
-- behind `<leader>ta`. Three adapters claim JavaScript and neotest walks its adapter map with
-- `pairs`, so without a decision here the adapter that runs a file, or a directory, varies
-- between runs.
--
-- The adapter modules and `neotest.setup` are stubbed, so this runs headless with no plugin
-- installed and no test runner process. What is NOT stubbed is the routing's own reading of the
-- tree, because that is where ownership is decided: the fixture below is a real package layout
-- under `vim.fn.tempname()`, so every rule is exercised against real manifests and real file
-- contents rather than against a description of them.
--
-- The stubs follow each pinned adapter's own shape. jest REBINDS the upvalue its
-- `adapter.is_test_file` reads rather than replacing the function, so `adapter.is_test_file` is
-- the same object before and after the call and capturing it to compose against would recurse.
-- node:test returns a whole new adapter table. vitest is neither, because the routing does not
-- construct it: its `__call` puts its own dependency gate ahead of the caller's predicate, so the
-- routing copies the module and overrides the field, and the stub is a plain table.
--
-- Tree-sitter is NOT stubbed either. The routing asks a parser which nodes are imports, so the
-- runner's `--clean` start is given back the directory nvim-treesitter installs grammars into.
-- A machine carrying no JavaScript grammar, which is how CI installs Neovim, exercises the stated
-- fallback instead: a file whose language cannot be parsed has no node:test import to find.

local plugin_spec = require("plugins.neotest")[1]

vim.opt.runtimepath:append(vim.fn.stdpath("data") .. "/site")
local _, javascript_grammar = pcall(vim.treesitter.language.add, "javascript")
javascript_grammar = javascript_grammar == true

-- Realpath'd, because `:cd` resolves symlinks and macOS puts the temporary directory behind
-- one, so a working directory read back after `:cd` would not string-compare against the path
-- the fixture was written to.
local fixture_root = vim.fn.tempname()
vim.fn.mkdir(fixture_root, "p")
fixture_root = vim.uv.fs_realpath(fixture_root) or fixture_root

local function write_fixture(relative, contents)
  local path = fixture_root .. "/" .. relative
  vim.fn.mkdir(vim.fn.fnamemodify(path, ":h"), "p")
  local handle = assert(io.open(path, "w"), "could not write " .. path)
  handle:write(contents)
  handle:close()
  return path
end

-- No manifest anywhere above it, so nothing declares a runner for this one.
local node_file = write_fixture("tests/node.test.js", 'import { test } from "node:test";\n')

-- One package declaring both runners.
local dual = write_fixture("dual/package.json", '{ "devDependencies": { "vitest": "3.2.7", "jest": "29.7.0" } }')
local dual_file = write_fixture("dual/tests/math.test.js", 'import { test } from "vitest";\n')

-- A vitest repository holding a nested package that declares jest and nothing else.
write_fixture("vitest-root/package.json", '{ "devDependencies": { "vitest": "3.2.7" } }')
local vitest_root_file = write_fixture("vitest-root/tests/root.test.js", 'import { test } from "vitest";\n')
local nested =
  write_fixture("vitest-root/packages/jest-only/package.json", '{ "devDependencies": { "jest": "29.7.0" } }')
local nested_file =
  write_fixture("vitest-root/packages/jest-only/tests/nested.test.js", 'test("nested", function () {});\n')
write_fixture("vitest-root/packages/silent/package.json", "{}")
local silent_nested_file =
  write_fixture("vitest-root/packages/silent/tests/quiet.test.js", 'import { test } from "vitest";\n')

-- node:test files whose names the pinned node:test matcher rejects: it claims `e2e-spec` but not
-- `e2e`, and none of the three pinned matchers claims `.mjs` or `.cjs`.
local e2e_node_file = write_fixture("vitest-root/tests/suite.e2e.js", 'import { test } from "node:test";\n')
local mjs_node_file = write_fixture("vitest-root/tests/mod.test.mjs", 'import { test } from "node:test";\n')
local cjs_node_file = write_fixture("vitest-root/tests/mod.test.cjs", 'const { test } = require("node:test");\n')

-- The shapes a node:test import takes, and two mentions that are not imports at all.
local commented_file =
  write_fixture("vitest-root/tests/note.test.js", '// do not import from "node:test"\nimport { test } from "vitest";\n')
local block_commented_file = write_fixture(
  "vitest-root/tests/block.test.js",
  '/* prefer "node:test" once this migrates */\nimport { test } from "vitest";\n'
)
local multiline_file =
  write_fixture("vitest-root/tests/multi.test.ts", 'import {\n  test,\n  describe,\n} from "node:test";\n')
local required_file = write_fixture("vitest-root/tests/required.test.js", 'const { test } = require("node:test");\n')

-- A string that only looks like an import, and a real import beside a URL on the same line.
local snippet_file = write_fixture(
  "vitest-root/tests/snippet.test.js",
  'import { test } from "vitest";\nconst snippet = \'require("node:test")\';\n'
)
local url_file =
  write_fixture("vitest-root/tests/url.test.js", 'const url = "https://example"; import { test } from "node:test";\n')

-- A regular expression is neither an import nor a place one can hide.
local regex_file =
  write_fixture("vitest-root/tests/regex.test.js", 'const re = /["\']/;\nimport { test } from "node:test";\n')
local regex_lookalike_file = write_fixture(
  "vitest-root/tests/lookalike.test.js",
  'import { test } from "vitest";\nconst bad = /require("node:test")/;\n'
)

-- A template substitution holds code; a literal nested inside one does not.
local template_file =
  write_fixture("vitest-root/tests/template.test.js", 'const runner = `${(await import("node:test")).test}`;\n')
local nested_template_file = write_fixture(
  "vitest-root/tests/nested-template.test.js",
  'import { test } from "vitest";\nconst s = `${ `require("node:test")` }`;\n'
)

-- Type-only declarations name node:test at compile time and are erased before anything runs.
local type_import_file = write_fixture(
  "vitest-root/tests/typed.test.ts",
  'import type { TestContext } from "node:test";\nimport { test } from "vitest";\n'
)
local type_export_file = write_fixture(
  "vitest-root/tests/reexport.test.ts",
  'export type { TestContext } from "node:test";\nimport { test } from "vitest";\n'
)

-- A language with no grammar to parse it.
local coffee_file = write_fixture("vitest-root/tests/legacy.test.coffee", 'test = require("node:test")\n')

-- A real node:test import pushed past the head of the file by a generated preamble.
local preamble = (" * a generated preamble line, one of many, padding this header out\n"):rep(120)
local late_import_file =
  write_fixture("vitest-root/tests/late.test.js", "/*\n" .. preamble .. ' */\nimport { test } from "node:test";\n')

-- A runner declared by an intermediate package, behind a nested manifest that declares nothing.
write_fixture("mono/package.json", '{ "name": "mono" }')
write_fixture("mono/packages/web/package.json", '{ "devDependencies": { "vitest": "3.2.7" } }')
local behind_empty = write_fixture("mono/packages/web/fixtures/package.json", "{}")
local behind_empty_file = write_fixture("mono/packages/web/fixtures/a.test.js", 'import { test } from "vitest";\n')

-- A vitest repository holding a package that declares jest instead.
local conflict = write_fixture("conflict/package.json", '{ "devDependencies": { "vitest": "3.2.7" } }')
write_fixture("conflict/tests/root.test.js", 'import { test } from "vitest";\n')
write_fixture("conflict/packages/api/package.json", '{ "devDependencies": { "jest": "29.7.0" } }')
write_fixture("conflict/packages/api/tests/api.test.js", 'test("api", function () {});\n')

-- A vitest repository whose only other manifests sit where a scan has no business looking.
local pruned = write_fixture("pruned/package.json", '{ "devDependencies": { "vitest": "3.2.7" } }')
write_fixture("pruned/tests/root.test.js", 'import { test } from "vitest";\n')
write_fixture("pruned/src/node_modules/dependency/package.json", '{ "devDependencies": { "jest": "29.7.0" } }')
write_fixture("pruned/.cache/old/package.json", '{ "devDependencies": { "jest": "29.7.0" } }')

-- A vitest repository whose conflicting package sits deeper than a monorepo usually puts one.
local deep = write_fixture("deep/package.json", '{ "devDependencies": { "vitest": "3.2.7" } }')
write_fixture("deep/tests/root.test.js", 'import { test } from "vitest";\n')
write_fixture("deep/packages/team/apps/api/package.json", '{ "devDependencies": { "jest": "29.7.0" } }')

-- A git repository, declaring no runner, nested under a JavaScript package that declares one.
-- `.git` is a FILE here, the form a worktree or submodule uses, not a directory.
write_fixture("gitparent/package.json", '{ "devDependencies": { "vitest": "3.2.7" } }')
local inner_git = write_fixture("gitparent/inner/.git", "gitdir: /nowhere\n")
local inner_file = write_fixture("gitparent/inner/tests/inner.test.js", 'test("inner", function () {});\n')

-- A Python project carrying a stray manifest that names no runner.
local pystray = write_fixture("pystray/pyproject.toml", '[project]\nname = "pystray"\n')
write_fixture("pystray/package.json", "{}")
write_fixture("pystray/tests/test_math.py", "def test_adds():\n    assert 1 + 1 == 2\n")

-- A manifest no JSON parser can read.
local broken = write_fixture("broken/package.json", '{ "devDependencies": { "vitest": }')
write_fixture("broken/tests/x.test.js", 'import { test } from "vitest";\n')

-- A package that names no runner at all.
local silent = write_fixture("silent/package.json", '{ "name": "silent", "version": "1.0.0" }')
write_fixture("silent/tests/thing.test.js", 'test("thing", function () {});\n')

local function directory_of(manifest)
  return vim.fn.fnamemodify(manifest, ":h")
end

--- The root all three JavaScript adapters resolve, faithful to their pins: every one of them
--- roots on the nearest ancestor holding a package.json.
local function package_root(path)
  return vim.fs.root(path, "package.json")
end

--- neotest-python roots on a Python project marker, not on a package.json.
local function python_root(path)
  return vim.fs.root(path, "pyproject.toml")
end

local function no_root()
  return nil
end

--- Run the plugin spec's `config` against stubbed adapters and return each JavaScript adapter's
--- configured `is_test_file`, the adapter list neotest was handed, and a way to press
--- `<leader>ta`.
local function route()
  local vitest = { name = "neotest-vitest", root = package_root }

  local jest = { name = "neotest-jest", root = package_root }
  local jest_is_test_file
  jest.is_test_file = function(path)
    return jest_is_test_file(path)
  end
  setmetatable(jest, {
    __call = function(_, opts)
      jest_is_test_file = opts.isTestFile
      return jest
    end,
  })

  -- The pin builds a whole new adapter whose `is_test_file` delegates to the configured predicate.
  local node = setmetatable({}, {
    __call = function(_, opts)
      return { name = "neotest-nodejs", root = package_root, is_test_file = opts.isTestFile }
    end,
  })

  local captured, ran
  local stubs = {
    ["neotest"] = {
      setup = function(options)
        captured = options
      end,
      run = {
        run = function(args)
          ran = args
        end,
      },
    },
    ["neotest-vitest"] = vitest,
    ["neotest-jest"] = jest,
    ["neotest-nodejs"] = node,
    ["neotest-python"] = { name = "neotest-python", root = python_root },
    ["neotest-golang"] = setmetatable({}, {
      __call = function()
        return { name = "neotest-golang", root = no_root, constructed = true }
      end,
    }),
    ["neotest-busted"] = { name = "neotest-busted", root = no_root },
    ["neotest-swift-testing"] = { name = "neotest-swift-testing", root = no_root },
  }
  -- Left in place rather than restored: the configured predicates are called after this returns,
  -- and none of these names is a real module under the headless runner.
  for name, stub in pairs(stubs) do
    package.loaded[name] = stub
  end

  local ok, err = pcall(plugin_spec.config)
  assert(ok, "config() failed: " .. tostring(err))
  assert(captured, "neotest.setup was never called")

  local by_name = {}
  for _, adapter in ipairs(captured.adapters) do
    by_name[adapter.name] = adapter
  end
  -- `<leader>ta` reads the live adapter list to work out which adapters attach where.
  package.loaded["neotest.config"] = { adapters = captured.adapters }

  local run_all
  for _, key in ipairs(plugin_spec.keys) do
    if key[1] == "<leader>ta" then
      run_all = key[2]
    end
  end
  assert(run_all, "<leader>ta is not in the plugin spec's keys")

  --- Press `<leader>ta` in `directory`. `choose` is the item the operator picks when asked.
  ---@return { ran: table|string|nil, prompted: string[]|nil, notified: string[] }
  local function press_run_all(directory, choose)
    ran = nil
    local prompted, notified = nil, {}
    local previous_select, previous_notify = vim.ui.select, vim.notify
    local previous_directory = vim.fn.getcwd()
    vim.ui.select = function(items, _, on_choice)
      prompted = vim.tbl_map(function(item)
        return item.name
      end, items)
      on_choice(choose and items[choose] or nil)
    end
    vim.notify = function(message)
      notified[#notified + 1] = tostring(message)
    end
    vim.cmd.cd(directory)
    local pressed, pressed_err = pcall(run_all)
    vim.cmd.cd(previous_directory)
    vim.ui.select, vim.notify = previous_select, previous_notify
    assert(pressed, "<leader>ta failed: " .. tostring(pressed_err))
    return { ran = ran, prompted = prompted, notified = notified }
  end

  return {
    vitest = by_name["neotest-vitest"].is_test_file,
    jest = by_name["neotest-jest"].is_test_file,
    node = by_name["neotest-nodejs"].is_test_file,
    by_name = by_name,
    press_run_all = press_run_all,
  }
end

--- The one adapter claiming `path`, or nil. Two claimants is itself a failure, so every case
--- that asks this is also asserting that ownership stayed single.
local function owner(routed, path)
  local claimed = {}
  for name, predicate in pairs({
    ["neotest-vitest"] = routed.vitest,
    ["neotest-jest"] = routed.jest,
    ["neotest-nodejs"] = routed.node,
  }) do
    if predicate(path) then
      claimed[#claimed + 1] = name
    end
  end
  table.sort(claimed)
  assert(#claimed <= 1, "more than one adapter claimed " .. path .. ": " .. table.concat(claimed, " "))
  return claimed[1]
end

--- The adapter that must own a file importing node:test: node:test itself where the JavaScript
--- grammar is installed, and `without_grammar` where none is. Both are real answers this routing
--- gives, and which one a machine sees is decided by what it has installed.
local function node_or(without_grammar)
  return javascript_grammar and "neotest-nodejs" or without_grammar
end

local cases = {}

cases["one package declaring both runners gives its files to vitest"] = function()
  local routed = route()
  assert(owner(routed, dual_file) == "neotest-vitest", "the documented precedence gives vitest the file")
end

cases["a nested jest package inside a vitest repository keeps its own files"] = function()
  -- The nearest package that names a runner is the one that meant it, so a nested jest package
  -- is not swallowed by a vitest ancestor. Precedence only settles a package declaring both.
  local routed = route()
  assert(owner(routed, nested_file) == "neotest-jest", "the nested jest package lost its own file")
  assert(owner(routed, vitest_root_file) == "neotest-vitest", "vitest gave up a file in the package that declares it")
  -- A nested package naming no runner declares nothing, so the ancestor's choice still stands.
  assert(owner(routed, silent_nested_file) == "neotest-vitest", "an empty nested manifest took the file from vitest")
end

cases["a nested git repository does not inherit a runner from outside it"] = function()
  -- Walking to the filesystem root reaches a package.json belonging to some unrelated ancestor,
  -- and a repository that declares no runner then runs its tests under one it never chose. The
  -- same boundary rejects an adapter whose root sits outside the repository.
  local routed = route()
  assert(owner(routed, inner_file) == nil, "the nested repository inherited a runner from outside it")
  -- And a plain run is not a safe fallback once a root has been refused: neotest picks a
  -- directory's adapter without asking `is_test_file`, so it would reach for the very adapter
  -- rooted outside the repository that was just turned down.
  local run = routed.press_run_all(directory_of(inner_git))
  assert(not run.prompted, "the run offered adapters rooted outside the repository")
  assert(not run.ran, "the run fell back to letting neotest choose, got " .. vim.inspect(run.ran))
  assert(#run.notified == 1, "the operator was not told why nothing ran")
end

cases["a runner an intermediate package declares is found through a nested empty manifest"] = function()
  -- Reading only the nearest manifest stops at the empty one, and the adapters' own dependency
  -- detection looks at the working directory and the git root, neither of which is the package
  -- that declares the runner. The file ends up with no claimant at all.
  local routed = route()
  assert(owner(routed, behind_empty_file) == "neotest-vitest", "the intermediate package's runner was lost")
end

cases["node:test owns the file names its own matcher rejects"] = function()
  -- The three pinned matchers disagree about which names are test files: vitest claims `.e2e.js`
  -- and node:test claims `.e2e-spec.js`, and none of the three claims `.mjs` or `.cjs`. Deciding
  -- ownership with one rule and standing down with another left those files with no claimant.
  local routed = route()
  for _, path in ipairs({ e2e_node_file, mjs_node_file, cjs_node_file }) do
    assert(owner(routed, path) == node_or("neotest-vitest"), "a node:test file had the wrong claimant: " .. path)
  end
end

cases["a regular expression is neither an import nor a place to hide one"] = function()
  -- A scanner copying every non-comment slash as code loses the import written after a regular
  -- expression holding a quote, and reads the contents of one holding a require call as an import.
  local routed = route()
  assert(owner(routed, regex_file) == node_or("neotest-vitest"), "a regular expression hid a real import")
  assert(owner(routed, regex_lookalike_file) == "neotest-vitest", "a regular expression read as an import")
end

cases["a template substitution is code and a literal nested in one is not"] = function()
  local routed = route()
  assert(owner(routed, template_file) == node_or("neotest-vitest"), "a template substitution hid a real import")
  assert(owner(routed, nested_template_file) == "neotest-vitest", "a nested literal read as an import")
end

cases["a type-only import or re-export is not a runtime dependency"] = function()
  -- TypeScript erases these before anything runs, so they name node:test at compile time only.
  -- The file's real runner is the one its runtime import names.
  local routed = route()
  assert(owner(routed, type_import_file) == "neotest-vitest", "a type-only import claimed the file for node:test")
  assert(owner(routed, type_export_file) == "neotest-vitest", "a type-only re-export claimed the file for node:test")
end

cases["a file whose language has no grammar has no node:test owner"] = function()
  -- Nothing parses CoffeeScript here, so the import cannot be seen and is not guessed at. The
  -- package's declared runner takes the file instead.
  local routed = route()
  assert(owner(routed, coffee_file) == "neotest-vitest", "an unparseable language was given to node:test")
end

cases["a node:test mention inside a comment is not an import"] = function()
  -- Otherwise a vitest test that merely names the module in a note is claimed by node:test
  -- while vitest and jest both stand down for it.
  local routed = route()
  for _, path in ipairs({ commented_file, block_commented_file }) do
    assert(owner(routed, path) == "neotest-vitest", "a comment cost the package's own runner a file: " .. path)
  end
end

cases["a string that looks like an import is not one, and a URL does not hide one"] = function()
  -- Both are ordinary JavaScript. A textual strip reads `'require("node:test")'`, a plain string,
  -- as an import, and a `//` inside a URL literal ends the line early, losing the real import
  -- written beside it.
  local routed = route()
  assert(owner(routed, snippet_file) == "neotest-vitest", "a quoted snippet read as an import")
  assert(owner(routed, url_file) == node_or("neotest-vitest"), "a URL literal hid a real import")
end

cases["the multiline and require shapes of a node:test import both count"] = function()
  local routed = route()
  for _, path in ipairs({ multiline_file, required_file }) do
    assert(owner(routed, path) == node_or("neotest-vitest"), "an import shape went unrecognized: " .. path)
  end
end

cases["a node:test import past the head of a long file is still an import"] = function()
  -- A license or generated preamble pushes the real import down the file, so a reader that
  -- stops after a prefix hands the file to the runner the package declares instead.
  local routed = route()
  assert(owner(routed, late_import_file) == node_or("neotest-vitest"), "a preamble hid the import")
end

cases["a file importing node:test is node:test's, whatever the project declares"] = function()
  -- Nothing above this file declares a runner, so without a grammar nobody owns it at all.
  local routed = route()
  assert(owner(routed, node_file) == node_or(nil), "a node:test file had the wrong claimant")
end

cases["the configured predicates answer a nil path instead of raising"] = function()
  -- neotest types `file_path` as optional, and an error raised inside `is_test_file` loses the
  -- position silently rather than surfacing, which reads as a runner flake. The filename rule
  -- answers first, and that answer is what keeps a nil path away from everything that reads the
  -- tree. Reversing the terms is what this goes red on.
  local routed = route()
  for name, predicate in pairs({ vitest = routed.vitest, jest = routed.jest, node = routed.node }) do
    local ok, err = pcall(predicate, nil)
    assert(ok, name .. " raised on a nil path: " .. tostring(err))
  end
end

cases["the predicates answer inside a fast event, where a Vimscript read would raise"] = function()
  -- Every other case asks from the main loop, where `vim.fn.readfile` works. neotest does not:
  -- it asks from its own async contexts, and a Vimscript call raises E5560 there, which a `pcall`
  -- turns into "no import" rather than an error anyone sees. A libuv callback is the strictest of
  -- those contexts, so ask from one, on a deadline. Both reads are covered: the parse of the
  -- node:test file, and the outward manifest walk for the vitest one. Tree-sitter's own first
  -- load creates an augroup and raises there too, which is why the routing warms it up front.
  local routed = route()
  local answers, failure, done = {}, nil, false
  local timer = assert(vim.uv.new_timer())
  timer:start(0, 0, function()
    local ok, result = pcall(function()
      return {
        node = routed.node(node_file) == (javascript_grammar and true or false),
        vitest = routed.vitest(dual_file),
      }
    end)
    if ok then
      answers = result
    else
      failure = result
    end
    done = true
  end)
  local ran = vim.wait(2000, function()
    return done
  end, 10)
  timer:close()
  assert(ran, "the fast event never ran")
  assert(not failure, "a predicate raised inside a fast event: " .. tostring(failure))
  assert(answers.node, "the import read answered wrong inside a fast event")
  assert(answers.vitest, "the manifest walk answered no inside a fast event")
end

cases["neotest-golang is constructed, because its options are populated only by the call"] = function()
  -- At the pinned commit `M.Adapter.options` is assigned inside `__call` alone and read by
  -- `filter_dir`, so the bare module raises in any Go module with a subdirectory.
  local routed = route()
  assert(routed.by_name["neotest-golang"], "neotest-golang is not in the adapter list")
  assert(routed.by_name["neotest-golang"].constructed, "neotest-golang was passed as a bare module")
end

cases["a directory run names the adapter the nearest package declares"] = function()
  -- neotest accepts a directory without asking any adapter's `is_test_file`, then picks through
  -- `pairs`, so a package with two attached adapters ran a different one between presses
  -- (measured: 11 vitest and 9 jest over 20 fresh processes). An adapter id is `<name>:<root>`
  -- and `run.run` takes one verbatim, so naming it is what makes the choice explicit.
  local routed = route()

  local both = routed.press_run_all(directory_of(dual))
  assert(both.ran, "<leader>ta ran nothing")
  assert(both.ran[1] == directory_of(dual), "the run did not name the working directory")
  assert(
    both.ran.adapter == "neotest-vitest:" .. directory_of(dual),
    "expected vitest by name, got " .. tostring(both.ran.adapter)
  )
  assert(not both.prompted, "a declared runner should not need asking")

  local jest_package = routed.press_run_all(directory_of(nested))
  assert(
    jest_package.ran and jest_package.ran.adapter == "neotest-jest:" .. directory_of(nested),
    "expected jest by name, got " .. tostring(jest_package.ran and jest_package.ran.adapter)
  )

  -- The intermediate package's runner reaches the directory run too, through the empty manifest
  -- that the adapter roots on.
  local intermediate = routed.press_run_all(directory_of(behind_empty))
  assert(
    intermediate.ran and intermediate.ran.adapter == "neotest-vitest:" .. directory_of(behind_empty),
    "expected vitest by name, got " .. tostring(intermediate.ran and intermediate.ran.adapter)
  )
end

cases["a directory run dispatches the only non-JavaScript adapter rather than asking"] = function()
  -- A stray package.json in a Python project roots all three JavaScript adapters, and none of
  -- them is what the operator meant. Nothing here declares a JavaScript runner and exactly one
  -- other adapter is rooted, so there is nothing to ask about.
  local routed = route()
  local python = routed.press_run_all(directory_of(pystray))
  assert(not python.prompted, "the run asked when only one adapter could have been meant")
  assert(
    python.ran and python.ran.adapter == "neotest-python:" .. directory_of(pystray),
    "expected python by name, got " .. tostring(python.ran and python.ran.adapter)
  )
end

cases["a directory run refuses a root whose packages declare different runners"] = function()
  -- One adapter id covers one tree, so dispatching the root's own runner either skips the nested
  -- package or runs its tests under a runner ownership never gave them. From the nested package
  -- itself the run is unambiguous and still goes ahead.
  local routed = route()
  local root_run = routed.press_run_all(directory_of(conflict))
  assert(not root_run.ran, "the run dispatched one runner over packages declaring two")
  assert(not root_run.prompted, "the run asked instead of naming the conflict")
  assert(#root_run.notified == 1, "the operator was not told")
  assert(
    root_run.notified[1]:find("packages/api", 1, true),
    "the message does not name the conflicting package: " .. tostring(root_run.notified[1])
  )

  local package_run = routed.press_run_all(directory_of(conflict) .. "/packages/api")
  assert(
    package_run.ran and package_run.ran.adapter:match("^neotest%-jest:"),
    "running from the package root should still dispatch, got " .. vim.inspect(package_run.ran)
  )
end

cases["a directory run ignores manifests under dependencies and hidden directories"] = function()
  -- `vim.fs.dir` reports paths relative to the root it was given, so comparing the whole path
  -- against "node_modules" lets `src/node_modules` through, and every hidden directory but the
  -- root's own `.git` with it. Installed packages and caches are not this repository's packages.
  local routed = route()
  local run = routed.press_run_all(directory_of(pruned))
  assert(
    #run.notified == 0,
    "a dependency or a cache was reported as a conflict: " .. table.concat(run.notified, " | ")
  )
  assert(
    run.ran and run.ran.adapter == "neotest-vitest:" .. directory_of(pruned),
    "expected vitest by name, got " .. vim.inspect(run.ran)
  )
end

cases["a directory run finds a conflicting package however deep it sits"] = function()
  -- A fixed depth is a guess about someone else's layout, and the packages past it are exactly
  -- the ones whose tests would silently run under the wrong runner.
  local routed = route()
  local run = routed.press_run_all(directory_of(deep))
  assert(not run.ran, "a conflicting package below the old ceiling went unseen")
  assert(
    run.notified[1] and run.notified[1]:find("packages/team/apps/api", 1, true),
    "the message does not name the deep package: " .. tostring(run.notified[1])
  )
end

cases["a directory run stops on a package.json it cannot parse"] = function()
  -- A manifest that cannot be read is not the same answer as a manifest naming no runner, and
  -- collapsing the two put the operator in front of a chooser instead of in front of the
  -- reason. The prompt would have offered a real choice built on a manifest nobody could read.
  local routed = route()
  local broken_run = routed.press_run_all(directory_of(broken))
  assert(not broken_run.ran, "an unreadable manifest still dispatched a run")
  assert(not broken_run.prompted, "an unreadable manifest fell through to the prompt")
  assert(#broken_run.notified == 1, "the operator was not told")
  assert(broken_run.notified[1]:find(broken, 1, true), "the message does not name the manifest")
end

cases["a directory run with no runner declared asks rather than picking one"] = function()
  local routed = route()

  local asked = routed.press_run_all(directory_of(silent), 2)
  assert(asked.prompted, "the run picked an adapter silently")
  assert(#asked.prompted == 3, "expected all three attached adapters to be offered")
  assert(
    asked.ran and asked.ran.adapter == asked.prompted[2] .. ":" .. directory_of(silent),
    "the operator's choice was not the adapter that ran"
  )

  local declined = routed.press_run_all(directory_of(silent))
  assert(not declined.ran, "declining the prompt still ran something")
end

cases["when the cases are done, the fixture tree is deleted"] = function()
  -- The runner exits through `os.exit`, which skips the cleanup Neovim does for its own temporary
  -- directory, so every run of this spec left its whole fixture tree behind. Sorted case names are
  -- what put this one last, so a case added later under a name that sorts after it fails here
  -- rather than finding the fixtures already gone.
  -- The ordering complaint is recorded and raised LAST, after the tree is gone. Raising it first
  -- skipped the delete and leaked the very tree this case exists to remove.
  local last = "when the cases are done, the fixture tree is deleted"
  local sorts_after
  for name in pairs(cases) do
    if name > last then
      sorts_after = name
    end
  end
  vim.fn.delete(fixture_root, "rf")
  assert(vim.uv.fs_stat(fixture_root) == nil, "the fixture tree outlived the run: " .. fixture_root)
  assert(not sorts_after, "a case sorts after the cleanup and would find no fixtures: " .. tostring(sorts_after))
end

return cases
