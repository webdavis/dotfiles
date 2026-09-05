-- The JavaScript adapter routing in lua/plugins/neotest.lua (spec 5.3), and the directory run
-- behind `<leader>ta`. Three adapters claim JavaScript and neotest walks its adapter map with
-- `pairs`, so without a decision here the adapter that runs a file, or a directory, varies
-- between runs.
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

local node_file = write_fixture("tests/node.test.js", 'import { test } from "node:test";\n')
local plain_file = write_fixture("tests/plain.test.js", 'import { test } from "vitest";\n')

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
--- configured `is_test_file`, plus the adapter list neotest was handed.
---@param owns { vitest: fun(path: string?): boolean, jest: fun(path: string?): boolean, node: fun(path: string?): boolean }
local function route(owns)
  local vitest = { name = "neotest-vitest", root = package_root, is_test_file = owns.vitest }
  setmetatable(vitest, {
    __call = function(_, opts)
      -- The pin ASSIGNS a new closure here. Its dependency gate ahead of the caller's predicate
      -- is not modelled: measured against the pin, that gate answers a nil path rather than
      -- raising, so it changes no answer this spec asks for.
      vitest.is_test_file = opts.is_test_file
      return vitest
    end,
  })

  local jest = { name = "neotest-jest", root = package_root }
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
    ["neotest-jest.jest-util"] = { defaultIsTestFile = owns.jest },
    ["neotest-nodejs"] = node,
    ["neotest-nodejs.node-util"] = { defaultIsTestFile = owns.node },
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
  ---@return { ran: table|string|nil, prompted: string[]|nil }
  local function press_run_all(directory, choose)
    ran = nil
    local prompted
    local previous_select = vim.ui.select
    local previous_directory = vim.fn.getcwd()
    vim.ui.select = function(items, _, on_choice)
      prompted = vim.tbl_map(function(item)
        return item.name
      end, items)
      on_choice(choose and items[choose] or nil)
    end
    vim.cmd.cd(directory)
    local pressed, pressed_err = pcall(run_all)
    vim.cmd.cd(previous_directory)
    vim.ui.select = previous_select
    assert(pressed, "<leader>ta failed: " .. tostring(pressed_err))
    return { ran = ran, prompted = prompted }
  end

  return {
    vitest = by_name["neotest-vitest"].is_test_file,
    jest = by_name["neotest-jest"].is_test_file,
    node = by_name["neotest-nodejs"].is_test_file,
    by_name = by_name,
    press_run_all = press_run_all,
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

  ["a directory run names the adapter the nearest package declares"] = function()
    -- neotest accepts a directory without asking any adapter's `is_test_file`, then picks through
    -- `pairs`, so a package with two attached adapters ran a different one between presses
    -- (measured: 11 vitest and 9 jest over 20 fresh processes). An adapter id is `<name>:<root>`
    -- and `run.run` takes one verbatim, so naming it is what makes the choice explicit.
    local routed = route(all_claim())

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
  end,

  ["a directory run with no runner declared asks rather than picking one"] = function()
    local routed = route(all_claim())

    local asked = routed.press_run_all(directory_of(silent), 2)
    assert(asked.prompted, "the run picked an adapter silently")
    assert(#asked.prompted == 3, "expected all three attached adapters to be offered")
    assert(
      asked.ran and asked.ran.adapter == asked.prompted[2] .. ":" .. directory_of(silent),
      "the operator's choice was not the adapter that ran"
    )

    local declined = routed.press_run_all(directory_of(silent))
    assert(not declined.ran, "declining the prompt still ran something")
  end,
}
