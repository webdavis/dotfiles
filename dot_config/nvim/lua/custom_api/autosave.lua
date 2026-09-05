-- Whether auto-save.nvim may write a buffer (spec 5.3, item 42).
--
-- claudecode.nvim opens a proposed edit in a scratch buffer and treats a write
-- as "accepted". Auto-save fires on a timer, so without this rule an agent's
-- proposal is accepted by nobody, seconds after it appears. The exclusions are
-- the ones claudecode's own README documents for okuuva/auto-save.nvim.
--
-- Name and buftype are arguments rather than read off a buffer here: a function
-- that reads its own buffer state is a function nothing can test.

local M = {}

--- Whether a buffer is safe for auto-save.nvim to write.
---@param name string the buffer's name, as `nvim_buf_get_name` returns it
---@param buftype string the buffer's `&buftype`
---@return boolean
function M.should_save(name, buftype)
  -- Lua patterns, so the parentheses and the dash are escaped with `%`.
  if name:match("%(proposed%)") or name:match("%(NEW FILE %- proposed%)") then
    return false
  end

  -- claudecode's diff buffers are `acwrite`, which catches a proposed buffer
  -- whose name a future version of the plugin spells differently.
  if buftype == "acwrite" then
    return false
  end

  return true
end

return M
