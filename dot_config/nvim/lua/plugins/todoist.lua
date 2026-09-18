-- todoist.nvim: Todoist from inside the editor, and the other half of the
-- herdr-todoist pane, which enters `nvim +"Todoist task <id>"` on `e`.
--
-- `views` carries the same names as the herdr pane's `[[views]]` entries
-- (dot_config/herdr/plugins/config/herdr-todoist/config.toml), so one word
-- opens one list in both.
--
-- The token is an indirection: this names the KeePassXC entry it comes from
-- and never holds the value. The plugin has no third way to reach one.
return {
  "webdavis/todoist.nvim",
  commit = "f52aee6002e9dadbab9c608f7747ba012ad8aa9d",
  cmd = "Todoist",
  opts = {
    token_command = {
      "keepassxc-cli",
      "show",
      "--quiet",
      "--show-protected",
      "--attributes",
      "Password",
      "/Users/stephen/Library/Mobile Documents/iCloud~com~strongbox/Documents/keepass.kdbx",
      "Todoist :: API Token",
    },
    views = {
      today = "today | overdue",
      upcoming = "7 days",
      dotfiles = "#dotfiles",
    },
  },
  keys = {
    { "<leader>Tt", "<Cmd>Todoist<CR>", desc = "Todoist: every open task" },
    { "<leader>Td", "<Cmd>Todoist today<CR>", desc = "Todoist: today" },
    { "<leader>Tb", "<Cmd>Todoist toggle<CR>", desc = "Todoist: toggle the sidebar" },
    { "<leader>Tp", "<Cmd>Todoist pick<CR>", desc = "Todoist: search the tasks" },
    { "<leader>Th", "<Cmd>Todoist completed<CR>", desc = "Todoist: the completed history" },
    { "<leader>Tc", "<Cmd>Todoist capture<CR>", desc = "Todoist: capture a task from here" },
    { "<leader>Tc", ":Todoist capture<CR>", mode = "x", desc = "Todoist: capture this selection" },
  },
}
