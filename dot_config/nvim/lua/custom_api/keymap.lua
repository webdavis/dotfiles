---@class custom_api.keymap
---Key mapping. `map` supports multiple keys, automatic silent handling, and
---merging user-provided options.
local M = {}

-- ╭───────────────────╮
-- │  Helper Functions │
-- ╰───────────────────╯

---Determines whether a mapping should be silent based on its right-hand side (rhs).
---Mappings that execute Vim commands (starting with `:`) are considered *not silent*.
---All other mappings (like Lua functions or non-command strings) are silent by default.
---
---Example:
---```lua
---is_command_silent(":echo 'hi'") --> false
---is_command_silent(function() print("hi") end) --> true
---```
---@param rhs string|fun():any The right-hand side of the mapping.
---@return boolean `true` if the mapping should be silent, otherwise `false`.
local is_command_silent = function(rhs)
  -- Returns true if `rhs` is not a Vim command starting with :.
  -- Mappings with such commands should be silent.

  return not (type(rhs) == "string" and rhs:match("^:"))
end

-- ╭──────╮
-- │  API │
-- ╰──────╯

---@class custom_api.keymap.map
---@field mode string|table Vim mode(s) for the mapping (e.g., "n", "i", { "n", "v" }).
---@field lhs string|table The left-hand side key(s) to map.
---@field rhs string|fun():any The right-hand side action to execute.
---@field desc string|nil Description of the mapping (shown in tools like `which-key`).
---@field sequence boolean|nil If true, treats the mapping as part of a key sequence.
---@field expr boolean|nil If true, evaluates the `rhs` as an expression.
---@field nowait boolean|nil If true, mapping doesn’t wait for additional input.
---@field remap boolean|nil If true, allows recursive mapping.

---Creates one or more key mappings with smart defaults. _(Note: this function is globally accessible.)_
---
---`opts` table schema:
---```lua
---{
---  mode      : string|table       Required: Vim mode(s) for the mapping, e.g., "n" or { "n", "v" }.
---  lhs       : string|table       Required: Left-hand side key(s) to map.
---  rhs       : string|fun():any   Required: Right-hand side action to execute.
---  desc      : string|nil         Optional: Description for which-key or documentation.
---  sequence? : boolean|nil        Optional: If true, treats mapping as part of a key sequence.
---  expr?     : boolean|nil        Optional: Evaluate RHS as an expression.
---  nowait?   : boolean|nil        Optional: Don’t wait for additional input.
---  remap?    : boolean|nil        Optional: Allow recursive mapping.
---}
---```
---
---This function wraps `vim.keymap.set` and:
---
---  1. Automatically applies `noremap = true`, by default.
---  2. Automatically sets `silent` unless the `rhs` runs a Nvim/Vim command.
---  3. Supports mapping multiple `lhs` keys in a single call.
---  4. Merges additional options like `expr`, `nowait`, or `remap`.
---
---Examples:
---```lua
----- Normal mode mapping, Vim command
---map({
---  mode = "n",
---  lhs = "<leader>ff",
---  rhs = "Telescope find_files",
---  desc = "Telescope: find files"
---})
---
----- Insert mode, multiple keys mapped to Lua function
---map({
---  mode = "i",
---  lhs = { "jk", "jj" },
---  rhs = function()
---    local escape = vim.api.nvim_replace_termcodes("<Esc>", true, false, true)
---    return vim.api.nvim_feedkeys(escape, "n", true)
---  end,
---  desc = "Exit insert mode"
---})
---```
---@param opts custom_api.keymap.map The options table containing the mapping configuration.
---@return nil
function M.map(opts)
  local mode = opts.mode
  local lhs_list = opts.lhs
  local rhs = opts.rhs
  local sequence = opts.sequence

  local desc = opts.desc

  if type(lhs_list) == "string" then
    lhs_list = { lhs_list }
  end

  -- Start with the defaults.
  local keymap_opts = { noremap = true, desc = desc, silent = is_command_silent(rhs) }

  -- Merge any extra options from opts (e.g., expr, nowait, remap).
  for k, v in pairs(opts) do
    if not (k == "mode" or k == "lhs" or k == "rhs" or k == "sequence") then
      keymap_opts[k] = v
    end
  end

  for _, lhs in ipairs(lhs_list) do
    if sequence or not keymap_opts.silent then
      vim.keymap.set(mode, lhs, rhs, keymap_opts)
    else
      vim.keymap.set(mode, lhs, function()
        if type(rhs) == "function" then
          return rhs()
        elseif type(rhs) == "string" then
          vim.cmd(rhs)
        end
      end, keymap_opts)
    end
  end
end

return M
