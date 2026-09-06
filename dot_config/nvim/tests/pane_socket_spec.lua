-- custom_api.pane_socket (spec 7.3): the socket a Neovim in a herdr pane
-- listens on so the nvim-mcp resolver can derive its path from the pane id
-- alone. Real `serverstart` calls, because the behavior under test is what the
-- server layer does with a taken or unsafe name; a fake would answer for
-- nothing. `-l` mode starts no default server, so every list assertion is
-- relative to whatever this process already has.

local function pane_socket()
  return require("custom_api.pane_socket")
end

-- Run `fn` with these environment variables set (`false` unsets one), and put
-- every one of them back afterwards whether or not `fn` raised.
local function with_env(vars, fn)
  local saved = {}
  for name, value in pairs(vars) do
    saved[name] = vim.env[name]
    vim.env[name] = value or nil
  end
  local ok, err = pcall(fn)
  for name in pairs(vars) do
    vim.env[name] = saved[name]
  end
  assert(ok, err)
end

-- A private run root per case, under Neovim's own per-process temp tree, so
-- the sockets these cases bind never land in the real shared root. Short, and
-- removed when this process exits.
local function private_root()
  local dir = vim.fn.tempname()
  assert(vim.fn.mkdir(dir, "p") == 1, "could not create " .. dir)
  return dir
end

local function serving(path)
  return vim.tbl_contains(vim.fn.serverlist(), path)
end

return {
  ["a pane id becomes a socket path in the run root, with its colon written as a dot"] = function()
    local root = private_root()
    with_env({ XDG_RUNTIME_DIR = root }, function()
      assert(pane_socket().path("w1:p2") == root .. "/herdr-pane-w1.p2.sock", pane_socket().path("w1:p2"))
    end)
  end,

  ["an id that cannot sit in a socket path yields no path"] = function()
    local unsafe = { "../w1:p2", "w1:p2/x", "w1 p2", "w1\np2", "", ("a"):rep(65), 42, nil }
    for index = 1, 8 do
      local id = unsafe[index]
      assert(pane_socket().path(id) == nil, "accepted " .. vim.inspect(id))
    end
    assert(pane_socket().path(("a"):rep(64)) ~= nil, "refused a 64-character id, which is inside the bound")
  end,

  ["without XDG_RUNTIME_DIR the root is the parent of stdpath('run'), never that per-process dir"] = function()
    local parent = vim.fs.dirname(vim.fn.stdpath("run"))
    with_env({ XDG_RUNTIME_DIR = false }, function()
      assert(pane_socket().root() == parent, pane_socket().root())
    end)
  end,

  ["with XDG_RUNTIME_DIR exported empty, Neovim has no run dir and the pane gets no socket path"] = function()
    with_env({ XDG_RUNTIME_DIR = "" }, function()
      -- Neovim's own reading of that environment: no run dir at all.
      assert(vim.fn.stdpath("run") == "", "stdpath('run') is " .. vim.fn.stdpath("run"))
      assert(pane_socket().root() == nil, "root is " .. tostring(pane_socket().root()))
      assert(pane_socket().path("w1:p2") == nil, "path is " .. tostring(pane_socket().path("w1:p2")))
    end)
  end,

  ["with HERDR_PANE_ID set, listen() puts the pane socket in serverlist()"] = function()
    local root = private_root()
    local expected = root .. "/herdr-pane-w1.p2.sock"
    with_env({ XDG_RUNTIME_DIR = root, HERDR_PANE_ID = "w1:p2" }, function()
      pane_socket().listen()
      assert(serving(expected), "not serving " .. expected .. ": " .. vim.inspect(vim.fn.serverlist()))
      vim.fn.serverstop(expected)
    end)
  end,

  ["with an unsafe HERDR_PANE_ID nothing extra is started"] = function()
    local root = private_root()
    with_env({ XDG_RUNTIME_DIR = root, HERDR_PANE_ID = "../w1:p2" }, function()
      local before = vim.fn.serverlist()
      pane_socket().listen()
      assert(vim.deep_equal(vim.fn.serverlist(), before), "started " .. vim.inspect(vim.fn.serverlist()))
    end)
  end,

  ["with no HERDR_PANE_ID nothing is started"] = function()
    with_env({ HERDR_PANE_ID = false }, function()
      local before = vim.fn.serverlist()
      pane_socket().listen()
      assert(vim.deep_equal(vim.fn.serverlist(), before), "started " .. vim.inspect(vim.fn.serverlist()))
    end)
  end,

  ["requiring config.autocmds binds nothing until VimEnter"] = function()
    -- auto_reload_spec requires that file in this same runner, whose os.exit
    -- skips socket cleanup: a bind at require time left a stale socket named
    -- for the REAL pane in the operator's real run root after every run.
    local before = vim.fn.serverlist()
    package.loaded["config.autocmds"] = nil
    require("config.autocmds")
    assert(vim.deep_equal(vim.fn.serverlist(), before), "require bound " .. vim.inspect(vim.fn.serverlist()))
    local wired = vim.api.nvim_get_autocmds({ group = "nvim_config_pane_socket", event = "VimEnter" })
    assert(#wired == 1, "expected one VimEnter autocmd in nvim_config_pane_socket, found " .. #wired)
  end,

  ["with the name already taken, listen() is silent and the servers are unchanged"] = function()
    local root = private_root()
    local expected = root .. "/herdr-pane-w1.p2.sock"
    with_env({ XDG_RUNTIME_DIR = root, HERDR_PANE_ID = "w1:p2" }, function()
      -- The earlier Neovim in this pane, which owns the name.
      assert(vim.fn.serverstart(expected) == expected)
      local before = vim.fn.serverlist()
      local servername = vim.v.servername
      pane_socket().listen()
      assert(vim.deep_equal(vim.fn.serverlist(), before), "servers changed: " .. vim.inspect(vim.fn.serverlist()))
      assert(vim.v.servername == servername, "v:servername changed to " .. vim.v.servername)
      vim.fn.serverstop(expected)
    end)
  end,
}
