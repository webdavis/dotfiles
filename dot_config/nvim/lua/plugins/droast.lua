-- Dockerfile linting through the `droast` CLI (Homebrew formula `droast`).
--
-- No `opts` here on purpose: upstream's own `plugin/droast.lua` calls
-- `require("droast").setup()` when the plugin loads, and its defaults already are
-- `command = "droast"` and `on_save = true`. An `opts` table makes lazy.nvim call
-- `setup()` a SECOND time after that file has run, so add one only to change a
-- default, never to restate one.
--
-- This runs ALONGSIDE the nvim-lint `hadolint` linter in `nvim-lint.lua`,
-- deliberately, and needs no adapter of its own because it publishes into its
-- own `vim.diagnostic` namespace (so neither linter clears the other).
-- Overlap on instruction rules is near-total, and upstream ships the authoritative
-- map rather than a hand-copied list: `droast --hadolint-compatible
-- --hadolint-compatibility-report` maps 52 DL rules, then "All other DL rules:
-- unmatched". hadolint stays for what that leaves out, measured on droast 1.7.0:
-- DL3048 on `LABEL Maintainer=` (unmatched, droast reports nothing), and DL3025 on a
-- `HEALTHCHECK CMD`, which the map calls equivalent to DF018,DF025 even though
-- `--only DF018,DF025` finds nothing there.
-- droast CAN lint `RUN` bodies as shell, it just ships that off: `--shellcheck auto`
-- reproduces hadolint's SC2086 on an unquoted expansion. Nothing here passes it, so
-- shell findings come from hadolint today; `opts = { args = { "--shellcheck", "auto" } }`
-- moves them to droast, at the cost of the second `setup()` call noted above.
return {
  "immanuwell/droast.nvim",
  ft = "dockerfile",
  cmd = { "DroastLint", "DroastQuickfix" },
}
