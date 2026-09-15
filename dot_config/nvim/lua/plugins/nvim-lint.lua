-- Diagnostics from linters that are not language servers. nvim-lint runs the
-- command for a buffer's filetype and publishes the result into its own
-- `vim.diagnostic` namespace, which is what the none-ls diagnostic sources used
-- to do through a fake language server.
--
-- The three rows are the none-ls diagnostic sources that actually ran, with the
-- same filetype split. none-ls expressed that split as a filetype list plus a
-- `disabled_filetypes` list, so `actionlint` was registered for `yaml` and then
-- disabled for `yaml.ansible`; nvim-lint matches a filetype exactly, so the
-- compound filetype simply gets its own row and the yaml row never reaches an
-- Ansible playbook.
--
-- `hadolint` runs ALONGSIDE droast.nvim on a Dockerfile, deliberately, and for
-- the reason recorded in plugins/droast.lua: each publishes into its own
-- namespace, and hadolint still reports rules droast leaves out.
--
-- Two none-ls diagnostic sources are deliberately NOT carried over:
--
--   * `dotenv_linter` was registered for the single filetype `sh` and then
--     disabled for `sh` and `bash`, so it ran on nothing at all.
--   * `eslint` duplicated the `eslint` language server, which is in the Mason
--     roster in plugins/lsp.lua and publishes the same diagnostics for the same
--     filetypes. The none-ls source only ever ran from `node_modules/.bin`;
--     nvim-lint's own eslint linter falls back to a bare `eslint` on PATH and
--     notifies when it is missing, which on this machine is every project.

return {
  {
    "mfussenegger/nvim-lint",
    event = { "BufReadPre", "BufNewFile" },
    config = function()
      local lint = require("lint")

      lint.linters_by_ft = {
        dockerfile = { "hadolint" },
        yaml = { "actionlint" },
        ["yaml.ansible"] = { "ansible_lint" },
      }

      local lint_group = vim.api.nvim_create_augroup("NvimLintGroup", { clear = true })

      -- The same three moments none-ls refreshed its diagnostics on: the buffer
      -- arriving, a write, and leaving insert mode. actionlint and hadolint read
      -- the buffer over stdin, so they see an unwritten edit; ansible-lint is
      -- handed the file name and reads what is on disk, which is why the write
      -- event is in the list rather than a text-change one.
      vim.api.nvim_create_autocmd({ "BufReadPost", "BufWritePost", "InsertLeave" }, {
        group = lint_group,
        callback = function()
          lint.try_lint()
        end,
      })
    end,
  },
}
