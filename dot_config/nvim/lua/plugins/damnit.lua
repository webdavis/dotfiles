-- damnit.nvim: dam's tasks from inside the editor, and the editor half of the
-- herdr-damnit pane. `:Dam task <oid>` opens one object as a buffer.
--
-- `views` carries the same names and queries as the herdr pane's `[[views]]`
-- entries (dot_config/herdr/plugins/config/herdr-damnit/config.toml), so one
-- word opens one list in both. Each query is in dam's grammar, and `!done`
-- keeps a completed task out, because `due:today` matches one too.
--
-- The plugin holds no token: dam reads the Todoist credential named in
-- ~/.config/dam/config.toml.
return {
  "webdavis/damnit.nvim",
  commit = "a60975baf023e79174a43c52169ee390c0aa004c",
  cmd = "Dam",
  opts = {
    views = {
      today = "!done & (due:today | overdue)",
      upcoming = "!done & (due:this-week | due:next-week)",
      -- The Todoist projects webdavis > dotfiles, which dam keeps as this
      -- path, matching this repo's own path (webdavis/dotfiles).
      dotfiles = "!done & path:webdavis/dotfiles/",
    },
  },
  keys = {
    { "<leader>Ts", "<Cmd>Dam<CR>", desc = "dam: the staging window" },
    { "<leader>Tt", "<Cmd>Dam list<CR>", desc = "dam: every open task" },
    { "<leader>Td", "<Cmd>Dam list today<CR>", desc = "dam: today" },
    { "<leader>Tb", "<Cmd>Dam toggle<CR>", desc = "dam: toggle the sidebar" },
    { "<leader>Tp", "<Cmd>Dam pick<CR>", desc = "dam: search the tasks" },
    { "<leader>Th", "<Cmd>Dam done<CR>", desc = "dam: the completed history" },
    { "<leader>Tc", "<Cmd>Dam capture<CR>", desc = "dam: capture a task from here" },
    { "<leader>Tc", ":Dam capture<CR>", mode = "x", desc = "dam: capture this selection" },
  },
}
