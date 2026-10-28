-- Routes JavaScript test files to one adapter, because neotest walks its adapter map with `pairs`.
-- Adapters are stubbed. The fixture is a real package layout, and tree-sitter is real when a grammar is installed.

local plugin_spec = require("plugins.neotest")[1]

vim.opt.runtimepath:append(vim.fn.stdpath("data") .. "/site")
local _, javascript_grammar = pcall(vim.treesitter.language.add, "javascript")
javascript_grammar = javascript_grammar == true

-- Made-up names, because the real grammar directory is on the runtimepath. The gate remembers an empty wait, so each failing build gets its own name.
local PARSERLESS_LANGUAGE = "language_with_no_parser_on_disk"
local STALLED_LANGUAGE = "language_whose_build_never_finishes"
local FAILING_LANGUAGE = "language_whose_build_fails"
for _, language in ipairs({ PARSERLESS_LANGUAGE, STALLED_LANGUAGE, FAILING_LANGUAGE }) do
  assert(
    #vim.api.nvim_get_runtime_file("parser/" .. language .. ".*", true) == 0,
    "a parser for " .. language .. " exists, so the gate's waiting path is no longer measured"
  )
end

-- Keys whose handler goes through the parser gate and takes only the buffer. `<leader>ta` also takes a directory, so it is pressed separately.
local GATED_KEYS = { "<leader>tt", "<leader>tf", "<leader>ts" }

-- Realpath'd, because `:cd` resolves the macOS temp-directory symlink.
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

-- JSX written before the import, which the TypeScript grammar cannot parse.
local jsx_file =
  write_fixture("vitest-root/tests/view.test.tsx", 'const view = <div />;\nimport test from "node:test";\n')

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

-- Two non-JavaScript adapters rooted together, once with a stray manifest the JavaScript ones
-- root on, and once with the only manifest outside the repository boundary.
local polyglot = write_fixture("polyglot/pyproject.toml", '[project]\nname = "polyglot"\n')
write_fixture("polyglot/.busted", "return {}\n")
write_fixture("polyglot/package.json", "{}")
write_fixture("outside/package.json", '{ "devDependencies": { "vitest": "3.2.7" } }')
local walled = write_fixture("outside/repo/.git", "gitdir: /nowhere\n")
write_fixture("outside/repo/pyproject.toml", '[project]\nname = "walled"\n')
write_fixture("outside/repo/.busted", "return {}\n")

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

-- Two JavaScript repositories declaring no runner, alike but for one bash test file.
-- `.git` at the top of both is what neotest-bashunit roots on, so the bash file tells them apart.
local jsnobash = write_fixture("jsnobash/package.json", '{ "name": "jsnobash", "version": "1.0.0" }')
write_fixture("jsnobash/.git", "gitdir: /nowhere\n")
write_fixture("jsnobash/tests/thing.test.js", 'test("thing", function () {});\n')

local jsbash = write_fixture("jsbash/package.json", '{ "name": "jsbash", "version": "1.0.0" }')
write_fixture("jsbash/.git", "gitdir: /nowhere\n")
write_fixture("jsbash/tests/thing.test.js", 'test("thing", function () {});\n')
write_fixture("jsbash/tests/thing.test.sh", "function test_thing() {\n  assert_same 1 1\n}\n")

local function directory_of(manifest)
  return vim.fn.fnamemodify(manifest, ":h")
end

--- The root all three JavaScript adapters resolve: the nearest ancestor holding a package.json.
local function package_root(path)
  return vim.fs.root(path, "package.json")
end

--- neotest-python roots on a Python project marker, not on a package.json.
local function python_root(path)
  return vim.fs.root(path, "pyproject.toml")
end

--- neotest-busted roots on its own markers, one of which is a `.busted` file.
local function busted_root(path)
  return vim.fs.root(path, ".busted")
end

--- neotest-bashunit trusts a `.bashunitrc`, and otherwise claims a repository only when a `*.test.sh` is reachable inside it.
local function bashunit_root(path)
  local root = vim.fs.root(path, { ".bashunitrc", ".git" })
  if not root or vim.uv.fs_stat(root .. "/.bashunitrc") then
    return root
  end
  local found = vim.fs.find(function(name)
    return name:sub(-8) == ".test.sh"
  end, { path = root, type = "file", limit = 1 })
  return found[1] and root or nil
end

local function no_root()
  return nil
end

--- Run the plugin `config` against stubbed adapters. Returns each JavaScript `is_test_file`, the adapter list, and key pressers.
--- @param rust_discover_positions (fun(file_path: string): table)? Stubs `rustaceanvim.neotest.discover_positions`.
local function route(rust_discover_positions)
  -- Real `nio.sleep` raises outside an nio coroutine, so count calls instead of sleeping.
  local nio_sleep_calls = 0
  package.loaded["nio"] = {
    sleep = function()
      nio_sleep_calls = nio_sleep_calls + 1
    end,
  }

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

  -- nvim-treesitter is faked. `pwait` answers `false, "timeout"` for a timeout, `true, false` for an empty build, and `true, true` for a parser.
  local pwait_answers = {
    [PARSERLESS_LANGUAGE] = { true, true },
    [STALLED_LANGUAGE] = { false, "timeout" },
    [FAILING_LANGUAGE] = { true, false },
  }
  local parser = { installs = {}, available = vim.tbl_keys(pwait_answers) }

  local captured, ran, requests = nil, nil, 0
  local stubs = {
    ["pns.integrations.neotest"] = { consumer = function() end },
    ["nvim-treesitter"] = {
      install = function(languages, options)
        local answer
        for _, language in ipairs(languages) do
          parser.installs[#parser.installs + 1] = language
          assert(options and options.force, "an install of a missing parser must force: " .. language)
          answer = pwait_answers[language]
          assert(answer, "no pwait answer is declared for " .. language)
        end
        -- The task `install` returns. The gate uses `pwait`, which reports instead of raising.
        return {
          pwait = function()
            return answer[1], answer[2]
          end,
        }
      end,
    },
    ["nvim-treesitter.config"] = {
      get_available = function()
        return vim.deepcopy(parser.available)
      end,
    },
    ["neotest"] = {
      setup = function(options)
        captured = options
      end,
      run = {
        run = function(args)
          requests = requests + 1
          ran = args
        end,
      },
      -- `<leader>ts` reaches the module here, so count it.
      summary = {
        toggle = function()
          requests = requests + 1
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
    -- A plain table like the real module, rooted by the real rule, so an over-claiming bash adapter shows in directory runs.
    ["neotest-bashunit"] = { name = "neotest-bashunit", root = bashunit_root },
    ["neotest-busted"] = { name = "neotest-busted", root = busted_root },
    ["neotest-swift-testing"] = { name = "neotest-swift-testing", root = no_root },
    -- rustaceanvim ships its adapter under its own module name.
    ["rustaceanvim.neotest"] = {
      name = "rustaceanvim",
      root = no_root,
      discover_positions = rust_discover_positions,
    },
    ["neotest-java"] = setmetatable({}, {
      __call = function()
        return { name = "neotest-java", root = no_root, constructed = true }
      end,
    }),
    ["neotest-elixir"] = { name = "neotest-elixir", root = no_root },
    -- A plain table like the real module, because `config` requires that shape.
    ["neotest-zig"] = { name = "neotest-zig", root = no_root },
  }
  -- Left in place, because the configured predicates run after this returns.
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

  local run_all, gated = nil, {}
  for _, key in ipairs(plugin_spec.keys) do
    if key[1] == "<leader>ta" then
      run_all = key[2]
    elseif vim.list_contains(GATED_KEYS, key[1]) then
      gated[key[1]] = key[2]
    end
  end
  assert(run_all, "<leader>ta is not in the plugin spec's keys")
  for _, key in ipairs(GATED_KEYS) do
    assert(gated[key], key .. " is not in the plugin spec's keys")
  end

  --- Call `press` from a scratch buffer of `filetype`, then restore the buffer list. A nil `filetype` uses the current buffer.
  ---@param filetype string?
  ---@param press fun()
  ---@return boolean, string?
  local function press_in_buffer(filetype, press)
    if not filetype then
      return pcall(press)
    end
    local buffer = vim.api.nvim_create_buf(false, true)
    local previous_buffer = vim.api.nvim_get_current_buf()
    vim.api.nvim_set_option_value("filetype", filetype, { buf = buffer })
    vim.api.nvim_set_current_buf(buffer)
    local pressed, pressed_error = pcall(press)
    vim.api.nvim_set_current_buf(previous_buffer)
    vim.api.nvim_buf_delete(buffer, { force = true })
    return pressed, pressed_error
  end

  --- Press `<leader>ta` in `directory`. `choose` is the item picked, and `filetype` is the buffer the press comes from.
  ---@return { ran: table|string|nil, prompted: string[]|nil, notified: string[], installs: string[] }
  local function press_run_all(directory, choose, filetype)
    ran = nil
    parser.installs = {}
    local installs, prompted, notified = parser.installs, nil, {}
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
    local pressed, pressed_err = press_in_buffer(filetype, run_all)
    vim.cmd.cd(previous_directory)
    vim.ui.select, vim.notify = previous_select, previous_notify
    assert(pressed, "<leader>ta failed: " .. tostring(pressed_err))
    return { ran = ran, prompted = prompted, notified = notified, installs = installs }
  end

  --- Press one of `GATED_KEYS` from a `filetype` buffer and report what the request waited for.
  --- Notify only records, because headless noice drops the continuation.
  ---@param key string
  ---@param filetype string
  ---@return { installs: string[], requests: integer, notified: string[] }
  local function press_gated(key, filetype)
    parser.installs = {}
    local installs, notified, before = parser.installs, {}, requests
    local previous_notify = vim.notify
    vim.notify = function(message)
      notified[#notified + 1] = tostring(message)
    end
    local pressed, pressed_error = press_in_buffer(filetype, gated[key])
    vim.notify = previous_notify
    assert(pressed, key .. " failed: " .. tostring(pressed_error))
    return { installs = installs, requests = requests - before, notified = notified }
  end

  return {
    vitest = by_name["neotest-vitest"].is_test_file,
    jest = by_name["neotest-jest"].is_test_file,
    node = by_name["neotest-nodejs"].is_test_file,
    by_name = by_name,
    press_run_all = press_run_all,
    press_gated = press_gated,
    nio_sleep_calls = function()
      return nio_sleep_calls
    end,
  }
end

--- The one adapter claiming `path`, or nil. Two claimants fail the case.
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

--- node:test where the JavaScript grammar is installed, otherwise `without_grammar`.
local function node_or(without_grammar)
  return javascript_grammar and "neotest-nodejs" or without_grammar
end

local cases = {}

cases["one package declaring both runners gives its files to vitest"] = function()
  local routed = route()
  assert(owner(routed, dual_file) == "neotest-vitest", "the documented precedence gives vitest the file")
end

cases["a nested jest package inside a vitest repository keeps its own files"] = function()
  -- The nearest package naming a runner wins. Precedence only settles a package declaring both.
  local routed = route()
  assert(owner(routed, nested_file) == "neotest-jest", "the nested jest package lost its own file")
  assert(owner(routed, vitest_root_file) == "neotest-vitest", "vitest gave up a file in the package that declares it")
  -- A nested package naming no runner declares nothing, so the ancestor's choice still stands.
  assert(owner(routed, silent_nested_file) == "neotest-vitest", "an empty nested manifest took the file from vitest")
end

cases["a nested git repository does not inherit a runner from outside it"] = function()
  -- The repository boundary stops the manifest walk and rejects adapters rooted outside it.
  local routed = route()
  assert(owner(routed, inner_file) == nil, "the nested repository inherited a runner from outside it")
  -- neotest picks a directory adapter without `is_test_file`, so a refused root must not fall back to a plain run.
  local run = routed.press_run_all(directory_of(inner_git))
  assert(not run.prompted, "the run offered adapters rooted outside the repository")
  assert(not run.ran, "the run fell back to letting neotest choose, got " .. vim.inspect(run.ran))
  assert(#run.notified == 1, "the operator was not told why nothing ran")
end

cases["a runner an intermediate package declares is found through a nested empty manifest"] = function()
  -- The runner comes from the declaring package behind the empty manifest.
  local routed = route()
  assert(owner(routed, behind_empty_file) == "neotest-vitest", "the intermediate package's runner was lost")
end

cases["node:test owns the file names its own matcher rejects"] = function()
  -- The pinned matchers disagree on names, so ownership and standing down use one rule.
  local routed = route()
  for _, path in ipairs({ e2e_node_file, mjs_node_file, cjs_node_file }) do
    assert(owner(routed, path) == node_or("neotest-vitest"), "a node:test file had the wrong claimant: " .. path)
  end
end

cases["a regular expression is neither an import nor a place to hide one"] = function()
  -- Regex literals hide no import and fake none.
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
  -- Type-only imports vanish before runtime, so the runtime import picks the runner.
  local routed = route()
  assert(owner(routed, type_import_file) == "neotest-vitest", "a type-only import claimed the file for node:test")
  assert(owner(routed, type_export_file) == "neotest-vitest", "a type-only re-export claimed the file for node:test")
end

cases["JSX written before an import does not swallow it"] = function()
  -- The TypeScript grammar has no JSX, so the import lands in an error node and the package runner wins.
  local _, tsx = pcall(vim.treesitter.language.add, "tsx")
  local routed = route()
  assert(
    owner(routed, jsx_file) == (tsx == true and "neotest-nodejs" or "neotest-vitest"),
    "JSX before the import cost node:test the file"
  )
end

cases["a parse happens once per file version, and never without the literal"] = function()
  -- Cache the answer per file size and mtime, because all three adapters ask and the neotest filter pass never yields.
  -- A file that never names node:test is answered from its bytes without parsing.
  local routed = route()
  local parses = 0
  local original = vim.treesitter.get_string_parser
  vim.treesitter.get_string_parser = function(...)
    parses = parses + 1
    return original(...)
  end

  local path = write_fixture("cached/tests/a.test.js", 'import { test } from "node:test";\n')
  local answers = { routed.vitest(path), routed.jest(path), routed.node(path) }
  local after_first = parses

  -- Still names node:test, so this version is a real question again and not the same answer.
  write_fixture("cached/tests/a.test.js", 'import { test } from "vitest";\n// no longer from "node:test"\n')
  local mentioned = routed.node(path)
  local after_change = parses

  -- Names it nowhere, so no parse tree can hold an import of it.
  write_fixture("cached/tests/a.test.js", 'import { test } from "vitest";\n')
  local unmentioned = { routed.vitest(path), routed.jest(path), routed.node(path) }
  local after_unmentioned = parses
  vim.treesitter.get_string_parser = original

  -- Without a grammar there is nothing to parse, and the count is what says so.
  local per_version = javascript_grammar and 1 or 0
  assert(after_first == per_version, "expected " .. per_version .. " parse for three adapters, got " .. after_first)
  assert(answers[3] == javascript_grammar, "the node:test answer changed under the cache")
  assert(not answers[1] and not answers[2], "an adapter claimed a file it does not own")
  assert(after_change == per_version * 2, "a rewritten file was not parsed again, parses: " .. after_change)
  assert(not mentioned, "a mention in a comment was read as an import, or the cache outlived the bytes")
  assert(after_unmentioned == after_change, "a file that never names node:test was parsed anyway")
  assert(not unmentioned[3], "a file importing nothing of node:test was claimed by it")
end

cases["a file whose language has no grammar has no node:test owner"] = function()
  -- Nothing parses CoffeeScript, so the package's declared runner takes the file.
  local routed = route()
  assert(owner(routed, coffee_file) == "neotest-vitest", "an unparseable language was given to node:test")
end

cases["a node:test mention inside a comment is not an import"] = function()
  -- A note naming the module is not an import.
  local routed = route()
  for _, path in ipairs({ commented_file, block_commented_file }) do
    assert(owner(routed, path) == "neotest-vitest", "a comment cost the package's own runner a file: " .. path)
  end
end

cases["a string that looks like an import is not one, and a URL does not hide one"] = function()
  -- A string holding `require("node:test")` is not an import, and a `//` inside a URL ends no line.
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
  -- A long preamble pushes the import down, so read the whole file.
  local routed = route()
  assert(owner(routed, late_import_file) == node_or("neotest-vitest"), "a preamble hid the import")
end

cases["a file importing node:test is node:test's, whatever the project declares"] = function()
  -- Nothing above this file declares a runner, so without a grammar nobody owns it at all.
  local routed = route()
  assert(owner(routed, node_file) == node_or(nil), "a node:test file had the wrong claimant")
end

cases["the configured predicates answer a nil path instead of raising"] = function()
  -- The filename rule answers first, so a nil `file_path` never reaches the tree reads.
  local routed = route()
  for name, predicate in pairs({ vitest = routed.vitest, jest = routed.jest, node = routed.node }) do
    local ok, err = pcall(predicate, nil)
    assert(ok, name .. " raised on a nil path: " .. tostring(err))
  end
end

cases["the predicates answer inside a fast event, where a Vimscript read would raise"] = function()
  -- neotest asks from async contexts where Vimscript calls raise E5560, so ask from a libuv callback.
  -- The routing warms tree-sitter up front, because its first load creates an augroup.
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
  -- `M.Adapter.options` is set only in `__call`, so the bare module raises in a Go module with a subdirectory.
  local routed = route()
  assert(routed.by_name["neotest-golang"], "neotest-golang is not in the adapter list")
  assert(routed.by_name["neotest-golang"].constructed, "neotest-golang was passed as a bare module")
end

cases["a directory run names the adapter the nearest package declares"] = function()
  -- neotest picks a directory adapter through `pairs`, so name the `<name>:<root>` adapter id explicitly.
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

  -- The intermediate package's runner reaches the directory run through the empty manifest.
  local intermediate = routed.press_run_all(directory_of(behind_empty))
  assert(
    intermediate.ran and intermediate.ran.adapter == "neotest-vitest:" .. directory_of(behind_empty),
    "expected vitest by name, got " .. tostring(intermediate.ran and intermediate.ran.adapter)
  )
end

cases["a directory run dispatches the only non-JavaScript adapter rather than asking"] = function()
  -- A stray package.json roots the JavaScript adapters, but exactly one other adapter is rooted, so nothing is asked.
  local routed = route()
  local python = routed.press_run_all(directory_of(pystray))
  assert(not python.prompted, "the run asked when only one adapter could have been meant")
  assert(
    python.ran and python.ran.adapter == "neotest-python:" .. directory_of(pystray),
    "expected python by name, got " .. tostring(python.ran and python.ran.adapter)
  )
end

cases["a directory run refuses a root whose packages declare different runners"] = function()
  -- One adapter id covers one tree, so a nested package with another runner blocks the root run.
  -- From the nested package itself the run goes ahead.
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
  -- Compare relative path parts, so nested `node_modules` and hidden directories are skipped.
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
  -- Packages at any depth count.
  local routed = route()
  local run = routed.press_run_all(directory_of(deep))
  assert(not run.ran, "a conflicting package below the old ceiling went unseen")
  assert(
    run.notified[1] and run.notified[1]:find("packages/team/apps/api", 1, true),
    "the message does not name the deep package: " .. tostring(run.notified[1])
  )
end

cases["a directory run offers every non-JavaScript adapter rooted here"] = function()
  -- Two non-JavaScript adapters rooted is not a JavaScript question, so offer those two.
  local routed = route()

  local stray = routed.press_run_all(directory_of(polyglot), 2)
  assert(
    vim.deep_equal(stray.prompted, { "neotest-python", "neotest-busted" }),
    "expected both non-JavaScript adapters, got " .. vim.inspect(stray.prompted)
  )
  assert(
    stray.ran and stray.ran.adapter == "neotest-busted:" .. directory_of(polyglot),
    "the operator's choice was not the adapter that ran"
  )

  -- The same pair, with every JavaScript root refused for sitting outside the repository.
  local refused = routed.press_run_all(directory_of(walled), 1)
  assert(
    vim.deep_equal(refused.prompted, { "neotest-python", "neotest-busted" }),
    "a refusal reached the operator instead of the adapters that did root here"
  )
  assert(
    refused.ran and refused.ran.adapter == "neotest-python:" .. directory_of(walled),
    "the operator's choice was not the adapter that ran"
  )
end

cases["a directory run stops on a package.json it cannot parse"] = function()
  -- An unreadable manifest reports its reason instead of opening a chooser.
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

cases["a directory run reaches bash only where a bash test actually lives"] = function()
  -- `.git` tops every repository, so a project with no bash tests must not run through bashunit.
  local routed = route()

  local without = routed.press_run_all(directory_of(jsnobash), 2)
  assert(without.prompted, "a JavaScript project with no bash test picked an adapter silently")
  assert(
    not vim.tbl_contains(without.prompted, "neotest-bashunit"),
    "bash was offered in a project holding no bash test: " .. vim.inspect(without.prompted)
  )
  assert(
    without.ran and not without.ran.adapter:find("neotest-bashunit", 1, true),
    "a project holding no bash test still ran bash: " .. vim.inspect(without.ran)
  )

  -- The same layout plus one `.test.sh`, where bash is the one adapter rooted on a marker of its
  -- own, so it runs without asking.
  local with = routed.press_run_all(directory_of(jsbash))
  assert(not with.prompted, "the run asked even though one adapter rooted on its own marker")
  assert(
    with.ran and with.ran.adapter == "neotest-bashunit:" .. directory_of(jsbash),
    "the bash test's own repository did not run bash: " .. vim.inspect(with.ran)
  )
end

cases["a test request waits for the parser its discovery needs"] = function()
  -- Discovery needs the grammar, and parsers install asynchronously, so the request waits for its parser.
  local routed = route()

  -- Every gated key discovers from the current buffer.
  for _, key in ipairs(GATED_KEYS) do
    local waited = routed.press_gated(key, PARSERLESS_LANGUAGE)
    assert(
      vim.deep_equal(waited.installs, { PARSERLESS_LANGUAGE }),
      key .. " did not wait for its parser: " .. vim.inspect(waited.installs)
    )
    assert(waited.requests == 1, key .. " never reached neotest")
    assert(#waited.notified == 1, key .. " blocked on a build without saying why")
  end

  -- `lua` is bundled, so a loaded parser is never reinstalled.
  local loaded = routed.press_gated("<leader>tt", "lua")
  assert(#loaded.installs == 0, "a parser already loaded was installed again: " .. vim.inspect(loaded.installs))
  assert(loaded.requests == 1, "the request never reached neotest")
  assert(#loaded.notified == 0, "an installed parser still announced an install")

  -- No grammar is named after snacks notification buffers, so nothing is waited for.
  local unbuildable = routed.press_gated("<leader>tt", "snacks_notif")
  assert(
    #unbuildable.installs == 0,
    "a filetype no grammar exists for was installed: " .. vim.inspect(unbuildable.installs)
  )
  assert(unbuildable.requests == 1, "the request never reached neotest")

  -- A build that delivers no parser is waited for once per session, whichever way it ends.
  for _, language in ipairs({ STALLED_LANGUAGE, FAILING_LANGUAGE }) do
    local first = routed.press_gated("<leader>tt", language)
    assert(vim.deep_equal(first.installs, { language }), "the first press did not wait for " .. language)
    local retried = routed.press_gated("<leader>tt", language)
    assert(
      #retried.installs == 0,
      "a build that already delivered nothing was waited for again: " .. vim.inspect(retried.installs)
    )
    assert(retried.requests == 1, "the request never reached neotest")
  end

  -- `<leader>ta` parses the buffer it was pressed from, so it goes through the gate too.
  -- A parserless buffer, so the per-session memo stays unarmed.
  local directory_run = routed.press_run_all(directory_of(dual), nil, PARSERLESS_LANGUAGE)
  assert(
    vim.deep_equal(directory_run.installs, { PARSERLESS_LANGUAGE }),
    "<leader>ta did not wait for its parser: " .. vim.inspect(directory_run.installs)
  )
  assert(directory_run.ran, "<leader>ta never reached neotest")
end

cases["the rust readiness wrapper retries once for the load-window race, then stops retrying"] = function()
  -- Call 1 simulates rust-analyzer not answering yet. Call 3 is a test-less file that must not be retried.
  local call_count = 0
  local good_tree = {
    children = function()
      return { {}, {} }
    end,
  }
  local routed = route(function()
    call_count = call_count + 1
    if call_count == 1 or call_count == 3 then
      error("rust-analyzer not ready")
    end
    return good_tree
  end)

  local rust = routed.by_name["rustaceanvim"]
  assert(rust, "rustaceanvim is not in the adapter list")
  assert(rust.discover_positions, "the retry wrapper was not installed")

  local tree = rust.discover_positions("/tmp/first.rs")
  assert(tree == good_tree, "the wrapper did not return the tree rust-analyzer produced once ready")
  assert(call_count == 2, "the wrapper did not retry exactly once before succeeding: got " .. call_count)
  assert(routed.nio_sleep_calls() == 1, "the wrapper did not wait between its retries")

  -- Once warmed, a test-less file that raises is not retried to the deadline.
  local no_tree = rust.discover_positions("/tmp/second-no-tests.rs")
  assert(no_tree == nil, "a warmed call that raised was retried instead of returning nil once")
  assert(call_count == 3, "a warmed call retried the underlying adapter: got " .. call_count)
  assert(routed.nio_sleep_calls() == 1, "a warmed call slept, defeating the point of warming up")
end

cases["the Zig adapter is in the adapter list, and its row loads on a .zig buffer"] = function()
  -- The adapter reaches `neotest.setup`, and its spec row loads on the filetype a `.zig` file gets.
  local routed = route()
  assert(routed.by_name["neotest-zig"], "neotest-zig is not in the adapter list")

  local filetype = vim.filetype.match({ filename = "/scratch/project/src/root.zig" })
  assert(filetype == "zig", "a .zig path resolves to filetype " .. tostring(filetype))

  local row
  for _, spec in ipairs(require("plugins.neotest")) do
    if spec[1] == "webdavis/neotest-zig" then
      row = spec
    end
  end
  assert(row, "no plugin spec row installs webdavis/neotest-zig")
  assert(row.ft == filetype, "the row loads on " .. vim.inspect(row.ft) .. ", not " .. filetype)
  assert(row.commit, "the row is unpinned")
end

cases["when the cases are done, the fixture tree is deleted"] = function()
  -- `os.exit` skips the temp cleanup, so the last case deletes the fixture tree.
  -- The ordering complaint is raised after the delete.
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
