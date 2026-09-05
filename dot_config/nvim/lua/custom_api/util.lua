---@class custom_api.util
---My custom Neovim API.
local M = {}

-- ╭──────╮
-- │  API │
-- ╰──────╯

---Remove surrounding whitespace from `s`.
---
---The outer parentheses are load-bearing: they drop `gsub`'s substitution
---count, which every caller in tail position would otherwise return as a
---second value of its own.
function M.trim(s)
  return ((s or ""):gsub("^%s*(.-)%s*$", "%1"))
end

---Remove surrounding whitespace and convert `s` to lowercase.
function M.sanitize_input(s)
  return M.trim(s):lower()
end

function M.get_cwd_basename()
  return vim.fn.fnamemodify(vim.fn.getcwd(), ":t")
end

function M.copy_to_system_clipboard(data)
  vim.fn.setreg("+", data)
end

function M.normalize(message)
  message = M.trim(message)
  return (message ~= "" and message) or nil
end

---The directory holding `path`, for a command's `cwd`.
---
---Git and `gh` both answer for the repository of the directory they run in, not
---for the repository of the path they are handed, so a command about a file has
---to run beside that file. With nvim's own cwd outside the repository, `git
---blame` on an absolute path inside it reports `fatal: not a git repository`
---and `gh repo view` answers for whatever repository nvim was started in.
---
---Symlinks are resolved first, because git answers for the real file: a link
---and its target can sit in two different checkouts.
---
---Answers nil for a buffer with no file of its own, which leaves nvim's cwd in
---place: that is the only directory there is to run in.
function M.file_dir(path)
  if not path or path == "" then
    return nil
  end

  local dir = vim.fn.fnamemodify(vim.uv.fs_realpath(path) or path, ":p:h")

  return vim.fn.isdirectory(dir) == 1 and dir or nil
end

---Run a command and return its exit code and trimmed output.
---
---`cmd` is a TABLE of argv words or a STRING. The table form runs the words
---directly with no shell, which is the only form safe for a command carrying an
---interpolated branch name, remote, repository or path. The string form is the
---deliberate escape hatch for a real shell pipeline, and the caller owns
---quoting whatever it interpolates.
---
---`stdin` is text to feed the command, for the argv form only: it is what lets
---a caller blame a buffer it has not saved through `git blame --contents -`.
---`cwd` is the directory to run it in, and defaults to nvim's own.
function M.run_shell_command(opts)
  opts = opts or error("Missing `command` argument. Provide a table with a `command` field.")
  local cmd = opts.cmd
  local notify_error = opts.notify_error

  local output, exit_code
  if type(cmd) == "table" then
    local result = vim.system(cmd, { text = true, stdin = opts.stdin, cwd = opts.cwd }):wait()
    exit_code = result.code
    -- stderr joins the value only on failure, so a tool that warns on stderr
    -- cannot glue its notice onto a URL the caller is about to use.
    output = result.stdout or ""
    if exit_code ~= 0 then
      output = output .. (result.stderr or "")
    end
  elseif type(cmd) == "string" then
    output = vim.fn.system(cmd)
    exit_code = vim.v.shell_error
  else
    error(("Invalid `command` type: %s. Must be a string or table."):format(type(cmd)))
  end

  if exit_code ~= 0 and notify_error then
    if type(notify_error) == "boolean" then
      return nil, output
    end
  end

  return exit_code, M.trim(output)
end

return M
