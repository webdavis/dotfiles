-- `hadolint` runs alongside droast.nvim because it reports rules droast leaves out.
-- luacheck finds unused variables and globals, and emmylua_check adds type errors.

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

      -- emmylua_check reads the file from disk, so it runs only after a write.
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
