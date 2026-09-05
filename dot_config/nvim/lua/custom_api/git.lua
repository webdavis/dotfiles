local M = {}

local util = require("custom_api.util")

-- The shell seam (spec 6.2). Every shell call in this module goes through
-- `M.runner`, so a test replaces this one field instead of reaching into
-- `util`, and nothing here shells out behind the seam's back.
M.runner = util.run_shell_command

-- ╭──────────╮
-- │  Helpers │
-- ╰──────────╯
local function convert_remote_protocol(remote_url, from_prefix, to_prefix)
  local user_repo = remote_url:match("^" .. from_prefix .. "(.+)")

  if user_repo then
    return to_prefix .. user_repo
  elseif remote_url:match("^" .. to_prefix) then
    return remote_url
  end

  return nil
end

local function to_https_protocol(remote_url)
  return convert_remote_protocol(remote_url, "git@github.com:", "https://github.com/")
end

local function to_ssh_protocol(remote_url)
  return convert_remote_protocol(remote_url, "https://github.com/", "git@github.com:")
end

-- ╭──────╮
-- │  API │
-- ╰──────╯
local function initialized(opts)
  local _ = opts
  local code, _ = M.runner({ cmd = { "git", "rev-parse", "--git-dir" } })

  if code ~= 0 then
    return nil, "Project hasn't been initialized. Run `git init` to start tracking."
  end

  return true
end

local function top_level(opts)
  local _ = opts

  local code, top_level_dir = M.runner({ cmd = { "git", "rev-parse", "--show-toplevel" }, notify_error = true })

  if code ~= 0 then
    return nil
  end

  if not opts.full_path or opts.full_path == false then
    top_level_dir = vim.fn.fnamemodify(top_level_dir, ":t")
  end

  return top_level_dir
end

local function normalize_branch(branch)
  return util.trim(branch:gsub("^[*+]%s+", ""))
end

local function is_current_branch(line)
  return line:match("^%*") == "*"
end

local function fetch_branches()
  local exit_code, branches_output = M.runner({ cmd = { "git", "branch", "-vv" } })

  if exit_code ~= 0 then
    local message = {
      { 'Project may not be initialized or is in a detache "HEAD" state.' },
      { "Current working directory: `" .. util.get_cwd_basename() .. "`" },
    }
    return nil, table.concat(message, "\n")
  end

  local _, current_name = M.runner({
    cmd = { "git", "branch", "--show-current" },
    notify_error = true,
  })

  return branches_output, current_name
end

local function extract_upstream(tokens)
  -- Upstream branch specification always starts at index 3, if it exists at all.
  local i = 3

  if not tokens[i] or tokens[i]:sub(1, 1) ~= "[" then
    return nil, i
  end

  local parts = {}

  while tokens[i] do
    if tokens[i]:sub(-1) == "]" then
      table.insert(parts, tokens[i])
      i = i + 1
      break
    end
    table.insert(parts, tokens[i])
    i = i + 1
  end

  local upstream = table.concat(parts, " ")

  return upstream, i
end

local function parse_branch_line(line)
  local normalized_line = normalize_branch(line)

  local tokens = {}
  for token in normalized_line:gmatch("%S+") do
    table.insert(tokens, token)
  end

  local name = tokens[1]
  local hash = tokens[2]

  local upstream, message_start_index = extract_upstream(tokens)
  message_start_index = message_start_index or 3

  local message = ""
  for i = message_start_index, #tokens do
    message = message .. tokens[i] .. (i < #tokens and " " or "")
  end
  message = message == "" and nil or message

  local indicator = line:sub(1, 1)
  local status
  if indicator == "*" then
    status = "active"
  elseif indicator == "+" then
    status = "previous"
  else
    status = "inactive"
  end

  return {
    status = status,
    name = name,
    hash = hash,
    upstream = upstream,
    message = message,
  }
end

local function empty_repo_branch(name)
  return { name = name, hash = nil, upstream = nil, message = nil }
end

local function with_branch_list_helper(opts)
  opts = opts or {}
  local current_only = opts.current

  local branches_output, current_name_or_err_msg = fetch_branches()
  if not branches_output then
    return nil, current_name_or_err_msg
  end

  -- Handle empty repo or detached HEAD.
  if branches_output == "" and current_name_or_err_msg ~= "" then
    local branch = empty_repo_branch(current_name_or_err_msg)
    return current_only and branch or { branch }
  end

  local branch_list = {}

  for line in branches_output:gmatch("[^\r\n]+") do
    local is_current = is_current_branch(line)

    local branch = parse_branch_line(line)

    if current_only and is_current then
      return branch
    elseif not current_only then
      if is_current then
        table.insert(branch_list, 1, branch)
      else
        table.insert(branch_list, branch)
      end
    end
  end

  return branch_list
end

local function all_branches()
  return with_branch_list_helper()
end

local function current_branch()
  return with_branch_list_helper({ current = true })
end

-- `cwd` is the directory git runs in, and defaults to Neovim's own, which is
-- only the right repository when the caller happens to be sitting in it.
-- `blame_sha` derives its own from the file it blames; this function has no
-- file to derive one from, so the caller states it. `repo_name` does NOT do
-- this job: it labels this function's error messages and selects nothing.
local function latest_commit(opts)
  opts = opts or {}
  local repo_name = opts.repo_name
  local cwd = opts.cwd

  if not repo_name then
    error("Missing required argument `repo_name`")
  end

  local hash_exit, hash = M.runner({ cmd = { "git", "rev-parse", "--short", "HEAD" }, cwd = cwd })
  if hash_exit ~= 0 then
    return nil,
      string.format(
        "Unable to find latest commit.\n\nThis may occur if no commits have been made to *%s* yet.",
        repo_name
      )
  end

  local message_exit, message = M.runner({ cmd = { "git", "log", "-1", "--pretty=%B" }, cwd = cwd })
  if message_exit ~= 0 then
    return { hash = hash }, string.format("Commit `%s` has no message.", hash)
  end

  local summary, body = message:match("([^\n]*)\n?(.*)")

  return { hash = hash, summary = util.normalize(summary), body = util.normalize(body) }
end

-- Local only (spec 6.2): the remote-tracking refs this checkout already has,
-- and nothing else. Asking GitHub is `github.default_branch`, which the keymap
-- layer falls through to; keeping the network call here is what gave this
-- function a repository argument it then read off a string (item 4).
local function default_branch()
  local main_ok = M.runner({ cmd = { "git", "show-ref", "--verify", "--quiet", "refs/remotes/origin/main" } })
  if main_ok == 0 then
    return "main"
  end

  local master_ok = M.runner({ cmd = { "git", "show-ref", "--verify", "--quiet", "refs/remotes/origin/master" } })
  if master_ok == 0 then
    return "master"
  end

  return nil, "no default branch"
end

local function url(opts)
  opts = opts or {}
  local remote = opts.remote
  local account_name = opts.account_name
  local repo_name = opts.repo_name

  if not remote then
    error("Missing required argument `remote`")
  end
  if not account_name then
    error("Missing required argument `account_name`")
  end
  if not repo_name then
    error("Missing required argument `repo_name`")
  end

  local code, remote_url = M.runner({ cmd = { "git", "config", "--get", "remote." .. remote .. ".url" } })

  if code ~= 0 then
    local lines = {
      ("Couldn't find URL for `%s` remote!"):format(remote),
      ("To set the remote URL for '%s', run: `Git remote set-url %s git@github.com/%s/%s.git`"):format(
        remote,
        remote,
        account_name,
        repo_name
      ),
    }
    return nil, table.concat(lines, "\n\n")
  end

  return remote_url
end

local function copy_url_to_clipboard(opts)
  opts = opts or {}
  local remote = opts.remote
  local protocol = opts.protocol
  local remote_url = opts.url

  if not remote then
    error("Missing required argument `remote`")
  end
  if not protocol then
    error("Missing required argument `protocol`")
  end
  if not remote_url then
    error("Missing required argument `url`")
  end

  local converted_URL
  if protocol == "https" then
    converted_URL = to_https_protocol(remote_url)
  else
    converted_URL = to_ssh_protocol(remote_url)
  end

  local final_URL = converted_URL or remote_url

  util.copy_to_system_clipboard(final_URL)

  if not converted_URL then
    return nil,
      table.concat({
        ("Warning: Couldn't convert '%s' remote to %s: unrecognized protocol!"):format(remote, protocol:upper()),
        ("Copied original URL to clipboard instead: `%s`"):format(final_URL),
      }, "\n")
  end

  return ("Copied *%s* %s URL to clipboard: `%s`"):format(remote, protocol:upper(), final_URL)
end

-- The first porcelain line is `<sha> <orig-line> <final-line> <count>`. Only the
-- SHA is read; `%s` after the token is what keeps a `fatal:` from matching as
-- hex on its leading letters.
local function parse_blame_porcelain(text)
  local sha = (text or ""):match("^(%x+)%s")

  if not sha then
    return nil, "no blame line to read"
  end

  -- What git blame prints as the SHA of a line that is not committed yet is a
  -- SHA of zeros: forty of them in a SHA-1 repository and sixty-four in a
  -- SHA-256 one, so the digits are what identify it and never their count.
  if sha:match("^0+$") then
    return nil, "not committed yet"
  end

  return sha
end

-- What each `fileformat` puts between lines when the buffer is written out.
local LINE_SEPARATOR = { unix = "\n", dos = "\r\n", mac = "\r" }

-- The text the editor is showing for `file`. Blaming the saved copy answers for
-- a line the operator may have already replaced, so what the buffer holds is
-- what goes to git; git reports a line that is only in the buffer as
-- uncommitted, which is the true answer. The keymaps blame the current buffer,
-- so a name that does not match it means there is nothing unsaved to send.
--
-- These have to be the bytes the file itself would hold, not the lines glued
-- with LF and a newline stapled on: a `fileformat=dos` buffer rejoined with LF,
-- or a file with no trailing newline given one, differs from its own committed
-- blob, so every line of an UNCHANGED file blamed as uncommitted.
local function buffer_contents(file)
  if vim.api.nvim_buf_get_name(0) ~= file then
    return nil
  end

  -- `binary` writes the buffer out with LF between the lines whatever
  -- `fileformat` says, so it settles the separator before the format is read.
  local separator = vim.bo.binary and "\n" or (LINE_SEPARATOR[vim.bo.fileformat] or "\n")
  local lines = vim.api.nvim_buf_get_lines(0, 0, -1, false)

  -- A `fileformat=mac` buffer is split on carriage returns, and nvim shows an LF
  -- byte sitting INSIDE one of those lines as a carriage return as well. Joining
  -- with a carriage return would write the file's own LF back out as a line
  -- break, so the two are told apart here: a carriage return still inside a line
  -- is the LF it was read from.
  if separator == "\r" then
    lines = vim.tbl_map(function(line)
      return (line:gsub("\r", "\n"))
    end, lines)
  end

  local text = table.concat(lines, separator)

  return vim.bo.endofline and text .. separator or text
end

local function blame_sha(opts)
  opts = opts or {}
  local file = opts.file
  local line = opts.line

  if not file then
    error("Missing required argument `file`")
  end
  if not line then
    error("Missing required argument `line`")
  end

  local range = ("%d,%d"):format(line, line)
  local contents = buffer_contents(file)

  -- A symlink gives nvim the TARGET's text under the LINK's name, and the link's
  -- own blob holds a path rather than that text, so blaming the link compared
  -- the two and called every line uncommitted. The link's name is what the
  -- buffer is called and stays the key for matching it; git gets the real file.
  local blamed = vim.uv.fs_realpath(file) or file

  local cmd = { "git", "blame", "-L", range, "--porcelain" }
  if contents then
    vim.list_extend(cmd, { "--contents", "-" })
  end
  vim.list_extend(cmd, { "--", blamed })

  local code, output = M.runner({ cmd = cmd, stdin = contents, cwd = util.file_dir(blamed) })

  if code ~= 0 then
    return nil, output
  end

  return parse_blame_porcelain(output)
end

-- The pure helpers are exported so the spec can call them directly (spec 6.3);
-- they take strings and tables and reach nothing outside this file.
M.convert_remote_protocol = convert_remote_protocol
M.parse_blame_porcelain = parse_blame_porcelain
M.normalize_branch = normalize_branch
M.is_current_branch = is_current_branch
M.extract_upstream = extract_upstream
M.parse_branch_line = parse_branch_line

M.initialized = initialized
M.top_level = top_level
M.current_branch = current_branch
M.all_branches = all_branches
M.default_branch = default_branch
M.latest_commit = latest_commit
M.copy_url_to_clipboard = copy_url_to_clipboard
M.url = url
M.blame_sha = blame_sha

return M
