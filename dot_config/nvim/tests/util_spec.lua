-- custom_api.util's pure string helpers (spec 6.3), plus the arity and trimming
-- of run_shell_command, the caller the trim fix actually narrowed.

local util = require("custom_api.util")

-- `#` is undefined on a table with an embedded nil, so the count comes from
-- `select` instead.
local function collect(...)
  return select("#", ...), { ... }
end

return {
  ["trim strips surrounding whitespace"] = function()
    assert(util.trim("  hi there  ") == "hi there")
  end,

  ["trim reads a missing string as empty"] = function()
    assert(util.trim(nil) == "")
  end,

  ["trim returns only the trimmed string"] = function()
    -- gsub's second return value is the substitution count. Leaking it makes
    -- every tail-position caller a two-value expression, and util.lua's own
    -- run_shell_command returned three values because of it.
    local first, second = util.trim(" x ")
    assert(first == "x", "trimmed to " .. tostring(first))
    assert(second == nil, "leaked a second value: " .. tostring(second))
  end,

  ["sanitize_input trims and lowercases"] = function()
    assert(util.sanitize_input("  HeLLo World  ") == "hello world")
  end,

  ["normalize returns the trimmed message"] = function()
    assert(util.normalize("  a message\t") == "a message")
  end,

  ["normalize reads a blank message as nil"] = function()
    assert(util.normalize("   \n  ") == nil)
  end,

  ["map has left util for custom_api.keymap"] = function()
    assert(util.map == nil, "util.map is still " .. type(util.map))
    assert(type(require("custom_api.keymap").map) == "function", "custom_api.keymap.map is missing")
  end,

  ["overseer_runner has left util for custom_api.overseer"] = function()
    assert(util.overseer_runner == nil, "util.overseer_runner is still " .. type(util.overseer_runner))
    assert(
      type(require("custom_api.overseer").overseer_runner) == "function",
      "custom_api.overseer.overseer_runner is missing"
    )
  end,

  -- A link and its target can sit in two different checkouts, and git and `gh`
  -- both answer for the directory they RUN in, so the directory to run in is the
  -- real file's. `github.commit_url` hands this the buffer's own name, which for
  -- a symlinked file is the link, so resolving here is what keeps that call in
  -- the repository the file actually belongs to.
  ["file_dir answers with the directory of the file a symlink points at"] = function()
    local root = vim.fn.tempname()
    vim.fn.mkdir(root .. "/target-side", "p")
    vim.fn.mkdir(root .. "/link-side", "p")
    local target = root .. "/target-side/file.txt"
    local link = root .. "/link-side/file.txt"
    vim.fn.writefile({ "contents" }, target)
    vim.uv.fs_symlink(target, link)

    -- macOS rewrites `/var` to `/private/var`, so the directory asked for comes
    -- off the resolver rather than off the string built above.
    local want = vim.fn.fnamemodify(vim.uv.fs_realpath(target), ":h")
    local got = util.file_dir(link)

    assert(got == want, "answered " .. tostring(got) .. ", not " .. want)
  end,

  ["run_shell_command runs a table command as argv, with no shell in between"] = function()
    -- The table form exists to keep interpolated values (branch names, remote
    -- URLs, repository names) out of a shell. Joining it back into shell text
    -- is what this pins against, so the fixture value is one the shell WOULD
    -- execute: the marker file appears only if a shell saw it.
    local marker = vim.fn.tempname() .. "-nvim-shell-safety-PWNED"
    local payload = "a$(touch " .. marker .. ")b"

    -- Escaping the payload and running it through a shell anyway would satisfy
    -- every assertion below on its own, so 'shell was avoided' is pinned
    -- separately: point 'shell' at a path that does not exist, and any run that
    -- reaches `vim.fn.system` fails outright. Only the argv path survives it.
    local real_shell = vim.o.shell
    vim.o.shell = "/nonexistent/nvim-shell-safety-no-such-shell"
    local ok, code, output = pcall(util.run_shell_command, { cmd = { "printf", "%s", payload } })
    vim.o.shell = real_shell

    assert(ok, "the table form went through a shell: " .. tostring(code))
    assert(vim.uv.fs_stat(marker) == nil, "a shell ran the substitution: " .. marker .. " exists")
    assert(code == 0, "exit code was " .. tostring(code))
    assert(output == payload, "callee received " .. tostring(output) .. ", not the literal value")
  end,

  -- The blame path's `--contents -` is the only caller that sends stdin, and its
  -- own case replaces `git.runner`, so nothing there would notice this function
  -- dropping the field on the floor. `cat` writes back exactly what it was
  -- given, and the payload carries no surrounding whitespace, so the trim on the
  -- way out cannot hide a difference: these are the bytes the command received.
  -- A run that never got stdin reads EOF and prints nothing.
  --
  -- The payload is LF only on purpose. `text = true` rewrites CRLF to LF in what
  -- comes BACK, so a CR here would fail the comparison for a reason that has
  -- nothing to do with what the command was fed.
  ["run_shell_command hands stdin to the command it runs"] = function()
    local payload = "first line\nsecond line\nthird"
    local code, output = util.run_shell_command({ cmd = { "cat" }, stdin = payload })
    assert(code == 0, "exit code was " .. tostring(code))
    assert(output == payload, "cat wrote back " .. string.format("%q", tostring(output)))
  end,

  ["run_shell_command returns the exit code and the trimmed output, and nothing else"] = function()
    -- This is the caller the trim fix narrowed from three values to two: gsub's
    -- substitution count used to ride out of `trim` and become a third result.
    -- The shell is real because running a command is what this function is for.
    local count, values = collect(util.run_shell_command({ cmd = "printf '  out  '" }))
    assert(count == 2, "returned " .. count .. " values")
    assert(values[1] == 0, "exit code was " .. tostring(values[1]))
    assert(values[2] == "out", "output was " .. tostring(values[2]))
  end,
}
