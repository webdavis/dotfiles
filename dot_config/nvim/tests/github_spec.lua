-- custom_api.github (spec 6.2): every shell call goes through the injected
-- `github.runner`, so a spec can answer for `gh` without one running.

local github = require("custom_api.github")

-- `#` is undefined on a table with an embedded nil, and `nil, "message"` is
-- exactly the shape under test, so the count comes from `select` instead.
local function collect(...)
  return select("#", ...), { ... }
end

-- `replies` maps a command string to the `{ exit_code, output }` the real
-- runner would have returned, already trimmed the way it trims. An unlisted
-- command is an error rather than a fallthrough, so a call that asked for the
-- wrong thing cannot pass quietly.
--
-- It cannot catch a call that skipped `github.runner` altogether, though: a
-- real `gh` that fails reports the same `nil, message` shape the fake does, so
-- a bypassed failure case still goes green. `seen` records the commands the
-- runner was handed, and a case that reads it closes that hole.
local seen = {}

local function with_shell(replies, fn)
  local real_runner = github.runner
  seen = {}
  github.runner = function(opts)
    table.insert(seen, opts.cmd)
    local reply = replies[opts.cmd] or error("unexpected shell command: " .. tostring(opts.cmd))
    return reply[1], reply[2]
  end
  local count, results = collect(pcall(fn))
  github.runner = real_runner
  assert(results[1], results[2])
  return unpack(results, 2, count)
end

local REPO_COMMAND = "gh repo view --json nameWithOwner --jq '.nameWithOwner'"

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
-- halves are there because `vim.fn.system` captures stderr as well as stdout.
local GH_NOT_FOUND = '{"message":"Not Found","status":"404"}gh: Not Found (HTTP 404)'

return {
  ["repo reads the owner and the name out of one gh call"] = function()
    local repo, err = with_shell({ [REPO_COMMAND] = { 0, OWNER .. "/" .. NAME } }, github.repo)
    assert(err == nil, "reported " .. tostring(err))
    assert(type(repo) == "table", "repo returned a " .. type(repo))
    assert(repo.owner == OWNER, "owner was " .. tostring(repo.owner))
    assert(repo.name == NAME, "name was " .. tostring(repo.name))
    assert(repo.nameWithOwner == OWNER .. "/" .. NAME, "nameWithOwner was " .. tostring(repo.nameWithOwner))
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
    assert(seen[1] == DEFAULT_BRANCH_COMMAND, "asked for " .. tostring(seen[1]))
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
    assert(seen[1] == DEFAULT_BRANCH_COMMAND, "asked for " .. tostring(seen[1]))
  end,

  ["default_branch treats an empty answer from a successful gh call as a failure"] = function()
    local branch, err = with_shell({ [DEFAULT_BRANCH_COMMAND] = { 0, "" } }, function()
      return github.default_branch({ owner = OWNER, name = NAME })
    end)
    assert(branch == nil, "returned a branch for an empty answer")
    assert(type(err) == "string", "err was a " .. type(err))
    assert(#seen == 1, "asked the shell " .. #seen .. " times, not once")
    assert(seen[1] == DEFAULT_BRANCH_COMMAND, "asked for " .. tostring(seen[1]))
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
}
