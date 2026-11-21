-- The plugin runs only the WebSocket server.
-- The CLI connects with `claude --ide` or `/ide`.
-- `VeryLazy` writes the lock file before the CLI connects.
-- `cond` reads argv, because lazy.nvim evaluates it before a UI attaches.
-- Headless runs skip the plugin.

local highest_tcp_port = 65535
local moshi_hook_highest_port_probed = 49151

local claudecode_safe_port_range = {
  min = moshi_hook_highest_port_probed + 1,
  max = highest_tcp_port,
}

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
      port_range = claudecode_safe_port_range,
      terminal = {
        provider = "none",
      },
    }
  end,
}
