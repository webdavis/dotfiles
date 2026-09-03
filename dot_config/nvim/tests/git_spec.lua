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
  -- ╭───────────────╮
  -- │ Pure helpers  │
  -- ╰───────────────╯
  ["convert_remote_protocol rewrites an ssh remote to https"] = function()
    local url =
      git.convert_remote_protocol("git@github.com:webdavis/dotfiles.git", "git@github.com:", "https://github.com/")
    assert(url == "https://github.com/webdavis/dotfiles.git", "rewrote to " .. tostring(url))
  end,

  ["convert_remote_protocol leaves a remote already in the target protocol alone"] = function()
    local url =
      git.convert_remote_protocol("https://github.com/webdavis/dotfiles.git", "git@github.com:", "https://github.com/")
    assert(url == "https://github.com/webdavis/dotfiles.git", "returned " .. tostring(url))
  end,

  ["convert_remote_protocol returns nil for a remote in neither protocol"] = function()
    local url =
      git.convert_remote_protocol("git@gitlab.com:webdavis/dotfiles.git", "git@github.com:", "https://github.com/")
    assert(url == nil, "returned " .. tostring(url))
  end,

  ["normalize_branch strips the current and previous branch markers"] = function()
    assert(git.normalize_branch("* main abc1234") == "main abc1234", "star not stripped")
    assert(git.normalize_branch("+ topic def5678") == "topic def5678", "plus not stripped")
    assert(git.normalize_branch("  topic def5678") == "topic def5678", "not trimmed")
  end,

  ["is_current_branch reads the leading star and nothing else"] = function()
    assert(git.is_current_branch("* main abc1234"), "the checked-out branch was not current")
    assert(not git.is_current_branch("  main abc1234"), "a plain branch reported current")
    assert(not git.is_current_branch("+ main abc1234"), "a worktree branch reported current")
  end,

  -- Bug #8. The loop in extract_upstream already steps past the closing "]",
  -- so `i` is the first message token; returning `i + 1` skipped it and ate
  -- the first word of every commit message on a branch with an upstream.
  ["extract_upstream points at the first message token"] = function()
    local tokens = { "main", "abc1234", "[origin/main]", "the", "first", "word" }
    local upstream, index = git.extract_upstream(tokens)
    assert(upstream == "[origin/main]", "upstream was " .. tostring(upstream))
    assert(index == 4, "index was " .. tostring(index) .. ", so tokens[index] is " .. tostring(tokens[index]))
  end,

  ["extract_upstream spans a multi-token upstream"] = function()
    local tokens = { "main", "abc1234", "[origin/main:", "ahead", "1]", "the", "message" }
    local upstream, index = git.extract_upstream(tokens)
    assert(upstream == "[origin/main: ahead 1]", "upstream was " .. tostring(upstream))
    assert(index == 6, "index was " .. tostring(index) .. ", so tokens[index] is " .. tostring(tokens[index]))
  end,

  ["extract_upstream reports no upstream without consuming a token"] = function()
    local tokens = { "main", "abc1234", "the", "message" }
    local upstream, index = git.extract_upstream(tokens)
    assert(upstream == nil, "upstream was " .. tostring(upstream))
    assert(index == 3, "index was " .. tostring(index))
  end,

  ["parse_branch_line keeps every word of a message on a branch with an upstream"] = function()
    local branch = git.parse_branch_line("* main abc1234 [origin/main] the first word matters")
    assert(branch.status == "active", "status was " .. tostring(branch.status))
    assert(branch.name == "main", "name was " .. tostring(branch.name))
    assert(branch.hash == "abc1234", "hash was " .. tostring(branch.hash))
    assert(branch.upstream == "[origin/main]", "upstream was " .. tostring(branch.upstream))
    assert(branch.message == "the first word matters", "the message lost a word: " .. tostring(branch.message))
  end,

  ["parse_branch_line reads a branch with no upstream"] = function()
    local branch = git.parse_branch_line("  topic def5678 second line here")
    assert(branch.status == "inactive", "status was " .. tostring(branch.status))
    assert(branch.name == "topic", "name was " .. tostring(branch.name))
    assert(branch.upstream == nil, "upstream was " .. tostring(branch.upstream))
    assert(branch.message == "second line here", "message was " .. tostring(branch.message))
  end,

  ["parse_branch_line marks the previous branch"] = function()
    local branch = git.parse_branch_line("+ topic def5678 [origin/topic] a message")
    assert(branch.status == "previous", "status was " .. tostring(branch.status))
    assert(branch.message == "a message", "message was " .. tostring(branch.message))
  end,

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
