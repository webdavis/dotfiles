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
-- No startup trigger here: `cmd` and `keys` are the load triggers, so the plugin
-- costs nothing until a command or a key reaches it. The lock file has to exist
-- before the CLI connects, which is why a later pull request moves this to
-- `event = "VeryLazy"` (spec 9).
return {
  "coder/claudecode.nvim",
  commit = "2390c6e45c4789072c293ac69de051d169668b29",
  dependencies = {
    "folke/snacks.nvim",
  },
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
    -- spec because `<leader>C` is the Claude group and a `keys` entry is what
    -- keeps the whole group lazy.
    {
      "<leader>Cp",
      function()
        require("custom_api.herdr").send_selection_or_paragraph()
      end,
      mode = { "n", "x" },
      desc = "Claude: send selection or paragraph",
    },
  },
  opts = {
    terminal = {
      provider = "none",
    },
  },
}
