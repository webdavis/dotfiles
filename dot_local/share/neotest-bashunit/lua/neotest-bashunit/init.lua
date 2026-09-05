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

---@param name string
---@return boolean
function adapter.filter_dir(name)
  -- `.bashunit` is bashunit's own run state, `target` is cargo's: both are
  -- large, neither can hold a test file.
  return name ~= ".git" and name ~= ".bashunit" and name ~= "node_modules" and name ~= "target"
end

---Whether any bashunit test file is reachable below `root`.
---
---`vim.fs.dir` rather than `vim.fs.find`: find's downward walk has no way to
---prune a directory, so on a root holding no test it queues `node_modules` and
---`.git` and reads the whole tree. `vim.fs.dir` is a lazy iterator, so the
---first match ends the walk and the pruned directories are never opened.
---
---`skip` is handed the path RELATIVE to `root`, so the comparison is against
---the last component: whole-path equality would let `src/node_modules`
---through. `fixtures` is skipped here but NOT in `filter_dir`, because this is
---only the question of whether to attach at all; a fixture tree is where a
---sample test file would sit without being anybody's test to run, while
---`filter_dir` decides what neotest may DISCOVER and must not hide a real one.
---@param root string
---@return boolean
local function holds_a_test(root)
  for name, kind in
    vim.fs.dir(root, {
      depth = math.huge,
      skip = function(relative)
        local base = vim.fs.basename(relative)
        return adapter.filter_dir(base) and base ~= "fixtures"
      end,
    })
  do
    if kind == "file" and parse.is_test_file(name) then
      return true
    end
  end
  return false
end

---@param dir string
---@return string|nil
function adapter.root(dir)
  local root = vim.fs.root(dir, { ".bashunitrc", ".git" })
  -- The marker alone is not the answer. `.git` sits at the top of every
  -- repository there is, so claiming on it attached this adapter to all of
  -- them, and neotest hands a whole-directory run to the single non-JavaScript
  -- adapter that attached: a project with no bash in it ran its "all tests"
  -- through bashunit. A `.bashunitrc` is somebody writing the configuration on
  -- purpose and is taken at its word, test files or not; a bare `.git` has to
  -- be backed by a test file that is actually there.
  if not root or vim.uv.fs_stat(root .. "/.bashunitrc") then
    return root
  end
  return holds_a_test(root) and root or nil
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

---Every test function discovered in the file that holds this test position,
---the selected one included. Empty when neotest handed over a detached node.
---@param test_node neotest.Tree
---@return string[]
local function sibling_function_names(test_node)
  local file_node = test_node:parent()
  if not file_node then
    return {}
  end
  local names = {}
  for _, node in file_node:iter_nodes() do
    local data = node:data()
    if data.type == "test" then
      names[#names + 1] = data.id:match("::(.*)$")
    end
  end
  return names
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
    -- One test, by function name, and one test only. `--filter` is a SUBSTRING
    -- match on 0.50.1 (`case "$fn" in test_*${needle}*`, needle being the name
    -- with `test_` stripped), so `--filter test_alpha` on its own also runs
    -- `test_alpha_extended` AND `test_beta_alpha`, measured. It is not a regular
    -- expression, so anchoring it matches nothing at all. Every sibling the
    -- needle would drag in is named back as an `--exclude-filter`, which is
    -- repeatable and reduces the run to the one test (measured).
    local selected = position.id:match("::(.*)$")
    vim.list_extend(command, { "--filter", selected })
    for _, sibling in ipairs(parse.exclude_filters(selected, sibling_function_names(args.tree))) do
      vim.list_extend(command, { "--exclude-filter", sibling })
    end
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

  local positions, results = {}, {}
  for _, node in tree:iter_nodes() do
    local data = node:data()
    if data.type == "test" then
      positions[#positions + 1] = data
      -- Discovery already found this title on a second test in the same file.
      -- No report row can be attributed to either side, so both are failed with
      -- the collision named rather than one of them taking the other's verdict.
      if data.ambiguous then
        results[data.id] = { status = "failed", short = data.ambiguous, output = result.output }
      end
    end
  end

  local failing_lines = parse.failing_lines(read_file(result.output))
  -- match_rows never returns an ambiguous position, so nothing below overwrites
  -- an ambiguity verdict recorded above.
  for id, row in pairs(parse.match_rows(rows, positions)) do
    local entry = { status = row.status, output = result.output }
    if row.message ~= "" then
      entry.short = row.message
      entry.output = write_output(row.message)
    end
    if row.status == "failed" then
      local candidates = failing_lines[row.file .. "\0" .. row.name]
      local line
      if candidates and #candidates == 1 then
        line = candidates[1]
      else
        -- bashunit listed every assertion in the function, or none, so the jump
        -- goes to where the test starts. Saying so beats a confident jump to an
        -- assertion that passed.
        line = parse.message_line(row.message)
        if candidates then
          entry.short = ("%s\n\nneotest-bashunit: bashunit listed %d assertions under Source:, which does not say which one failed, so this points at the test's own line."):format(
            entry.short or "",
            #candidates
          )
        end
      end
      entry.errors = { { message = row.message, line = line and line - 1 or nil } }
    end
    results[id] = entry
  end
  return results
end

return adapter
