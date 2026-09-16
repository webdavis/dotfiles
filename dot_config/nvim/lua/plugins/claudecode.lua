-- claudecode.nvim with terminal provider "none" (spec 7.2). The plugin runs the
-- WebSocket server and writes ~/.claude/ide/<port>.lock; it opens no window. The
-- CLI in the herdr agent pane connects with `claude --ide`, or `/ide` inside a
-- running session.
--
-- The research doc's eight open questions (inventory item 78), each with its
-- disposition:
--
-- 1. Reverse-engineered protocol: accepted; the pin is a commit, not a tag, and a
--    protocol break shows up as a failed section 10.8 check at bump time, never
--    silently.
-- 2. No auto-launch under "none": by design; the launch helper (7.2, a later
--    pull request) is the convenience.
-- 3. The send queue clears on the connection timeout: documented; a send with no
--    client connected is lost after the timeout, so connect first.
-- 4. Connection ordering: Neovim first (it writes the lock file), then
--    `claude --ide` or `/ide`.
-- 5. The snacks dependency under "none": declared because the README requires it;
--    whether "none" exercises it is checked once here by loading with snacks
--    present, which it always is.
-- 6. Diff auto-accept versus auto-save: the auto-save exclusions in
--    custom_api/autosave.lua keep auto-save off the `(proposed)` buffers, so the
--    diff is accepted or denied only by the keymaps below.
-- 7. The "beta" label: accepted as the cost of decision A.
-- 8. Local-install PATH: `claude` is on PATH in every herdr pane through the
--    bashrc; nothing to do.
--
-- `event = "VeryLazy"` is the load trigger, because question 4 above wants the
-- lock file written before the CLI connects and nothing presses a key first.
--
-- That trigger cost two warnings and a stack trace on every interactive start
-- between 2026-09-12 and 2026-09-15: something local made a PLAIN HTTP request
-- to the plugin's port within a second of the server coming up, the plugin
-- answered `Bad WebSocket upgrade request`, the prober hung up, and the
-- plugin's own unguarded `tcp_handle:close()` in `server/client.lua` raised
-- through `vim.schedule`. It was briefly fixed by dropping this trigger, at the
-- cost of that ordering.
--
-- THE PROBER IS `moshi-hook`, identified on 2026-09-15 with a decoy server that
-- logged its peer: `GET / HTTP/1.1`, `User-Agent: Go-http-client/1.1`, from the
-- `moshi-hook serve` daemon. `moshi-hook servers` is documented as "Probe local
-- TCP listeners and print HTTP servers for SSH preflight", and the daemon runs
-- it continuously, so it reaches any listener it considers eligible.
--
-- `port_range` below is what stops it, and the number is measured rather than
-- guessed. Decoy servers were probed at 20000, 30000, 40000 and 49151, and left
-- alone at 49152, 50000 and 55000. 49152 is where macOS starts allocating
-- ephemeral ports (`sysctl net.inet.ip.portrange.first`), which is the range a
-- listener-discovery pass has no reason to fingerprint. Pinning the server into
-- it keeps every start silent AND keeps the eager trigger, so neither half of
-- the trade is paid. The crash itself is third-party code and is not patched.
--
-- `moshi-hook set scan-ports <list>` WOULD be a second layer, and is deliberately
-- not set: `~/.config/moshi/config.toml` reads `scan_ports = "all"`, because the
-- operator wants moshi to pick up a development server on any port while they are
-- away from the desk. The five-start proof was taken with `all` in place for
-- exactly that reason. So `port_range` is the ONLY thing keeping the probe off this
-- listener. Do not read this paragraph as a spare and remove the pin.
--
-- `cond` keeps the plugin out of headless Neovim entirely (a `nvim --headless`
-- launch or a `-l` script run, both used by this repo's own test suites and
-- tooling), rather than merely quieting its logger the way `opts.log_level` below
-- does. It scans `vim.v.argv` for the literal flags `--headless` and `-l`, not
-- `#vim.api.nvim_list_uis() == 0`: lazy.nvim evaluates a spec's `cond` during its
-- early spec-parse/resolve pass (`lazy/core/meta.lua`, `fix_cond`), well before
-- any of this spec's triggers fire (`VeryLazy` first, then a `cmd` or one of the
-- `keys`) and before an interactive session's own UI is guaranteed to have
-- attached, so a UI-count check is unreliable at that point. The `opts.log_level` check below runs at a different,
-- later evaluation point (actual plugin load), where a UI count is reliable,
-- which is why it keeps using it.

---The lowest port the listener probe leaves alone, which is what this number is
---FOR; what it IS is the first port macOS allocates as ephemeral
---(`sysctl net.inet.ip.portrange.first`). Measured rather than assumed: decoy
---servers were probed at every port below it and at none at or above it.
local LOWEST_UNPROBED_PORT = 49152

local function is_headless()
  for _, arg in ipairs(vim.v.argv) do
    if arg == "--headless" or arg == "-l" then
      return true
    end
  end
  return false
end

return {
  "coder/claudecode.nvim",
  commit = "2390c6e45c4789072c293ac69de051d169668b29",
  cond = not is_headless(),
  dependencies = {
    "folke/snacks.nvim",
  },
  event = "VeryLazy",
  cmd = {
    "ClaudeCodeAdd",
    "ClaudeCodeDiffAccept",
    "ClaudeCodeDiffDeny",
    "ClaudeCodeSend",
    "ClaudeCodeStatus",
  },
  keys = {
    { "<leader>Cs", "<cmd>ClaudeCodeSend<cr>", mode = "v", desc = "Claude: send selection" },
    { "<leader>Ca", "<cmd>ClaudeCodeAdd %<cr>", desc = "Claude: add current file" },
    { "<leader>Cy", "<cmd>ClaudeCodeDiffAccept<cr>", desc = "Claude: accept diff" },
    { "<leader>Cn", "<cmd>ClaudeCodeDiffDeny<cr>", desc = "Claude: deny diff" },
    -- Not a claudecode.nvim command: the shared herdr seam (spec 7.4) sends raw
    -- text to the agent's herdr pane, which is the path that reaches a free-text
    -- prompt, a non-Claude agent and an unsaved scratch buffer. It lives on this
    -- spec because `<leader>C` is the Claude group, and every key of that group
    -- is declared in one place.
    --
    -- The seam needs no WebSocket server. With `event = "VeryLazy"` the server is
    -- already up before any key is pressed, so this key starts nothing and there is
    -- no server for moving it out to save; the reason it stays here is simply that
    -- `<leader>C` is the Claude group and one group lives in one file.
    {
      "<leader>Cp",
      function()
        require("custom_api.herdr").send_selection_or_paragraph()
      end,
      mode = { "n", "x" },
      desc = "Claude: send selection or paragraph",
    },
    -- The convenience for question 2 above: `none` auto-launches nothing, so
    -- this prompts `/ide` at the Claude agent already in the workspace, or
    -- splits a pane beside the editor and starts one there.
    {
      "<leader>Cc",
      function()
        require("custom_api.herdr").launch_or_attach()
      end,
      desc = "Claude: launch or attach --ide",
    },
  },
  -- The plugin's logger sends WARN and ERROR through `vim.notify`, and INFO, DEBUG
  -- and TRACE through `nvim_echo`, which is STDERR in a headless run. So every
  -- `--headless` start that reached `VimLeavePre` printed `[ClaudeCode] [init]
  -- [INFO] Claude Code integration stopped` (`logger.lua`, `init.lua:597` at the
  -- pinned commit) and failed the zero-stderr startup gate. A global `warn` is too
  -- wide: `:ClaudeCodeStatus` answers at INFO as well (`init.lua:619-621`), and
  -- would go silent. A headless session is the one kind with no UI attached, and
  -- `opts` is evaluated when the plugin loads, which in an interactive session is
  -- `VeryLazy` (lazy.nvim fires it from `UIEnter`, or from a `vim.schedule` after
  -- `VimEnter`) rather than a `cmd` or a key, so the UI is already attached by then.
  -- This quiets exactly the sessions whose INFO was noise and nothing else. The one gap is a UI that
  -- attaches AFTER the plugin loaded (an `--embed` client that ran commands before
  -- attaching): `init` closes it by raising the level back to the plugin's default
  -- through the logger's own `setup` on the first `UIEnter`, and only when the
  -- logger has already been loaded, so a normal interactive start (UI first, plugin
  -- on `VeryLazy`) is untouched.
  init = function()
    -- Not `once`: a UI can attach and detach before the plugin loads (an
    -- `--embed` client attaching, detaching, and then running a `ClaudeCode*`
    -- command with no UI), and a one-shot hook consumed then would leave a later
    -- attach unable to restore INFO. The hook stays until it has something to
    -- restore, and removes itself only after it has done so.
    local group = vim.api.nvim_create_augroup("claudecode_restore_info", { clear = true })
    vim.api.nvim_create_autocmd("UIEnter", {
      group = group,
      desc = "claudecode.nvim: restore INFO logging once a UI is attached",
      callback = function()
        local logger = package.loaded["claudecode.logger"]
        if not logger then
          return
        end
        logger.setup({ log_level = "info" })
        vim.api.nvim_del_augroup_by_id(group)
      end,
    })
  end,
  opts = function()
    return {
      log_level = #vim.api.nvim_list_uis() == 0 and "warn" or "info",
      -- The upstream default is 10000 to 65535, which put the server where
      -- `moshi-hook`'s listener discovery reaches it. See the measurement in
      -- the comment block above: 49152 is the first port it left alone.
      port_range = { min = LOWEST_UNPROBED_PORT, max = 65535 },
      terminal = {
        provider = "none",
      },
    }
  end,
}
