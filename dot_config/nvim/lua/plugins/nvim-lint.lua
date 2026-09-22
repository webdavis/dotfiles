-- Diagnostics from linters that are not language servers. nvim-lint runs the
-- command for a buffer's filetype and publishes the result into its own
-- `vim.diagnostic` namespace, which is what the none-ls diagnostic sources used
-- to do through a fake language server.
--
-- The first three rows are the none-ls diagnostic sources that actually ran, with the
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
--
-- Lua is separate from that migration. luacheck keeps its unused-variable and
-- global checks, while emmylua_check adds the type errors enforced by treefmt.

return {
  {
    "mfussenegger/nvim-lint",
    event = { "BufReadPre", "BufNewFile" },
    config = function()
      local lint = require("lint")
      local config_dir = vim.fn.stdpath("config")

      local luacheck = lint.linters.luacheck
      table.insert(luacheck.args, 1, config_dir .. "/.luacheckrc")
      table.insert(luacheck.args, 1, "--config")

      lint.linters.emmylua_check = {
        cmd = "emmylua_check",
        cwd = config_dir,
        stdin = false,
        args = {
          "--config",
          config_dir .. "/.emmyrc.json",
          "--output-format",
          "sarif",
          "--severity",
          "error",
        },
        ignore_exitcode = true,
        parser = require("lint.parser").for_sarif(),
      }

      lint.linters_by_ft = {
        dockerfile = { "hadolint" },
        lua = { "luacheck", "emmylua_check" },
        yaml = { "actionlint" },
        ["yaml.ansible"] = { "ansible_lint" },
      }

      local lint_group = vim.api.nvim_create_augroup("NvimLintGroup", { clear = true })

      -- The same three moments none-ls refreshed its diagnostics on: the buffer
      -- arriving, a write, and leaving insert mode. emmylua_check is the one
      -- disk-backed exception, so it runs only after a write while the other
      -- linters keep their existing schedule.
      vim.api.nvim_create_autocmd({ "BufReadPost", "BufWritePost", "InsertLeave" }, {
        group = lint_group,
        callback = function(args)
          lint.try_lint(nil, {
            filter = function(linter)
              return args.event == "BufWritePost" or linter.name ~= "emmylua_check"
            end,
          })
        end,
      })
    end,
  },
}
