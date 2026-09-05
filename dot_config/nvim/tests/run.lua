-- Headless Lua test runner for the custom_api specs (spec 6.3).
--
-- Usage: nvim --headless --clean -l tests/run.lua [--config <dir>] [<name>_spec]
--
-- `--config` names the CONFIG ROOT whose `lua/` goes on `package.path`, and it
-- defaults to this file's grandparent. That one argument is why `tests/` can
-- stay out of $HOME (it is chezmoiignored): the same runner tests the source
-- tree from `just test-nvim` and the deployed tree from the bootstrap, and the
-- Lua under test is whichever root it was pointed at. The spec files always
-- come from this file's own directory.
--
-- A spec file returns a table of `["what it does"] = function() ... end` cases
-- and asserts with plain `assert`. No plenary, no busted.

local tests_dir = arg[0]:match("(.*)/") or "."
local config_root = tests_dir .. "/.."
local only

local index = 1
while arg[index] do
  if arg[index] == "--config" then
    config_root = arg[index + 1] or error("--config needs a directory")
    index = index + 2
  else
    only = arg[index]
    index = index + 1
  end
end

package.path = ("%s/lua/?.lua;%s/lua/?/init.lua;%s"):format(config_root, config_root, package.path)

local spec_files
if only then
  spec_files = { ("%s/%s.lua"):format(tests_dir, only) }
else
  spec_files = vim.fn.glob(tests_dir .. "/*_spec.lua", false, true)
end

-- A run that found nothing to do would otherwise exit 0 and read as a pass.
if #spec_files == 0 then
  error("no spec files matched under " .. tests_dir)
end

local failures = 0

-- Not `print`: under `nvim -l` that routes through the message system, which
-- swallows the newline after a line exactly `columns` wide (80 by default, so a
-- case whose report lands on 80 characters merges into the next one) and leaves
-- the final line unterminated. `io.write` goes straight to stdout and is
-- flushed by `os.exit`.
local function report(line)
  io.write(line, "\n")
end

for _, path in ipairs(spec_files) do
  local spec = path:match("([^/]+)%.lua$")
  local cases = dofile(path)

  -- A spec with no cases reports nothing and adds nothing to the failure count,
  -- so gutting one would leave the run green and silent. Named, because the
  -- aggregate run is where a gutted spec would otherwise disappear.
  if type(cases) ~= "table" or next(cases) == nil then
    error(spec .. " returned no cases")
  end

  -- Sorted, so a run reports its cases in the same order every time.
  local names = {}
  for name in pairs(cases) do
    table.insert(names, name)
  end
  table.sort(names)

  for _, name in ipairs(names) do
    local ok, err = pcall(cases[name])
    if ok then
      report(("ok %s: %s"):format(spec, name))
    else
      failures = failures + 1
      report(("FAIL %s: %s: %s"):format(spec, name, err))
    end
  end
end

os.exit(failures == 0 and 0 or 1)
