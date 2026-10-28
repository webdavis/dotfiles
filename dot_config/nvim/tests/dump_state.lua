-- Dumps keymaps, which-key groups and plugin lazy/loaded flags as JSON lines, for diffing two configs.
-- Run it as nvim --headless -u <config>/init.lua -l tests/dump_state.lua <out.json>
-- `-u` is required, because `-l` alone skips init.lua.
-- Callbacks are fingerprinted by source location, not by behavior.

local out_path = arg[1]
if not out_path or out_path == "" then
  error("usage: nvim --headless -l dump_state.lua <out.json>")
end

local dotfiles = os.getenv("DOTFILES")
if not dotfiles or dotfiles == "" then
  error("DOTFILES environment variable is not set or empty; export it before invoking dump_state.lua")
end
local justfile = dotfiles .. "/justfile"
if vim.fn.filereadable(justfile) ~= 1 then
  error("$DOTFILES/justfile is not readable at " .. justfile)
end

local MODES = { "n", "v", "x", "s", "o", "i", "c", "t" }
local CONFIG_ROOT = vim.fn.stdpath("config")

-- Strip the config root, so a scratch deployment fingerprints like the live config.
local function normalize_source(source)
  if source and source:sub(1, 1) == "@" then
    local path = source:sub(2)
    if path:sub(1, #CONFIG_ROOT) == CONFIG_ROOT then
      return "@<config>" .. path:sub(#CONFIG_ROOT + 1)
    end
  end
  return source or "?"
end

-- Fire VeryLazy, because a headless run never reaches the lazy.nvim schedule after `UIEnter`.
vim.cmd("doautocmd User VeryLazy")

local wk_runtime = require("lazy.core.config").plugins["which-key.nvim"]
if not (wk_runtime and wk_runtime._ and wk_runtime._.loaded) then
  error("which-key.nvim did not load after firing User VeryLazy; the VeryLazy firing did not take")
end

local keymap_rows = {}

local function project_keymap(m)
  local row = {
    mode = m.mode,
    lhs = m.lhs,
    buffer = m.buffer,
    desc = m.desc,
    noremap = m.noremap,
    silent = m.silent,
    expr = m.expr,
    nowait = m.nowait,
    rhs = m.rhs,
  }
  if m.callback then
    -- Fingerprint a callback by source location, because its address changes every run.
    local info = debug.getinfo(m.callback, "Su")
    local fingerprint = string.format("%s:%d", normalize_source(info.source), info.linedefined or -1)
    -- `map()` wraps most mappings in one shared closure, so fingerprint the action it closes over.
    for i = 1, (info.nups or 0) do
      local up_name, up_value = debug.getupvalue(m.callback, i)
      if up_name == "rhs" then
        if type(up_value) == "string" then
          fingerprint = fingerprint .. ":str:" .. up_value
        elseif type(up_value) == "function" then
          local up_info = debug.getinfo(up_value, "S")
          fingerprint = fingerprint .. ":fn:" .. normalize_source(up_info.source) .. ":" .. (up_info.linedefined or -1)
        end
        break
      end
    end
    row.rhs = "<callback:" .. fingerprint .. ">"
  end
  if row.rhs then
    -- `<SNR>` numbers follow sourcing order and vary between runs.
    row.rhs = row.rhs:gsub("<SNR>%d+_", "<SNR>_")
  end
  return row
end

for _, mode in ipairs(MODES) do
  for _, m in ipairs(vim.api.nvim_get_keymap(mode)) do
    table.insert(keymap_rows, project_keymap(m))
  end
end

-- Wait for `]g`, because `vim.b.gitsigns_head` is set before `on_attach` runs.
vim.cmd("edit " .. vim.fn.fnameescape(justfile))
local attached = vim.wait(5000, function()
  return vim.fn.maparg("]g", "n", false, true).buffer == 1
end)
if not attached then
  error("timed out waiting for gitsigns to attach a buffer-local ]g map on " .. justfile)
end
for _, mode in ipairs(MODES) do
  for _, m in ipairs(vim.api.nvim_buf_get_keymap(0, mode)) do
    table.insert(keymap_rows, project_keymap(m))
  end
end

table.sort(keymap_rows, function(a, b)
  if a.mode ~= b.mode then
    return a.mode < b.mode
  end
  if a.lhs ~= b.lhs then
    return a.lhs < b.lhs
  end
  return a.buffer < b.buffer
end)

-- A block `mode` applies to its nested items, defaulting to normal. Sorted for stable JSON.
local function sorted_modes(mode)
  local list = type(mode) == "table" and mode or { mode or "n" }
  local copy = {}
  for _, m in ipairs(list) do
    table.insert(copy, m)
  end
  table.sort(copy)
  return copy
end

-- Walks every nesting depth and carries the nearest block `mode` down to its rows.
local function collect_groups(node, out, inherited_mode)
  local mode = node.mode or inherited_mode
  for _, item in ipairs(node) do
    if type(item) == "table" then
      local item_mode = item.mode or mode
      if type(item[1]) == "string" and item.group ~= nil then
        table.insert(out, { prefix = item[1], name = item.group, mode = sorted_modes(item_mode) })
      end
      collect_groups(item, out, item_mode)
    end
  end
end

local which_key_spec = dofile(CONFIG_ROOT .. "/lua/plugins/which-key.lua")
local groups = {}
collect_groups(which_key_spec.opts.spec, groups)
if #groups == 0 then
  error("which-key group capture produced zero groups; refusing to write an empty-by-construction dump")
end
table.sort(groups, function(a, b)
  return a.prefix < b.prefix
end)

-- Records lazy and loaded flags, because a name stays the same when a plugin flips between eager and lazy.
local plugin_rows = {}
for _, p in ipairs(require("lazy").plugins()) do
  table.insert(plugin_rows, {
    name = p.name,
    lazy = (p.lazy == true),
    loaded = (p._ ~= nil and p._.loaded ~= nil),
  })
end
table.sort(plugin_rows, function(a, b)
  return a.name < b.name
end)

-- One JSON object per line with keys in a fixed order, because Lua table order varies between runs.
local function json_line(kind, fields)
  local parts = { string.format('"kind":%s', vim.json.encode(kind)) }
  for _, pair in ipairs(fields) do
    local key, value = pair[1], pair[2]
    if value ~= nil then
      table.insert(parts, string.format("%s:%s", vim.json.encode(key), vim.json.encode(value)))
    end
  end
  return "{" .. table.concat(parts, ",") .. "}"
end

local f = assert(io.open(out_path, "w"))
for _, row in ipairs(keymap_rows) do
  f:write(
    json_line("keymap", {
      { "mode", row.mode },
      { "lhs", row.lhs },
      { "buffer", row.buffer },
      { "desc", row.desc },
      { "noremap", row.noremap },
      { "silent", row.silent },
      { "expr", row.expr },
      { "nowait", row.nowait },
      { "rhs", row.rhs },
    }),
    "\n"
  )
end
for _, row in ipairs(groups) do
  f:write(
    json_line("whichkey_group", {
      { "prefix", row.prefix },
      { "name", row.name },
      { "mode", row.mode },
    }),
    "\n"
  )
end
for _, row in ipairs(plugin_rows) do
  f:write(
    json_line("plugin", {
      { "name", row.name },
      { "lazy", row.lazy },
      { "loaded", row.loaded },
    }),
    "\n"
  )
end
f:close()
