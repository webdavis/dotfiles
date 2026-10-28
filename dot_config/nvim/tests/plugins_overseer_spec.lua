-- Reads the errorformat from the shipped config, with `overseer` faked, and feeds it real tool output.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

-- Capture the setup() options without loading overseer itself.
local captured_hooks = {}
local captured_maps = {}
local captured_overseer
local captured_view_opts

local function captured_setup_opts()
  local opts
  captured_hooks = {}
  captured_maps = {}
  captured_view_opts = nil
  local overseer_fake = {
    setup = function(o)
      opts = o
    end,
    run_task = function() end,
    run_action = function() end,
    add_template_hook = function(hook_opts, hook)
      table.insert(captured_hooks, { opts = hook_opts, hook = hook })
    end,
    preload_task_cache = function() end,
    create_task_output_view = function(_, view_opts)
      captured_view_opts = view_opts
    end,
    new_task = function()
      return { start = function() end }
    end,
    list_tasks = function()
      return {}
    end,
    STATUS = { SUCCESS = "SUCCESS", FAILURE = "FAILURE", CANCELED = "CANCELED", RUNNING = "RUNNING" },
  }

  local saved_overseer = package.loaded["overseer"]
  local saved_map = _G.map
  local saved_user_command = vim.api.nvim_create_user_command
  local saved_autocmd = vim.api.nvim_create_autocmd

  package.loaded["overseer"] = overseer_fake
  captured_overseer = overseer_fake
  -- `map` is a global installed by init.lua, which this runner never loads.
  _G.map = function(spec)
    for _, lhs in ipairs(type(spec.lhs) == "table" and spec.lhs or { spec.lhs }) do
      captured_maps[lhs] = spec.rhs
    end
  end
  vim.api.nvim_create_user_command = function() end
  -- Stub the preload autocmd, so this spec leaves none behind in the runner.
  vim.api.nvim_create_autocmd = function() end

  local ok, err = pcall(function()
    local spec = dofile(config_root .. "/lua/plugins/overseer.lua")
    -- lazy installs the keymaps from the spec `keys` rows, so read them there.
    for _, row in ipairs(spec.keys or {}) do
      captured_maps[row[1]] = row[2]
    end
    spec.config()
  end)

  package.loaded["overseer"] = saved_overseer
  _G.map = saved_map
  vim.api.nvim_create_user_command = saved_user_command
  vim.api.nvim_create_autocmd = saved_autocmd

  assert(ok, err)
  return assert(opts, "overseer.setup() was never called")
end

-- The `on_output_quickfix` entry of the `default` alias, as shipped.
local function quickfix_component()
  local default = assert(captured_setup_opts().component_aliases.default, "no default component alias")
  for _, entry in ipairs(default) do
    if type(entry) == "table" and entry[1] == "on_output_quickfix" then
      return entry
    end
  end
  error("the default alias has no on_output_quickfix entry")
end

---The hook the config registers for EVERY template, the one with no filter.
---@return fun(task_defn: table, util: table)
local function hook_for_all_templates()
  captured_setup_opts()
  for _, entry in ipairs(captured_hooks) do
    if entry.opts == nil then
      return entry.hook
    end
  end
  error("no template hook is registered for every template")
end

---The hook the config registers for the `just` provider.
---@return fun(task_defn: table, util: table)
local function hook_for_just()
  captured_setup_opts()
  for _, entry in ipairs(captured_hooks) do
    if entry.opts and entry.opts.module == "^just$" then
      return entry.hook
    end
  end
  error("no template hook is registered for the just provider")
end

-- The namespace overseer hands a hook. Only the pieces the hooks here touch.
local hook_util = {
  add_component = function(task_defn, comp)
    task_defn.components = task_defn.components or {}
    table.insert(task_defn.components, comp)
  end,
  has_component = function(task_defn, name)
    for _, comp in ipairs(task_defn.components or {}) do
      if comp == name or (type(comp) == "table" and comp[1] == name) then
        return true
      end
    end
    return false
  end,
}

-- One line through the shipped errorformat. Returns the filename or nil, the line number, and the message.
local function parse(line)
  local defn = {}
  hook_for_all_templates()(defn, hook_util)
  local item = vim.fn.getqflist({ lines = { line }, efm = defn.default_component_params.errorformat }).items[1]
  if item.valid ~= 1 then
    return nil
  end
  return vim.fn.bufname(item.bufnr), item.lnum, (item.text or ""):gsub("^%s+", "")
end

---The `experimental_wrap_builtins.condition` the config ships.
---@return fun(cmd: string[], caller: table?, opts: table?): boolean
local function wrap_condition()
  local wrap = assert(captured_setup_opts().experimental_wrap_builtins, "no experimental_wrap_builtins")
  return assert(wrap.condition, "experimental_wrap_builtins ships no condition")
end

return {
  ["a command whose binary is missing is not wrapped"] = function()
    -- The overseer wrapper returns a nil handle on a failed spawn, so `vim.system(cmd):wait()` would index nil.
    assert(
      wrap_condition()({ "kulala-no-such-binary-xyz", "--version" }) == false,
      "a missing binary is still wrapped, so vim.system returns nil instead of raising"
    )
  end,

  ["a command whose binary exists is still wrapped"] = function()
    -- Background jobs stay wrapped, so they stay inspectable.
    assert(
      wrap_condition()({ vim.v.progpath, "--version" }) == true,
      "an existing binary is no longer wrapped, so the feature is off"
    )
  end,

  ["a shell-string jobstart is still wrapped and an empty command is not"] = function()
    -- The string form of `jobstart` always spawns the shell, so it never gets a nil handle.
    assert(wrap_condition()("echo hi") == true, "a shell-string jobstart was declined")
    assert(wrap_condition()({}) == false, "an empty command was wrapped")
  end,

  ["the alias leaves a template's own errorformat alone"] = function()
    -- Setting errorformat in the alias would override each template's own, Cargo's among them.
    assert(
      quickfix_component().errorformat == nil,
      "the alias pins an errorformat, so a template's own is ignored: " .. tostring(quickfix_component().errorformat)
    )
  end,

  ["a template without its own errorformat gets the generic one"] = function()
    local defn = { components = {} }
    hook_for_all_templates()(defn, hook_util)
    assert(defn.default_component_params, "the hook set no default_component_params")
    assert(
      defn.default_component_params.errorformat:match("^FAIL "),
      "the generic format was not applied: " .. tostring(defn.default_component_params.errorformat)
    )
  end,

  ["a template that ships an errorformat keeps it"] = function()
    local defn = { default_component_params = { errorformat = "%f|%l| %m" } }
    hook_for_all_templates()(defn, hook_util)
    assert(
      defn.default_component_params.errorformat == "%f|%l| %m",
      "the template's own errorformat was replaced with " .. tostring(defn.default_component_params.errorformat)
    )
  end,

  ["the just hook scopes uniqueness to the working directory"] = function()
    -- `unique` compares names only, so the name carries the directory.
    local hook = hook_for_just()
    local first = { name = "just test", cwd = "/repo/a" }
    local second = { name = "just test", cwd = "/repo/b" }
    hook(first, hook_util)
    hook(second, hook_util)
    assert(first.name ~= second.name, "both worktrees still produce the name " .. first.name)
    assert(first.name:match("just test"), "the recipe name was lost: " .. first.name)
    assert(first.name:match("/repo/a"), "the directory is not in the name: " .. first.name)
    assert(hook_util.has_component(first, "unique"), "unique was not attached")
  end,

  ["the hook copies the components it was handed"] = function()
    -- `components` is the shared alias table, and component setup writes into it in place.
    local shared = { { "on_output_quickfix" } }
    local defn = { components = shared }
    hook_for_all_templates()(defn, hook_util)
    assert(defn.components ~= shared, "the hook kept the shared alias table")
    assert(defn.components[1] ~= shared[1], "the hook kept a shared component entry")
    assert(defn.components[1][1] == "on_output_quickfix", "the copy lost the component name")
  end,

  ["the just hook names a task the builder left unnamed"] = function()
    -- The `just` builder returns no `name`, so the hook supplies one.
    local hook = hook_for_just()
    local test = { cmd = { "just", "test" }, cwd = "/repo" }
    local lint = { cmd = { "just", "lint-gate" }, cwd = "/repo" }
    hook(test, hook_util)
    hook(lint, hook_util)
    assert(not test.name:match("nil"), "the name is still nil-derived: " .. test.name)
    assert(test.name ~= lint.name, "both recipes share the name " .. test.name)
    -- The command is json-encoded, so the name holds argv words, not a shell line.
    assert(test.name:match("just") and test.name:match("test"), "the recipe is missing from " .. test.name)
  end,

  ["the just hook keeps argument boundaries in the name"] = function()
    -- argv joined with spaces would give {"a b", "c"} and {"a", "b c"} one name.
    local hook = hook_for_just()
    local first = { cmd = { "just", "r", "a b", "c" }, cwd = "/repo" }
    local second = { cmd = { "just", "r", "a", "b c" }, cwd = "/repo" }
    hook(first, hook_util)
    hook(second, hook_util)
    assert(first.name ~= second.name, "both argument shapes share the name " .. first.name)
  end,

  ["the live output view follows the newest task, not a hovered one"] = function()
    -- A new task takes the view, even after hovering an older one.
    captured_setup_opts()
    local rhs = assert(captured_maps["<leader>ov"], "<leader>ov is not mapped")
    local real_cmd = vim.cmd
    local real_overseer = package.loaded["overseer"]
    vim.cmd = function() end
    package.loaded["overseer"] = captured_overseer
    local ok, err = pcall(rhs)
    package.loaded["overseer"] = real_overseer
    vim.cmd = real_cmd
    assert(ok, err)
    local view = assert(captured_view_opts, "the mapping never built an output view")

    local old = { time_start = 100, name = "old", metadata = { start_sequence = 1 } }
    local new = { time_start = 200, name = "new", metadata = { start_sequence = 2 } }
    local chosen = view.select({}, { old, new }, old)
    assert(chosen == new, "the view chose " .. tostring(chosen and chosen.name) .. " while hovering old")
  end,

  ["the live output view separates two starts in the same second"] = function()
    -- `time_start` has one-second resolution, so the start sequence orders tasks.
    captured_setup_opts()
    local rhs = assert(captured_maps["<leader>ov"], "<leader>ov is not mapped")
    local real_cmd = vim.cmd
    local real_overseer = package.loaded["overseer"]
    vim.cmd = function() end
    package.loaded["overseer"] = captured_overseer
    local ok, err = pcall(rhs)
    package.loaded["overseer"] = real_overseer
    vim.cmd = real_cmd
    assert(ok, err)
    local view = assert(captured_view_opts, "the mapping never built an output view")

    local a = { time_start = 100, name = "a", metadata = { start_sequence = 1 } }
    local b = { time_start = 100, name = "b", metadata = { start_sequence = 2 } }
    assert(view.select({}, { a, b }) == b, "the newer of two same-second starts did not win")

    -- And an OLDER task restarted last must win, which a creation order cannot do.
    a.metadata.start_sequence = 3
    assert(view.select({}, { a, b }) == a, "a restarted task did not become the newest")
  end,

  ["the quickfix keeps only lines that parse as a location"] = function()
    assert(quickfix_component().items_only == true, "items_only is not set; every output line reaches the quickfix")
  end,

  ["the quickfix errorformat resolves this repo's test-runner failures"] = function()
    -- `tests/run.lua` prints `FAIL <spec>: <case>: <file>:<line>: <message>`.
    -- The stock `%f` would swallow the whole prefix.
    local file, lnum, text = parse("FAIL util_spec: trim strips whitespace: lua/custom_api/util.lua:42: boom")
    assert(file == "lua/custom_api/util.lua", "resolved the wrong file: " .. tostring(file))
    assert(lnum == 42, "resolved the wrong line: " .. tostring(lnum))
    assert(text == "boom", "resolved the wrong message: " .. tostring(text))
  end,

  ["the skipped failure prefix stops at the assertion location"] = function()
    -- A non-greedy prefix, so a second `file:line` in the message never wins.
    local file, lnum, text = parse("FAIL util_spec: assertion: actual.lua:42: unexpected location: other.lua:99: boom")
    assert(file == "actual.lua", "resolved the wrong file: " .. tostring(file))
    assert(lnum == 42, "resolved the wrong line: " .. tostring(lnum))
    assert(text:match("^unexpected location"), "message was " .. tostring(text))
  end,

  ["the quickfix errorformat still resolves plain file, line and column output"] = function()
    -- luacheck, and every other tool here that reports a column.
    local file, lnum = parse("lua/plugins/overseer.lua:12:3: unused variable x")
    assert(file == "lua/plugins/overseer.lua", "resolved the wrong file: " .. tostring(file))
    assert(lnum == 12, "resolved the wrong line: " .. tostring(lnum))
  end,

  ["the quickfix errorformat rejects a passing test line"] = function()
    assert(parse("ok util_spec: trim strips whitespace") == nil, "a passing line became a quickfix location")
  end,

  ["the quickfix errorformat rejects a bare recipe failure"] = function()
    -- `just` reports a line number from the justfile, not from a file the message names.
    assert(
      parse("error: recipe `test-nvim` failed on line 183 with exit code 1") == nil,
      "just's own failure line became a quickfix location"
    )
  end,
}
