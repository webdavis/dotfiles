-- damnit.nvim: Todoist from inside the editor, and the other half of the
-- herdr-damnit pane, which enters `nvim +"Todoist task <id>"` on `e`.
--
-- `views` carries the same names as the herdr pane's `[[views]]` entries
-- (dot_config/herdr/plugins/config/herdr-damnit/config.toml), so one word
-- opens one list in both.
--
-- The token is an indirection: this names the source it comes from and
-- never holds the value. The plugin has no third way to reach one.
--
-- vim.system closes stdin, so an interactive vault CLI (keepassxc-cli) can
-- never resolve here. The macOS keychain is read non-interactively instead;
-- KeePassXC stays the entry of record (see the herdr-damnit config for the
-- one-time operator setup).
--
-- Pinned to the last commit before the plugin's own rename PR, which is also
-- the last commit that registers a user command. Move this pin forward only
-- once the dam cutover lands and damnit.nvim answers to something again.
return {
  "webdavis/damnit.nvim",
  commit = "f52aee6002e9dadbab9c608f7747ba012ad8aa9d",
  cmd = "Todoist",
  opts = {
    token_command = {
      "security",
      "find-generic-password",
      "-w",
      "-s",
      "Todoist API Token",
    },
    views = {
      today = "today | overdue",
      upcoming = "7 days",
      -- "dotfiles" names two projects; the parent qualifier picks the one
      -- nested under webdavis, matching this repo's own path.
      dotfiles = "##webdavis & #dotfiles",
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
