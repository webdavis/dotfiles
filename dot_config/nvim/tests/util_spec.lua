local util = require("custom_api.util")

-- `select` counts the values, because `#` is undefined with an embedded nil.
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
    -- The gsub substitution count must not leak out as a second value.
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

  ["run_shell_command runs a table command in the directory it was given"] = function()
    -- Without a cwd, every git helper answers for the Neovim cwd.
    local dir = vim.fn.tempname()
    assert(vim.fn.mkdir(dir, "p") == 1, "mkdir " .. dir)
    local code, output = util.run_shell_command({ cmd = { "pwd" }, cwd = dir })
    assert(code == 0, "exit " .. tostring(code) .. ": " .. tostring(output))
    assert(vim.fn.resolve(output) == vim.fn.resolve(dir), "ran in " .. tostring(output) .. ", not " .. dir)
  end,

  -- git and `gh` answer for the directory they run in, and a link and its target can sit in different checkouts.
  ["file_dir answers with the directory of the file a symlink points at"] = function()
    local root = vim.fn.tempname()
    vim.fn.mkdir(root .. "/target-side", "p")
    vim.fn.mkdir(root .. "/link-side", "p")
    local target = root .. "/target-side/file.txt"
    local link = root .. "/link-side/file.txt"
    vim.fn.writefile({ "contents" }, target)
    vim.uv.fs_symlink(target, link)

    -- macOS rewrites `/var` to `/private/var`, so ask the resolver.
    local want = vim.fn.fnamemodify(vim.uv.fs_realpath(target), ":h")
    local got = util.file_dir(link)

    assert(got == want, "answered " .. tostring(got) .. ", not " .. want)
  end,

  ["run_shell_command runs a table command as argv, with no shell in between"] = function()
    -- The marker file appears only if a shell ran the payload.
    local marker = vim.fn.tempname() .. "-nvim-shell-safety-PWNED"
    local payload = "a$(touch " .. marker .. ")b"

    -- Point 'shell' at a missing path, so any run through `vim.fn.system` fails.
    local real_shell = vim.o.shell
    vim.o.shell = "/nonexistent/nvim-shell-safety-no-such-shell"
    local ok, code, output = pcall(util.run_shell_command, { cmd = { "printf", "%s", payload } })
    vim.o.shell = real_shell

    assert(ok, "the table form went through a shell: " .. tostring(code))
    assert(vim.uv.fs_stat(marker) == nil, "a shell ran the substitution: " .. marker .. " exists")
    assert(code == 0, "exit code was " .. tostring(code))
    assert(output == payload, "callee received " .. tostring(output) .. ", not the literal value")
  end,

  -- `cat` echoes stdin, so the output is the bytes the command received.
  -- LF only, because `text = true` rewrites CRLF in the output.
  ["run_shell_command hands stdin to the command it runs"] = function()
    local payload = "first line\nsecond line\nthird"
    local code, output = util.run_shell_command({ cmd = { "cat" }, stdin = payload })
    assert(code == 0, "exit code was " .. tostring(code))
    assert(output == payload, "cat wrote back " .. string.format("%q", tostring(output)))
  end,

  ["run_shell_command returns the exit code and the trimmed output, and nothing else"] = function()
    -- Two values, not three: the gsub count stays inside `trim`.
    local count, values = collect(util.run_shell_command({ cmd = "printf '  out  '" }))
    assert(count == 2, "returned " .. count .. " values")
    assert(values[1] == 0, "exit code was " .. tostring(values[1]))
    assert(values[2] == "out", "output was " .. tostring(values[2]))
  end,
}
