-- The toggle `get` reads `package.loaded`, so painting the entry never loads gitsigns off its trigger.
-- `Snacks` and gitsigns are faked, so no plugin runs.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

---Run the snacks spec `init` with `Snacks` faked, and return the toggle options declared under `name`.
---@param name string
---@return table
local function toggle_named(name)
  local captured = {}
  -- Every `:map(lhs)` call returns something chainable.
  local chainable = {}
  chainable.map = function()
    return chainable
  end
  local toggle = setmetatable({}, {
    __call = function(_, opts)
      captured[opts.name] = opts
      return chainable
    end,
    -- Every builder returns the same chainable.
    __index = function(_, key)
      return function(...)
        local opts = select(key == "option" and 2 or 1, ...)
        if type(opts) == "table" and opts.name then
          captured[opts.name] = opts
        end
        return chainable
      end
    end,
  })

  local saved_snacks, saved_print, saved_map = _G.Snacks, vim.print, _G.map
  local saved_create = vim.api.nvim_create_autocmd
  _G.Snacks = {
    toggle = toggle,
    debug = { inspect = function() end, backtrace = function() end },
  }
  -- The callback also installs keymaps through the config's global helper.
  _G.map = function() end

  -- Call the callback directly, because `nvim_exec_autocmds` swallows its errors.
  local very_lazy
  local ok, err = pcall(function()
    vim.api.nvim_create_autocmd = function(event, opts)
      if event == "User" and opts.pattern == "VeryLazy" then
        very_lazy = opts.callback
        return 0
      end
      return saved_create(event, opts)
    end
    local spec = dofile(config_root .. "/lua/plugins/snacks.lua")
    for _, plugin in ipairs(spec) do
      if plugin[1] == "folke/snacks.nvim" then
        plugin.init()
      end
    end
    vim.api.nvim_create_autocmd = saved_create
    assert(very_lazy, "the spec's init registered no User VeryLazy callback")
    very_lazy()
  end)

  vim.api.nvim_create_autocmd = saved_create
  _G.Snacks, vim.print, _G.map = saved_snacks, saved_print, saved_map
  assert(ok, err)

  return assert(captured[name], "no toggle named " .. name .. " was declared")
end

---Run `fn` with `gitsigns.config` loaded under `signcolumn`, then restore.
local function with_gitsigns_loaded(signcolumn, fn)
  local saved = { package.loaded["gitsigns.config"] }
  package.loaded["gitsigns.config"] = { config = { signcolumn = signcolumn } }
  local ok, err = pcall(fn)
  package.loaded["gitsigns.config"] = saved[1]
  assert(ok, err)
end

---Run `fn` with gitsigns unloaded but loadable through `package.preload` stand-ins that record each load.
---@param fn fun(loads: string[])
local function with_gitsigns_loadable(fn)
  local names = { "gitsigns.config", "gitsigns" }
  local loads, saved_preload, saved_loaded = {}, {}, {}
  for _, name in ipairs(names) do
    saved_preload[name], saved_loaded[name] = package.preload[name], package.loaded[name]
    package.loaded[name] = nil
    package.preload[name] = function()
      table.insert(loads, name)
      return { config = { signcolumn = true }, toggle_signs = function() end }
    end
  end
  local ok, err = pcall(fn, loads)
  for _, name in ipairs(names) do
    package.preload[name], package.loaded[name] = saved_preload[name], saved_loaded[name]
  end
  assert(ok, err)
end

return {
  ["never loads gitsigns to answer while it is unloaded"] = function()
    local toggle = toggle_named("Git Signs")
    with_gitsigns_loadable(function(loads)
      local value = toggle.get()
      assert(#loads == 0, "get loaded " .. table.concat(loads, ", ") .. " with gitsigns unloaded")
      assert(value == false, "expected false with gitsigns unloaded, got " .. vim.inspect(value))
      -- Once gitsigns is loaded the getter reads the loaded table. The stand-in answers true, so false proves it.
      with_gitsigns_loaded(false, function()
        assert(toggle.get() == false, "expected the loaded module's signcolumn=false")
      end)
      assert(#loads == 0, "get loaded " .. table.concat(loads, ", ") .. " with gitsigns loaded")
    end)
  end,

  ["reports signs off when gitsigns has not loaded"] = function()
    local saved = { package.loaded["gitsigns.config"] }
    package.loaded["gitsigns.config"] = nil
    local toggle = toggle_named("Git Signs")
    local ok, value = pcall(toggle.get)
    package.loaded["gitsigns.config"] = saved[1]
    assert(ok, "get raised with gitsigns unloaded: " .. tostring(value))
    assert(value == false, "expected false with gitsigns unloaded, got " .. vim.inspect(value))
  end,

  ["reports the signcolumn setting once gitsigns has loaded"] = function()
    local toggle = toggle_named("Git Signs")
    with_gitsigns_loaded(true, function()
      assert(toggle.get() == true, "expected true when signcolumn is on")
    end)
    with_gitsigns_loaded(false, function()
      assert(toggle.get() == false, "expected false when signcolumn is off")
    end)
  end,
}
