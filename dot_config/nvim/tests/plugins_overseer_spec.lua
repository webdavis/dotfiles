-- The quickfix wiring in `lua/plugins/overseer.lua`.
--
-- The subject is the `on_output_quickfix` entry of the `default` component alias:
-- the errorformat it ships and the `items_only` flag beside it. Both are values,
-- not behavior, so this spec runs the plugin file's `config` with `overseer`
-- faked, captures the table handed to `setup()`, and then feeds the captured
-- errorformat real lines of output from the tools this repo actually runs.
--
-- Reading the value back out of the config the plugin file ships, rather than
-- restating the pattern here, is the point: a spec holding its own copy of the
-- errorformat passes while the shipped one rots.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

-- Capture the setup() options without loading overseer itself.
local captured_hooks = {}

local function captured_setup_opts()
  local opts
  captured_hooks = {}
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
    create_task_output_view = function() end,
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
  -- `map` is a global installed by init.lua, which this runner never loads.
  _G.map = function() end
  vim.api.nvim_create_user_command = function() end
  -- The spec never fires VimEnter, but the config registers a preload autocmd;
  -- stubbing it keeps this spec from leaving one behind in the runner's process.
  vim.api.nvim_create_autocmd = function() end

  local ok, err = pcall(function()
    dofile(config_root .. "/lua/plugins/overseer.lua").config()
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

---The hook the config registers for EVERY template (the one with no filter).
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

-- One line through the shipped errorformat. Returns the resolved filename (or
-- nil when the entry is not a valid location), the line number, and the message.
local function parse(line)
  local defn = {}
  hook_for_all_templates()(defn, hook_util)
  local item = vim.fn.getqflist({ lines = { line }, efm = defn.default_component_params.errorformat }).items[1]
  if item.valid ~= 1 then
    return nil
  end
  return vim.fn.bufname(item.bufnr), item.lnum, (item.text or ""):gsub("^%s+", "")
end

return {
  ["the alias leaves a template's own errorformat alone"] = function()
    -- `on_output_quickfix.errorformat` has `default_from_task`, which only fills
    -- in when the component does NOT set it. Setting it in the alias overrode
    -- every template that ships one, Cargo's among them, so a compiler error
    -- landed on the nonexistent `--> src/lib.rs` buffer.
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
    -- `unique` compares task NAMES only, so `just test` started in a second
    -- worktree stopped and disposed the first worktree's running task. The name
    -- has to carry the directory for the comparison to tell them apart.
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
    -- Overseer expands aliases before calling a hook, so `components` IS the
    -- shared alias table. Component initialization fills `default_from_task`
    -- into those params in place, so the first task built wrote its errorformat
    -- into the alias every later task shares: an ordinary task first made Cargo
    -- inherit the generic format, and Cargo first contaminated ordinary tasks.
    local shared = { { "on_output_quickfix" } }
    local defn = { components = shared }
    hook_for_all_templates()(defn, hook_util)
    assert(defn.components ~= shared, "the hook kept the shared alias table")
    assert(defn.components[1] ~= shared[1], "the hook kept a shared component entry")
    assert(defn.components[1][1] == "on_output_quickfix", "the copy lost the component name")
  end,

  ["the quickfix keeps only lines that parse as a location"] = function()
    assert(quickfix_component().items_only == true, "items_only is not set; every output line reaches the quickfix")
  end,

  ["the quickfix errorformat resolves this repo's test-runner failures"] = function()
    -- `tests/run.lua` prints `FAIL <spec>: <case>: <file>:<line>: <message>`.
    -- Under the stock errorformat `%f` swallows the whole prefix, so the entry
    -- names a file that does not exist and jumps nowhere.
    local file, lnum, text = parse("FAIL util_spec: trim strips whitespace: lua/custom_api/util.lua:42: boom")
    assert(file == "lua/custom_api/util.lua", "resolved the wrong file: " .. tostring(file))
    assert(lnum == 42, "resolved the wrong line: " .. tostring(lnum))
    assert(text == "boom", "resolved the wrong message: " .. tostring(text))
  end,

  ["the skipped failure prefix stops at the assertion location"] = function()
    -- The prefix pattern must not be greedy. A message carrying a second
    -- `file:line` used to win, so the entry pointed at whatever the assertion
    -- text mentioned rather than at where it failed.
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
    -- `just` reports its own failure with a line number that belongs to the
    -- justfile, not to any file the message names.
    assert(
      parse("error: recipe `test-nvim` failed on line 183 with exit code 1") == nil,
      "just's own failure line became a quickfix location"
    )
  end,
}
