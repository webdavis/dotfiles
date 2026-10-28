-- Headless Lua test runner for the custom_api specs.
-- Run it as nvim --headless --clean -l tests/run.lua [--config <dir>] [<name>_spec]
-- `--config` names the config root whose `lua/` goes on `package.path`. Specs come from this file's directory.
-- A spec returns a table of named case functions and asserts with plain `assert`.

-- Clear the git hook variables, so scratch repositories and the code under test never touch the real repository.
for _, name in ipairs({ "GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE" }) do
  vim.env[name] = nil
end

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

-- `io.write`, because `print` under `nvim -l` drops the newline after a line exactly `columns` wide.
local function report(line)
  io.write(line, "\n")
end

for _, path in ipairs(spec_files) do
  local spec = path:match("([^/]+)%.lua$")
  local cases = dofile(path)

  -- A spec with no cases fails by name, so a gutted spec cannot pass silently.
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
