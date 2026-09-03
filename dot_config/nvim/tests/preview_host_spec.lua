-- custom_api.preview_host, the address markdown-preview.nvim advertises (bug #11).
--
-- `resolve` takes both values as arguments rather than reading the environment
-- itself, so these cases can name a suffix without touching the process env.

local preview_host = require("custom_api.preview_host")

return {
  ["appends the suffix to the hostname when one is set"] = function()
    local resolved = preview_host.resolve("dresden", "home.webdavis.io")
    assert(resolved == "dresden.home.webdavis.io", "resolved to " .. tostring(resolved))
  end,

  ["falls back to loopback when no suffix is set"] = function()
    local resolved = preview_host.resolve("dresden", nil)
    assert(resolved == "127.0.0.1", "resolved to " .. tostring(resolved))
  end,

  ["reads an empty suffix as no suffix"] = function()
    -- An exported-but-empty NVIM_MKDP_HOST arrives as "" rather than nil, and a
    -- bare truthiness test would build the hostname with a trailing dot.
    local resolved = preview_host.resolve("dresden", "")
    assert(resolved == "127.0.0.1", "resolved to " .. tostring(resolved))
  end,
}
