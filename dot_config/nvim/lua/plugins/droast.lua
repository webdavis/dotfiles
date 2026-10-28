-- Leaves out `opts` because the plugin calls `setup()` itself, and `opts` makes lazy.nvim call it again.
-- Runs beside hadolint, which checks the rules droast leaves out and the shell code in `RUN` lines.
return {
  "immanuwell/droast.nvim",
  ft = "dockerfile",
  cmd = { "DroastLint", "DroastQuickfix" },
}
