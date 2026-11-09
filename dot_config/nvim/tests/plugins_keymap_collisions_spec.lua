-- Two owners for one key in one mode silently overwrite each other.
-- Scans every spec `keys` list and the `map()` calls in config/keymaps.lua. Keys a plugin installs at load time are out of reach.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

-- Adding a key here is the owner's decision that losing the Neovim default is fine.
local deliberate_overrides = {
  ["n <C-E>"] = true,
  ["n <C-Left>"] = true,
  ["n <C-N>"] = true,
  ["n <C-P>"] = true,
  ["n <C-Right>"] = true,
  ["n <C-Y>"] = true,
  ["n N"] = true,
  ["n P"] = true,
  ["n gP"] = true,
  ["n gp"] = true,
  ["n p"] = true,
  ["n y"] = true,
  ["n Y"] = true,
  ["n ZZ"] = true,
  ["n [P"] = true,
  ["n [T"] = true,
  ["n [f"] = true,
  ["n [p"] = true,
  ["n [q"] = true,
  ["n ]P"] = true,
  ["n ]T"] = true,
  ["n ]f"] = true,
  ["n ]p"] = true,
  ["n ]q"] = true,
  ["n go"] = true,
  ["n gs"] = true,
  ["n j"] = true,
  ["n k"] = true,
  ["n n"] = true,
  ["x <lt>"] = true,
  ["x >"] = true,
  ["x P"] = true,
  ["x R"] = true,
  ["x X"] = true,
  ["x Y"] = true,
  ["x p"] = true,
  ["x y"] = true,
}

local function overlapping_modes(mode)
  return mode == "v" and { "x", "s" } or { mode }
end

-- Several plugin specs require snacks at file scope.
local function stub_snacks()
  package.preload["snacks"] = function()
    return setmetatable({}, {
      __index = function()
        return function() end
      end,
    })
  end
end

---Every (mode, lhs) pair mapped to the sources that claim it, plus unreadable files.
local function survey()
  local real_preload = package.preload["snacks"]
  local real_loaded = package.loaded["snacks"]
  package.loaded["snacks"] = nil
  stub_snacks()

  local owners, unreadable = {}, {}

  local function claim(mode, lhs, source)
    local id = mode .. " " .. lhs
    owners[id] = owners[id] or {}
    table.insert(owners[id], source)
  end

  local function claim_all(modes, lhs_list, source)
    modes = type(modes) == "table" and modes or { modes }
    lhs_list = type(lhs_list) == "table" and lhs_list or { lhs_list }
    for _, lhs in ipairs(lhs_list) do
      for _, mode in ipairs(modes) do
        for _, overlapping_mode in ipairs(overlapping_modes(mode)) do
          claim(overlapping_mode, lhs, source)
        end
      end
    end
  end

  local function collect(spec, source)
    local rows = spec.keys
    if type(rows) == "function" then
      local ok, called = pcall(rows)
      if not ok then
        table.insert(unreadable, source .. ": keys() raised " .. tostring(called))
        return
      end
      rows = called
    end
    for _, row in ipairs(rows or {}) do
      row = type(row) == "string" and { row } or row
      claim_all(row.mode or "n", row[1], ("%s (%s)"):format(spec[1], source))
    end
  end

  local function walk(node, source)
    if type(node) ~= "table" then
      return
    end
    if type(node[1]) == "string" and node.keys ~= nil then
      collect(node, source)
    end
    for _, child in pairs(node) do
      walk(child, source)
    end
  end

  for _, path in ipairs(vim.fn.glob(config_root .. "/lua/plugins/*.lua", false, true)) do
    local source = path:match("([^/]+%.lua)$")
    local ok, spec = pcall(dofile, path)
    if ok then
      walk(spec, source)
    else
      table.insert(unreadable, source .. ": " .. tostring(spec))
    end
  end

  local real_map = _G.map
  _G.map = function(opts)
    claim_all(opts.mode, opts.lhs, "config/keymaps.lua")
  end
  local ok, err = pcall(dofile, config_root .. "/lua/config/keymaps.lua")
  _G.map = real_map
  if not ok then
    table.insert(unreadable, "config/keymaps.lua: " .. tostring(err))
  end

  package.preload["snacks"] = real_preload
  package.loaded["snacks"] = real_loaded

  return owners, unreadable
end

local function normalized(lhs)
  return vim.fn.keytrans(vim.keycode(lhs))
end

-- The index writes `s` as `["x]s` and `y` as `["x]y{motion}`, and a mapping claims
-- the key without the register or the argument.
local function index_key(field)
  local key = field:gsub('%["x%]', ""):gsub("{.-}", ""):gsub("CTRL%-(.)", "<C-%1>"):gsub("%s+", "")
  return key ~= "" and normalized(key) or nil
end

local index_tag_modes = {
  i = { "i" },
  c = { "c" },
  o = { "o" },
  v = { "x" },
}

---Every (mode, lhs) Neovim gives a meaning to before any configuration loads:
---the built-in commands in its key index, plus the mappings a clean start sets.
local function neovim_defaults()
  local defaults = {}

  for line in io.lines(vim.env.VIMRUNTIME .. "/doc/index.txt") do
    local tag, field = line:match("^|([^|]+)|%s+([^\t]+)")
    if tag and not tag:find("^:") and not tag:find("^t_") then
      local key = index_key(field)
      local prefix = tag:match("^(%a)_")
      for _, mode in ipairs(key and index_tag_modes[prefix] or key and { "n" } or {}) do
        defaults[mode .. " " .. key] = true
      end
      -- Only the text objects among the visual entries also work after an operator.
      if key and prefix == "v" and key:find("^[ai].") then
        defaults["o " .. key] = true
      end
    end
  end

  local dump = [[lua local rows = {}
    for _, mode in ipairs({ "n", "x", "o", "i", "c", "s" }) do
      for _, map in ipairs(vim.api.nvim_get_keymap(mode)) do
        table.insert(rows, mode .. " " .. map.lhs)
      end
    end
    io.stdout:write(table.concat(rows, "\n"))]]
  local clean = vim.fn.system({ vim.v.progpath, "--clean", "--headless", "-c", dump, "-c", "qa!" })
  for row in vim.gsplit(clean, "\n", { trimempty = true }) do
    local mode, lhs = row:match("^(%a) (.+)$")
    if mode then
      defaults[mode .. " " .. normalized(lhs)] = true
    end
  end

  return defaults
end

return {
  ["no key overrides a Neovim default without being listed as deliberate"] = function()
    local owners = survey()
    local defaults = neovim_defaults()
    local unlisted = {}
    for id, sources in pairs(owners) do
      local mode, lhs = id:match("^(%a) (.+)$")
      local key = mode .. " " .. normalized(lhs)
      if defaults[key] and not deliberate_overrides[key] then
        table.insert(unlisted, ("%s overridden by %s"):format(key, table.concat(sources, " and ")))
      end
    end
    table.sort(unlisted)
    assert(#unlisted == 0, "overrides not listed as deliberate:\n  " .. table.concat(unlisted, "\n  "))
  end,
  ["no key swallows a longer key that another plugin owns"] = function()
    local owners = survey()
    local swallowed = {}
    local function canonical(id)
      local mode, lhs = id:match("^(%a) (.+)$")
      return mode .. " " .. normalized(lhs)
    end
    for short, short_sources in pairs(owners) do
      local short_key = canonical(short)
      for long, long_sources in pairs(owners) do
        local long_key = canonical(long)
        if
          long_key ~= short_key
          and long_key:sub(1, #short_key) == short_key
          and short_sources[1] ~= long_sources[1]
        then
          table.insert(swallowed, ("%s (%s) swallows %s (%s)"):format(short, short_sources[1], long, long_sources[1]))
        end
      end
    end
    table.sort(swallowed)
    assert(#swallowed == 0, "swallowed keys:\n  " .. table.concat(swallowed, "\n  "))
  end,
  ["every declaring file is readable, so the scan cannot shrink unnoticed"] = function()
    local _, unreadable = survey()
    assert(#unreadable == 0, "unread sources:\n  " .. table.concat(unreadable, "\n  "))
  end,
  ["no key is claimed twice in the same mode"] = function()
    local owners = survey()
    local clashes = {}
    for id, sources in pairs(owners) do
      if #sources > 1 then
        table.insert(clashes, ("%s claimed by %s"):format(id, table.concat(sources, " and ")))
      end
    end
    table.sort(clashes)
    assert(#clashes == 0, "overwritten keys:\n  " .. table.concat(clashes, "\n  "))
  end,
  ["the scan reaches both declaring places"] = function()
    local owners = survey()
    -- One key from each source proves both sources were read.
    assert(owners["n <leader>p"], "no plugin spec keys were collected")
    assert(owners["n <C-s>"], "no config/keymaps.lua mappings were collected")
  end,
  ["visual mappings claim both visual and select modes"] = function()
    local modes = overlapping_modes("v")
    assert(vim.deep_equal(modes, { "x", "s" }), "visual mode was not expanded")
  end,
  ["the survey restores the snacks module state"] = function()
    local real_preload = package.preload["snacks"]
    local real_loaded = package.loaded["snacks"]
    survey()
    assert(package.preload["snacks"] == real_preload, "package.preload was not restored")
    assert(package.loaded["snacks"] == real_loaded, "package.loaded was not restored")
  end,
}
