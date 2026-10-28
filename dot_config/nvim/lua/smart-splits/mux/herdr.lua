local types = require("smart-splits.types")
local Direction = types.Direction
local log = require("smart-splits.log")
local utils = require("smart-splits.utils")

-- Custom smart-splits.nvim multiplexer backend for the herdr terminal
-- multiplexer (https://herdr.dev). smart-splits resolves backends with a plain
-- `require('smart-splits.mux.' .. integration)` and has no allow-list, so
-- dropping this file on the runtimepath + `multiplexer_integration = 'herdr'`
-- wires herdr in with no fork of smart-splits. Each method shells out to the
-- `herdr` CLI (one socket round-trip); smart-splits only calls these at a split
-- edge, so the latency is fine.
local herdr_bin = vim.env.HERDR_BIN_PATH or "herdr"

---Run a herdr CLI subcommand and return its decoded JSON result, or nil on failure.
---@param args string[]
---@return table|nil
local function herdr_json(args)
  local cmd = { herdr_bin }
  vim.list_extend(cmd, args)
  local text, code = utils.system(cmd)
  if code ~= 0 or not text or #text == 0 then
    return nil
  end
  local ok, decoded = pcall(vim.json.decode, text)
  if not ok then
    log.error("herdr mux: invalid JSON from `" .. table.concat(args, " ") .. "`")
    return nil
  end
  return decoded
end

---@type SmartSplitsMultiplexer
local M = {} ---@diagnostic disable-line: missing-fields

M.type = "herdr" ---@diagnostic disable-line: assign-type-mismatch

---herdr exports HERDR_PANE_ID into every pane it owns; its presence means we are
---inside a herdr session.
function M.is_in_session()
  local pane = vim.env.HERDR_PANE_ID
  return pane ~= nil and #pane > 0
end

---@return string|nil
function M.current_pane_id()
  if not M.is_in_session() then
    return nil
  end
  local data = herdr_json({ "pane", "current", "--current" })
  local pane = data and data.result and data.result.pane
  if pane and type(pane.pane_id) == "string" and #pane.pane_id > 0 then
    return pane.pane_id
  end
  return nil
end

---`herdr pane edges` reports, per side, whether this pane is AT the edge there:
---true = at the edge (no neighbor that direction), false = a neighbor exists.
function M.current_pane_at_edge(direction)
  if not M.is_in_session() then
    return false
  end
  local data = herdr_json({ "pane", "edges", "--current" })
  local edges = data and data.result and data.result.edges
  if type(edges) ~= "table" then
    return false
  end
  return edges[direction] == true
end

function M.current_pane_is_zoomed()
  local data = herdr_json({ "pane", "current", "--current" })
  local pane = data and data.result and data.result.pane
  return pane ~= nil and pane.zoomed == true
end

function M.next_pane(direction)
  if not M.is_in_session() then
    return false
  end
  -- herdr returns exit 0 + JSON even when there is no neighbor; smart-splits
  -- guards real movement by comparing current_pane_id() before/after, so a
  -- truthy return here is safe.
  return herdr_json({ "pane", "focus", "--direction", direction, "--current" }) ~= nil
end

function M.resize_pane(direction, amount)
  if not M.is_in_session() then
    return false
  end
  return herdr_json({ "pane", "resize", "--direction", direction, "--amount", tostring(amount), "--current" }) ~= nil
end

---herdr only splits right|down; map left->right and up->down.
function M.split_pane(direction, size)
  local herdr_dir = (direction == Direction.left or direction == Direction.right) and "right" or "down"
  local args = { "pane", "split", "--current", "--direction", herdr_dir }
  if size then
    table.insert(args, "--ratio")
    table.insert(args, tostring(size))
  end
  return herdr_json(args) ~= nil
end

return M
