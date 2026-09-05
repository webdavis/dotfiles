local M = {}

local util = require("custom_api.util")

-- The shell seam (spec 6.2). Every shell call in this module goes through
-- `M.runner`, so a test replaces this one field instead of reaching into
-- `util`, and nothing here shells out behind the seam's back.
M.runner = util.run_shell_command

-- ╭──────╮
-- │  API │
-- ╰──────╯
local function account(opts)
  _ = opts or {}

  local fullname_exit, fullname = M.runner({ cmd = { "git", "config", "--get", "user.name" } })

  if fullname_exit ~= 0 then
    fullname_exit, fullname = M.runner({ cmd = { "gh", "api", "user", "--jq", ".name" } })
  end

  if fullname_exit ~= 0 then
    return nil,
      "Unable to read gitconfig *user.name* and not logged into GitHub CLI.\n"
        .. 'Run `git config --global user.name "Aaron H. Swartz"` to set it.\n\n'
        .. "Additionally, run `gh auth login` to login to GitHub"
  end

  local username_exit, username = M.runner({ cmd = { "git", "config", "--get", "github.username" } })

  if username_exit ~= 0 then
    username_exit, username = M.runner({ cmd = { "gh", "api", "user", "--jq", ".login" } })
  end

  if username_exit ~= 0 then
    return nil,
      "Unable to read gitconfig *github.username* and not logged into GitHub CLI.\n"
        .. 'Run `git config --global github.username "github_account_name"` to set it.\n\n'
        .. "Additionally, run `gh auth login` to login to GitHub"
  end

  return { fullname = fullname, username = username }
end

-- `gh` reports the repository of the directory it runs in, so the answer
-- follows the buffer rather than the directory nvim happened to start in.
-- `opts.cwd` overrides that; a buffer with no file of its own leaves nvim's cwd
-- in place, which is what every caller got before.
local function repo(opts)
  opts = opts or {}
  local json_field = "nameWithOwner"
  local jq_filter = ".nameWithOwner"

  local exit, result = M.runner({
    cmd = { "gh", "repo", "view", "--json", json_field, "--jq", jq_filter },
    cwd = opts.cwd or util.file_dir(vim.api.nvim_buf_get_name(0)),
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

local function default_branch(opts)
  opts = opts or {}
  local owner = opts.owner
  local name = opts.name

  if not owner then
    error("Missing required argument `owner`")
  end
  if not name then
    error("Missing required argument `name`")
  end

  local exit, result = M.runner({
    cmd = { "gh", "api", ("repos/%s/%s"):format(owner, name), "--jq", ".default_branch" },
  })

  if exit ~= 0 or not result or result == "" then
    return nil,
      string.format(
        "Failed to get the default branch of '%s/%s'.\n"
          .. "Make sure you're logged in with `gh auth login` and the repo exists.",
        owner,
        name
      )
  end

  return result
end

-- `repo` answers `nil, message` when `gh` cannot; that failure is passed on
-- as a result rather than read as a table, which raised on the index.
local function commit_url(sha)
  if not sha then
    error("Missing required argument `sha`")
  end

  local repository, err = repo()
  if not repository then
    return nil, err
  end

  return ("https://github.com/%s/commit/%s"):format(repository.nameWithOwner, sha)
end

M.account = account
M.repo = repo
M.default_branch = default_branch
M.commit_url = commit_url

return M
