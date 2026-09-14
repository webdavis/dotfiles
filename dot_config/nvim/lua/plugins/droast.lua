-- Dockerfile linting through the `droast` CLI (Homebrew formula `droast`).
--
-- No `opts` here on purpose: upstream's own `plugin/droast.lua` calls
-- `require("droast").setup()` when the plugin loads, and its defaults already are
-- `command = "droast"` and `on_save = true`. An `opts` table makes lazy.nvim call
-- `setup()` a SECOND time after that file has run, so add one only to change a
-- default, never to restate one.
--
-- This runs ALONGSIDE the none-ls `diagnostics.hadolint` source in `lsp.lua`,
-- deliberately, and needs no none-ls adapter of its own because it publishes into
-- its own `vim.diagnostic` namespace (so neither linter clears the other).
-- Measured on the same two Dockerfiles: droast reproduces every hadolint finding
-- about Dockerfile INSTRUCTIONS (DL3007/3008/3009/3015/3003/3042/3064/3025 all have
-- a DF equivalent) and adds its own, but it does not read `RUN` bodies as shell at
-- all, so hadolint's embedded shellcheck (SC2086 on an unquoted expansion) and its
-- checks on a `HEALTHCHECK CMD` have no droast counterpart. Dropping hadolint would
-- lose that; keeping both only costs duplicate instruction diagnostics.
return {
  "immanuwell/droast.nvim",
  ft = "dockerfile",
  cmd = { "DroastLint", "DroastQuickfix" },
}
