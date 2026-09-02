-- Keymap, which-key group, and plugin-state dump for the zero-behavior-change proof
-- (docs/superpowers/specs/2026-09-01-nvim-overhaul-design-v4.md, 3.7 check 5).
--
-- Usage: nvim --headless -u <config>/init.lua -l tests/dump_state.lua <out.json>
-- Run WITHOUT --clean, from cd "$BENCH". The `-u` flag is load-bearing: `-l` on its
-- own skips source-state initialization entirely (`:help -l`, `:help startup`, item
-- 9 "-es/-Es/-l ... skipped"), so a bare `nvim --headless -l dump_state.lua` never
-- loads init.lua, never bootstraps lazy.nvim, and silently dumps nothing.
--
-- This is a keymap-metadata dump, not a full behavior proof: a Lua callback is
-- fingerprinted by its source file and defined line, not its actual behavior, so
-- two different callbacks defined on the same line of the same file (impossible in
-- practice) would compare equal, and a callback's runtime behavior is never
-- exercised. Buffer-local coverage is the gitsigns surface on a plain text buffer
-- only (opening $DOTFILES/justfile); filetype-local maps (markdown, octo, etc.) are
-- not captured here.
--
-- Covered: keymaps, which-key group metadata, and each plugin's lazy/loaded flags.
-- NOT covered, at all: Vim options, autocmds, user commands, LSP server config, the
-- colorscheme, and a plugin's own `opts` table. A change confined to that uncovered
-- set moves zero rows here (measured: flipping opt.ignorecase produced no diff). A
-- PR whose change lives outside the covered set cannot rest its zero-behavior-change
-- claim on this dump's diff alone.

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

-- Normalize the config root out of a `debug.getinfo` source string: a pre-merge
-- preview runs against a scratch deployment at a different absolute path than
-- the live config, and an "@/abs/path/to/plugins/noice.lua:53" fingerprint would
-- otherwise differ by root alone even when the file content is identical.
local function normalize_source(source)
  if source and source:sub(1, 1) == "@" then
    local path = source:sub(2)
    if path:sub(1, #CONFIG_ROOT) == CONFIG_ROOT then
      return "@<config>" .. path:sub(#CONFIG_ROOT + 1)
    end
  end
  return source or "?"
end

-- Fire the event lazy.nvim's own VeryLazy-triggered specs wait on. Headless `+qa`
-- never reaches lazy.nvim's own `vim.schedule` after `UIEnter` (spec 9.1), and
-- this dump runs in its own process rather than inheriting the phase block's
-- startup runs, so without this the dump misses every VeryLazy-triggered plugin
-- (which-key, noice, textobjects, unimpaired, claudecode after PR 30c9) on BOTH
-- sides of a diff, which is a comparison that passes for the wrong reason.
vim.cmd("doautocmd User VeryLazy")

local wk_runtime = require("lazy.core.config").plugins["which-key.nvim"]
if not (wk_runtime and wk_runtime._ and wk_runtime._.loaded) then
  error("which-key.nvim did not load after firing User VeryLazy; the VeryLazy firing did not take")
end

-- Global pass.
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
    -- A function address differs every run and can't be JSON-encoded. Fingerprint
    -- its source location instead of a flat "<callback>" placeholder, so a keymap
    -- whose callback moves to a different function (same lhs, different action)
    -- shows up as a changed row instead of comparing equal (amendment I6).
    local info = debug.getinfo(m.callback, "Su")
    local fingerprint = string.format("%s:%d", normalize_source(info.source), info.linedefined or -1)
    -- custom_api.util's `map()` wraps every mapping whose rhs is not a bare Vim
    -- command in the SAME literal closure, defined once at one fixed line
    -- (util.lua:141 today). Source+line alone therefore fingerprints every one
    -- of those wrapped mappings identically regardless of the action it runs:
    -- 305 real mappings collapsed to one row, and changing `<leader>00` from
    -- `quit` to `quit!` produced no diff (sol finding 1). Fingerprint what the
    -- wrapper closes over instead of where the wrapper lives: a closed-over
    -- string action is encoded directly, a closed-over function action is
    -- fingerprinted by its OWN source location, the same way as above.
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
    -- A classic Vimscript script-local function reference (a compat shim some
    -- plugins use, e.g. unimpaired's `<Plug>` mappings) is numbered by a
    -- per-process sourcing-order counter, confirmed to vary between separate
    -- nvim invocations of the SAME unchanged config (<SNR>126_ one run,
    -- <SNR>124_ the next). Strip the number; the function name is the part
    -- that carries behavior.
    row.rhs = row.rhs:gsub("<SNR>%d+_", "<SNR>_")
  end
  return row
end

for _, mode in ipairs(MODES) do
  for _, m in ipairs(vim.api.nvim_get_keymap(mode)) do
    table.insert(keymap_rows, project_keymap(m))
  end
end

-- Buffer-local pass. `]g` is the first map gitsigns `on_attach` sets (git.lua
-- today); `vim.b.gitsigns_head` is NOT the signal, it is set before `on_attach`
-- runs. Octo's dynamic `<localleader>` groups are which-key metadata registered
-- on the `octo` FileType, not buffer keymaps, so they are never captured here
-- and are excluded on purpose (PR 24 checks them by hand).
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

-- A block's own `mode` field (a string or a list) applies to every item nested
-- inside it that does not set its own; which-key itself defaults to normal mode
-- when nothing in the chain sets one. Returned sorted, so the JSON encoding of
-- an unordered set is stable across runs.
local function sorted_modes(mode)
  local list = type(mode) == "table" and mode or { mode or "n" }
  local copy = {}
  for _, m in ipairs(list) do
    table.insert(copy, m)
  end
  table.sort(copy)
  return copy
end

-- Which-key pass. `opts.spec` is a list of blocks; each block carries a shared
-- `mode` and nests its actual group rows one level deeper as its own array part
-- (`{ mode = {...}, { "<C-g>", group = "git-1" }, ... }`). A read of `opts.spec`
-- itself finds no `.group` fields on either side and compares equal on zero
-- groups (amendment I4), so this walks every nesting depth instead of assuming
-- exactly one. `inherited_mode` carries the nearest enclosing block's `mode`
-- down to its nested rows: a naive read of only the row's own (absent) `.mode`
-- field left every row's mode blank regardless of the block's, so narrowing
-- `which-key.lua:10` from normal-plus-visual to normal-only left all 52 group
-- rows identical (sol finding 6).
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

-- Plugin state pass. A plugin's NAME does not change when it flips between eager
-- and lazy (amendment I5), so this carries the runtime lazy and loaded flags per
-- plugin rather than a bare sorted name list.
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

-- One JSON object per line, discriminated by `kind`, each section already sorted
-- above, so a diff names the exact row that changed. Key order is built by hand
-- (not `vim.json.encode` on a whole table) because a Lua table's hash-part
-- iteration order is not guaranteed stable across separate process runs: the
-- same plugin row was observed encoding as both
-- `{"lazy":false,"kind":"plugin","loaded":true,"name":"witch-line"}` and
-- `{"kind":"plugin","name":"witch-line","lazy":false,"loaded":true}` between
-- two runs of the identical config, which would make every line in this file
-- compare unequal regardless of any real change.
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
