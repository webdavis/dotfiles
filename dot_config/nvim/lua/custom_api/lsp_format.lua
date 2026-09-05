--- The admission rule for lsp-format's per-buffer formatter queue.
---
--- It lives here rather than inside the plugin spec because two events have to
--- apply it: `LspAttach`, and a server gaining `textDocument/formatting` after
--- it has already attached. The configured ESLint server does exactly that, so
--- an attach-time-only guard excludes it from the queue for the life of the
--- buffer and its formatting never runs.

local M = {}

--- Hands a formatting-capable client to lsp-format, at most once per buffer.
---
---@param client vim.lsp.Client the client to consider
---@param bufnr integer the buffer it is attached to
---@param lsp_format table the `lsp-format` module
---@return boolean admitted true only when this call queued the client
function M.admit(client, bufnr, lsp_format)
  -- Formatting-capable clients only. lsp-format's queue runner returns WITHOUT
  -- advancing when it reaches a client that cannot format, so a single such
  -- client in the queue makes the whole save send zero formatting requests.
  if not client:supports_method("textDocument/formatting", bufnr) then
    return false
  end

  -- The plugin's own queue is the record of what is already admitted, so no
  -- second table can drift from it. Its `on_attach` appends unconditionally,
  -- and a client queued twice is formatted twice per save.
  if vim.tbl_contains(lsp_format.buffers[bufnr] or {}, client.id) then
    return false
  end

  -- Registers the client for :Format. It also hangs lsp-format's own asynchronous
  -- BufWritePost formatter on the buffer, which would format a second time after the
  -- synchronous BufWritePre in the plugin spec has already run; drop that one.
  lsp_format.on_attach(client, bufnr)
  vim.api.nvim_clear_autocmds({ group = "Format", buffer = bufnr })
  return true
end

return M
