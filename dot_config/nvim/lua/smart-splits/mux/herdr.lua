local types = require("smart-splits.types")
local Direction = types.Direction
local log = require("smart-splits.log")
local utils = require("smart-splits.utils")

-- smart-splits.nvim backend for herdr, loaded by `multiplexer_integration = "herdr"`.
local herdr_bin = vim.env.HERDR_BIN_PATH or "herdr"

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

---HERDR_PANE_ID is set in every herdr pane.
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

---`herdr pane edges` reports true on each side with no neighbor.
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
  -- smart-splits checks for real movement itself, so any reply counts as success.
  return herdr_json({ "pane", "focus", "--direction", direction, "--current" }) ~= nil
end

function M.resize_pane(direction, amount)
  if not M.is_in_session() then
    return false
  end
  return herdr_json({ "pane", "resize", "--direction", direction, "--amount", tostring(amount), "--current" }) ~= nil
end

---herdr splits only right or down.
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
