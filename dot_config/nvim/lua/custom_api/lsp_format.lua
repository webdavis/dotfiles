--- The admission rule for lsp-format's per-buffer formatter queue.
---
--- It lives here rather than inside the plugin spec because two events have to
--- apply it: `LspAttach`, and a server gaining `textDocument/formatting` after
--- it has already attached. The configured ESLint server does exactly that, so
--- an attach-time-only guard excludes it from the queue for the life of the
--- buffer and its formatting never runs.

local M = {}

-- Neovim's own capability handlers, captured the first time the wrappers are
-- installed. A later setup that wrapped the wrapper would chain handlers, and the
-- chain would go on holding the previous lsp-format module and its buffer state.
local native_handler = {}

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
  -- second table can drift from it. Its `on_attach` appends unconditionally, so
  -- re-admitting a client would store its id a second time. That does not format
  -- twice: the plugin enumerates the buffer's active clients once and keeps the
  -- ones this list names. The duplicate is redundant state, kept out rather than
  -- cleaned up later.
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

--- Removes a client from lsp-format's queue for a buffer once it can no longer
--- format it. A server may unregister `textDocument/formatting` at any point, and
--- the configured ESLint server does so whenever its configuration changes.
---
---@param client vim.lsp.Client the client to consider
---@param bufnr integer the buffer it is attached to
---@param lsp_format table the `lsp-format` module
---@return boolean withdrawn true only when this call removed the client
function M.withdraw(client, bufnr, lsp_format)
  if client:supports_method("textDocument/formatting", bufnr) then
    return false
  end

  local members = lsp_format.buffers[bufnr]
  if not members then
    return false
  end

  -- Backwards, so a removal cannot skip the element after it.
  local withdrawn = false
  for index = #members, 1, -1 do
    if members[index] == client.id then
      table.remove(members, index)
      withdrawn = true
    end
  end
  return withdrawn
end

--- Wraps Neovim's capability handlers so a buffer's formatter membership follows
--- its clients' capabilities instead of a snapshot taken when they attached. The
--- native handler runs first, because it is what records the change.
---
--- Safe to call again: a later call replaces the wrapper rather than wrapping it.
---
---@param lsp_format table the `lsp-format` module
function M.install_handlers(lsp_format)
  for method, reconcile in pairs({
    ["client/registerCapability"] = M.admit,
    ["client/unregisterCapability"] = M.withdraw,
  }) do
    native_handler[method] = native_handler[method] or vim.lsp.handlers[method]
    vim.lsp.handlers[method] = function(err, result, ctx, config)
      local response = native_handler[method](err, result, ctx, config)
      local client = vim.lsp.get_client_by_id(ctx.client_id)
      if client then
        for bufnr in pairs(client.attached_buffers) do
          reconcile(client, bufnr, lsp_format)
        end
      end
      return response
    end
  end
end

return M
