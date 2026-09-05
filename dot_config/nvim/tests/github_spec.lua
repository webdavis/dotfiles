-- custom_api.github (spec 6.2): every shell call goes through the injected
-- `github.runner`, so a spec can answer for `gh` without one running.

local github = require("custom_api.github")

-- `#` is undefined on a table with an embedded nil, and `nil, "message"` is
-- exactly the shape under test, so the count comes from `select` instead.
local function collect(...)
  return select("#", ...), { ... }
end

-- Every call here interpolates a value, so argv is the only form allowed
-- through the seam and the fake refuses shell text outright. `replies` maps the
-- argv words joined by spaces to the `{ exit_code, output }` the real runner
-- would have returned, already trimmed the way it trims. An unlisted command is
-- an error rather than a fallthrough, so a call that asked for the wrong thing
-- cannot pass quietly.
--
-- It cannot catch a call that skipped `github.runner` altogether, though: a
-- real `gh` that fails reports the same `nil, message` shape the fake does, so
-- a bypassed failure case still goes green. `seen` records the whole `opts` of
-- every call the runner was handed, so a case can read its argv and the
-- directory it was told to run in, and a case that reads it closes that hole.
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

-- A throwaway scratch buffer, made current, so a case can ask what these calls
-- do with a buffer that HAS a file of its own. `fn` is handed the buffer's own
-- name, because nvim rewrites the name it is given (`/var` becomes
-- `/private/var` on macOS).
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

-- Deliberately not this repository. A call that reached the real `gh` would
-- answer with the true remote, so these values cannot pass by accident.
local OWNER = "sentinel-owner"
local NAME = "sentinel-repo"

-- Not "main" and not "master": the two names `git.default_branch` answers with
-- on its own, so a case that reached the local checks instead of `gh` cannot
-- pass by accident either.
local TRUNK = "sentinel-trunk"

local DEFAULT_BRANCH_COMMAND = ("gh api repos/%s/%s --jq .default_branch"):format(OWNER, NAME)

-- What a real `gh api` prints for a repository it cannot see, measured. Both
-- halves are there because the runner appends stderr to a failed command's output.
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

  -- Most callers pair `repo` with `git.current_branch`, `git.initialized` and
  -- `git.default_branch`, which all answer for nvim's cwd. A `repo` that
  -- followed the buffer on its own therefore let `build_remote_repo_info`
  -- compose `repo-B/tree/<branch-from-A>`, so asking is now the caller's job
  -- and a bare call leaves nvim's cwd alone even with a file buffer current.
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

  -- Bugs #4 and #4b. The fallback used to live in `git.default_branch`, where
  -- it built its command with two `%s` and one argument, so every call that
  -- reached it raised `bad argument #3 to 'format'`. Both halves of the
  -- repository are in the command string below, and `with_shell` refuses any
  -- command it was not given, so a one-argument format goes red here.
  ["default_branch reads the repository's default branch from one gh call"] = function()
    local branch, err = with_shell({ [DEFAULT_BRANCH_COMMAND] = { 0, TRUNK } }, function()
      return github.default_branch({ owner = OWNER, name = NAME })
    end)
    assert(err == nil, "reported " .. tostring(err))
    assert(branch == TRUNK, "branch was " .. tostring(branch))
    assert(#seen == 1, "asked the shell " .. #seen .. " times, not once")
    assert(command_at(1) == DEFAULT_BRANCH_COMMAND, "asked for " .. tostring(command_at(1)))
  end,

  -- The output is deliberately not empty. A `gh` that cannot see the repository
  -- exits non-zero and still prints, so the exit code is the only thing that
  -- catches it; an empty reply here would let the `result == ""` clause carry
  -- the case on its own and leave the exit code unchecked, which is the shape
  -- that actually reaches the mapping.
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

  -- `repo` hands back a table whose `owner` and `name` are nil when the answer
  -- carried no slash, so a half-filled repository does reach here.
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

  -- Item 1. `<C-g>i` builds a `gh repo clone <user>/<project>` line out of
  -- `account().username`, so a username that never resolved has to arrive as a
  -- failure and not as a table with a nil field: the mapping would otherwise
  -- concatenate nil and raise deep inside the second prompt's callback.
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

  -- `repo` reports a `gh` that cannot answer as `nil, message`. Reading a field
  -- off that nil raised `attempt to index a nil value` on a machine where `gh`
  -- could not read its config, so the failure has to come back as a result the
  -- keymap layer can notify, not as a raise.
  -- The blame path IS the one that has to follow the buffer: `<C-g>Bo` pairs
  -- this with `git.blame_sha`, which runs beside the same file, so the
  -- repository in the URL and the commit the SHA came from must be one repository.
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
