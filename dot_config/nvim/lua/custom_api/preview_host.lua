-- The address markdown-preview.nvim advertises for its preview server (bug #11).
-- The host used to be one machine's name spelled out in the spec, so the URL it
-- echoed pointed at that machine from every other one.
--
-- `resolve` takes both values as arguments rather than reading `vim.env` itself:
-- a function that reads its own environment is a function nothing can test.

local M = {}

--- The address to advertise for the preview server.
---@param hostname string this machine's hostname
---@param suffix_env string|nil domain suffix to append; nil or empty means loopback
---@return string
function M.resolve(hostname, suffix_env)
  -- An exported-but-empty NVIM_MKDP_HOST arrives as "", which a bare truthiness
  -- test would accept and turn into a hostname with a trailing dot.
  if suffix_env and suffix_env ~= "" then
    -- `hostname()` is not guaranteed to be short: it can arrive as `dresden.local`,
    -- already qualified, or uppercase with a trailing root dot. Appending the suffix
    -- to any of those builds a name that does not resolve, so keep the first label
    -- only, lowercased.
    return hostname:match("^[^.]*"):lower() .. "." .. suffix_env
  end
  return "127.0.0.1"
end

return M
