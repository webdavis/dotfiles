-- custom_api.git (spec 6.2): latest_commit's (table, err) result and the error
-- texts that name their own parameters. Extended in PR 7a to 7c.

local git = require("custom_api.git")
local util = require("custom_api.util")

-- git.lua reaches the shell through `util.run_shell_command`, looked up on the
-- table at call time, so a spec can answer for it. `replies` maps a command
-- string to the `{ exit_code, output }` the real runner would have returned,
-- already trimmed the way it trims. PR 7b replaces this with an injected
-- `git.runner`.
-- `#` is undefined on a table with an embedded nil, and `nil, "message"` is
-- exactly the shape under test, so the count comes from `select` instead.
local function collect(...)
  return select("#", ...), { ... }
end

local function with_shell(replies, fn)
  local real_runner = util.run_shell_command
  util.run_shell_command = function(opts)
    local reply = replies[opts.cmd] or error("unexpected shell command: " .. tostring(opts.cmd))
    return reply[1], reply[2]
  end
  local count, results = collect(pcall(fn))
  util.run_shell_command = real_runner
  assert(results[1], results[2])
  return unpack(results, 2, count)
end

local function latest_commit(replies)
  return with_shell(replies, function()
    return git.latest_commit({ repo_name = "dotfiles" })
  end)
end

return {
  ["latest_commit returns the commit as one table"] = function()
    local commit, err = latest_commit({
      ["git rev-parse --short HEAD"] = { 0, "abc1234" },
      ["git log -1 --pretty=%B"] = { 0, "the summary\n\nthe body" },
    })
    assert(err == nil, "reported " .. tostring(err))
    assert(type(commit) == "table", "latest_commit returned a " .. type(commit))
    assert(commit.hash == "abc1234", "hash was " .. tostring(commit.hash))
    assert(commit.summary == "the summary", "summary was " .. tostring(commit.summary))
    assert(commit.body == "the body", "body was " .. tostring(commit.body))
  end,

  ["latest_commit reads a one-line commit as a summary with no body"] = function()
    local commit = latest_commit({
      ["git rev-parse --short HEAD"] = { 0, "abc1234" },
      ["git log -1 --pretty=%B"] = { 0, "the summary" },
    })
    assert(commit.summary == "the summary", "summary was " .. tostring(commit.summary))
    assert(commit.body == nil, "body was " .. tostring(commit.body))
  end,

  ["latest_commit reports a repository with no commits as an operational failure"] = function()
    local commit, err = latest_commit({ ["git rev-parse --short HEAD"] = { 128, "" } })
    assert(commit == nil, "returned a commit anyway")
    assert(type(err) == "string", "err was a " .. type(err))
    assert(err:find("dotfiles", 1, true), "the message does not name the repository: " .. err)
  end,

  ["latest_commit still carries the hash of a commit with no message"] = function()
    local commit, err = latest_commit({
      ["git rev-parse --short HEAD"] = { 0, "abc1234" },
      ["git log -1 --pretty=%B"] = { 1, "" },
    })
    assert(type(commit) == "table", "latest_commit returned a " .. type(commit))
    assert(commit.hash == "abc1234", "hash was " .. tostring(commit.hash))
    assert(commit.summary == nil, "summary was " .. tostring(commit.summary))
    assert(err:find("abc1234", 1, true), "the message does not name the commit: " .. err)
  end,

  ["latest_commit names the argument it actually wants"] = function()
    -- It wanted `repo_name` and said `project` (item 20).
    local ok, err = pcall(git.latest_commit, {})
    assert(not ok, "accepted a call with no repo_name")
    assert(err:find("repo_name", 1, true), "the message does not name repo_name: " .. err)
  end,

  ["url names its missing repo_name too"] = function()
    -- url guards three arguments and only two were pinned, so a mutation of
    -- this one survived.
    local ok, err = pcall(git.url, { remote = "origin", account_name = "webdavis" })
    assert(not ok, "accepted a call with no repo_name")
    assert(err:find("repo_name", 1, true), "the message does not name repo_name: " .. err)
  end,

  ["url names the argument it actually wants"] = function()
    -- It wanted `account_name` and said `user` (item 20).
    local ok, err = pcall(git.url, { remote = "origin", repo_name = "dotfiles" })
    assert(not ok, "accepted a call with no account_name")
    assert(err:find("account_name", 1, true), "the message does not name account_name: " .. err)
  end,
}
