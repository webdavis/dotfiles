-- custom_api.git (spec 6.2): latest_commit's (table, err) result and the error
-- texts that name their own parameters. Extended in PR 7a to 7c.

local git = require("custom_api.git")

-- git.lua reaches the shell through `git.runner` (spec 6.2), so a spec answers
-- for the shell by replacing that one field. `replies` maps a command string to
-- the `{ exit_code, output }` the real runner would have returned, already
-- trimmed the way it trims.
-- `#` is undefined on a table with an embedded nil, and `nil, "message"` is
-- exactly the shape under test, so the count comes from `select` instead.
local function collect(...)
  return select("#", ...), { ... }
end

local function with_shell(replies, fn)
  local real_runner = git.runner
  git.runner = function(opts)
    local reply = replies[opts.cmd] or error("unexpected shell command: " .. tostring(opts.cmd))
    return reply[1], reply[2]
  end
  local count, results = collect(pcall(fn))
  git.runner = real_runner
  assert(results[1], results[2])
  return unpack(results, 2, count)
end

local function latest_commit(replies)
  return with_shell(replies, function()
    return git.latest_commit({ repo_name = "dotfiles" })
  end)
end

-- `with_shell` errors on an unlisted command, which catches a call that asked
-- the shell for the wrong thing. It cannot catch a call that skipped `runner`
-- altogether, and this repository HAS an `origin/main`, so a real shell would
-- answer "main" and a bypassed test would still pass. These cases therefore
-- also assert WHICH commands the runner was handed, from `seen`.
local function default_branch(replies)
  local seen = {}
  local real_runner = git.runner
  git.runner = function(opts)
    table.insert(seen, opts.cmd)
    local reply = replies[opts.cmd] or error("unexpected shell command: " .. tostring(opts.cmd))
    return reply[1], reply[2]
  end
  local count, results = collect(pcall(git.default_branch))
  git.runner = real_runner
  assert(results[1], results[2])
  local name, err = unpack(results, 2, count)
  return { seen = seen, name = name, err = err }
end

local MAIN_REF = "git show-ref --verify --quiet refs/remotes/origin/main"
local MASTER_REF = "git show-ref --verify --quiet refs/remotes/origin/master"

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

  -- Bug #4. `default_branch` read `opts.repo` and the caller handed it a
  -- string, so the read silently found nil; called with nothing at all it
  -- raised on the index. It takes no argument now.
  ["default_branch names main when origin/main is present"] = function()
    local run = default_branch({ [MAIN_REF] = { 0, "" } })
    assert(run.name == "main", "name was " .. tostring(run.name))
    assert(run.err == nil, "reported " .. tostring(run.err))
    assert(#run.seen == 1, "asked the shell " .. #run.seen .. " times, not once")
    assert(run.seen[1] == MAIN_REF, "asked for " .. tostring(run.seen[1]))
  end,

  ["default_branch falls back to master when only origin/master is present"] = function()
    local run = default_branch({ [MAIN_REF] = { 1, "" }, [MASTER_REF] = { 0, "" } })
    assert(run.name == "master", "name was " .. tostring(run.name))
    assert(run.err == nil, "reported " .. tostring(run.err))
    assert(#run.seen == 2, "asked the shell " .. #run.seen .. " times, not twice")
    assert(run.seen[2] == MASTER_REF, "second command was " .. tostring(run.seen[2]))
  end,

  -- The GitHub fallback lives in `github.default_branch` now (item 56), so
  -- there is nothing left here to reach for when both refs are missing.
  ["default_branch reports no default branch when neither ref is present"] = function()
    local run = default_branch({ [MAIN_REF] = { 1, "" }, [MASTER_REF] = { 1, "" } })
    assert(run.name == nil, "returned " .. tostring(run.name))
    assert(run.err == "no default branch", "err was " .. tostring(run.err))
  end,

  ["url names its missing repo_name too"] = function()
    -- url guards three arguments and only two were pinned, so a mutation of
    -- this one survived.
    local ok, err = pcall(git.url, { remote = "origin", account_name = "webdavis" })
    assert(not ok, "accepted a call with no repo_name")
    assert(err:find("repo_name", 1, true), "the message does not name repo_name: " .. err)
  end,

  ["copy_URL_to_clipboard is spelled copy_url_to_clipboard"] = function()
    -- Decision E: the acronym is lowercased like every other name in this API.
    assert(git.copy_URL_to_clipboard == nil, "the old spelling is still " .. type(git.copy_URL_to_clipboard))
    assert(type(git.copy_url_to_clipboard) == "function", "copy_url_to_clipboard is missing")
  end,

  ["url names the argument it actually wants"] = function()
    -- It wanted `account_name` and said `user` (item 20).
    local ok, err = pcall(git.url, { remote = "origin", repo_name = "dotfiles" })
    assert(not ok, "accepted a call with no account_name")
    assert(err:find("account_name", 1, true), "the message does not name account_name: " .. err)
  end,
}
