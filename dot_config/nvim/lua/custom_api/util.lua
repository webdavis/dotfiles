---@class custom_api.util
local M = {}

---The outer parentheses drop the `gsub` substitution count.
function M.trim(s)
  return ((s or ""):gsub("^%s*(.-)%s*$", "%1"))
end

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
---Git and `gh` answer for the directory they run in, and git for the real file, so resolve symlinks.
function M.file_dir(path)
  if not path or path == "" then
    return nil
  end

  local dir = vim.fn.fnamemodify(vim.uv.fs_realpath(path) or path, ":p:h")

  return vim.fn.isdirectory(dir) == 1 and dir or nil
end

---Run a command and return its exit code and trimmed output.
---A table `cmd` runs argv with no shell. A string runs a shell, and the caller owns quoting.
---`stdin` and `cwd` apply to the table form.
function M.run_shell_command(opts)
  opts = opts or error("Missing `command` argument. Provide a table with a `command` field.")
  local cmd = opts.cmd
  local notify_error = opts.notify_error

  local output, exit_code
  if type(cmd) == "table" then
    local result = vim.system(cmd, { text = true, stdin = opts.stdin, cwd = opts.cwd }):wait()
    exit_code = result.code
    -- Append stderr only on failure, so a warning never joins a URL the caller uses.
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
