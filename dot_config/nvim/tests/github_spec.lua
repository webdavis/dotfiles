local github = require("custom_api.github")

-- `select` counts the values, because `#` is undefined with an embedded nil.
local function collect(...)
  return select("#", ...), { ... }
end

-- Accepts argv only and errors on any unlisted command. `replies` maps the joined argv to `{ exit_code, output }`.
-- `seen` records each call's `opts`, so a case can check it went through `github.runner`.
local seen = {}

local function command_at(index)
  return seen[index] and table.concat(seen[index].cmd, " ")
end

local function with_shell(replies, fn)
  local real_runner = github.runner
  seen = {}
  github.runner = function(opts)
    assert(type(opts.cmd) == "table", "runner was handed shell text: " .. tostring(opts.cmd))
    local command = table.concat(opts.cmd, " ")
    table.insert(seen, opts)
    local reply = replies[command] or error("unexpected shell command: " .. command)
    return reply[1], reply[2]
  end
  local count, results = collect(pcall(fn))
  github.runner = real_runner
  assert(results[1], results[2])
  return unpack(results, 2, count)
end

-- A scratch file buffer, made current. `fn` gets the name nvim gave it, because macOS turns `/var` into `/private/var`.
local function in_buffer(fn)
  local previous = vim.api.nvim_get_current_buf()
  local bufnr = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_name(bufnr, vim.fn.tempname())
  vim.api.nvim_set_current_buf(bufnr)
  local count, results = collect(pcall(fn, vim.api.nvim_buf_get_name(bufnr)))
  vim.api.nvim_set_current_buf(previous)
  vim.api.nvim_buf_delete(bufnr, { force = true })
  assert(results[1], results[2])
  return unpack(results, 2, count)
end

local REPO_COMMAND = "gh repo view --json nameWithOwner --jq .nameWithOwner"

-- Not this repository, so a call that reached the real `gh` fails.
local OWNER = "sentinel-owner"
local NAME = "sentinel-repo"

-- Neither `main` nor `master`, so a case that fell back to the local checks fails.
local TRUNK = "sentinel-trunk"

local DEFAULT_BRANCH_COMMAND = ("gh api repos/%s/%s --jq .default_branch"):format(OWNER, NAME)

-- Real `gh api` output for an unseen repository: stdout plus the stderr the runner appends.
local GH_NOT_FOUND = '{"message":"Not Found","status":"404"}gh: Not Found (HTTP 404)'

-- A commit that is not in this repository either; the URL is asserted whole.
local SHA = "581dae8e37117196fb31ce1658a1c55ec3128b19"

return {
  ["repo reads the owner and the name out of one gh call"] = function()
    local repo, err = with_shell({ [REPO_COMMAND] = { 0, OWNER .. "/" .. NAME } }, github.repo)
    assert(err == nil, "reported " .. tostring(err))
    assert(type(repo) == "table", "repo returned a " .. type(repo))
    assert(repo.owner == OWNER, "owner was " .. tostring(repo.owner))
    assert(repo.name == NAME, "name was " .. tostring(repo.name))
    assert(repo.nameWithOwner == OWNER .. "/" .. NAME, "nameWithOwner was " .. tostring(repo.nameWithOwner))
  end,

  -- Callers pair `repo` with `git` helpers that answer for the nvim cwd, so a bare call stays there.
  ["repo asked for no directory runs gh in nvim's own cwd"] = function()
    local repo = in_buffer(function()
      return with_shell({ [REPO_COMMAND] = { 0, OWNER .. "/" .. NAME } }, github.repo)
    end)
    assert(repo.nameWithOwner == OWNER .. "/" .. NAME, "nameWithOwner was " .. tostring(repo.nameWithOwner))
    assert(#seen == 1, "asked the shell " .. #seen .. " times, not once")
    assert(seen[1].cwd == nil, "ran in " .. tostring(seen[1].cwd) .. " rather than leaving cwd alone")
  end,

  ["repo reports a failed gh call as an operational failure"] = function()
    local repo, err = with_shell({ [REPO_COMMAND] = { 1, "" } }, github.repo)
    assert(repo == nil, "returned a repo anyway")
    assert(type(err) == "string", "err was a " .. type(err))
    assert(err:find("gh auth login", 1, true), "the message does not say how to fix it: " .. err)
  end,

  ["repo treats an empty answer from a successful gh call as a failure"] = function()
    local repo, err = with_shell({ [REPO_COMMAND] = { 0, "" } }, github.repo)
    assert(repo == nil, "returned a repo for an empty answer")
    assert(type(err) == "string", "err was a " .. type(err))
  end,

  -- `with_shell` refuses any command it was not given, so a wrong format string fails here.
  ["default_branch reads the repository's default branch from one gh call"] = function()
    local branch, err = with_shell({ [DEFAULT_BRANCH_COMMAND] = { 0, TRUNK } }, function()
      return github.default_branch({ owner = OWNER, name = NAME })
    end)
    assert(err == nil, "reported " .. tostring(err))
    assert(branch == TRUNK, "branch was " .. tostring(branch))
    assert(#seen == 1, "asked the shell " .. #seen .. " times, not once")
    assert(command_at(1) == DEFAULT_BRANCH_COMMAND, "asked for " .. tostring(command_at(1)))
  end,

  -- The output is non-empty, so only the exit code can catch the failure.
  ["default_branch reports a failed gh call as an operational failure"] = function()
    local branch, err = with_shell({ [DEFAULT_BRANCH_COMMAND] = { 1, GH_NOT_FOUND } }, function()
      return github.default_branch({ owner = OWNER, name = NAME })
    end)
    assert(branch == nil, "returned a branch anyway")
    assert(type(err) == "string", "err was a " .. type(err))
    assert(err:find("gh auth login", 1, true), "the message does not say how to fix it: " .. err)
    assert(#seen == 1, "asked the shell " .. #seen .. " times, not once")
    assert(command_at(1) == DEFAULT_BRANCH_COMMAND, "asked for " .. tostring(command_at(1)))
  end,

  ["default_branch treats an empty answer from a successful gh call as a failure"] = function()
    local branch, err = with_shell({ [DEFAULT_BRANCH_COMMAND] = { 0, "" } }, function()
      return github.default_branch({ owner = OWNER, name = NAME })
    end)
    assert(branch == nil, "returned a branch for an empty answer")
    assert(type(err) == "string", "err was a " .. type(err))
    assert(#seen == 1, "asked the shell " .. #seen .. " times, not once")
    assert(command_at(1) == DEFAULT_BRANCH_COMMAND, "asked for " .. tostring(command_at(1)))
  end,

  -- `repo` returns nil `owner` and `name` when its answer has no slash.
  ["default_branch refuses a repository missing its owner or its name"] = function()
    local ok, err = pcall(github.default_branch, { owner = OWNER })
    assert(not ok, "accepted a repository with no name")
    assert(err:find("name", 1, true), "the message does not name `name`: " .. tostring(err))

    local ok_owner, err_owner = pcall(github.default_branch, { name = NAME })
    assert(not ok_owner, "accepted a repository with no owner")
    assert(err_owner:find("owner", 1, true), "the message does not name `owner`: " .. tostring(err_owner))
  end,

  ["account resolves the name and the username from gitconfig"] = function()
    local person, err = with_shell({
      ["git config --get user.name"] = { 0, "Sentinel Person" },
      ["git config --get github.username"] = { 0, OWNER },
    }, github.account)
    assert(err == nil, "reported " .. tostring(err))
    assert(person.fullname == "Sentinel Person", "fullname was " .. tostring(person.fullname))
    assert(person.username == OWNER, "username was " .. tostring(person.username))
  end,

  ["account falls back to the GitHub API when gitconfig has neither"] = function()
    local person = with_shell({
      ["git config --get user.name"] = { 1, "" },
      ["gh api user --jq .name"] = { 0, "Sentinel Person" },
      ["git config --get github.username"] = { 1, "" },
      ["gh api user --jq .login"] = { 0, OWNER },
    }, github.account)
    assert(person.fullname == "Sentinel Person", "fullname was " .. tostring(person.fullname))
    assert(person.username == OWNER, "username was " .. tostring(person.username))
  end,

  -- An unresolved username must come back as a failure, not a table with a nil field.
  ["account reports an operational failure when only the username is unresolvable"] = function()
    local person, err = with_shell({
      ["git config --get user.name"] = { 0, "Sentinel Person" },
      ["git config --get github.username"] = { 1, "" },
      ["gh api user --jq .login"] = { 1, "" },
    }, github.account)
    assert(person == nil, "returned a person with no username")
    assert(type(err) == "string", "err was a " .. type(err))
    assert(err:find("github.username", 1, true), "the message does not name the setting: " .. err)
  end,

  ["account reports an operational failure when neither source answers"] = function()
    local person, err = with_shell({
      ["git config --get user.name"] = { 1, "" },
      ["gh api user --jq .name"] = { 1, "" },
    }, github.account)
    assert(person == nil, "returned a person anyway")
    assert(err:find("gh auth login", 1, true), "the message does not say how to fix it: " .. err)
  end,

  ["commit_url builds the commit's GitHub URL from the repository gh reports"] = function()
    local url, err = with_shell({ [REPO_COMMAND] = { 0, OWNER .. "/" .. NAME } }, function()
      return github.commit_url(SHA)
    end)
    assert(err == nil, "reported " .. tostring(err))
    assert(url == ("https://github.com/%s/%s/commit/%s"):format(OWNER, NAME, SHA), "url was " .. tostring(url))
  end,

  -- The blame path runs `gh` beside the file, so the URL and the SHA name one repository.
  ["commit_url runs gh beside the buffer's file"] = function()
    local dir
    local url = in_buffer(function(file)
      dir = vim.fn.fnamemodify(file, ":h")
      return with_shell({ [REPO_COMMAND] = { 0, OWNER .. "/" .. NAME } }, function()
        return github.commit_url(SHA)
      end)
    end)
    assert(url == ("https://github.com/%s/%s/commit/%s"):format(OWNER, NAME, SHA), "url was " .. tostring(url))
    assert(#seen == 1, "asked the shell " .. #seen .. " times, not once")
    assert(seen[1].cwd == dir, "ran in " .. tostring(seen[1].cwd) .. ", not " .. tostring(dir))
  end,

  ["commit_url reports a gh that cannot answer instead of indexing nil"] = function()
    local url, err = with_shell({ [REPO_COMMAND] = { 1, "" } }, function()
      return github.commit_url(SHA)
    end)
    assert(url == nil, "returned a url anyway: " .. tostring(url))
    assert(type(err) == "string", "err was a " .. type(err))
    assert(err:find("gh auth login", 1, true), "the message does not say how to fix it: " .. err)
  end,
}
