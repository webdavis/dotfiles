-- custom_api.lsp_format, the admission rule for lsp-format's per-buffer queue.
--
-- lsp-format is a plugin, so it is absent under `nvim --clean`; the double is a
-- stand-in for the two parts of it this module touches, `buffers` and
-- `on_attach`, both mirrored from the plugin's own source.

local lsp_format = require("custom_api.lsp_format")

local function double()
  local plugin = { buffers = {} }
  plugin.on_attach = function(client, bufnr)
    plugin.buffers[bufnr] = plugin.buffers[bufnr] or {}
    table.insert(plugin.buffers[bufnr], client.id)
    -- The plugin's own asynchronous formatter, which the module is expected to drop.
    local group = vim.api.nvim_create_augroup("Format", { clear = false })
    vim.api.nvim_create_autocmd("BufWritePost", { group = group, buffer = bufnr, callback = function() end })
  end
  return plugin
end

local function client(supports)
  return {
    id = 42,
    supports_method = function(_, method, _)
      return supports() and method == "textDocument/formatting"
    end,
  }
end

local function queued(plugin, bufnr)
  return #(plugin.buffers[bufnr] or {})
end

return {
  ["admits a client that gains formatting after it attached"] = function()
    local plugin, bufnr = double(), vim.api.nvim_create_buf(false, true)
    local supports = false
    local eslint = client(function()
      return supports
    end)

    assert(lsp_format.admit(eslint, bufnr, plugin) == false, "admitted before it could format")
    assert(queued(plugin, bufnr) == 0, "queued " .. queued(plugin, bufnr) .. " before registration")

    supports = true
    assert(lsp_format.admit(eslint, bufnr, plugin) == true, "not admitted after registration")
    assert(queued(plugin, bufnr) == 1, "queued " .. queued(plugin, bufnr) .. " after registration")
  end,

  ["keeps a client that never formats out of the queue"] = function()
    local plugin, bufnr = double(), vim.api.nvim_create_buf(false, true)
    local lua_ls = client(function()
      return false
    end)

    assert(lsp_format.admit(lua_ls, bufnr, plugin) == false, "admitted a non-formatting client")
    assert(lsp_format.admit(lua_ls, bufnr, plugin) == false, "admitted on the second call")
    assert(queued(plugin, bufnr) == 0, "queued " .. queued(plugin, bufnr))
  end,

  ["admits a client at most once per buffer"] = function()
    -- lsp-format's on_attach appends unconditionally, so a second admission would
    -- queue the client twice and send two formatting requests per save.
    local plugin, bufnr = double(), vim.api.nvim_create_buf(false, true)
    local gopls = client(function()
      return true
    end)

    assert(lsp_format.admit(gopls, bufnr, plugin) == true, "not admitted on the first call")
    assert(lsp_format.admit(gopls, bufnr, plugin) == false, "admitted twice")
    assert(queued(plugin, bufnr) == 1, "queued " .. queued(plugin, bufnr))
  end,

  ["removes a client that lost formatting from the queue"] = function()
    -- The plugin's queue runner returns WITHOUT advancing when it reaches a member
    -- that cannot format, so a stale member silences every sibling formatter.
    local plugin, bufnr = double(), vim.api.nvim_create_buf(false, true)
    local supports = true
    local eslint = client(function()
      return supports
    end)

    lsp_format.admit(eslint, bufnr, plugin)
    supports = false

    assert(lsp_format.withdraw(eslint, bufnr, plugin) == true, "not withdrawn")
    assert(queued(plugin, bufnr) == 0, "queued " .. queued(plugin, bufnr))
  end,

  ["leaves a client that still formats in the queue"] = function()
    local plugin, bufnr = double(), vim.api.nvim_create_buf(false, true)
    local gopls = client(function()
      return true
    end)

    lsp_format.admit(gopls, bufnr, plugin)

    assert(lsp_format.withdraw(gopls, bufnr, plugin) == false, "withdrew a client that can still format")
    assert(queued(plugin, bufnr) == 1, "queued " .. queued(plugin, bufnr))
  end,

  ["withdrawing a client that was never admitted changes nothing"] = function()
    local plugin, bufnr = double(), vim.api.nvim_create_buf(false, true)
    local lua_ls = client(function()
      return false
    end)

    assert(lsp_format.withdraw(lua_ls, bufnr, plugin) == false, "claimed to withdraw an absent client")
    assert(queued(plugin, bufnr) == 0, "queued " .. queued(plugin, bufnr))
  end,

  ["drops the plugin's own formatter autocmd from the buffer"] = function()
    -- It runs on BufWritePost, so it would format a second time after the
    -- synchronous BufWritePre save hook has already run.
    local plugin, bufnr = double(), vim.api.nvim_create_buf(false, true)
    local gopls = client(function()
      return true
    end)

    lsp_format.admit(gopls, bufnr, plugin)

    local left = vim.api.nvim_get_autocmds({ group = "Format", buffer = bufnr })
    assert(#left == 0, #left .. " Format autocmds left on the buffer")
  end,
}
