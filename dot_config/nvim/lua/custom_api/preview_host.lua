-- The host markdown-preview.nvim advertises for its preview server.

local M = {}

---@param hostname string this machine's hostname
---@param suffix_env string|nil domain suffix to append; nil or empty means loopback
---@return string
function M.resolve(hostname, suffix_env)
  -- An exported but empty NVIM_MKDP_HOST means loopback.
  if suffix_env and suffix_env ~= "" then
    -- Keep only the first label, lowercased, because `hostname()` may already be qualified.
    return hostname:match("^[^.]*"):lower() .. "." .. suffix_env
  end
  return "127.0.0.1"
end

return M
