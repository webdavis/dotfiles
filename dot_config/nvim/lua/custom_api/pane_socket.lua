-- The pane-named RPC socket (spec 7.3). A Neovim started in a herdr pane
-- listens on `<run root>/herdr-pane-<pane id>.sock`, so the nvim-mcp resolver
-- (~/.local/libexec/nvim-mcp/nvim-mcp-connect.sh) derives the path from the
-- HERDR_PANE_ID in its own environment and connects. Nothing is recorded and
-- nothing is cleaned up: the socket IS the registration, Neovim removes it on
-- exit, and a socket a crash left behind is replaced by the next start
-- (measured on 0.12.5: serverstart() on a stale path succeeds and answers).
--
-- The first Neovim in a pane owns the name, by design. A nested Neovim, or one
-- in a terminal split, inherits the same pane id and finds the name taken;
-- `listen` swallows that and keeps its default socket, so the outer editor,
-- the one the operator sees in the pane, is the one the agent reaches.

local M = {}

-- The one directory both sides derive the same way, or nil when Neovim has
-- none. `stdpath("run")` is unusable as that directory: with XDG_RUNTIME_DIR
-- unset (macOS) it is a PER-PROCESS directory, `$TMPDIR/nvim.<user>/<random>`,
-- so no other process can compute it. Its parent, `$TMPDIR/nvim.<user>`, is
-- the 0700 directory Neovim itself creates and checks, and it is the listing
-- root `:help serverstart()` documents. With XDG_RUNTIME_DIR set (Linux) that
-- directory is the run dir itself. Exported EMPTY, Neovim reads the variable
-- as unset here but reports an empty run dir and starts no default server
-- (measured on 0.12.5), so there is no root and the pane gets no socket.
function M.root()
  local runtime = vim.env.XDG_RUNTIME_DIR
  if runtime then
    return runtime
  end
  local run = vim.fn.stdpath("run")
  if run:sub(1, 1) ~= "/" then
    return nil
  end
  return vim.fs.dirname(run)
end

-- The socket path for `pane_id`, or nil when the id cannot name one. herdr ids
-- look like `wW:p3K`; only alphanumerics, `:` and `-` are accepted, bounded,
-- so nothing that reaches the filesystem carries a separator or a length the
-- 104-byte socket path limit cannot absorb. The colon is written as a dot,
-- because serverstart() reads any address holding a colon as TCP
-- (`:help serverstart()`), and a dot is outside the accepted set so the
-- mapping cannot collide. nvim-mcp-connect.sh spells the same rule in bash.
function M.path(pane_id)
  local root = M.root()
  if not root or type(pane_id) ~= "string" or #pane_id > 64 or not pane_id:match("^[%w:-]+$") then
    return nil
  end
  return ("%s/herdr-pane-%s.sock"):format(root, (pane_id:gsub(":", ".")))
end

-- Listen on this pane's socket, silently, if there is a pane and the name is
-- free. Every failure is swallowed on purpose: "address already in use" is an
-- earlier Neovim in the same pane that should keep the name, and anything else
-- (a root that does not exist, a path too long) only means this instance is
-- reached by a pin rather than by pane.
function M.listen()
  local path = M.path(vim.env.HERDR_PANE_ID)
  if path then
    pcall(vim.fn.serverstart, path)
  end
end

return M
