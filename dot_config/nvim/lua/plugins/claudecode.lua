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
    --
    -- KNOWN SIDE EFFECT: reaching the seam through this spec loads the plugin,
    -- so the first `<leader>Cp` also starts the WebSocket server and writes
    -- ~/.claude/ide/<port>.lock, which the seam itself does not need. Kept
    -- deliberately: the lock has to exist before any CLI connects (question 4
    -- above), so an early start is the direction this config wants anyway, and
    -- moving the key out would split one Claude group across two files for a
    -- server that `<leader>Cc` starts a keystroke later regardless.
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
    -- Also not a claudecode.nvim command: the line annotator (spec 7.7) writes
    -- what the operator would otherwise retype into herdr-nvim's own annotation
    -- store. Delivery is not its job, so this key sends nothing anywhere;
    -- `<leader>As` and `<leader>AS` are what paste or send a pending comment.
    --
    -- `annotate.line()` reports a buffer it cannot annotate as `nil, reason` and
    -- never notifies, so the keymap-layer error boundary (spec 6.1) is the one
    -- place that refusal, and any raise out of herdr-nvim, becomes a message.
    {
      "<leader>Cx",
      function()
        require("custom_api.try")(function()
          return require("custom_api.annotate").line()
        end, { label = "annotate.line" })
      end,
      desc = "Claude: annotate line with diagnostic and blame",
    },
  },
  opts = {
    terminal = {
      provider = "none",
    },
  },
}
