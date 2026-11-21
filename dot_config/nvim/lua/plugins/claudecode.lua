local highest_tcp_port = 65535
local moshi_hook_highest_port_probed = 49151

local claudecode_nvim_safe_port_range = {
  min = moshi_hook_highest_port_probed + 1,
  max = highest_tcp_port,
}

local claudecode_nvim_server_start_event = "VeryLazy"
local quiet_log_level = "warn"
local interactive_log_level = "info"

local function argv_has_headless_flag()
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
  cond = not argv_has_headless_flag(),
  dependencies = {
    "folke/snacks.nvim",
  },
  event = claudecode_nvim_server_start_event,
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
  init = function()
    local group = vim.api.nvim_create_augroup("claudecode_restore_info", { clear = true })
    vim.api.nvim_create_autocmd("UIEnter", {
      group = group,
      desc = "claudecode.nvim: restore INFO logging once a UI is attached",
      callback = function()
        local logger = package.loaded["claudecode.logger"]
        local plugin_loaded = logger ~= nil
        if not plugin_loaded then
          return
        end
        logger.setup({ log_level = interactive_log_level })
        vim.api.nvim_del_augroup_by_id(group)
      end,
    })
  end,
  opts = function()
    local ui_attached = #vim.api.nvim_list_uis() > 0
    return {
      log_level = ui_attached and interactive_log_level or quiet_log_level,
      port_range = claudecode_nvim_safe_port_range,
      terminal = {
        provider = "none",
      },
    }
  end,
}
