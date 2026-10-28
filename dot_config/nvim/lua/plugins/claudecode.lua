-- The plugin runs only the WebSocket server.
-- The CLI in the herdr agent pane connects with `claude --ide` or `/ide`.
-- `VeryLazy` writes the lock file before the CLI connects.
-- `cond` reads argv, because lazy.nvim evaluates it before a UI attaches.
-- Headless runs skip the plugin.

---First macOS ephemeral port. moshi-hook's listener probe skips ports from here up.
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
    -- Sends raw text through herdr to the agent pane.
    -- It sits in this list because `<leader>C` is the Claude group.
    {
      "<leader>Cp",
      function()
        require("custom_api.herdr").send_selection_or_paragraph()
      end,
      mode = { "n", "x" },
      desc = "Claude: send selection or paragraph",
    },
    -- Prompts `/ide` at the workspace's Claude agent, or starts one in a split beside the editor.
    {
      "<leader>Cc",
      function()
        require("custom_api.herdr").launch_or_attach()
      end,
      desc = "Claude: launch or attach --ide",
    },
  },
  -- INFO logs reach stderr in a headless run, so `opts` lowers the level without a UI
  -- and `init` restores it once one attaches.
  init = function()
    -- Stays until the logger exists, because a UI can attach and detach before the plugin loads.
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
      -- moshi-hook probes every lower port, and the plugin crashes on that probe.
      -- This range is the only guard.
      port_range = { min = LOWEST_UNPROBED_PORT, max = 65535 },
      terminal = {
        provider = "none",
      },
    }
  end,
}
