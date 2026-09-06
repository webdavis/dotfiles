-- The pane socket (spec 7.3). A Neovim started in a herdr pane listens on
-- `<run root>/herdr-<session>-<terminal>.sock`, so the nvim-mcp resolver
-- (~/.local/libexec/nvim-mcp/nvim-mcp-connect.sh) can derive the same path
-- for the pane the agent runs in and connect. Nothing is recorded and nothing
-- is tracked: the socket IS the registration, Neovim removes it on exit, and a
-- socket a crash left behind is replaced by the next start in that pane
-- (measured on 0.12.5: serverstart() on a stale path succeeds and answers)
-- and refused by the resolver's probe until then. There is no sweep: a
-- probe-then-unlink of another pane's stale socket races that pane's next
-- start (its replacement can bind and answer between the two steps and be
-- unlinked), and macOS clears $TMPDIR at boot and daily for items idle three
-- days anyway (com.apple.bsd.dirhelper, CLEAN_FILES_OLDER_THAN_DAYS=3).
--
-- The NAME is herdr's own identity for the pane, asked from herdr rather than
-- read from the environment, for two measured reasons. A pane id (`wW:p3K`)
-- is per SESSION: two herdr sessions each count from `w1:p1`, so two editors
-- in different sessions would share a name; the session half of the name is
-- the first six hex digits of the sha256 of HERDR_SOCKET_PATH, the one value
-- both sides carry that differs per session. And a pane id CHANGES when the
-- pane is moved to another workspace while the launch-time HERDR_PANE_ID does
-- not, so a socket named for it would be invisible to its new siblings; the
-- terminal id herdr reports (`term_65a9c8766b9261`) survives the move, and
-- `herdr pane current --current` answers for the CALLER's terminal even from
-- a process whose environment was cleared (measured on 0.8.2).
--
-- The first Neovim in a pane owns the name, by design. A nested Neovim, or one
-- in a terminal split, shares the terminal and finds the name taken; `listen`
-- swallows that and keeps its default socket, so the outer editor, the one
-- the operator sees in the pane, is the one the agent reaches.

local M = {}

-- The longest socket path a unix socket accepts: sun_path is 104 bytes on macOS
-- and 108 on Linux, NUL included, and the smaller bound applies everywhere so
-- the same name works on both. Over it, serverstart() fails with a bare
-- "invalid argument", so the check runs first and says what happened.
M.MAX_PATH_BYTES = 103

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

-- True when `dir` is a directory this user owns at mode 0700, the only shape
-- the run root may have. Neovim itself falls back to `<temp>/nvim.<random>`
-- when `nvim.<user>` is mis-owned or too open (measured on 0.12.5), and that
-- directory's parent is `<temp>` itself, which can be a shared /tmp where any
-- account may pre-create a pane socket. A supplied XDG_RUNTIME_DIR gets the
-- same check.
function M.private(dir)
  local stat = vim.uv.fs_stat(dir)
  return stat ~= nil and stat.type == "directory" and stat.uid == vim.uv.getuid() and stat.mode % 512 == 448
end

-- The session half of the name. nvim-mcp-connect.sh spells the same rule
-- with `shasum -a 256`.
function M.session()
  return vim.fn.sha256(vim.env.HERDR_SOCKET_PATH or ""):sub(1, 6)
end

-- The socket path for herdr terminal id `terminal`, or nil when the id cannot
-- name one. Only word characters, `_` and `-` are accepted, bounded, so
-- nothing that reaches the filesystem carries a separator or a length the
-- 104-byte socket path limit cannot absorb.
function M.path(terminal)
  local root = M.root()
  if not root or type(terminal) ~= "string" or #terminal > 64 or not terminal:match("^[%w_-]+$") then
    return nil
  end
  return ("%s/herdr-%s-%s.sock"):format(root, M.session(), terminal)
end

-- Ask herdr which terminal this Neovim runs in, then `on_done(terminal)`,
-- with nil when herdr is missing, fails, answers something else or takes
-- longer than two seconds. Asynchronous, so a slow herdr never holds startup.
function M.identity(on_done)
  local ok = pcall(
    vim.system,
    { "herdr", "pane", "current", "--current" },
    { text = true, timeout = 2000 },
    function(result)
      local decoded_ok, decoded = pcall(vim.json.decode, result.stdout or "")
      local pane = decoded_ok and type(decoded) == "table" and type(decoded.result) == "table" and decoded.result.pane
      on_done(result.code == 0 and type(pane) == "table" and pane.terminal_id or nil)
    end
  )
  if not ok then
    on_done(nil)
  end
end

-- Listen on this pane's socket, silently, if this Neovim runs under herdr, the
-- run root is private and the name is free. Every failure of the bind is swallowed on purpose:
-- "address already in use" is an earlier Neovim in the same pane that should
-- keep the name, and anything else (a path too long) only means this instance
-- is reached by a pin rather than by pane. A herdr that does not answer is
-- said once, because that is a pane the agent cannot reach without a pin.
function M.listen()
  if not vim.env.HERDR_ENV then
    return
  end
  local root = M.root()
  if not root or not M.private(root) then
    vim.notify(
      ("nvim-mcp: not listening for this pane, the run root %s is not a directory this user owns at mode 0700"):format(
        tostring(root)
      ),
      vim.log.levels.WARN
    )
    return
  end
  M.identity(function(terminal)
    vim.schedule(function()
      local path = M.path(terminal)
      if path and #path > M.MAX_PATH_BYTES then
        vim.notify(
          ("nvim-mcp: not listening for this pane, the socket path %s is %d bytes and unix sockets allow %d; the agent reaches this Neovim only through NVIM_MCP_SOCKET"):format(
            path,
            #path,
            M.MAX_PATH_BYTES
          ),
          vim.log.levels.WARN
        )
      elseif path then
        pcall(vim.fn.serverstart, path)
      else
        vim.notify(
          "nvim-mcp: not listening for this pane, herdr did not report its terminal; the agent reaches this Neovim only through NVIM_MCP_SOCKET",
          vim.log.levels.WARN
        )
      end
    end)
  end)
end

return M
