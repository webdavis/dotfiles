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

local plugin_spec = require("plugins.neotest")[1]

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

-- A real node:test import pushed past the head of the file by a generated preamble.
local preamble = (" * a generated preamble line, one of many, padding this header out\n"):rep(120)
local late_import_file =
  write_fixture("vitest-root/tests/late.test.js", "/*\n" .. preamble .. ' */\nimport { test } from "node:test";\n')

-- A runner declared by an intermediate package, behind a nested manifest that declares nothing.
write_fixture("mono/package.json", '{ "name": "mono" }')
write_fixture("mono/packages/web/package.json", '{ "devDependencies": { "vitest": "3.2.7" } }')
local behind_empty = write_fixture("mono/packages/web/fixtures/package.json", "{}")
local behind_empty_file = write_fixture("mono/packages/web/fixtures/a.test.js", 'import { test } from "vitest";\n')

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
    ["neotest-python"] = { name = "neotest-python", root = no_root },
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

--- How many of the three JavaScript adapters claim `path`.
local function claim_count(routed, path)
  local count = 0
  for _, predicate in ipairs({ routed.vitest, routed.jest, routed.node }) do
    if predicate(path) then
      count = count + 1
    end
  end
  return count
end

local cases = {}

cases["one package declaring both runners gives its files to vitest"] = function()
  local routed = route()
  assert(claim_count(routed, dual_file) == 1, "expected exactly one adapter to claim the file")
  assert(routed.vitest(dual_file), "the documented precedence gives vitest the file")
  assert(not routed.jest(dual_file), "jest did not stand down for vitest")
end

cases["a nested jest package inside a vitest repository keeps its own files"] = function()
  -- The nearest package that names a runner is the one that meant it, so a nested jest package
  -- is not swallowed by a vitest ancestor. Precedence only settles a package declaring both.
  local routed = route()
  assert(routed.jest(nested_file), "the nested jest package lost its own file")
  assert(not routed.vitest(nested_file), "vitest crossed a package boundary")
  assert(claim_count(routed, nested_file) == 1, "expected exactly one adapter to claim the file")
  assert(routed.vitest(vitest_root_file), "vitest gave up a file in the package that declares it")
  assert(claim_count(routed, vitest_root_file) == 1, "expected exactly one adapter to claim the file")
  -- A nested package naming no runner declares nothing, so the ancestor's choice still stands.
  assert(routed.vitest(silent_nested_file), "an empty nested manifest took the file from vitest")
  assert(claim_count(routed, silent_nested_file) == 1, "expected exactly one adapter to claim the file")
end

cases["a runner an intermediate package declares is found through a nested empty manifest"] = function()
  -- Reading only the nearest manifest stops at the empty one, and the adapters' own dependency
  -- detection looks at the working directory and the git root, neither of which is the package
  -- that declares the runner. The file ends up with no claimant at all.
  local routed = route()
  assert(routed.vitest(behind_empty_file), "the intermediate package's runner was lost")
  assert(claim_count(routed, behind_empty_file) == 1, "expected exactly one adapter to claim the file")
end

cases["node:test owns the file names its own matcher rejects"] = function()
  -- The three pinned matchers disagree about which names are test files: vitest claims `.e2e.js`
  -- and node:test claims `.e2e-spec.js`, and none of the three claims `.mjs` or `.cjs`. Deciding
  -- ownership with one rule and standing down with another left those files with no claimant.
  local routed = route()
  for _, path in ipairs({ e2e_node_file, mjs_node_file, cjs_node_file }) do
    assert(routed.node(path), "a node:test file had no claimant: " .. path)
    assert(claim_count(routed, path) == 1, "expected exactly one adapter to claim " .. path)
  end
end

cases["a node:test mention inside a comment is not an import"] = function()
  -- Otherwise a vitest test that merely names the module in a note is claimed by node:test
  -- while vitest and jest both stand down for it.
  local routed = route()
  for _, path in ipairs({ commented_file, block_commented_file }) do
    assert(not routed.node(path), "a comment read as an import: " .. path)
    assert(routed.vitest(path), "a comment cost the package's own runner a file: " .. path)
  end
end

cases["the multiline and require shapes of a node:test import both count"] = function()
  local routed = route()
  for _, path in ipairs({ multiline_file, required_file }) do
    assert(routed.node(path), "an import shape went unrecognized: " .. path)
    assert(claim_count(routed, path) == 1, "expected exactly one adapter to claim " .. path)
  end
end

cases["a node:test import past the head of a long file is still an import"] = function()
  -- A license or generated preamble pushes the real import down the file, so a reader that
  -- stops after a prefix hands the file to the runner the package declares instead.
  local routed = route()
  assert(routed.node(late_import_file), "a preamble hid the import")
  assert(not routed.vitest(late_import_file), "vitest took a node:test file")
  assert(claim_count(routed, late_import_file) == 1, "expected exactly one adapter to claim the file")
end

cases["a file importing node:test is node:test's, whatever the project declares"] = function()
  local routed = route()
  assert(routed.node(node_file), "a node:test file had no claimant")
  assert(not routed.vitest(node_file), "vitest did not stand down for a node:test file")
  assert(not routed.jest(node_file), "jest did not stand down for a node:test file")
  assert(claim_count(routed, node_file) == 1, "expected exactly one adapter to claim the file")
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

return cases
