-- Two owners for one key in one mode is a silent overwrite: whichever spec
-- lazy.nvim resolves last wins and the other key simply stops working, with no
-- warning at startup and nothing in `:messages`.
--
-- The scan reads the two places this configuration declares a key statically:
-- the `keys` list of every spec under `lua/plugins`, walked recursively so a
-- nested spec and a spec declared as a dependency are both reached, and the
-- `map()` calls in `lua/config/keymaps.lua`, collected by standing a recording
-- `map` in front of that file. A `keys` field may be a function, which lazy.nvim
-- calls at resolve time, so this calls it too.
--
-- Keys a third-party plugin installs from its own defaults are outside the scan,
-- because they exist only once that plugin has loaded. nvim-surround's `ys`,
-- `ds`, `cs` and visual `S` and nvim-various-textobjs' `i`/`a` family are the
-- two large sets in that category.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

local function overlapping_modes(mode)
  return mode == "v" and { "x", "s" } or { mode }
end

-- todo-comments.lua requires snacks at file scope, so the module has to answer
-- something for the file to load at all.
local function stub_snacks()
  package.preload["snacks"] = function()
    return setmetatable({}, {
      __index = function()
        return function() end
      end,
    })
  end
end

---Every (mode, lhs) pair the configuration declares, each mapped to the list of
---sources that claim it, plus the files that could not be read.
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

return {
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
    -- One key from each source. A collector that stopped reading either place
    -- would report no collisions at all, which is the failure this rules out.
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
