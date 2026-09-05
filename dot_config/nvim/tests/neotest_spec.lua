-- The JavaScript adapter routing in lua/plugins/neotest.lua (spec 5.3). Three adapters claim
-- JavaScript and neotest walks its adapter map with `pairs`, so without a decision here the
-- adapter that runs a file varies between runs.
--
-- The adapter modules and `neotest.setup` are stubbed, so this runs headless with no plugin
-- installed and no test runner process. What is NOT stubbed is the routing's own reading of the
-- tree: the node:test rule looks inside the file and the precedence rule reads the nearest
-- package.json, so the fixture below is a real package layout under `vim.fn.tempname()`, a
-- directory Neovim removes when it exits.
--
-- The stubs follow each pinned adapter's own shape. jest REBINDS the upvalue its
-- `adapter.is_test_file` reads rather than replacing the function, so `adapter.is_test_file` is
-- the same object before and after the call and capturing it to compose against would recurse.
-- vitest instead ASSIGNS a new closure over the caller's predicate, which is what makes
-- capturing its default before the call safe. node:test returns a whole new adapter table.

local plugin_spec = require("plugins.neotest")[1]

local fixture_root = vim.fn.tempname()

local function write_fixture(relative, contents)
  local path = fixture_root .. "/" .. relative
  vim.fn.mkdir(vim.fn.fnamemodify(path, ":h"), "p")
  local handle = assert(io.open(path, "w"), "could not write " .. path)
  handle:write(contents)
  handle:close()
  return path
end

local node_file = write_fixture("tests/node.test.js", 'import { test } from "node:test";\n')
local plain_file = write_fixture("tests/plain.test.js", 'import { test } from "vitest";\n')

-- One package declaring both runners.
write_fixture("dual/package.json", '{ "devDependencies": { "vitest": "3.2.7", "jest": "29.7.0" } }')
local dual_file = write_fixture("dual/tests/math.test.js", 'import { test } from "vitest";\n')

-- A vitest repository holding a nested package that declares jest and nothing else.
write_fixture("vitest-root/package.json", '{ "devDependencies": { "vitest": "3.2.7" } }')
local vitest_root_file = write_fixture("vitest-root/tests/root.test.js", 'import { test } from "vitest";\n')
write_fixture("vitest-root/packages/jest-only/package.json", '{ "devDependencies": { "jest": "29.7.0" } }')
local nested_file =
  write_fixture("vitest-root/packages/jest-only/tests/nested.test.js", 'test("nested", function () {});\n')
write_fixture("vitest-root/packages/silent/package.json", "{}")
local silent_nested_file =
  write_fixture("vitest-root/packages/silent/tests/quiet.test.js", 'import { test } from "vitest";\n')

--- Run the plugin spec's `config` against stubbed adapters and return each JavaScript adapter's
--- configured `is_test_file`, plus the adapter list neotest was handed.
---@param owns { vitest: fun(path: string?): boolean, jest: fun(path: string?): boolean, node: fun(path: string?): boolean }
local function route(owns)
  local vitest = { name = "neotest-vitest", is_test_file = owns.vitest }
  setmetatable(vitest, {
    __call = function(_, opts)
      -- The pin ASSIGNS a new closure here. Its dependency gate ahead of the caller's predicate
      -- is not modelled: measured against the pin, that gate answers a nil path rather than
      -- raising, so it changes no answer this spec asks for.
      vitest.is_test_file = opts.is_test_file
      return vitest
    end,
  })

  local jest = { name = "neotest-jest" }
  local jest_is_test_file = owns.jest
  jest.is_test_file = function(path)
    return jest_is_test_file(path)
  end
  setmetatable(jest, {
    __call = function(_, opts)
      jest_is_test_file = opts.isTestFile
      return jest
    end,
  })

  -- The pin builds a whole new adapter whose `is_test_file` delegates to the configured
  -- predicate, so there is no default left on it to capture.
  local node = setmetatable({}, {
    __call = function(_, opts)
      return { name = "neotest-nodejs", is_test_file = opts.isTestFile }
    end,
  })

  local captured
  local stubs = {
    ["neotest"] = {
      setup = function(options)
        captured = options
      end,
    },
    ["neotest-vitest"] = vitest,
    ["neotest-jest"] = jest,
    ["neotest-jest.jest-util"] = { defaultIsTestFile = owns.jest },
    ["neotest-nodejs"] = node,
    ["neotest-nodejs.node-util"] = { defaultIsTestFile = owns.node },
    ["neotest-python"] = { name = "neotest-python" },
    ["neotest-golang"] = setmetatable({}, {
      __call = function()
        return { name = "neotest-golang", constructed = true }
      end,
    }),
    ["neotest-busted"] = { name = "neotest-busted" },
    ["neotest-swift-testing"] = { name = "neotest-swift-testing" },
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
  return {
    vitest = by_name["neotest-vitest"].is_test_file,
    jest = by_name["neotest-jest"].is_test_file,
    node = by_name["neotest-nodejs"].is_test_file,
    by_name = by_name,
  }
end

local function no()
  return false
end

--- A claim faithful to all three pins: each adapter's default predicate opens with a nil guard,
--- then matches the conventional test-file names.
local function any_javascript(path)
  return path ~= nil and path:match("%.test%.js$") ~= nil
end

local function all_claim()
  return { vitest = any_javascript, jest = any_javascript, node = any_javascript }
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

return {
  ["one package declaring both runners gives its files to vitest"] = function()
    local routed = route(all_claim())
    assert(claim_count(routed, dual_file) == 1, "expected exactly one adapter to claim the file")
    assert(routed.vitest(dual_file), "the documented precedence gives vitest the file")
    assert(not routed.jest(dual_file), "jest did not stand down for vitest")
  end,

  ["a nested jest package inside a vitest repository keeps its own files"] = function()
    -- vitest's own detection reads the working directory and the git root as well as the nearest
    -- manifest, so it answers yes for a file whose nearest package declares jest alone. Running
    -- that file through vitest is wrong: the package that names a runner is the one that meant
    -- it. Precedence only settles a package that declares both.
    local routed = route(all_claim())
    assert(routed.jest(nested_file), "the nested jest package lost its own file")
    assert(not routed.vitest(nested_file), "vitest crossed a package boundary")
    assert(claim_count(routed, nested_file) == 1, "expected exactly one adapter to claim the file")
    assert(routed.vitest(vitest_root_file), "vitest gave up a file in the package that declares it")
    assert(claim_count(routed, vitest_root_file) == 1, "expected exactly one adapter to claim the file")
    -- A nested package that names no runner declares nothing, so it overrides nothing: the file
    -- stays with whichever adapter's own detection reaches the ancestor.
    assert(routed.vitest(silent_nested_file), "an empty nested manifest took the file from vitest")
    assert(claim_count(routed, silent_nested_file) == 1, "expected exactly one adapter to claim the file")
  end,

  ["a file importing node:test is node:test's, whatever the project declares"] = function()
    -- node:test attaches to every package holding a package.json and no filename rule tells its
    -- files from vitest's or jest's, so the import is what identifies them. Without this the
    -- file has no claimant at all: vitest and jest both answer no for a project that declares
    -- neither, and dropping the adapter left nothing behind them.
    local routed = route(all_claim())
    assert(routed.node(node_file), "a node:test file had no claimant")
    assert(not routed.vitest(node_file), "vitest did not stand down for a node:test file")
    assert(not routed.jest(node_file), "jest did not stand down for a node:test file")
    assert(claim_count(routed, node_file) == 1, "expected exactly one adapter to claim the file")
  end,

  ["a jest project vitest does not claim stays jest's"] = function()
    local routed = route({ vitest = no, jest = any_javascript, node = any_javascript })
    assert(routed.jest(plain_file), "jest gave up a file vitest does not claim")
    assert(claim_count(routed, plain_file) == 1, "expected exactly one adapter to claim the file")
  end,

  ["the configured predicates answer a nil path instead of raising"] = function()
    -- neotest types `file_path` as optional, and an error raised inside `is_test_file` loses the
    -- position silently rather than surfacing, which reads as a runner flake. Each rule asks its
    -- own adapter's default FIRST, and that answer is what keeps a nil path away from the helper
    -- that opens the file. Reversing the terms is what this goes red on.
    local routed = route(all_claim())
    for name, predicate in pairs({ vitest = routed.vitest, jest = routed.jest, node = routed.node }) do
      local ok, err = pcall(predicate, nil)
      assert(ok, name .. " raised on a nil path: " .. tostring(err))
    end
  end,

  ["neotest-golang is constructed, because its options are populated only by the call"] = function()
    -- At the pinned commit `M.Adapter.options` is assigned inside `__call` alone and read by
    -- `filter_dir`, so the bare module raises in any Go module with a subdirectory.
    local routed = route(all_claim())
    assert(routed.by_name["neotest-golang"], "neotest-golang is not in the adapter list")
    assert(routed.by_name["neotest-golang"].constructed, "neotest-golang was passed as a bare module")
  end,
}
