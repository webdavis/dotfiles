-- The pane-named RPC socket (spec 7.3). A Neovim started in a herdr pane
-- listens on `<run root>/herdr-pane-<pane id>.sock`, so the nvim-mcp resolver
-- (~/.local/libexec/nvim-mcp/nvim-mcp-connect.sh) derives the path from the
-- HERDR_PANE_ID in its own environment and connects. Nothing is recorded and
-- nothing is tracked: the socket IS the registration, Neovim removes it on
-- exit, a socket a crash left behind is replaced by the next start in that
-- pane (measured on 0.12.5: serverstart() on a stale path succeeds and
-- answers), and `sweep` below clears the ones no pane starts in again.
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

-- Remove the pane sockets in `root` whose Neovim is gone, so crashed editors
-- do not pile names up there. Hygiene, not correctness: the resolver refuses a
-- dead socket by probing it, and macOS clears $TMPDIR at reboot and after
-- three days idle anyway. Our names only, matched on the entry NAME, so the
-- scan never stats the thousands of per-process directories Neovim leaves in
-- this root and never touches Neovim's own `nvim.<pid>.0` sockets. Liveness
-- is one connect: refused means nobody is behind the socket (about 0.1 ms),
-- accepted means somebody is, and that channel is closed with nothing sent.
-- ponytail: another Neovim between bind and listen reads as refused for a few
-- microseconds; accepted, it costs that editor its pane name and nothing else.
function M.sweep(root)
  for name in vim.fs.dir(root) do
    if name:match("^herdr%-pane%-.+%.sock$") then
      local path = root .. "/" .. name
      local ok, channel = pcall(vim.fn.sockconnect, "pipe", path, { rpc = true })
      if ok then
        vim.fn.chanclose(channel)
      else
        vim.uv.fs_unlink(path)
      end
    end
  end
end

-- Sweep, then listen on this pane's socket, silently, if there is a pane and
-- the name is free. Every failure is swallowed on purpose: "address already in
-- use" is an earlier Neovim in the same pane that should keep the name, and
-- anything else (a root that does not exist, a path too long) only means this
-- instance is reached by a pin rather than by pane.
function M.listen()
  local path = M.path(vim.env.HERDR_PANE_ID)
  if path then
    M.sweep(vim.fs.dirname(path))
    pcall(vim.fn.serverstart, path)
  end
end

return M
