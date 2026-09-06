-- custom_api.pane_socket (spec 7.3): the socket a Neovim in a herdr pane
-- listens on so the nvim-mcp resolver can derive its path from herdr's
-- identity for that pane. Real `serverstart` calls, because the behavior
-- under test is what the server layer does with a taken or unsafe name; a
-- fake would answer for nothing. herdr IS faked: a script on a private PATH
-- that checks the exact invocation and answers what the case tells it to.
-- `-l` mode starts no default server, so every list assertion is relative to
-- whatever this process already has.

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
-- the sockets these cases bind never land in the real shared root. Short,
-- removed when this process exits, and 0700 like the real one.
local function private_root()
  local dir = vim.fn.tempname()
  assert(vim.fn.mkdir(dir, "p", 448) == 1, "could not create " .. dir)
  return dir
end

-- The herdr stand-in, written once. It refuses any invocation other than the
-- one the module makes, logs each call, and answers with the document in
-- HERDR_STUB_JSON or fails when HERDR_STUB_FAIL is set.
local stub_dir = private_root()
do
  local script = stub_dir .. "/herdr"
  local handle = assert(io.open(script, "w"))
  handle:write(table.concat({
    "#!/bin/bash",
    'printf \'%s\\n\' "$*" >>"$HERDR_STUB_LOG"',
    '[[ "$*" == "pane current --current" ]] || { printf \'unexpected herdr argv: %s\\n\' "$*" >&2; exit 99; }',
    "[[ -z ${HERDR_STUB_FAIL:-} ]] || exit 1",
    "printf '%s' \"$HERDR_STUB_JSON\"",
    "",
  }, "\n"))
  handle:close()
  assert(vim.uv.fs_chmod(script, 493), "could not chmod " .. script) -- 0755
end

local function serving(path)
  return vim.tbl_contains(vim.fn.serverlist(), path)
end

local function herdr_calls(log)
  local handle = io.open(log)
  if not handle then
    return {}
  end
  local lines = vim.split(handle:read("a"), "\n", { trimempty = true })
  handle:close()
  return lines
end

-- Every listen() case runs under herdr (HERDR_ENV), in a private root, with
-- the stub first on PATH answering `terminal` for this pane. Those cases use a
-- SHORT terminal id: the private root under this process's temp tree is about
-- ten characters deeper than the real `$TMPDIR/nvim.<user>`, and a real-length
-- id would push the socket path past the 104-byte limit that production stays
-- inside (measured: 99 bytes for a seven-character user name).
local function herdr_env(root, terminal, extra)
  local env = {
    HERDR_ENV = "1",
    HERDR_SOCKET_PATH = "/s/a.sock",
    XDG_RUNTIME_DIR = root,
    PATH = stub_dir .. ":" .. vim.env.PATH,
    HERDR_STUB_LOG = root .. "/herdr.log",
    HERDR_STUB_JSON = terminal
        and ('{"id":"cli:pane:current","result":{"pane":{"pane_id":"w1:p2","tab_id":"w1:t1","terminal_id":"%s","workspace_id":"w1"}}}'):format(
          terminal
        )
      or "",
    HERDR_STUB_FAIL = false,
  }
  for name, value in pairs(extra or {}) do
    env[name] = value
  end
  return env
end

-- Capture vim.notify while `fn` runs.
local function capturing_notify(fn)
  local seen = {}
  local real = vim.notify
  vim.notify = function(message, level)
    table.insert(seen, { message = message, level = level })
  end
  local ok, err = pcall(fn)
  vim.notify = real
  assert(ok, err)
  return seen
end

-- sha256("/s/a.sock") starts with 9a663d; the resolver's test fixture pins the
-- same six characters, so the two sides are held to one rule.
local SESSION = "9a663d"

return {
  ["a terminal id becomes a socket path in the run root, namespaced by the session"] = function()
    local root = private_root()
    with_env({ XDG_RUNTIME_DIR = root, HERDR_SOCKET_PATH = "/s/a.sock" }, function()
      local expected = root .. "/herdr-" .. SESSION .. "-term_65a9c8766b9261.sock"
      assert(pane_socket().path("term_65a9c8766b9261") == expected, tostring(pane_socket().path("term_65a9c8766b9261")))
    end)
  end,

  ["two herdr sessions give the same terminal id two different names"] = function()
    local root = private_root()
    local a, b
    with_env({ XDG_RUNTIME_DIR = root, HERDR_SOCKET_PATH = "/s/a.sock" }, function()
      a = pane_socket().path("term_1")
    end)
    with_env({ XDG_RUNTIME_DIR = root, HERDR_SOCKET_PATH = "/s/b.sock" }, function()
      b = pane_socket().path("term_1")
    end)
    assert(a and b and a ~= b, ("session did not namespace: %s vs %s"):format(tostring(a), tostring(b)))
  end,

  ["an id that cannot sit in a socket path yields no path"] = function()
    local unsafe = { "../term_1", "term_1/x", "term 1", "term\n1", "", ("a"):rep(65), 42, nil }
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
      assert(pane_socket().path("term_1") == nil, "path is " .. tostring(pane_socket().path("term_1")))
    end)
  end,

  ["under herdr, listen() asks herdr for the terminal and serves its socket"] = function()
    local root = private_root()
    local expected = root .. "/herdr-" .. SESSION .. "-term_a1.sock"
    with_env(herdr_env(root, "term_a1"), function()
      pane_socket().listen()
      assert(
        vim.wait(2000, function()
          return serving(expected)
        end, 10),
        "not serving " .. expected .. ": " .. vim.inspect(vim.fn.serverlist())
      )
      assert(vim.deep_equal(herdr_calls(root .. "/herdr.log"), { "pane current --current" }))
      vim.fn.serverstop(expected)
    end)
  end,

  ["a run root not owned by this user at 0700 starts nothing, says so once, and never asks herdr"] = function()
    local root = private_root()
    assert(vim.uv.fs_chmod(root, 493), "could not chmod " .. root) -- 0755: readable by other accounts
    with_env(herdr_env(root, "term_a1"), function()
      local before = vim.fn.serverlist()
      local seen = capturing_notify(function()
        pane_socket().listen()
        vim.wait(50)
      end)
      assert(vim.deep_equal(vim.fn.serverlist(), before), "started " .. vim.inspect(vim.fn.serverlist()))
      assert(#seen == 1 and seen[1].level == vim.log.levels.WARN, "notifications: " .. vim.inspect(seen))
      assert(seen[1].message:find("0700", 1, true), "the warning does not name the mode: " .. seen[1].message)
      assert(#herdr_calls(root .. "/herdr.log") == 0, "herdr was asked although the root is not private")
    end)
  end,

  ["outside herdr nothing is started and herdr is never asked"] = function()
    local root = private_root()
    with_env(herdr_env(root, "term_1", { HERDR_ENV = false }), function()
      local before = vim.fn.serverlist()
      pane_socket().listen()
      vim.wait(20)
      assert(vim.deep_equal(vim.fn.serverlist(), before), "started " .. vim.inspect(vim.fn.serverlist()))
      assert(#herdr_calls(root .. "/herdr.log") == 0, "herdr was asked outside herdr")
    end)
  end,

  ["a herdr that does not answer starts nothing and says so once"] = function()
    local root = private_root()
    with_env(herdr_env(root, nil, { HERDR_STUB_FAIL = "1" }), function()
      local before = vim.fn.serverlist()
      local seen = capturing_notify(function()
        pane_socket().listen()
        assert(
          vim.wait(2000, function()
            return #herdr_calls(root .. "/herdr.log") > 0
          end, 10),
          "herdr was never asked"
        )
        vim.wait(50)
      end)
      assert(vim.deep_equal(vim.fn.serverlist(), before), "started " .. vim.inspect(vim.fn.serverlist()))
      assert(#seen == 1 and seen[1].level == vim.log.levels.WARN, "notifications: " .. vim.inspect(seen))
      assert(seen[1].message:find("NVIM_MCP_SOCKET", 1, true), "the warning does not name the remedy")
    end)
  end,

  ["a terminal id that cannot name a socket starts nothing"] = function()
    local root = private_root()
    with_env(herdr_env(root, "../term_1"), function()
      local before = vim.fn.serverlist()
      capturing_notify(function()
        pane_socket().listen()
        assert(
          vim.wait(2000, function()
            return #herdr_calls(root .. "/herdr.log") > 0
          end, 10),
          "herdr was never asked"
        )
        vim.wait(50)
      end)
      assert(vim.deep_equal(vim.fn.serverlist(), before), "started " .. vim.inspect(vim.fn.serverlist()))
    end)
  end,

  ["requiring config.autocmds binds nothing; firing its VimEnter autocmd binds the pane socket"] = function()
    -- auto_reload_spec requires that file in this same runner, whose os.exit
    -- skips socket cleanup: a bind at require time once left a stale socket
    -- named for the REAL pane in the operator's real run root after every run.
    -- And an autocmd that is only counted certifies an empty callback, so the
    -- event is fired and the socket has to appear.
    local root = private_root()
    local expected = root .. "/herdr-" .. SESSION .. "-term_a1.sock"
    with_env(herdr_env(root, "term_a1"), function()
      local before = vim.fn.serverlist()
      package.loaded["config.autocmds"] = nil
      require("config.autocmds")
      vim.wait(50)
      assert(vim.deep_equal(vim.fn.serverlist(), before), "require bound " .. vim.inspect(vim.fn.serverlist()))
      assert(#herdr_calls(root .. "/herdr.log") == 0, "require asked herdr")
      vim.api.nvim_exec_autocmds("VimEnter", { group = "nvim_config_pane_socket" })
      assert(
        vim.wait(2000, function()
          return serving(expected)
        end, 10),
        "VimEnter did not bind " .. expected .. ": " .. vim.inspect(vim.fn.serverlist())
      )
      vim.fn.serverstop(expected)
    end)
  end,

  ["with the name already taken, listen() is silent and the servers are unchanged"] = function()
    local root = private_root()
    local expected = root .. "/herdr-" .. SESSION .. "-term_a1.sock"
    with_env(herdr_env(root, "term_a1"), function()
      -- The earlier Neovim in this pane, which owns the name.
      assert(vim.fn.serverstart(expected) == expected)
      local before = vim.fn.serverlist()
      local servername = vim.v.servername
      local seen = capturing_notify(function()
        pane_socket().listen()
        assert(
          vim.wait(2000, function()
            return #herdr_calls(root .. "/herdr.log") > 0
          end, 10),
          "herdr was never asked"
        )
        vim.wait(50)
      end)
      assert(vim.deep_equal(vim.fn.serverlist(), before), "servers changed: " .. vim.inspect(vim.fn.serverlist()))
      assert(vim.v.servername == servername, "v:servername changed to " .. vim.v.servername)
      assert(#seen == 0, "a taken name was reported: " .. vim.inspect(seen))
      vim.fn.serverstop(expected)
    end)
  end,
}
