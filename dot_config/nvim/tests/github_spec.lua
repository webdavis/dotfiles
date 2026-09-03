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
-- command is an error rather than a fallthrough, so a call that skipped the
-- runner cannot pass quietly.
local function with_shell(replies, fn)
  local real_runner = github.runner
  github.runner = function(opts)
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

  ["account reports an operational failure when neither source answers"] = function()
    local person, err = with_shell({
      ["git config --get user.name"] = { 1, "" },
      ["gh api user --jq .name"] = { 1, "" },
    }, github.account)
    assert(person == nil, "returned a person anyway")
    assert(err:find("gh auth login", 1, true), "the message does not say how to fix it: " .. err)
  end,
}
