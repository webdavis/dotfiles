local M = {}

local module_name = "custom_api.github"

local helpers = require("custom_api.helpers")
local util = require("custom_api.util")

-- ╭──────────╮
-- │  Helpers │
-- ╰──────────╯
local log_warning = vim.log.levels.WARN

-- ╭──────╮
-- │  API │
-- ╰──────╯
local function account(opts)
  _ = opts or {}

  local fullname_exit, fullname = util.run_shell_command({ cmd = "git config --get user.name" })

  if fullname_exit ~= 0 then
    fullname_exit, fullname = util.run_shell_command({ cmd = "gh api user --jq .name" })
  end

  if fullname_exit ~= 0 then
    return nil,
      "Unable to read gitconfig *user.name* and not logged into GitHub CLI.\n"
        .. 'Run `git config --global user.name "Aaron H. Swartz"` to set it.\n\n'
        .. "Additionally, run `gh auth login` to login to GitHub"
  end

  local username_exit, username = util.run_shell_command({ cmd = "git config --get github.username" })

  if username_exit ~= 0 then
    username_exit, username = util.run_shell_command({ cmd = "gh api user --jq .login" })
  end

  if username_exit ~= 0 then
    return nil,
      "Unable to read gitconfig *github.username* and not logged into GitHub CLI.\n"
        .. 'Run `git config --global github.username "github_account_name"` to set it.\n\n'
        .. "Additionally, run `gh auth login` to login to GitHub"
  end

  return { fullname = fullname, username = username }
end

local function repo()
  local json_field = "nameWithOwner"
  local jq_filter = ".nameWithOwner"

  local exit, result = util.run_shell_command({
    cmd = string.format("gh repo view --json %s --jq '%s'", json_field, jq_filter),
  })

  if exit ~= 0 or not result or result == "" then
    return nil,
      string.format(
        "Failed to get GitHub repository info for '%s'.\n"
          .. "Make sure you're logged in with `gh auth login` and the repo exists.",
        json_field
      )
  end

  local owner, name = result:match("([^/]+)/([^/]+)")

  return {
    nameWithOwner = result,
    owner = owner,
    name = name,
  }
end

-- ╭─────────────────────╮
-- │  Wrapped Functions  │
-- ╰─────────────────────╯
M.account = helpers.wrap(module_name, account, { log_level = log_warning })
M.repo = helpers.wrap(module_name, repo, { log_level = log_warning })

return M
