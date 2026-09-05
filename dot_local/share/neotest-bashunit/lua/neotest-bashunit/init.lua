-- neotest-bashunit: a neotest adapter for bashunit test files, the
-- `<name>.test.sh` shape this repository's bash corpus is migrating to.
--
-- Written rather than adopted because one unit-testing framework must never
-- call another, and because the three things wanted out of a bash adapter are
-- exactly the three a generic shell runner cannot give: output for ONE test,
-- a jump to the line that actually failed, and running a single test instead of
-- its whole file.
--
-- Every rule about bashunit's shapes lives in `parse.lua` as a pure function,
-- verified by `tests/` under a bare headless Neovim. This file is the part that
-- cannot be: the neotest interface, the file system and the command.

local parse = require("neotest-bashunit.parse")

---@type neotest.Adapter
local adapter = { name = "neotest-bashunit" }

---@param dir string
---@return string|nil
function adapter.root(dir)
  return vim.fs.root(dir, { ".bashunitrc", ".git" })
end

---@param name string
---@return boolean
function adapter.filter_dir(name)
  -- `.bashunit` is bashunit's own run state, `target` is cargo's: both are
  -- large, neither can hold a test file.
  return name ~= ".git" and name ~= ".bashunit" and name ~= "node_modules" and name ~= "target"
end

---@param file_path string
---@return boolean
function adapter.is_test_file(file_path)
  return parse.is_test_file(file_path)
end

---@param file_path string
---@return neotest.Tree
function adapter.discover_positions(file_path)
  local lines = vim.fn.readfile(file_path)
  return require("neotest.types").Tree.from_list(parse.positions(file_path, lines), function(position)
    return position.id
  end)
end

---@param args neotest.RunArgs
---@return neotest.RunSpec|nil
function adapter.build_spec(args)
  local position = args.tree:data()
  if position.type ~= "dir" and position.type ~= "file" and position.type ~= "test" then
    return nil
  end

  local report = vim.fn.tempname() .. ".json"
  local command = { "bashunit", position.path, "--report-json", report }

  if position.type == "test" then
    -- One test, by function name. `--filter` is a SUBSTRING match on 0.50.1
    -- (measured: `--filter test_alpha` also runs `test_alpha_extended`), which
    -- is why `results` matches report rows back by name instead of assuming the
    -- run held exactly this one. The sibling then gets its own correct result
    -- rather than this test silently inheriting the sibling's.
    vim.list_extend(command, { "--filter", position.id:match("::(.*)$") })
  end
  vim.list_extend(command, args.extra_args or {})

  return {
    command = command,
    cwd = adapter.root(position.path) or vim.fs.dirname(position.path),
    context = { report = report },
    -- NO_COLOR, not --no-color: the flag is ignored in either position on
    -- 0.50.1, and neotest runs its command under a pty, so bashunit would
    -- otherwise colour output that `parse.failing_lines` has to read.
    env = { NO_COLOR = "1" },
  }
end

---@param path string|nil
---@return string|nil
local function read_file(path)
  if not path or vim.fn.filereadable(path) ~= 1 then
    return nil
  end
  return table.concat(vim.fn.readfile(path, "b"), "\n")
end

---A file holding just this test's own output, which is what makes neotest's
---output window show one test rather than the whole run.
---@param message string
---@return string
local function write_output(message)
  local path = vim.fn.tempname()
  vim.fn.writefile(vim.split(message, "\n"), path)
  return path
end

---@param spec neotest.RunSpec
---@param result neotest.StrategyResult
---@param tree neotest.Tree
---@return table<string, neotest.Result>
function adapter.results(spec, result, tree)
  local rows, report_error = parse.report_rows(read_file(spec.context and spec.context.report))
  if not rows then
    -- The run never produced a report: bashunit is missing, the file did not
    -- parse, or the process died. Failing the position that was asked for, with
    -- the run's own output attached, says so; returning nothing would read as
    -- "nothing ran" and leave the tree looking untouched.
    return {
      [tree:data().id] = { status = "failed", short = report_error, output = result.output },
    }
  end

  local positions = {}
  for _, node in tree:iter_nodes() do
    if node:data().type == "test" then
      positions[#positions + 1] = node:data()
    end
  end

  local failing_lines = parse.failing_lines(read_file(result.output))
  local results = {}
  for id, row in pairs(parse.match_rows(rows, positions)) do
    local entry = { status = row.status, output = result.output }
    if row.message ~= "" then
      entry.short = row.message
      entry.output = write_output(row.message)
    end
    if row.status == "failed" then
      local line = failing_lines[row.name] or parse.message_line(row.message)
      entry.errors = { { message = row.message, line = line and line - 1 or nil } }
    end
    results[id] = entry
  end
  return results
end

return adapter
