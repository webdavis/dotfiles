-- `views` uses the same names and queries as the herdr-damnit pane, so one word opens one list in both.
-- `!done` leaves out completed tasks, because `due:today` matches them too.
return {
  "webdavis/damnit.nvim",
  commit = "a60975baf023e79174a43c52169ee390c0aa004c",
  cmd = "Dam",
  opts = {
    views = {
      today = "!done & (due:today | overdue)",
      upcoming = "!done & (due:this-week | due:next-week)",
      -- dam stores the Todoist project webdavis > dotfiles under this path.
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
