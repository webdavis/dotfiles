-- The JavaScript adapter routing in lua/plugins/neotest.lua (spec 5.3). vitest and jest both
-- claim `*.test.js` in a project that declares both runners, and neotest walks its adapter map
-- with `pairs`, so without a decision here the adapter that runs a file varies between runs.
--
-- The adapter modules and `neotest.setup` are stubbed, so this runs headless with no plugin
-- installed and no test runner process. The jest stub mirrors the contract of the pinned
-- adapter: jest REBINDS the upvalue its `adapter.is_test_file` reads rather than replacing the
-- function, so `adapter.is_test_file` is the same object before and after the call and
-- capturing it to compose against would recurse.

local plugin_spec = require("plugins.neotest")[1]

--- Run the plugin spec's `config` against stubbed adapters and return each JavaScript adapter's
--- configured `is_test_file`, plus the adapter list neotest was handed.
---@param owns { vitest: fun(path: string?): boolean, jest: fun(path: string?): boolean }
local function route(owns)
  local vitest = { name = "vitest", is_test_file = owns.vitest }

  local jest = { name = "jest" }
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
    ["neotest-python"] = { name = "python" },
    ["neotest-golang"] = setmetatable({}, {
      __call = function()
        return { name = "golang", constructed = true }
      end,
    }),
    ["neotest-busted"] = { name = "busted" },
    ["neotest-swift-testing"] = { name = "swift" },
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
  return { vitest = by_name.vitest.is_test_file, jest = by_name.jest.is_test_file, by_name = by_name }
end

local function no()
  return false
end

--- vitest's claim, faithful to the pin: `adapter.is_test_file` runs `string.match` on the path
--- with NO nil guard, so it RAISES on nil exactly as the real one does. That is what makes the
--- guard in the config's jest predicate load-bearing rather than decorative.
local function any_javascript(path)
  return path:match("%.test%.js$") ~= nil
end

--- jest's claim, faithful to the pin: `jest_util.defaultIsTestFile` opens with a nil guard.
local function any_javascript_nil_safe(path)
  return path ~= nil and path:match("%.test%.js$") ~= nil
end

--- How many of the two JavaScript adapters claim `path`.
local function claim_count(routed, path)
  local count = 0
  for _, predicate in ipairs({ routed.vitest, routed.jest }) do
    if predicate(path) then
      count = count + 1
    end
  end
  return count
end

local a_test_file = "/project/tests/math.test.js"

return {
  ["a project declaring both runners yields exactly one adapter"] = function()
    local routed = route({ vitest = any_javascript, jest = any_javascript_nil_safe })
    assert(claim_count(routed, a_test_file) == 1, "expected exactly one adapter to claim the file")
    assert(routed.vitest(a_test_file), "the documented precedence gives vitest the file")
    assert(not routed.jest(a_test_file), "jest did not stand down for vitest")
  end,

  ["a jest project vitest does not claim stays jest's"] = function()
    local routed = route({ vitest = no, jest = any_javascript_nil_safe })
    assert(routed.jest(a_test_file), "jest gave up a file vitest does not claim")
    assert(claim_count(routed, a_test_file) == 1, "expected exactly one adapter to claim the file")
  end,

  ["jest is asked through its own predicate, not a copy of its dependency detection"] = function()
    -- The pinned adapters resolve their manifests through the working directory and the git root
    -- as well as the nearest one, so a local reimplementation disagrees with them. Standing down
    -- has to follow vitest's OWN answer: flip only that, and the routing has to follow.
    local routed = route({ vitest = any_javascript, jest = any_javascript_nil_safe })
    assert(not routed.jest(a_test_file), "jest ignored vitest's answer")
    local other = route({ vitest = no, jest = any_javascript_nil_safe })
    assert(other.jest(a_test_file), "jest ignored vitest's answer")
  end,

  ["the jest predicate answers a nil path instead of raising"] = function()
    -- neotest does ask, and an error raised inside `is_test_file` loses the position silently
    -- rather than surfacing, which reads as a runner flake. vitest's predicate raises on nil, so
    -- what keeps this safe is the term ORDER: jest's own default answers false first and
    -- short-circuits. Reversing the two terms is what this goes red on.
    local routed = route({ vitest = any_javascript, jest = any_javascript_nil_safe })
    local ok, err = pcall(routed.jest, nil)
    assert(ok, "jest raised on a nil path: " .. tostring(err))
  end,

  ["neotest-golang is constructed, because its options are populated only by the call"] = function()
    -- At the pinned commit `M.Adapter.options` is assigned inside `__call` alone and read by
    -- `filter_dir`, so the bare module raises in any Go module with a subdirectory.
    local routed = route({ vitest = no, jest = no })
    assert(routed.by_name.golang, "neotest-golang is not in the adapter list")
    assert(routed.by_name.golang.constructed, "neotest-golang was passed as a bare module")
  end,
}
