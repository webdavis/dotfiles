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
local function captured_setup_opts()
  local opts
  local overseer_fake = {
    setup = function(o)
      opts = o
    end,
    run_task = function() end,
    run_action = function() end,
    list_tasks = function()
      return {}
    end,
    STATUS = { SUCCESS = "SUCCESS", FAILURE = "FAILURE", CANCELED = "CANCELED", RUNNING = "RUNNING" },
  }

  local saved_overseer = package.loaded["overseer"]
  local saved_map = _G.map
  local saved_user_command = vim.api.nvim_create_user_command

  package.loaded["overseer"] = overseer_fake
  -- `map` is a global installed by init.lua, which this runner never loads.
  _G.map = function() end
  vim.api.nvim_create_user_command = function() end

  local ok, err = pcall(function()
    dofile(config_root .. "/lua/plugins/overseer.lua").config()
  end)

  package.loaded["overseer"] = saved_overseer
  _G.map = saved_map
  vim.api.nvim_create_user_command = saved_user_command

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

-- One line through the shipped errorformat. Returns the resolved filename (or
-- nil when the entry is not a valid location), the line number, and the message.
local function parse(line)
  local item = vim.fn.getqflist({ lines = { line }, efm = quickfix_component().errorformat }).items[1]
  if item.valid ~= 1 then
    return nil
  end
  return vim.fn.bufname(item.bufnr), item.lnum, (item.text or ""):gsub("^%s+", "")
end

return {
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

  ["the quickfix errorformat still resolves plain file:line:col output"] = function()
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
