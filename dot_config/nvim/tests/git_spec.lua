-- custom_api.git (spec 6.2): latest_commit's (table, err) result and the error
-- texts that name their own parameters. Extended in PR 7a to 7c.

local git = require("custom_api.git")

-- git.lua reaches the shell through `git.runner` (spec 6.2), so a spec answers
-- for the shell by replacing that one field. Argv is the only form allowed
-- through the seam, so the fakes refuse shell text outright. `replies` maps the
-- argv words joined by spaces to the `{ exit_code, output }` the real runner
-- would have returned, already trimmed the way it trims.
-- `#` is undefined on a table with an embedded nil, and `nil, "message"` is
-- exactly the shape under test, so the count comes from `select` instead.
local function collect(...)
  return select("#", ...), { ... }
end

-- Every call's whole `opts` table, so a case can read the stdin the runner was
-- handed and not only its argv.
local seen = {}

local function with_shell(replies, fn)
  local real_runner = git.runner
  seen = {}
  git.runner = function(opts)
    assert(type(opts.cmd) == "table", "runner was handed shell text: " .. tostring(opts.cmd))
    table.insert(seen, opts)
    local command = table.concat(opts.cmd, " ")
    local reply = replies[command] or error("unexpected shell command: " .. command)
    return reply[1], reply[2]
  end
  local count, results = collect(pcall(fn))
  git.runner = real_runner
  assert(results[1], results[2])
  return unpack(results, 2, count)
end

-- `blame_sha` reads the text out of the CURRENT buffer, so a case that wants an
-- unsaved buffer has to make one current. A throwaway scratch buffer keeps the
-- name and the lines out of every other spec in the same nvim. `fn` is handed
-- the buffer's OWN name, because that is what the keymap passes as `file` and
-- because nvim rewrites the name it is given (`/var` becomes `/private/var` on
-- macOS), so the name asked for and the name stored are not the same string.
local function in_buffer(lines, fn)
  local previous = vim.api.nvim_get_current_buf()
  local bufnr = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_name(bufnr, vim.fn.tempname())
  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, lines)
  vim.api.nvim_set_current_buf(bufnr)
  local count, results = collect(pcall(fn, vim.api.nvim_buf_get_name(bufnr)))
  vim.api.nvim_set_current_buf(previous)
  vim.api.nvim_buf_delete(bufnr, { force = true })
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
    assert(type(opts.cmd) == "table", "runner was handed shell text: " .. tostring(opts.cmd))
    local command = table.concat(opts.cmd, " ")
    table.insert(seen, command)
    local reply = replies[command] or error("unexpected shell command: " .. command)
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

  ["url asks for the remote's URL with the remote name as one argv word"] = function()
    -- The remote name is interpolated into the command, so this is one of the
    -- two callers a shell must never see. `with_shell` refuses shell text, so a
    -- caller that went back to building a string fails here.
    local remote_url = with_shell({
      ["git config --get remote.upstream.url"] = { 0, "git@github.com:webdavis/dotfiles.git" },
    }, function()
      return git.url({ remote = "upstream", account_name = "webdavis", repo_name = "dotfiles" })
    end)

    assert(remote_url == "git@github.com:webdavis/dotfiles.git", "got " .. tostring(remote_url))
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

  -- ╭─────────────╮
  -- │ Blame (5.2) │
  -- ╰─────────────╯
  -- The first porcelain line is `<sha> <orig-line> <final-line> <count>`, so
  -- the SHA is the first token and only the first: a parse that read the second
  -- would hand back a line number that happens to look like a short hash.
  ["parse_blame_porcelain reads the SHA off the first porcelain line"] = function()
    local sha, err = git.parse_blame_porcelain(
      "581dae8e37117196fb31ce1658a1c55ec3128b19 1 1 1\nauthor Sentinel Person\nauthor-mail <sentinel@example.com>"
    )
    assert(err == nil, "reported " .. tostring(err))
    assert(sha == "581dae8e37117196fb31ce1658a1c55ec3128b19", "sha was " .. tostring(sha))
  end,

  -- git blame answers an uncommitted line with forty zeros and the author
  -- "Not Committed Yet". That is a soft error for the keymaps, not a SHA.
  ["parse_blame_porcelain reports an all-zero SHA as not committed yet"] = function()
    local sha, err = git.parse_blame_porcelain(
      "0000000000000000000000000000000000000000 305 305 1\nauthor Not Committed Yet\nauthor-mail <not.committed.yet>"
    )
    assert(sha == nil, "returned a sha for an uncommitted line: " .. tostring(sha))
    assert(type(err) == "string", "err was a " .. type(err))
    assert(err:lower():find("not committed", 1, true), "the message does not say the line is uncommitted: " .. err)
  end,

  ["blame_sha asks git for one porcelain line and returns its SHA"] = function()
    -- The line number and the path are interpolated into the command, so this
    -- is another caller a shell must never see, and `with_shell` refuses an
    -- argv that differs from this one by a single word.
    local sha, err = with_shell({
      ["git blame -L 7,7 --porcelain -- lua/plugins/git.lua"] = {
        0,
        "581dae8e37117196fb31ce1658a1c55ec3128b19 7 7 1\nauthor Sentinel Person",
      },
    }, function()
      return git.blame_sha({ file = "lua/plugins/git.lua", line = 7 })
    end)
    assert(err == nil, "reported " .. tostring(err))
    assert(sha == "581dae8e37117196fb31ce1658a1c55ec3128b19", "sha was " .. tostring(sha))
  end,

  -- Blaming the file on disk answers for a line the operator has already
  -- replaced: the old code returned the seed commit for a line that no longer
  -- exists. `--contents -` plus the buffer's own text is what makes git answer
  -- for what is on screen, and an unsaved line comes back as uncommitted.
  ["blame_sha blames the buffer's text rather than the saved file"] = function()
    local sha, err = in_buffer({ "unsaved one", "line two" }, function(file)
      return with_shell({
        [("git blame -L 1,1 --porcelain --contents - -- %s"):format(file)] = {
          0,
          "0000000000000000000000000000000000000000 1 1 1\nauthor Not Committed Yet",
        },
      }, function()
        return git.blame_sha({ file = file, line = 1 })
      end)
    end)
    assert(sha == nil, "returned a sha for an unsaved line: " .. tostring(sha))
    assert(type(err) == "string" and err:lower():find("not committed", 1, true), "err was " .. tostring(err))
    assert(#seen == 1, "asked the shell " .. #seen .. " times, not once")
    assert(seen[1].stdin == "unsaved one\nline two\n", "stdin was " .. tostring(seen[1].stdin))
  end,

  ["blame_sha reports a failed git call as an operational failure"] = function()
    local sha, err = with_shell({
      ["git blame -L 7,7 --porcelain -- outside.txt"] = { 128, "fatal: no such path 'outside.txt' in HEAD" },
    }, function()
      return git.blame_sha({ file = "outside.txt", line = 7 })
    end)
    assert(sha == nil, "returned a sha anyway: " .. tostring(sha))
    assert(type(err) == "string", "err was a " .. type(err))
    assert(err:find("outside.txt", 1, true), "the message does not name the path: " .. err)
  end,
}
