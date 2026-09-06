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

  ["strips a .local suffix off the hostname before appending"] = function()
    local resolved = preview_host.resolve("dresden.local", "home.webdavis.io")
    assert(resolved == "dresden.home.webdavis.io", "resolved to " .. tostring(resolved))
  end,

  ["does not append the suffix twice to an already-qualified hostname"] = function()
    local resolved = preview_host.resolve("dresden.home.webdavis.io", "home.webdavis.io")
    assert(resolved == "dresden.home.webdavis.io", "resolved to " .. tostring(resolved))
  end,

  ["lowercases the hostname and drops a fully-qualified trailing dot"] = function()
    local resolved = preview_host.resolve("DRESDEN.HOME.WEBDAVIS.IO.", "home.webdavis.io")
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
