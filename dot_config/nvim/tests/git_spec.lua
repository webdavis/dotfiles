local git = require("custom_api.git")

-- Clear the git hook variables for the whole process, so fixtures and the blame under test stay in the throwaway repository.
for name in pairs(vim.fn.environ()) do
  if name:match("^GIT_") then
    vim.env[name] = nil
  end
end

-- Fakes accept argv only. `replies` maps the joined argv to `{ exit_code, output }`.
-- `select` counts the values, because `#` is undefined with an embedded nil.
local function collect(...)
  return select("#", ...), { ... }
end

-- Every call's whole `opts`, so a case can read the stdin it was handed.
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

-- A scratch buffer made current, because `blame_sha` reads the current buffer. `fn` gets the name nvim stored.
local function in_buffer(lines, fn, name)
  local previous = vim.api.nvim_get_current_buf()
  local bufnr = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_name(bufnr, name or vim.fn.tempname())
  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, lines)
  vim.api.nvim_set_current_buf(bufnr)
  local count, results = collect(pcall(fn, vim.api.nvim_buf_get_name(bufnr)))
  vim.api.nvim_set_current_buf(previous)
  vim.api.nvim_buf_delete(bufnr, { force = true })
  assert(results[1], results[2])
  return unpack(results, 2, count)
end

-- A real repository with one committed file of exact bytes, so git itself confirms the bytes sent match the file.
local function in_committed_file(bytes, modifiers, fn)
  local dir = vim.fn.tempname()
  vim.fn.mkdir(dir, "p")

  -- Read no global or system config, so fixture commits skip the owner's hooks and signing.
  local no_user_config = { GIT_CONFIG_GLOBAL = "/dev/null", GIT_CONFIG_SYSTEM = "/dev/null" }

  local function git_command(...)
    local argv = { "git", ... }
    local result = vim.system(argv, { text = true, cwd = dir, env = no_user_config }):wait()
    assert(result.code == 0, table.concat(argv, " ") .. ": " .. (result.stderr or ""))
  end

  git_command("init", "-q")
  -- The blame reads the global config, so this repository setting keeps `core.autocrlf` from hiding CRs.
  git_command("config", "core.autocrlf", "false")
  -- Keep the blame from starting an fsmonitor daemon that outlives the repository.
  git_command("config", "core.fsmonitor", "false")
  git_command("config", "user.name", "Spec Fixture")
  git_command("config", "user.email", "spec@example.invalid")

  local path = dir .. "/fixture.txt"
  local handle = assert(io.open(path, "wb"))
  handle:write(bytes)
  handle:close()
  git_command("add", "fixture.txt")
  git_command("commit", "-q", "-m", "fixture")

  local previous = vim.api.nvim_get_current_buf()
  vim.cmd(("edit %s %s"):format(modifiers, vim.fn.fnameescape(path)))
  local bufnr = vim.api.nvim_get_current_buf()
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

-- Assert which commands the runner got, because this repository has an `origin/main` a bypassed call would find.
local function default_branch(replies)
  local commands = {}
  local real_runner = git.runner
  git.runner = function(opts)
    assert(type(opts.cmd) == "table", "runner was handed shell text: " .. tostring(opts.cmd))
    local command = table.concat(opts.cmd, " ")
    table.insert(commands, command)
    local reply = replies[command] or error("unexpected shell command: " .. command)
    return reply[1], reply[2]
  end
  local count, results = collect(pcall(git.default_branch))
  git.runner = real_runner
  assert(results[1], results[2])
  local name, err = unpack(results, 2, count)
  return { seen = commands, name = name, err = err }
end

local MAIN_REF = "git show-ref --verify --quiet refs/remotes/origin/main"
local MASTER_REF = "git show-ref --verify --quiet refs/remotes/origin/master"

return {
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
    local ok, err = pcall(git.latest_commit, {})
    assert(not ok, "accepted a call with no repo_name")
    assert(err:find("repo_name", 1, true), "the message does not name repo_name: " .. err)
  end,

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

  ["default_branch reports no default branch when neither ref is present"] = function()
    local run = default_branch({ [MAIN_REF] = { 1, "" }, [MASTER_REF] = { 1, "" } })
    assert(run.name == nil, "returned " .. tostring(run.name))
    assert(run.err == "no default branch", "err was " .. tostring(run.err))
  end,

  ["url asks for the remote's URL with the remote name as one argv word"] = function()
    -- The remote name is interpolated, so the call must use argv.
    local remote_url = with_shell({
      ["git config --get remote.upstream.url"] = { 0, "git@github.com:webdavis/dotfiles.git" },
    }, function()
      return git.url({ remote = "upstream", account_name = "webdavis", repo_name = "dotfiles" })
    end)

    assert(remote_url == "git@github.com:webdavis/dotfiles.git", "got " .. tostring(remote_url))
  end,

  ["url names its missing repo_name too"] = function()
    local ok, err = pcall(git.url, { remote = "origin", account_name = "webdavis" })
    assert(not ok, "accepted a call with no repo_name")
    assert(err:find("repo_name", 1, true), "the message does not name repo_name: " .. err)
  end,

  ["copy_URL_to_clipboard is spelled copy_url_to_clipboard"] = function()
    assert(git.copy_URL_to_clipboard == nil, "the old spelling is still " .. type(git.copy_URL_to_clipboard))
    assert(type(git.copy_url_to_clipboard) == "function", "copy_url_to_clipboard is missing")
  end,

  ["url names the argument it actually wants"] = function()
    local ok, err = pcall(git.url, { remote = "origin", repo_name = "dotfiles" })
    assert(not ok, "accepted a call with no account_name")
    assert(err:find("account_name", 1, true), "the message does not name account_name: " .. err)
  end,

  -- Only the first token is the SHA. The second is a line number that can look like a short hash.
  ["parse_blame_porcelain reads the SHA off the first porcelain line"] = function()
    local sha, err = git.parse_blame_porcelain(
      "581dae8e37117196fb31ce1658a1c55ec3128b19 1 1 1\nauthor Sentinel Person\nauthor-mail <sentinel@example.com>"
    )
    assert(err == nil, "reported " .. tostring(err))
    assert(sha == "581dae8e37117196fb31ce1658a1c55ec3128b19", "sha was " .. tostring(sha))
  end,

  -- An uncommitted line gets forty zeros, a soft error for the keymaps.
  ["parse_blame_porcelain reports an all-zero SHA as not committed yet"] = function()
    local sha, err = git.parse_blame_porcelain(
      "0000000000000000000000000000000000000000 305 305 1\nauthor Not Committed Yet\nauthor-mail <not.committed.yet>"
    )
    assert(sha == nil, "returned a sha for an uncommitted line: " .. tostring(sha))
    assert(type(err) == "string", "err was a " .. type(err))
    assert(err:lower():find("not committed", 1, true), "the message does not say the line is uncommitted: " .. err)
  end,

  ["parse_blame_porcelain reports a SHA-256 repository's zeros as not committed yet"] = function()
    local sha, err = git.parse_blame_porcelain(
      ("0"):rep(64) .. " 305 305 1\nauthor Not Committed Yet\nauthor-mail <not.committed.yet>"
    )
    assert(sha == nil, "returned a sha for an uncommitted line: " .. tostring(sha))
    assert(type(err) == "string", "err was a " .. type(err))
    assert(err:lower():find("not committed", 1, true), "the message does not say the line is uncommitted: " .. err)
  end,

  ["blame_sha asks git for one porcelain line and returns its SHA"] = function()
    -- The line and path are interpolated, so the call must use argv.
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

  -- `latest_commit` has no file to derive a directory from, so the caller passes one.
  ["latest_commit reads HEAD in the directory it was given"] = function()
    local commit = with_shell({
      ["git rev-parse --short HEAD"] = { 0, "581dae8" },
      ["git log -1 --pretty=%B"] = { 0, "add the thing\n\nbody" },
    }, function()
      return git.latest_commit({ repo_name = "other-repo", cwd = "/tmp/other-repo" })
    end)
    assert(commit.hash == "581dae8", "hash was " .. tostring(commit.hash))
    assert(#seen == 2, "asked the shell " .. #seen .. " times, not twice")
    for _, call in ipairs(seen) do
      assert(call.cwd == "/tmp/other-repo", table.concat(call.cmd, " ") .. " ran in " .. tostring(call.cwd))
    end
  end,

  -- `--contents -` makes git answer for what is on screen.
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

  -- Rejoining a dos buffer with LF would differ from the committed blob.
  ["blame_sha sends a dos buffer with the carriage returns its file has"] = function()
    local sha = in_buffer({ "crlf one", "crlf two" }, function(file)
      vim.bo.fileformat = "dos"
      return with_shell({
        [("git blame -L 1,1 --porcelain --contents - -- %s"):format(file)] = {
          0,
          "581dae8e37117196fb31ce1658a1c55ec3128b19 1 1 1\nauthor Sentinel Person",
        },
      }, function()
        return git.blame_sha({ file = file, line = 1 })
      end)
    end)
    assert(sha == "581dae8e37117196fb31ce1658a1c55ec3128b19", "sha was " .. tostring(sha))
    assert(seen[1].stdin == "crlf one\r\ncrlf two\r\n", "stdin was " .. string.format("%q", tostring(seen[1].stdin)))
  end,

  ["blame_sha sends a buffer with no final newline without adding one"] = function()
    local sha = in_buffer({ "nonl one", "nonl two" }, function(file)
      vim.bo.endofline = false
      return with_shell({
        [("git blame -L 1,1 --porcelain --contents - -- %s"):format(file)] = {
          0,
          "581dae8e37117196fb31ce1658a1c55ec3128b19 1 1 1\nauthor Sentinel Person",
        },
      }, function()
        return git.blame_sha({ file = file, line = 1 })
      end)
    end)
    assert(sha == "581dae8e37117196fb31ce1658a1c55ec3128b19", "sha was " .. tostring(sha))
    assert(seen[1].stdin == "nonl one\nnonl two", "stdin was " .. string.format("%q", tostring(seen[1].stdin)))
  end,

  -- In a mac buffer, nvim shows an LF inside a line as a CR.
  ["blame_sha restores the line breaks a mac buffer hides inside its lines"] = function()
    local sha, err = in_committed_file("one\ninside\rtwo\r", "++ff=mac", function(file)
      return git.blame_sha({ file = file, line = 1 })
    end)
    assert(sha, "row 1 of an unchanged file answered: " .. tostring(err))
  end,

  ["blame_sha sends a binary buffer LF whatever its fileformat says"] = function()
    local answers = in_committed_file("one\ntwo\n", "++bin ++ff=dos", function(file)
      local rows = {}
      for row = 1, 2 do
        local sha, err = git.blame_sha({ file = file, line = row })
        rows[row] = sha or ("no sha: " .. tostring(err))
      end
      return rows
    end)
    for row = 1, 2 do
      assert(answers[row]:match("^%x+$"), "row " .. row .. " of an unchanged file answered: " .. answers[row])
    end
  end,

  -- The link blob holds a path, not the text nvim shows.
  ["blame_sha blames the file a symlink points at, not the link"] = function()
    local dir = vim.fn.tempname()
    vim.fn.mkdir(dir, "p")
    local target = dir .. "/target.txt"
    local link = dir .. "/link.txt"
    vim.fn.writefile({ "line one" }, target)
    vim.uv.fs_symlink(target, link)
    -- macOS rewrites `/var` to `/private/var`, so ask the resolver.
    local resolved = vim.uv.fs_realpath(target)

    local sha = in_buffer({ "line one" }, function(file)
      assert(file ~= resolved, "the buffer is not named after the link: " .. file)
      return with_shell({
        [("git blame -L 1,1 --porcelain --contents - -- %s"):format(resolved)] = {
          0,
          "581dae8e37117196fb31ce1658a1c55ec3128b19 1 1 1\nauthor Sentinel Person",
        },
      }, function()
        return git.blame_sha({ file = file, line = 1 })
      end)
    end, link)

    assert(sha == "581dae8e37117196fb31ce1658a1c55ec3128b19", "sha was " .. tostring(sha))
    local dir_of_target = vim.fn.fnamemodify(resolved, ":h")
    assert(seen[1].cwd == dir_of_target, "ran in " .. tostring(seen[1].cwd) .. ", not " .. dir_of_target)
  end,

  -- Git answers for the directory it runs in.
  ["blame_sha runs git beside the buffer's file rather than in nvim's cwd"] = function()
    local dir
    local sha, err = in_buffer({ "line one" }, function(file)
      dir = vim.fn.fnamemodify(file, ":h")
      return with_shell({
        [("git blame -L 1,1 --porcelain --contents - -- %s"):format(file)] = {
          0,
          "581dae8e37117196fb31ce1658a1c55ec3128b19 1 1 1\nauthor Sentinel Person",
        },
      }, function()
        return git.blame_sha({ file = file, line = 1 })
      end)
    end)
    assert(err == nil, "reported " .. tostring(err))
    assert(sha == "581dae8e37117196fb31ce1658a1c55ec3128b19", "sha was " .. tostring(sha))
    assert(#seen == 1, "asked the shell " .. #seen .. " times, not once")
    assert(seen[1].cwd == dir, "ran in " .. tostring(seen[1].cwd) .. ", not " .. tostring(dir))
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
